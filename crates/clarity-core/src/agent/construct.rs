//! Agent construction, configuration, and utility methods.

use super::config::{AgentConfig, DEFAULT_MAX_CONTEXT_TOKENS};
use super::{Agent, AgentError, AgentInner, AgentState};
use crate::agent::compaction_service::CompactionService;
use crate::agent::enhanced::TokenUsage;
use crate::approval::{ApprovalMode, ApprovalRuntime};
use crate::compaction::CompactionConfig;
use crate::memory::{ChunkConfig, Chunker, Memory, MemoryStore, SharedMemoryTicker};
use crate::registry::ToolRegistry;
use crate::skills::SkillRegistry;
use clarity_llm::api::LlmProvider;
use clarity_wire::{Wire, WireMessage};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

impl Agent {
    /// Create a new Agent with the given registry and default config
    pub fn new(registry: ToolRegistry) -> Self {
        Self::with_config(registry, AgentConfig::default())
    }

    /// Create a new Agent with custom configuration
    pub fn with_config(registry: ToolRegistry, config: AgentConfig) -> Self {
        // Runtime self-check: log any tools pending configuration
        if let Ok((ready, pending)) = registry.self_check() {
            if !pending.is_empty() {
                warn!(
                    "ToolRegistry self-check: {} ready, {} pending — {:?}",
                    ready,
                    pending.len(),
                    pending
                );
            } else {
                info!("ToolRegistry self-check: all {} tools ready", ready);
            }
        }

        Self {
            registry,
            config: config.clone(),
            wire: None,
            event_bus: None,
            approval_runtime: None,
            compaction_config: CompactionConfig::default(),
            max_context_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
            compaction_service: config.compaction_service.as_ref().map(|c| {
                let mut service = CompactionService::new(c.clone());
                if let Some(ref path) = c.session_store_path {
                    if let Ok(store) = clarity_memory::SessionStore::new(path) {
                        let session_id = c
                            .session_id
                            .clone()
                            .or_else(|| config.session_id.clone())
                            .unwrap_or_else(|| "default".to_string());
                        service = service.with_session_store(Arc::new(store), session_id);
                    }
                }
                service
            }),
            memory_ticker: if let Some(turns) = config.memory_ticker_turns {
                let output_dir = config
                    .compiled_memory_dir
                    .clone()
                    .unwrap_or_else(|| config.working_dir.join(".clarity_memory"));
                Some(SharedMemoryTicker::new(clarity_memory::MemoryTicker::new(
                    &output_dir,
                    Some(turns),
                )))
            } else {
                None
            },
            hook_registry: None,
            llm_factory: None,
            memory_factory: None,
            skill_factory: None,
            plan_controller: Arc::new(tokio::sync::Mutex::new(None)),
            orchestrator: None,
            inner: Arc::new(parking_lot::RwLock::new(AgentInner {
                state: AgentState::Unconfigured,
                llm: None,
                memory_store: None,
                skill_registry: None,
                active_skill: None,
                file_prompt_cache: None,
                active_file_paths: Vec::new(),
                approval_mode: ApprovalMode::default(),
                git_context: None,
                active_files: None,
                project_metadata: None,
                provider_label: None,
                hook_registry: None,
                daily_cost_usd: 0.0,
                last_cost_date: chrono::Utc::now().date_naive(),
                vision_llm: None,
                turn_context: None,
                last_turn_message_count: 0,
                fallback_llms: Vec::new(),
                static_prompt_hash: None,
                jumpy_predictor: None,
                lsp_initialized: false,
                snapshot_service: None,
            })),
        }
    }

    /// Set the subagent orchestrator (builder pattern).
    pub fn with_orchestrator(
        mut self,
        orchestrator: Arc<dyn clarity_contract::subagent::SubagentOrchestrator>,
    ) -> Self {
        self.orchestrator = Some(orchestrator);
        self
    }

    /// Set the vision LLM provider (builder pattern).
    pub fn with_vision_llm(self, llm: Arc<dyn LlmProvider>) -> Self {
        {
            let mut inner = self.inner.write();
            inner.vision_llm = Some(llm);
        }
        self
    }

