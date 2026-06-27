//! Agent Loop - Core orchestration component
//!
//! The `Agent` manages the interaction loop between the LLM and tools.
//! It handles:
//! - Tool discovery for LLM
//! - Request routing to appropriate tools
//! - Context management
//! - Iteration limits and safety
//! - Error recovery with retry
//! - Execution tracing
//! - State persistence
//! - Parallel tool execution

pub mod compaction_service;
pub mod config;
pub mod controller;
pub mod cost_channel;
pub mod definition;
pub mod driver;
pub mod enhanced;
/// Error memory module.
pub mod error_memory;
pub mod flow;
/// Hooks module.
pub mod hooks;
pub mod lsp;
pub mod ops;
pub mod snapshot;
pub mod tool_map;
/// Tool parser module.
pub mod tool_parser;

mod construct;
mod execution;
mod executor;
pub mod jumpy;
/// Loop detector module.
pub mod loop_detector;
pub mod plan;
// B3: Re-export Plan types from `types.rs` to maintain backwards compatibility.
// New code should prefer `use clarity_core::types::{Plan, PlanResult, PlanStep}`.
pub use crate::types::{Plan, PlanResult, PlanStep};
pub mod lifecycle;
mod prompt;
mod run;
mod tool_prompt_manager;
mod turn_context;
mod yolo_guardrails;
pub use clarity_contract::subagent::AgentExecutor;

#[cfg(test)]
mod tests;

use crate::agent::compaction_service::CompactionService;
use crate::approval::{ApprovalMode, ApprovalRuntime};
use crate::compaction::CompactionConfig;
use crate::error::AgentError;
use crate::memory::{MemoryStore, SharedMemoryTicker};
use crate::registry::ToolRegistry;
use crate::skills::SkillRegistry;
use clarity_contract::subagent::{CapabilityToken, ParallelConfig, RunSpec};
use clarity_wire::Wire;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

// Re-export enhanced features
pub use controller::{AgentController, ControllerEvent};
pub use enhanced::{
    ConversationState, ErrorRecovery, ErrorRecoveryConfig, ExecutionStep, ExecutionSummary,
    ExecutionTracer, ParallelToolExecutor, RecoveryStrategy, StatePersistence, StepType,
    TokenUsage,
};
pub use ops::Op;

// Re-export core API types from their canonical locations for backwards compatibility.
// New code should import directly from `crate::types` or `clarity_contract`.
pub use crate::types::{FunctionCall, ToolCall};
pub use clarity_contract::{LlmProvider, LlmResponse, Message, MessageRole, StreamDelta};

// Re-export config
pub use config::AgentConfig;
pub use error_memory::*;

/// Lifecycle state of an Agent instance.
#[derive(Debug, Clone)]
pub enum AgentState {
    /// No LLM configured. `run()` is illegal.
    Unconfigured,
    /// Ready to accept a turn. `cancel_token` is guaranteed fresh.
    Idle,
    /// A turn is currently in progress on this (or a cloned) Agent.
    Running {
        /// Snapshot of the turn's token, used by `cancel()`.
        cancel_token: CancellationToken,
    },
    /// Previous turn was cancelled or the inner task panicked.
    /// Requires explicit reset (or implicit reset on next `run()` attempt).
    Stalled,
}

