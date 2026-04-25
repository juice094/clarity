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
pub mod enhanced;
pub mod ops;

mod construct;
mod execution;
pub mod plan;
pub use plan::{Plan, PlanResult, PlanStep};
mod prompt;
mod run;

#[cfg(test)]
mod tests;

use crate::agent::compaction_service::CompactionService;
use crate::approval::{ApprovalMode, ApprovalRuntime};
use crate::compaction::CompactionConfig;
use crate::error::AgentError;
use crate::memory::{MemoryStore, SharedMemoryTicker};
use crate::registry::ToolRegistry;
use crate::skills::SkillRegistry;
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
// New code should import directly from `crate::types` or `crate::llm::api`.
pub use crate::llm::api::{LlmProvider, LlmResponse, Message, MessageRole, StreamDelta};
pub use crate::types::{FunctionCall, ToolCall};

// Re-export config
pub use config::AgentConfig;

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
    session_usage: TokenUsage,
    active_skill: Option<String>,
    /// Snapshotted at turn start so that mid-turn set_active_skill() calls
    /// do not affect the in-flight turn.
    snapshotted_skill: Option<String>,
    file_prompt_cache: Option<String>,
    /// File paths representing the user's current operation.
    /// Used to dynamically activate skills whose `paths` patterns match.
    active_file_paths: Vec<std::path::PathBuf>,
    /// Approval mode (Interactive, Yolo, Plan)
    approval_mode: ApprovalMode,
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
                    tool_calls: vec![],
                }))
                .await;
        });
        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, _key: &str) {}
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
    /// Shared mutable runtime state.
    ///
    /// **Design choice: `std::sync::RwLock` is intentional.**
    /// `Agent` methods (getters/setters/cancel/reset) are synchronous and may
    /// be called from non-async contexts (e.g. TUI event loop, Gateway
    /// handlers). Migrating to `tokio::sync::RwLock` would force every
    /// lightweight accessor to become `async`, breaking the builder pattern
    /// and polluting the entire call-graph. All critical sections are
    /// short (field reads/writes only) and audit-confirmed safe (no await
    /// while holding the lock).
    inner: Arc<std::sync::RwLock<AgentInner>>,
}

impl Agent {
    /// Set the approval mode at runtime.
    pub fn set_approval_mode(&self, mode: ApprovalMode) {
        self.inner.write().unwrap().approval_mode = mode;
    }

    /// Get the current approval mode.
    pub fn approval_mode(&self) -> ApprovalMode {
        self.inner.read().unwrap().approval_mode
    }

    /// Spawn an async background task to extract structured notes from a turn transcript.
    /// Does nothing if `extract_memories` is disabled or no LLM is configured.
    pub(crate) fn maybe_extract_memories(&self, transcript: String) {
        if !self.config.extract_memories {
            return;
        }
        if let Some(ref llm) = self.inner.read().unwrap().llm {
            let llm = llm.clone();
            let working_dir = self.config.working_dir.clone();
            tokio::spawn(async move {
                let extractor = crate::memory::TurnMemoryExtractor::new(llm, working_dir);
                match extractor.extract(&transcript).await {
                    Ok(notes) => {
                        tracing::info!("Extracted session notes: {:?}", notes);
                    }
                    Err(e) => {
                        tracing::warn!("Memory extraction failed: {}", e);
                    }
                }
            });
        }
    }
}