    /// Get the vision LLM provider, falling back to the default provider.
    pub fn vision_llm(&self) -> Option<Arc<dyn LlmProvider>> {
        let inner = self.inner.read();
        inner.vision_llm.clone().or_else(|| inner.llm.clone())
    }

    /// Set fallback LLM providers.
    ///
    /// When non-empty, the primary LLM set via `with_llm` / `set_llm` is
    /// automatically wrapped in a [`ReliableProvider`](clarity_llm::ReliableProvider)
    /// so failures fall back through this chain.
    pub fn with_fallback_llms(self, fallbacks: Vec<Arc<dyn LlmProvider>>) -> Self {
        {
            let mut inner = self.inner.write();
            inner.fallback_llms = fallbacks;
            // Re-wrap existing LLM if any
            if let Some(ref existing) = inner.llm {
                let mut providers = vec![existing.clone()];
                providers.extend(inner.fallback_llms.clone());
                inner.llm = Some(Arc::new(clarity_llm::ReliableProvider::new(providers)));
            }
        }
        self
    }

    /// Set the LLM provider (builder pattern, for construction only)
    pub fn with_llm(self, llm: Arc<dyn LlmProvider>) -> Self {
        {
            let mut inner = self.inner.write();
            let llm = if inner.fallback_llms.is_empty() {
                llm
            } else {
                let mut providers = vec![llm];
                providers.extend(inner.fallback_llms.clone());
                Arc::new(clarity_llm::ReliableProvider::new(providers))
            };
            inner.llm = Some(llm);
            inner.state = AgentState::Idle;
        }
        self
    }

    /// Set the Jumpy World Model predictor (builder pattern).
    pub fn with_jumpy_predictor(
        self,
        predictor: Arc<dyn crate::agent::jumpy::predictor::OutcomePredictor>,
    ) -> Self {
        {
            let mut inner = self.inner.write();
            inner.jumpy_predictor = Some(predictor);
        }
        self
    }

    /// Hot-swap the LLM provider at runtime.
    /// All clones of this Agent will see the new provider immediately.
    pub fn set_llm(&self, llm: Arc<dyn LlmProvider>) {
        let mut inner = self.inner.write();
        let llm = if inner.fallback_llms.is_empty() {
            llm
        } else {
            let mut providers = vec![llm];
            providers.extend(inner.fallback_llms.clone());
            Arc::new(clarity_llm::ReliableProvider::new(providers))
        };
        inner.llm = Some(llm);
        if matches!(inner.state, AgentState::Unconfigured) {
            inner.state = AgentState::Idle;
        }
    }

    /// Remove the current LLM binding and revert to Unconfigured state.
    pub fn unset_llm(&self) {
        let mut inner = self.inner.write();
        inner.llm = None;
        inner.state = AgentState::Unconfigured;
    }

    /// Set the provider label for internal tracing/audit.
    /// This is NOT injected into the system prompt.
    pub fn set_provider_label(&self, label: impl Into<String>) {
        let mut inner = self.inner.write();
        inner.provider_label = Some(label.into());
    }

    /// Get the provider label (internal tracing only).
    pub fn provider_label(&self) -> Option<String> {
        self.inner.read().provider_label.clone()
    }

    /// Set the skill registry.
    pub fn with_skill_registry(self, registry: SkillRegistry) -> Self {
        registry.validate_tools(&self.registry);
        {
            let mut inner = self.inner.write();
            inner.skill_registry = Some(registry);
        }
        self
    }

    /// Set (or clear) the active skill by id.
    /// All clones of this Agent will see the change immediately.
    #[deprecated(note = "Use skill_registry().toggle_active() instead")]
    pub fn set_active_skill(&self, skill_id: Option<String>) {
        let mut inner = self.inner.write();
        inner.active_skill = skill_id;
    }

    /// Get the currently active skill id, if any.
    #[deprecated(note = "Use skill_registry().active_ids() instead")]
    pub fn active_skill(&self) -> Option<String> {
        self.inner.read().active_skill.clone()
    }