/// Shared mutable runtime state of an Agent.
struct AgentInner {
    state: AgentState,
    llm: Option<Arc<dyn LlmProvider>>,
    memory_store: Option<Arc<dyn MemoryStore>>,
    skill_registry: Option<SkillRegistry>,
    /// Deprecated: use `SkillRegistry::active_ids()` instead.
    /// Kept for backward compatibility and internal snapshotting.
    active_skill: Option<String>,
    file_prompt_cache: Option<String>,
    /// File paths representing the user's current operation.
    /// Used to dynamically activate skills whose `paths` patterns match.
    active_file_paths: Vec<std::path::PathBuf>,
    /// Approval mode (Interactive, Yolo, Plan)
    approval_mode: ApprovalMode,
    /// Cached Git context string for SystemPromptBuilder injection.
    git_context: Option<String>,
    /// Cached active files description for SystemPromptBuilder injection.
    active_files: Option<String>,
    /// Cached project metadata (Cargo.toml, package.json, etc.) for SystemPromptBuilder injection.
    project_metadata: Option<String>,
    /// Provider label for internal logging (e.g. "deepseek-chat", "claude-3-7-sonnet").
    /// NOT injected into the system prompt; used only for tracing/audit.
    provider_label: Option<String>,
    /// Optional lifecycle hook registry for intercepting tool calls and LLM input.
    hook_registry: Option<std::sync::Arc<tokio::sync::RwLock<hooks::HookRegistry>>>,
    /// Turn-level mutable state. Created by `begin_turn()` and cleared by `finish_turn()`.
    turn_context: Option<turn_context::TurnContext>,
    /// Lifecycle events emitted during the current turn. Consumed by the caller
    /// (e.g. AgentController) to persist into rollout/event log.
    lifecycle_events: Vec<(
        crate::agent::lifecycle::RunEvent,
        crate::agent::lifecycle::RunState,
    )>,
    /// Message count from the last completed turn (for subagent progress reporting).
    last_turn_message_count: usize,
    /// Accumulated estimated cost today (USD). Reset daily or per session.
    daily_cost_usd: f64,
    /// Date of the last cost record (to detect day boundary).
    last_cost_date: chrono::NaiveDate,
    /// Optional vision-capable LLM provider. Created lazily when needed.
    vision_llm: Option<Arc<dyn LlmProvider>>,
    /// Fallback LLM providers. When non-empty, the primary LLM is wrapped in a
    /// ReliableProvider so failures automatically fall back through this chain.
    fallback_llms: Vec<Arc<dyn LlmProvider>>,
    /// Blake3 hash of the static system prompt from the last turn.
    /// Used to detect when static content changed and local KV cache should be invalidated.
    static_prompt_hash: Option<String>,
    /// Optional Jumpy World Model predictor for skill-level planning.
    jumpy_predictor: Option<Arc<dyn crate::agent::jumpy::predictor::OutcomePredictor>>,
    /// Whether the LSP hook has already been initialized.
    lsp_initialized: bool,
    /// Optional snapshot service for per-turn workspace snapshots.
    snapshot_service: Option<std::sync::Arc<snapshot::SnapshotService>>,
}

/// Simple mock LLM for testing
pub struct MockLlm;

#[async_trait::async_trait]
impl LlmProvider for MockLlm {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &serde_json::Value,
    ) -> Result<LlmResponse, crate::error::AgentError> {
        Ok(LlmResponse {
            content: "This is a mock response".to_string(),
            tool_calls: vec![],
            is_complete: true,
        })
    }

    fn stream(
        &self,
        _messages: &[Message],
        _tools: &serde_json::Value,
    ) -> Result<
        tokio::sync::mpsc::Receiver<Result<StreamDelta, crate::error::AgentError>>,
        crate::error::AgentError,
    > {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx
                .send(Ok(StreamDelta {
                    content: Some("This is a mock response".to_string()),
                    reasoning_content: None,
                    tool_calls: vec![],
                }))
                .await;
        });
        Ok(rx)
    }

    fn set_prompt_cache_key(&self, _key: &str) {}
}

/// The main Agent struct
///
/// Manages the interaction between user, LLM, and tools.
///
/// # Example
///
/// ```rust,no_run
/// use clarity_core::{Agent, ToolRegistry};
/// use clarity_core::agent::AgentConfig;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let registry = ToolRegistry::with_builtin_tools();
///
///     let config = AgentConfig::new()
///         .with_max_iterations(10)
///         .with_read_only(false);
///
///     let agent = Agent::with_config(registry, config);
///
///     // This would need an actual LLM provider
///     // let response = agent.run("List all Rust files").await?;
///
///     Ok(())
/// }
/// ```
/// Factory type for lazy LLM initialization
pub type LlmFactoryFn = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<Arc<dyn LlmProvider>, AgentError>> + Send>>
        + Send
        + Sync,
>;

/// Factory type for lazy MemoryStore initialization
pub type MemoryFactoryFn = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<Arc<dyn MemoryStore>, AgentError>> + Send>>
        + Send
        + Sync,
>;

