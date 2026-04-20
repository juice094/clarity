//! Agent construction, configuration, and utility methods.

use super::config::{AgentConfig, DEFAULT_MAX_CONTEXT_TOKENS};
use super::Agent;
use crate::agent::compaction_service::CompactionService;
use crate::agent::enhanced::TokenUsage;
use crate::approval::{ApprovalMode, ApprovalRuntime};
use crate::compaction::CompactionConfig;
use crate::llm::api::LlmProvider;
use crate::memory::{ChunkConfig, Chunker, Memory, MemoryStore, MemoryTicker};
use crate::registry::ToolRegistry;
use crate::skills::SkillRegistry;
use clarity_wire::{Wire, WireMessage};
use std::sync::{Arc, Mutex};
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
            llm: Arc::new(std::sync::RwLock::new(None)),
            memory_store: None,
            memory_ticker: None,
            wire: None,
            approval_runtime: None,
            approval_mode: ApprovalMode::Interactive,
            compaction_config: CompactionConfig::default(),
            max_context_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
            compaction_service: config.compaction_service.map(CompactionService::new),
            cancel_token: CancellationToken::new(),
            session_usage: Arc::new(Mutex::new(TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            })),
            skill_registry: None,
            active_skill: Arc::new(std::sync::RwLock::new(None)),
            file_prompt_cache: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    /// Set the LLM provider (builder pattern, for construction only)
    pub fn with_llm(self, llm: Arc<dyn LlmProvider>) -> Self {
        *self.llm.write().unwrap() = Some(llm);
        self
    }

    /// Hot-swap the LLM provider at runtime.
    /// All clones of this Agent will see the new provider immediately.
    pub fn set_llm(&self, llm: Arc<dyn LlmProvider>) {
        *self.llm.write().unwrap() = Some(llm);
    }

    /// Set the skill registry.
    pub fn with_skill_registry(mut self, registry: SkillRegistry) -> Self {
        self.skill_registry = Some(registry);
        self
    }

    /// Set (or clear) the active skill by id.
    /// All clones of this Agent will see the change immediately.
    pub fn set_active_skill(&self, skill_id: Option<String>) {
        *self.active_skill.write().unwrap() = skill_id;
    }

    /// Get the currently active skill id, if any.
    pub fn active_skill(&self) -> Option<String> {
        self.active_skill.read().unwrap().clone()
    }

    /// Remove the LLM provider at runtime.
    pub fn clear_llm(&self) {
        *self.llm.write().unwrap() = None;
    }

    /// Set the memory store
    pub fn with_memory(mut self, store: Arc<dyn MemoryStore>) -> Self {
        self.memory_store = Some(store);
        self
    }

    /// Set the memory ticker
    pub fn with_memory_ticker(mut self, ticker: MemoryTicker) -> Self {
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

    /// Set the compaction service
    pub fn with_compaction_service(mut self, service: CompactionService) -> Self {
        self.compaction_service = Some(service);
        self
    }

    /// Cancel any in-flight agent run.
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// Reset the cancellation token so a new turn can run.
    pub fn reset_cancel_token(&mut self) {
        self.cancel_token = CancellationToken::new();
    }

    /// Access the configured approval runtime, if any.
    pub fn approval_runtime(&self) -> Option<Arc<dyn ApprovalRuntime>> {
        self.approval_runtime.clone()
    }

    /// Accumulate token usage into the session counter.
    pub(crate) fn accumulate_usage(&self, prompt_tokens: u32, completion_tokens: u32) {
        let mut usage = self.session_usage.lock().unwrap();
        usage.prompt_tokens += prompt_tokens;
        usage.completion_tokens += completion_tokens;
        usage.total_tokens = usage.prompt_tokens + usage.completion_tokens;
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
        self.session_usage.lock().unwrap().clone()
    }

    /// Get the tool registry
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Get the LLM provider (if configured)
    pub fn llm(&self) -> Option<Arc<dyn LlmProvider>> {
        self.llm.read().unwrap().clone()
    }

    /// Get the agent configuration
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }
}