    /// List all skills from the registry.
    pub fn list_skills(&self) -> Vec<crate::skills::Skill> {
        self.skill_registry()
            .map(|r| r.list_skills())
            .unwrap_or_default()
    }

    /// Get the set of active skill ids from the registry.
    pub fn skill_active_ids(&self) -> std::collections::HashSet<String> {
        self.skill_registry()
            .map(|r| r.active_ids())
            .unwrap_or_default()
    }

    /// Set a skill's active state in the registry.
    pub fn set_skill_active(&self, id: &str, active: bool) {
        if let Some(ref registry) = self.skill_registry() {
            let currently_active = registry.is_active(id);
            if active != currently_active {
                registry.toggle_active(id);
            }
        }
    }

    /// Discover skills for the current working directory.
    pub fn discover_skills(&self) -> Vec<String> {
        self.skill_registry()
            .map(|r| r.discover_for_path(&self.config.working_dir))
            .unwrap_or_default()
    }

    /// Set the file paths representing the current user operation.
    /// These paths are used to dynamically activate skills at turn start.
    pub fn set_active_file_paths(&self, paths: Vec<std::path::PathBuf>) {
        let mut inner = self.inner.write();
        inner.active_file_paths = paths;
    }

    /// Get the currently set active file paths.
    pub fn active_file_paths(&self) -> Vec<std::path::PathBuf> {
        self.inner.read().active_file_paths.clone()
    }

    /// Remove the LLM provider at runtime.
    pub fn clear_llm(&self) {
        let mut inner = self.inner.write();
        inner.llm = None;
        inner.state = AgentState::Unconfigured;
    }