/// Factory type for lazy SkillRegistry initialization
pub type SkillFactoryFn = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<SkillRegistry, AgentError>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
/// Core Clarity agent: owns tool registry, configuration, and turn orchestration.
pub struct Agent {
    registry: ToolRegistry,
    config: AgentConfig,
    memory_ticker: Option<SharedMemoryTicker>,
    /// Optional wire for UI communication
    wire: Option<Arc<Wire>>,
    /// Approval runtime for tool execution control
    approval_runtime: Option<Arc<dyn ApprovalRuntime>>,
    /// Compaction configuration for context management
    compaction_config: CompactionConfig,
    /// Maximum context tokens before compaction
    max_context_tokens: usize,
    /// Optional compaction service for proactive history compression
    compaction_service: Option<CompactionService>,
    /// Optional hook registry for lifecycle interception
    hook_registry: Option<crate::hooks::HookRegistry>,
    /// Lazy LLM factory — called on first `run()` if no LLM is set
    llm_factory: Option<LlmFactoryFn>,
    /// Lazy MemoryStore factory — called on first `run()` if no store is set
    memory_factory: Option<MemoryFactoryFn>,
    /// Lazy SkillRegistry factory — called on first `run()` if no registry is set
    skill_factory: Option<SkillFactoryFn>,
    /// Active plan execution controller, populated during `execute_plan`.
    /// Allows external callers (UI, API) to skip or retry steps mid-flight.
    /// Uses `tokio::sync::Mutex` because `execute_plan` is async and the
    /// controller must be accessible across await points.
    plan_controller: Arc<tokio::sync::Mutex<Option<crate::agent::plan::PlanExecutionController>>>,
    /// Optional subagent orchestrator — injected at build time.
    orchestrator: Option<Arc<dyn clarity_contract::subagent::SubagentOrchestrator>>,
    /// Shared mutable runtime state.
    ///
    /// **Design choice: `parking_lot::RwLock` is intentional.**
    /// `Agent` methods (getters/setters/cancel/reset) are synchronous and may
    /// be called from non-async contexts (e.g. TUI event loop, Gateway
    /// handlers). Migrating to `tokio::sync::RwLock` would force every
    /// lightweight accessor to become `async`, breaking the builder pattern
    /// and polluting the entire call-graph. All critical sections are
    /// short (field reads/writes only) and audit-confirmed safe (no await
    /// while holding the lock).
    inner: Arc<parking_lot::RwLock<AgentInner>>,
}

impl Agent {
    /// Set the approval mode at runtime.
    pub fn set_approval_mode(&self, mode: ApprovalMode) {
        self.inner.write().approval_mode = mode;
    }

    /// Record a lifecycle event/state pair for the current turn.
    pub(crate) fn record_lifecycle_event(
        &self,
        event: crate::agent::lifecycle::RunEvent,
        state: crate::agent::lifecycle::RunState,
    ) {
        self.inner.write().lifecycle_events.push((event, state));
    }

    /// Take all recorded lifecycle events for the current turn.
    ///
    /// ponytail: this drains the buffer; callers should persist the events
    /// before the next turn starts.
    pub fn take_lifecycle_events(
        &self,
    ) -> Vec<(
        crate::agent::lifecycle::RunEvent,
        crate::agent::lifecycle::RunState,
    )> {
        self.inner.write().lifecycle_events.drain(..).collect()
    }

    /// Get the current approval mode.
    pub fn approval_mode(&self) -> ApprovalMode {
        self.inner.read().approval_mode
    }

    /// Get the message count from the last completed turn.
    pub fn last_turn_message_count(&self) -> usize {
        self.inner.read().last_turn_message_count
    }

    /// Spawn an async background task to extract structured notes from a turn transcript.
    /// Does nothing if `extract_memories` is disabled or no orchestrator is configured.
    pub(crate) fn maybe_extract_memories(&self, transcript: String) {
        if !self.config.extract_memories {
            return;
        }
        let Some(ref orchestrator) = self.orchestrator else {
            return;
        };
        let orchestrator = orchestrator.clone();
        tokio::spawn(async move {
            let spec = RunSpec::new(
                "Extract structured notes from conversation turn",
                format!(
                    "Analyze the following conversation turn and extract structured notes. \
                     Respond with a JSON object containing exactly these keys: \
                     current_state (string), errors (array of strings), \
                     learnings (array of strings), key_results (array of strings).\n\n{}",
                    transcript
                ),
            )
            .with_type("memory-extractor")
            .with_capability_token(CapabilityToken::read_only())
            .without_git_context();

            match orchestrator
                .run_parallel(
                    vec![spec],
                    ParallelConfig::new().with_max_concurrency(1),
                    None,
                )
                .await
            {
                Ok(result) => {
                    if let Some(subagent_result) = result.results.into_iter().next() {
                        tracing::info!(
                            "Extracted session notes summary: {}",
                            subagent_result.summary
                        );
                    } else if let Some((id, err)) = result.failures.into_iter().next() {
                        tracing::warn!("Memory extraction subagent {} failed: {}", id, err);
                    }
                }
                Err(e) => {
                    tracing::warn!("Memory extraction failed: {}", e);
                }
            }
        });
    }

