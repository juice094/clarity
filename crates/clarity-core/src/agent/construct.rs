//! Agent construction, configuration, and utility methods.

use super::config::{AgentConfig, DEFAULT_MAX_CONTEXT_TOKENS};
use super::{Agent, AgentInner, AgentState};
use crate::agent::compaction_service::CompactionService;
use crate::agent::enhanced::TokenUsage;
use crate::approval::{ApprovalMode, ApprovalRuntime};
use crate::compaction::CompactionConfig;
use crate::llm::api::LlmProvider;
use crate::memory::{ChunkConfig, Chunker, Memory, MemoryStore, SharedMemoryTicker};
use std::collections::HashMap;
use std::future::Future;
use crate::registry::ToolRegistry;
use crate::skills::SkillRegistry;
use clarity_wire::{Wire, WireMessage};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::warn;

impl Agent {
    /// Create a new Agent with the given registry and default config
    pub fn new(registry: ToolRegistry) -> Self {
        Self::with_config(registry, AgentConfig::default())
    }

    /// Create a new Agent with custom configuration
    pub fn with_config(registry: ToolRegistry, config: AgentConfig) -> Self {
        Self {
            registry,
            config: config.clone(),
            memory_store: None,
            memory_ticker: None,
            wire: None,
            approval_runtime: None,
            approval_mode: ApprovalMode::Interactive,
            compaction_config: CompactionConfig::default(),
            max_context_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
            compaction_service: config.compaction_service.map(CompactionService::new),
            skill_registry: None,
            inner: Arc::new(std::sync::RwLock::new(AgentInner {
                state: AgentState::Unconfigured,
                llm: None,
                session_usage: TokenUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                },
                active_skill: None,
                snapshotted_skill: None,
                file_prompt_cache: None,
                active_file_paths: Vec::new(),
            })),
        }
    }

    /// Set the LLM provider (builder pattern, for construction only)
    pub fn with_llm(self, llm: Arc<dyn LlmProvider>) -> Self {
        {
            let mut inner = self.inner.write().unwrap();
            inner.llm = Some(llm);
            inner.state = AgentState::Idle;
        }
        self
    }

    /// Hot-swap the LLM provider at runtime.
    /// All clones of this Agent will see the new provider immediately.
    pub fn set_llm(&self, llm: Arc<dyn LlmProvider>) {
        let mut inner = self.inner.write().unwrap();
        inner.llm = Some(llm);
        if matches!(inner.state, AgentState::Unconfigured) {
            inner.state = AgentState::Idle;
        }
    }

    /// Set the skill registry.
    pub fn with_skill_registry(mut self, registry: SkillRegistry) -> Self {
        self.skill_registry = Some(registry);
        self
    }

    /// Set (or clear) the active skill by id.
    /// All clones of this Agent will see the change immediately.
    pub fn set_active_skill(&self, skill_id: Option<String>) {
        let mut inner = self.inner.write().unwrap();
        inner.active_skill = skill_id;
    }

    /// Get the currently active skill id, if any.
    pub fn active_skill(&self) -> Option<String> {
        self.inner.read().unwrap().active_skill.clone()
    }

    /// Set the file paths representing the current user operation.
    /// These paths are used to dynamically activate skills at turn start.
    pub fn set_active_file_paths(&self, paths: Vec<std::path::PathBuf>) {
        let mut inner = self.inner.write().unwrap();
        inner.active_file_paths = paths;
    }

    /// Get the currently set active file paths.
    pub fn active_file_paths(&self) -> Vec<std::path::PathBuf> {
        self.inner.read().unwrap().active_file_paths.clone()
    }

    /// Remove the LLM provider at runtime.
    pub fn clear_llm(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.llm = None;
        inner.state = AgentState::Unconfigured;
    }

    /// Set the memory store
    pub fn with_memory(mut self, store: Arc<dyn MemoryStore>) -> Self {
        self.memory_store = Some(store);
        self
    }

    /// Set the memory ticker (uses `clarity_memory::SharedMemoryTicker` for
    /// thread-safe async operation with per-session turn counting).
    pub fn with_memory_ticker(mut self, ticker: SharedMemoryTicker) -> Self {
        self.memory_ticker = Some(ticker);
        self
    }

    /// Set the wire for UI communication (builder pattern)
    pub fn with_wire(mut self, wire: Arc<Wire>) -> Self {
        self.wire = Some(wire);
        self
    }

    /// Send a wire message if wire is configured
    pub(crate) fn send_wire_message(&self, msg: WireMessage) {
        if let Some(ref wire) = self.wire {
            wire.soul_side().send(msg);
        }
    }

    /// Set the approval runtime
    pub fn with_approval_runtime(mut self, runtime: Arc<dyn ApprovalRuntime>) -> Self {
        self.approval_runtime = Some(runtime);
        self
    }

    /// Set the approval mode
    pub fn with_approval_mode(mut self, mode: ApprovalMode) -> Self {
        self.approval_mode = mode;
        self
    }

    /// Set the approval mode at runtime.
    pub fn set_approval_mode(&mut self, mode: ApprovalMode) {
        self.approval_mode = mode;
    }

    /// Get the current approval mode.
    pub fn approval_mode(&self) -> ApprovalMode {
        self.approval_mode
    }

    /// Set the compaction configuration
    pub fn with_compaction_config(mut self, config: CompactionConfig) -> Self {
        self.compaction_config = config;
        self
    }

    /// Set the maximum context tokens
    pub fn with_max_context_tokens(mut self, max_tokens: usize) -> Self {
        self.max_context_tokens = max_tokens;
        self
    }

    /// Set the memory compilation callback on the ticker, if one is configured.
    ///
    /// This allows the agent to perform OpenHanako-style four-level memory
    /// compilation (today → week → long-term → facts) when the turn threshold
    /// is reached.
    pub async fn set_memory_compile_callback<F, Fut>(&self, callback: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = clarity_memory::Result<HashMap<String, clarity_memory::CompileStatus>>> + Send + 'static,
    {
        if let Some(ref ticker) = self.memory_ticker {
            ticker.set_compile_callback(callback).await;
        }
    }

    /// Set the compaction service
    pub fn with_compaction_service(mut self, service: CompactionService) -> Self {
        self.compaction_service = Some(service);
        self
    }

    /// Set capability token for subagent permission isolation
    pub fn with_capability_token(
        mut self,
        token: crate::subagents::token::CapabilityToken,
    ) -> Self {
        self.config.capability_token = Some(token);
        self
    }

    /// Cancel any in-flight agent run.
    pub fn cancel(&self) {
        let mut inner = self.inner.write().unwrap();
        if let AgentState::Running { ref cancel_token } = inner.state {
            cancel_token.cancel();
            inner.state = AgentState::Stalled;
        }
    }

    /// Reset the agent state so a new turn can run.
    pub fn reset(&self) {
        let mut inner = self.inner.write().unwrap();
        if inner.llm.is_some() {
            inner.state = AgentState::Idle;
        } else {
            inner.state = AgentState::Unconfigured;
        }
    }

    /// Access the configured approval runtime, if any.
    pub fn approval_runtime(&self) -> Option<Arc<dyn ApprovalRuntime>> {
        self.approval_runtime.clone()
    }

    /// Accumulate token usage into the session counter.
    pub(crate) fn accumulate_usage(&self, prompt_tokens: u32, completion_tokens: u32) {
        let mut inner = self.inner.write().unwrap();
        inner.session_usage.prompt_tokens += prompt_tokens;
        inner.session_usage.completion_tokens += completion_tokens;
        inner.session_usage.total_tokens =
            inner.session_usage.prompt_tokens + inner.session_usage.completion_tokens;
    }

    /// Store a conversation memory, optionally chunking long content for better retrieval.
    pub(crate) async fn store_conversation_memory(&self, content: impl Into<String>) {
        let content = content.into();
        if let Some(ref store) = self.memory_store {
            // Store the full memory for context completeness
            let full_memory =
                Memory::new(content.clone()).with_tags(vec!["conversation".to_string()]);
            if let Err(e) = store.store(full_memory).await {
                warn!("Failed to store memory: {}", e);
            }

            // If content is long, also store chunks for granular retrieval
            const CHUNK_THRESHOLD: usize = 1024;
            if content.len() > CHUNK_THRESHOLD {
                let config = ChunkConfig::new().with_chunk_size(512).with_overlap(50);
                let chunks = Chunker::split(&content, &config);
                for chunk in chunks {
                    let chunk_memory = Memory::new(chunk.content)
                        .with_tags(vec!["conversation".to_string(), "chunk".to_string()]);
                    if let Err(e) = store.store(chunk_memory).await {
                        warn!("Failed to store memory chunk: {}", e);
                    }
                }
            }
        }
    }

    /// Get accumulated session token usage.
    pub fn get_session_usage(&self) -> TokenUsage {
        self.inner.read().unwrap().session_usage.clone()
    }

    /// Get the tool registry
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Get the LLM provider (if configured)
    pub fn llm(&self) -> Option<Arc<dyn LlmProvider>> {
        self.inner.read().unwrap().llm.clone()
    }

    /// Get the agent configuration
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Query the current agent state.
    pub fn state(&self) -> AgentState {
        self.inner.read().unwrap().state.clone()
    }

    /// Check whether the agent is currently running a turn.
    pub fn is_running(&self) -> bool {
        matches!(self.state(), AgentState::Running { .. })
    }

    /// Internal: atomically attempt to transition from Idle to Running.
    /// Returns the fresh CancellationToken on success.
    pub(crate) fn begin_turn(&self) -> Result<CancellationToken, crate::error::AgentError> {
        let mut inner = self.inner.write().unwrap();
        match &inner.state {
            AgentState::Unconfigured => Err(crate::error::AgentError::Unconfigured),
            AgentState::Running { .. } => Err(crate::error::AgentError::AlreadyRunning),
            AgentState::Stalled => Err(crate::error::AgentError::Stalled),
            AgentState::Idle => {
                let token = CancellationToken::new();
                inner.state = AgentState::Running {
                    cancel_token: token.clone(),
                };
                inner.session_usage = TokenUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                };
                inner.snapshotted_skill = inner.active_skill.clone();
                Ok(token)
            }
        }
    }

    /// Internal: transition from Running to Idle.
    pub(crate) fn finish_turn(&self) {
        let mut inner = self.inner.write().unwrap();
        if matches!(inner.state, AgentState::Running { .. }) {
            inner.state = AgentState::Idle;
        }
        inner.snapshotted_skill = None;
    }

    /// Internal: access the snapshotted active skill for the current turn.
    pub(crate) fn snapshotted_active_skill(&self) -> Option<String> {
        self.inner.read().unwrap().snapshotted_skill.clone()
    }

    /// Internal: access the file prompt cache.
    pub(crate) fn file_prompt_cache(&self) -> Option<String> {
        self.inner.read().unwrap().file_prompt_cache.clone()
    }

    /// Internal: set the file prompt cache.
    pub(crate) fn set_file_prompt_cache(&self, value: Option<String>) {
        self.inner.write().unwrap().file_prompt_cache = value;
    }

    /// Run multiple subagents in parallel.
    ///
    /// Creates a temporary `SubagentManager` with the agent's tool registry,
    /// working directory, and LLM configuration, then executes the given
    /// specs concurrently.
    pub async fn run_parallel(
        &self,
        specs: Vec<crate::subagents::RunSpec>,
        config: crate::subagents::ParallelConfig,
    ) -> anyhow::Result<crate::subagents::ParallelResult> {
        use crate::subagents::SubagentManager;

        let mut manager = SubagentManager::new(
            self.registry.clone(),
            &self.config.working_dir,
            self.config.working_dir.join("subagent_context"),
        );

        if let Some(llm) = self.llm() {
            manager = manager.with_llm(llm);
        }

        self.send_wire_message(WireMessage::TurnBegin {
            user_input: format!("parallel execution ({} tasks)", specs.len()),
        });

        let result = manager.run_parallel(specs, config).await;

        self.send_wire_message(WireMessage::TurnEnd);

        result
    }
}