    /// Set the memory store
    pub fn with_memory(self, store: Arc<dyn MemoryStore>) -> Self {
        {
            let mut inner = self.inner.write();
            inner.memory_store = Some(store);
        }
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

    /// Set the event bus for structured event output (builder pattern)
    pub fn with_event_bus(mut self, bus: clarity_wire::EventBus) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Send a wire message if wire is configured.
    /// Also bridges to the event bus if configured.
    pub(crate) fn send_wire_message(&self, msg: WireMessage) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(clarity_wire::Event::from(msg.clone()));
        }
        if let Some(ref wire) = self.wire {
            wire.soul_side().send(msg);
        }
    }

    /// Set the approval runtime
    pub fn with_approval_runtime(mut self, runtime: Arc<dyn ApprovalRuntime>) -> Self {
        self.approval_runtime = Some(runtime);
        self
    }

    /// Set the approval mode (builder pattern)
    pub fn with_approval_mode(self, mode: ApprovalMode) -> Self {
        {
            let mut inner = self.inner.write();
            inner.approval_mode = mode;
        }
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

    /// Set the memory compilation callback on the ticker, if one is configured.
    ///
    /// This allows the agent to perform OpenHanako-style four-level memory
    /// compilation (today → week → long-term → facts) when the turn threshold
    /// is reached.
    pub async fn set_memory_compile_callback<F, Fut>(&self, callback: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = clarity_memory::Result<HashMap<String, clarity_memory::CompileStatus>>>
            + Send
            + 'static,
    {
        if let Some(ref ticker) = self.memory_ticker {
            ticker.set_compile_callback(callback).await;
        }
    }

    /// Initialize memory compilation by wiring the MemoryCompiler to the ticker.
    ///
    /// Must be called after the LLM provider is set (via `with_llm` or `set_llm`).
    pub async fn initialize_memory_compilation(&self) {
        if let Some(ref _ticker) = self.memory_ticker {
            if let Some(ref compiled_dir) = self.config.compiled_memory_dir {
                if let Some(ref llm) = self.llm() {
                    let adapter = Arc::new(crate::memory::LlmProviderAdapter::new(llm.clone()));
                    let store = match clarity_memory::MemoryStore::new_in_memory() {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!(
                                "Failed to create in-memory store for memory compiler: {}",
                                e
                            );
                            return;
                        }
                    };
                    let session_store = self
                        .compaction_service
                        .as_ref()
                        .and_then(|s| s.session_store())
                        .map(|arc| (*arc).clone())
                        .unwrap_or_else(|| {
                            let path = self.config.working_dir.join(".clarity_sessions");
                            clarity_memory::SessionStore::new(&path).unwrap_or_else(|_| {
                                clarity_memory::SessionStore::new(
                                    std::env::temp_dir().join("clarity_sessions"),
                                )
                                .expect("Failed to create fallback session store")
                            })
                        });
                    let config = clarity_memory::CompileConfig::default();
                    let compiler = Arc::new(tokio::sync::Mutex::new(
                        clarity_memory::MemoryCompiler::new(store, session_store, adapter, config),
                    ));
                    let output_dir = compiled_dir.clone();
                    self.set_memory_compile_callback(move || {
                        let compiler = compiler.clone();
                        let output_dir = output_dir.clone();
                        async move {
                            let mut compiler = compiler.lock().await;
                            compiler.compile_all(&output_dir).await
                        }
                    })
                    .await;
                }
            }
        }
    }

    /// Set the compaction service
    pub fn with_compaction_service(mut self, service: CompactionService) -> Self {
        self.compaction_service = Some(service);
        self
    }

    /// Set the hook registry for lifecycle interception.
    pub fn with_hook_registry(mut self, registry: crate::hooks::HookRegistry) -> Self {
        self.hook_registry = Some(registry);
        self
    }

    /// Set the agent lifecycle hook registry (builder pattern).
    pub fn with_hooks(self, hooks: super::hooks::HookRegistry) -> Self {
        self.inner.write().hook_registry =
            Some(std::sync::Arc::new(tokio::sync::RwLock::new(hooks)));
        self
    }

    /// Set a lazy LLM factory — called on first `run()` if no LLM is set.
    ///
    /// This allows deferring heavy LLM initialization (e.g. model loading,
    /// API key validation) until the agent is actually used.
    pub fn with_llm_factory(mut self, factory: super::LlmFactoryFn) -> Self {
        self.llm_factory = Some(factory);
        self
    }

    /// Set a lazy MemoryStore factory — called on first `run()` if no store is set.
    ///
    /// This allows deferring SQLite connection and FTS5 index creation
    /// until the agent is actually used.
    pub fn with_memory_factory(mut self, factory: super::MemoryFactoryFn) -> Self {
        self.memory_factory = Some(factory);
        self
    }

    /// Set a lazy SkillRegistry factory — called on first `run()` if no registry is set.
    pub fn with_skill_factory(mut self, factory: super::SkillFactoryFn) -> Self {
        self.skill_factory = Some(factory);
        self
    }

    /// Ensure all lazy-initialized components are ready.
    ///
    /// Called automatically at the start of every `run()` variant.
    /// If a factory is configured but its component is not yet initialized,
    /// this method will call the factory and install the result.
    pub async fn ensure_initialized(&self) -> Result<(), AgentError> {
        // Initialize LLM if needed
        let needs_llm_init = {
            let inner = self.inner.read();
            inner.llm.is_none() && self.llm_factory.is_some()
        };
        if needs_llm_init {
            if let Some(ref factory) = self.llm_factory {
                let llm = factory().await?;
                self.set_llm(llm);
            }
        }

        // Initialize MemoryStore if needed
        let needs_memory_init = {
            let inner = self.inner.read();
            inner.memory_store.is_none() && self.memory_factory.is_some()
        };
        if needs_memory_init {
            if let Some(ref factory) = self.memory_factory {
                let store = factory().await?;
                let mut inner = self.inner.write();
                inner.memory_store = Some(store);
            }
        }

        // Initialize SkillRegistry if needed
        let needs_skill_init = {
            let inner = self.inner.read();
            inner.skill_registry.is_none() && self.skill_factory.is_some()
        };
        if needs_skill_init {
            if let Some(ref factory) = self.skill_factory {
                let registry = factory().await?;
                registry.validate_tools(&self.registry);
                let mut inner = self.inner.write();
                inner.skill_registry = Some(registry);
            }
        }

        // Initialize LSP hook if configured and not yet initialized
        let needs_lsp_init = {
            let inner = self.inner.read();
            !inner.lsp_initialized && self.config.lsp_config.is_some()
        };
        if needs_lsp_init {
            if let Some(ref lsp_config) = self.config.lsp_config {
                if lsp_config.enabled {
                    if let Some(hook) =
                        super::lsp::LspHook::try_new(lsp_config, &self.config.working_dir).await
                    {
                        let hook_registry_opt = {
                            let inner = self.inner.read();
                            inner.hook_registry.clone()
                        };
                        if let Some(arc) = hook_registry_opt {
                            let mut registry = arc.write().await;
                            registry.register(Box::new(hook));
                        } else {
                            let mut registry = super::hooks::HookRegistry::new();
                            registry.register(Box::new(hook));
                            let mut inner = self.inner.write();
                            inner.hook_registry =
                                Some(std::sync::Arc::new(tokio::sync::RwLock::new(registry)));
                        }
                    }
                }
            }
            let mut inner = self.inner.write();
            inner.lsp_initialized = true;
        }

        // Initialize snapshot service if configured and not yet initialized
        let needs_snapshot_init = {
            let inner = self.inner.read();
            inner.snapshot_service.is_none() && self.config.snapshot_config.is_some()
        };
        if needs_snapshot_init {
            if let Some(ref snapshot_config) = self.config.snapshot_config {
                if let Some(service) = super::snapshot::SnapshotService::try_new(
                    snapshot_config,
                    &self.config.working_dir,
                )
                .await
                {
                    let service = std::sync::Arc::new(service);
                    let tool = super::snapshot::GitRestoreTool::new(service.clone());
                    let _ = self.registry.register(tool);
                    let mut inner = self.inner.write();
                    inner.snapshot_service = Some(service);
                }
            }
        }

        Ok(())
    }

    /// Set snapshot configuration.
    pub fn with_snapshot_config(mut self, config: super::snapshot::SnapshotConfig) -> Self {
        self.config.snapshot_config = Some(config);
        self
    }

    /// Set capability token for subagent permission isolation
    pub fn with_capability_token(
        mut self,
        token: clarity_contract::subagent::CapabilityToken,
    ) -> Self {
        self.config.capability_token = Some(token);
        self
    }

    /// Cancel any in-flight agent run.
    pub fn cancel(&self) {
        let mut inner = self.inner.write();
        if let AgentState::Running { ref cancel_token } = inner.state {
            cancel_token.cancel();
            inner.state = AgentState::Stalled;
        }
    }

    /// Reset the agent state so a new turn can run.
    pub fn reset(&self) {
        let mut inner = self.inner.write();
        if inner.llm.is_some() {
            inner.state = AgentState::Idle;
        } else {
            inner.state = AgentState::Unconfigured;
        }
        inner.static_prompt_hash = None;
    }

    /// Access the configured approval runtime, if any.
    pub fn approval_runtime(&self) -> Option<Arc<dyn ApprovalRuntime>> {
        self.approval_runtime.clone()
    }

    /// Accumulate token usage into the session counter.
    pub(crate) fn accumulate_usage(&self, prompt_tokens: u32, completion_tokens: u32) {
        let mut inner = self.inner.write();
        if let Some(ref mut ctx) = inner.turn_context {
            ctx.session_usage.prompt_tokens += prompt_tokens;
            ctx.session_usage.completion_tokens += completion_tokens;
            ctx.session_usage.total_tokens =
                ctx.session_usage.prompt_tokens + ctx.session_usage.completion_tokens;
        }
    }

    /// Store a conversation memory, optionally chunking long content for better retrieval.
    pub(crate) async fn store_conversation_memory(&self, content: impl Into<String>) {
        let content = content.into();
        if let Some(ref store) = self.memory_store() {
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
        self.inner
            .read()
            .turn_context
            .as_ref()
            .map(|c| c.session_usage.clone())
            .unwrap_or(TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            })
    }

    /// Get the tool registry
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Bind a [`BackgroundTaskManager`](crate::background::BackgroundTaskManager) to the cron tools in the registry.
    ///
    /// Must be called after the task manager is created and before the agent
    /// starts processing requests that may invoke cron tools.
    pub fn with_cron_manager(&self, manager: Arc<crate::background::BackgroundTaskManager>) {
        self.registry.with_cron_manager(manager);
    }

    /// Get the LLM provider (if configured)
    pub fn llm(&self) -> Option<Arc<dyn LlmProvider>> {
        self.inner.read().llm.clone()
    }

    /// Get the memory store (if configured)
    pub fn memory_store(&self) -> Option<Arc<dyn MemoryStore>> {
        self.inner.read().memory_store.clone()
    }

    /// Get the skill registry (if configured)
    pub fn skill_registry(&self) -> Option<SkillRegistry> {
        self.inner.read().skill_registry.clone()
    }

    /// Get the agent configuration
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Query the current agent state.
    pub fn state(&self) -> AgentState {
        self.inner.read().state.clone()
    }

    /// Check whether the agent is currently running a turn.
    pub fn is_running(&self) -> bool {
        matches!(self.state(), AgentState::Running { .. })
    }

    /// Internal: atomically attempt to transition from Idle to Running.
    /// Returns the fresh CancellationToken on success.
    pub(crate) fn begin_turn(&self) -> Result<CancellationToken, crate::error::AgentError> {
        let mut inner = self.inner.write();
        match &inner.state {
            AgentState::Unconfigured => Err(crate::error::AgentError::Unconfigured),
            AgentState::Running { .. } => Err(crate::error::AgentError::AlreadyRunning),
            AgentState::Stalled => Err(crate::error::AgentError::Stalled),
            AgentState::Idle => {
                let token = CancellationToken::new();
                inner.state = AgentState::Running {
                    cancel_token: token.clone(),
                };
                inner.turn_context = Some(super::turn_context::TurnContext::new(
                    inner.active_skill.clone(),
                    3,
                ));
                Ok(token)
            }
        }
    }

    /// Internal: transition from Running to Idle.
    pub(crate) fn finish_turn(&self) {
        let mut inner = self.inner.write();
        if matches!(inner.state, AgentState::Running { .. }) {
            inner.state = AgentState::Idle;
        }
        inner.turn_context = None;
    }

    /// Internal: access the snapshotted active skill for the current turn.
    pub(crate) fn snapshotted_active_skill(&self) -> Option<String> {
        self.inner
            .read()
            .turn_context
            .as_ref()
            .and_then(|c| c.snapshotted_skill.clone())
    }

    /// Internal: access the file prompt cache.
    pub(crate) fn file_prompt_cache(&self) -> Option<String> {
        self.inner.read().file_prompt_cache.clone()
    }

    /// Internal: set the file prompt cache.
    pub(crate) fn set_file_prompt_cache(&self, value: Option<String>) {
        self.inner.write().file_prompt_cache = value;
    }

    /// Run multiple subagents in parallel.
    ///
    /// Creates a temporary `SubagentManager` with the agent's tool registry,
    /// working directory, and LLM configuration, then executes the given
    /// specs concurrently.
    pub async fn run_parallel(
        &self,
        specs: Vec<clarity_contract::subagent::RunSpec>,
        config: clarity_contract::subagent::ParallelConfig,
        progress: Option<clarity_contract::subagent::BatchProgressHandle>,
    ) -> Result<clarity_contract::subagent::ParallelResult, clarity_contract::subagent::SubagentError>
    {
        let orchestrator = self.orchestrator.as_ref().ok_or_else(|| {
            clarity_contract::subagent::SubagentError::BuildFailed(
                "No subagent orchestrator configured".to_string(),
            )
        })?;

        self.send_wire_message(WireMessage::TurnBegin {
            user_input: format!("parallel execution ({} tasks)", specs.len()),
        });

        let result = orchestrator.run_parallel(specs, config, progress).await;

        self.send_wire_message(WireMessage::TurnEnd);

        result
    }
}