    /// Snapshot pre-turn if the snapshot service is active.
    pub(crate) async fn maybe_snapshot_pre_turn(&self) {
        let svc_opt = self.inner.read().snapshot_service.clone();
        if let Some(ref svc) = svc_opt {
            if let Err(e) = svc.snapshot_pre_turn().await {
                tracing::warn!("Pre-turn snapshot failed: {}", e);
            }
        }
    }

    /// Snapshot post-turn if the snapshot service is active.
    pub(crate) async fn maybe_snapshot_post_turn(&self) {
        let svc_opt = self.inner.read().snapshot_service.clone();
        if let Some(ref svc) = svc_opt {
            if let Err(e) = svc.snapshot_post_turn().await {
                tracing::warn!("Post-turn snapshot failed: {}", e);
            }
        }
    }

    // ------------------------------------------------------------------
    // Sprint 39 — Snapshot UI API (egui integration)
    // ------------------------------------------------------------------

    /// Return a copy of all stored workspace snapshots.
    /// Returns an empty list if the snapshot service is not active.
    pub fn snapshot_list(&self) -> Vec<snapshot::SnapshotInfo> {
        let svc_opt = self.inner.read().snapshot_service.clone();
        svc_opt.map(|svc| svc.list()).unwrap_or_default()
    }

    /// Restore the workspace to the state of snapshot `id`.
    /// Returns an error if the snapshot service is not active or the id is unknown.
    pub async fn restore_snapshot(&self, id: usize) -> Result<(), crate::error::AgentError> {
        let svc_opt = self.inner.read().snapshot_service.clone();
        match svc_opt {
            Some(svc) => svc.restore(id).await,
            None => Err(crate::error::AgentError::ToolExecutionFailed(
                "git_restore".to_string(),
                "Snapshot service is not active".to_string(),
            )),
        }
    }

    // ------------------------------------------------------------------
    // Sprint 11 Phase A — Context Snapshot getters/setters
    // ------------------------------------------------------------------

    /// Set the cached Git context string.
    pub fn set_git_context(&self, ctx: Option<String>) {
        self.inner.write().git_context = ctx;
    }

    /// Get the cached Git context string.
    pub fn git_context(&self) -> Option<String> {
        self.inner.read().git_context.clone()
    }

    /// Set the cached active files description.
    pub fn set_active_files(&self, files: Option<String>) {
        self.inner.write().active_files = files;
    }

    /// Get the cached active files description.
    pub fn active_files(&self) -> Option<String> {
        self.inner.read().active_files.clone()
    }

    /// Set the cached project metadata string.
    pub fn set_project_metadata(&self, meta: Option<String>) {
        self.inner.write().project_metadata = meta;
    }

    /// Get the cached project metadata string.
    pub fn project_metadata(&self) -> Option<String> {
        self.inner.read().project_metadata.clone()
    }

    /// Refresh the cached context snapshot (Git, active files, project metadata).
    /// Called at the start of each turn so the System Prompt reflects current state.
    pub async fn refresh_context(&self) {
        let working_dir = &self.config.working_dir;

        // 1. Git context
        let git_ctx = clarity_contract::subagent::collect_git_context(working_dir).await;
        self.set_git_context(git_ctx);

        // 2. Active files
        let active_files = self.build_active_files_context();
        self.set_active_files(active_files);

        // 3. Project metadata
        let metadata = self.collect_project_metadata();
        self.set_project_metadata(metadata);
    }

    /// Build a text description of active files for the system prompt.
    /// Paths that resolve outside the working directory are redacted to `<external>`
    /// to prevent leaking host directory structure.
    fn build_active_files_context(&self) -> Option<String> {
        let paths = self.active_file_paths();
        if paths.is_empty() {
            return None;
        }
        let working_dir = &self.config.working_dir;
        let lines: Vec<String> = paths
            .iter()
            .map(|p| {
                // Resolve to absolute (relative paths are resolved against working_dir)
                let abs = if p.is_absolute() {
                    p.clone()
                } else {
                    working_dir.join(p)
                };
                // Only show the portion inside working_dir; anything else is redacted
                match abs.strip_prefix(working_dir) {
                    Ok(s) => s.to_string_lossy().to_string(),
                    Err(_) => "<external>".to_string(),
                }
            })
            .filter(|s| !s.is_empty())
            .collect();
        if lines.is_empty() {
            return None;
        }
        Some(format!(
            "The user is currently working with:\n- {}",
            lines.join("\n- ")
        ))
    }

    /// Collect project metadata (Cargo.toml or package.json) for the system prompt.
    fn collect_project_metadata(&self) -> Option<String> {
        let working_dir = &self.config.working_dir;

        // Try Cargo.toml first
        let cargo_toml = working_dir.join("Cargo.toml");
        if cargo_toml.exists() {
            match std::fs::read_to_string(&cargo_toml) {
                Ok(content) => {
                    let truncated = if content.len() > 2048 {
                        format!("{}...\n(truncated)", &content[..2048])
                    } else {
                        content
                    };
                    return Some(format!("```toml\n{}\n```", truncated));
                }
                Err(e) => {
                    tracing::warn!("Failed to read Cargo.toml: {}", e);
                }
            }
        }

        // Fallback to package.json
        let package_json = working_dir.join("package.json");
        if package_json.exists() {
            match std::fs::read_to_string(&package_json) {
                Ok(content) => {
                    let truncated = if content.len() > 2048 {
                        format!("{}...\n(truncated)", &content[..2048])
                    } else {
                        content
                    };
                    return Some(format!("```json\n{}\n```", truncated));
                }
                Err(e) => {
                    tracing::warn!("Failed to read package.json: {}", e);
                }
            }
        }

        None
    }

    /// Check if the estimated cost exceeds per-turn or per-day budget.
    /// Includes pending costs reported by background tasks (subagents, compaction, etc.).
    fn check_budget(&self, estimated_cost: f64) -> Result<(), AgentError> {
        let config = &self.config;
        let mut inner = self.inner.write();

        // Drain any cost reported by background tasks into the main tracker.
        let pending = cost_channel::drain_pending_cost();
        if pending > 0.0 {
            let today = chrono::Utc::now().date_naive();
            if inner.last_cost_date != today {
                inner.daily_cost_usd = 0.0;
                inner.last_cost_date = today;
            }
            inner.daily_cost_usd += pending;
        }

        // Day boundary reset
        let today = chrono::Utc::now().date_naive();
        if inner.last_cost_date != today {
            inner.daily_cost_usd = 0.0;
            inner.last_cost_date = today;
        }

        // Per-turn limit
        if let Some(limit) = config.max_cost_per_turn_usd {
            if estimated_cost > limit {
                return Err(AgentError::BudgetExceeded {
                    limit,
                    current: 0.0,
                    requested: estimated_cost,
                });
            }
        }

        // Per-day limit (includes background-task costs already drained above)
        if let Some(limit) = config.max_cost_per_day_usd {
            let projected = inner.daily_cost_usd + estimated_cost;
            if projected > limit {
                return Err(AgentError::BudgetExceeded {
                    limit,
                    current: inner.daily_cost_usd,
                    requested: estimated_cost,
                });
            }
        }

        Ok(())
    }

    /// Record actual cost after an LLM call.
    /// Also drains any pending costs from background tasks.
    fn record_cost(&self, cost: f64) {
        let pending = cost_channel::drain_pending_cost();
        let mut inner = self.inner.write();
        let today = chrono::Utc::now().date_naive();
        if inner.last_cost_date != today {
            inner.daily_cost_usd = 0.0;
            inner.last_cost_date = today;
        }
        inner.daily_cost_usd += cost + pending;
    }

    /// Execute a flow-driven skill.
    ///
    /// Each node in the flow becomes one agent turn. Decision nodes branch
    /// based on the LLM's `<choice>...</choice>` output.
    pub async fn run_flow(&self, flow: &flow::Flow, args: &str) -> Result<String, AgentError> {
        let runner = flow::FlowRunner::new(flow);
        runner
            .run(self, args)
            .await
            .map_err(|e| AgentError::FlowExecution(e.to_string()))
    }
}

// ------------------------------------------------------------------
// FlowExecutor integration
// ------------------------------------------------------------------

#[async_trait::async_trait]
impl flow::FlowExecutor for Agent {
    async fn execute(&self, prompt: &str) -> Result<String, AgentError> {
        self.run(prompt).await
    }
}
