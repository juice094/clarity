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
pub mod controller;
pub mod enhanced;
pub mod ops;

use crate::agent::compaction_service::{CompactionService, CompactionServiceConfig};
use crate::approval::{ApprovalMode, ApprovalResponse, ApprovalRuntime, ApprovalSource};
use crate::compaction::{estimate_message_tokens, CompactionConfig};
use crate::error::{AgentError, ToolError};
use crate::memory::{ChunkConfig, Chunker, Memory, MemoryStore, MemoryTicker};
use crate::registry::ToolRegistry;
use crate::skills::SkillRegistry;
use crate::tools::ToolContext;
use clarity_wire::{Wire, WireMessage};

/// Default max context size in tokens (approximate)
const DEFAULT_MAX_CONTEXT_TOKENS: usize = 8000;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

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

/// Configuration for the Agent
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum number of tool call iterations
    pub max_iterations: usize,
    /// Default timeout for tool execution (seconds)
    pub tool_timeout_secs: u64,
    /// Working directory for file operations
    pub working_dir: std::path::PathBuf,
    /// Read-only mode (prevents file modifications)
    pub read_only: bool,
    /// System prompt
    pub system_prompt: String,
    /// Entry-specific context appended to system prompt (methodology, persona, etc.)
    pub entry_context: String,
    /// Optional compaction service configuration
    pub compaction_service: Option<CompactionServiceConfig>,
    /// Directory containing Markdown prompt files
    pub prompts_dir: Option<std::path::PathBuf>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            tool_timeout_secs: 60,
            working_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            read_only: false,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            entry_context: String::new(),
            compaction_service: None,
            prompts_dir: None,
        }
    }
}

impl AgentConfig {
    /// Create a new config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set max iterations
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Set working directory
    pub fn with_working_dir(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.working_dir = path.into();
        self
    }

    /// Set read-only mode
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Set custom system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Set entry-specific context appended to system prompt
    pub fn with_entry_context(mut self, context: impl Into<String>) -> Self {
        self.entry_context = context.into();
        self
    }

    /// Set compaction service configuration
    pub fn with_compaction_service(mut self, config: CompactionServiceConfig) -> Self {
        self.compaction_service = Some(config);
        self
    }

    /// Set prompts directory for Markdown prompt files
    pub fn with_prompts_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.prompts_dir = Some(dir.into());
        self
    }
}

/// Load a prompt from a Markdown file, stripping YAML frontmatter if present.
fn load_prompt_from_file(path: &std::path::Path) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;

    let mut lines = contents.lines();

    // Check for YAML frontmatter starting with ---
    if let Some(first) = lines.next() {
        if first.trim() == "---" {
            // Skip until closing ---
            for line in lines.by_ref() {
                if line.trim() == "---" {
                    break;
                }
            }
            // Return remaining content
            let remaining: Vec<&str> = lines.collect();
            let result = remaining.join("\n").trim_start().to_string();
            if result.is_empty() {
                return None;
            }
            return Some(result);
        }
    }

    // No frontmatter, return full content
    Some(contents)
}

/// Default system prompt for the agent
const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a helpful AI assistant with access to various tools.
You can use these tools to help users with their tasks.

When you need to use a tool, respond with a tool call in the appropriate format.
After receiving the tool result, provide a helpful response to the user.

Available tools will be provided at the start of each conversation.
"#;

/// Simple mock LLM for testing
pub struct MockLlm;

#[async_trait::async_trait]
impl LlmProvider for MockLlm {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            content: "This is a mock response".to_string(),
            tool_calls: vec![],
            is_complete: true,
        })
    }

    fn stream(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
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
#[derive(Clone)]
pub struct Agent {
    registry: ToolRegistry,
    config: AgentConfig,
    llm: Arc<std::sync::RwLock<Option<Arc<dyn LlmProvider>>>>,
    memory_store: Option<Arc<dyn MemoryStore>>,
    memory_ticker: Option<MemoryTicker>,
    /// Optional wire for UI communication
    wire: Option<Arc<Wire>>,
    /// Approval runtime for tool execution control
    approval_runtime: Option<Arc<dyn ApprovalRuntime>>,
    /// Approval mode (Interactive, Yolo, Plan)
    approval_mode: ApprovalMode,
    /// Compaction configuration for context management
    compaction_config: CompactionConfig,
    /// Maximum context tokens before compaction
    max_context_tokens: usize,
    /// Optional compaction service for proactive history compression
    compaction_service: Option<CompactionService>,
    /// Cancellation token for interrupting in-flight runs
    cancel_token: CancellationToken,
    /// Session token usage accumulator
    session_usage: Arc<Mutex<TokenUsage>>,
    /// Optional skill registry for orchestration
    skill_registry: Option<SkillRegistry>,
    /// Currently active skill id (shared across clones)
    active_skill: Arc<std::sync::RwLock<Option<String>>>,
    /// Cached file prompt to avoid repeated disk I/O in build_system_prompt
    file_prompt_cache: Arc<std::sync::RwLock<Option<String>>>,
}

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
    fn send_wire_message(&self, msg: WireMessage) {
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
    fn accumulate_usage(&self, prompt_tokens: u32, completion_tokens: u32) {
        let mut usage = self.session_usage.lock().unwrap();
        usage.prompt_tokens += prompt_tokens;
        usage.completion_tokens += completion_tokens;
        usage.total_tokens = usage.prompt_tokens + usage.completion_tokens;
    }

    /// Store a conversation memory, optionally chunking long content for better retrieval.
    async fn store_conversation_memory(&self, content: impl Into<String>) {
        let content = content.into();
        if let Some(ref store) = self.memory_store {
            // Store the full memory for context completeness
            let full_memory = Memory::new(content.clone()).with_tags(vec!["conversation".to_string()]);
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

    /// Build the system prompt
    pub fn build_system_prompt(&self) -> String {
        let tool_descs = self.get_tool_descriptions();

        // Determine entry type from entry_context
        let entry = if self.config.entry_context.contains("方法论") || self.config.entry_context.contains("科学") {
            "window"
        } else if self.config.entry_context.contains("工程") {
            "cli"
        } else {
            "claw"
        };

        // Try to load prompt from file if prompts_dir is set.
        // Cache the result to avoid repeated disk I/O on every agent turn.
        let file_prompt = if self.config.prompts_dir.is_some() {
            let cached = self.file_prompt_cache.read().unwrap().clone();
            cached.or_else(|| {
                let prompt_path = self.config.prompts_dir.as_ref().unwrap().join(format!("{}.md", entry));
                let loaded = load_prompt_from_file(&prompt_path);
                *self.file_prompt_cache.write().unwrap() = loaded.clone();
                loaded
            })
        } else {
            None
        };

        let base = if let Some(prompt) = file_prompt {
            if tool_descs.is_empty() {
                prompt
            } else {
                format!("{}\n\n## Available Tools\n{}", prompt, tool_descs.join("\n"))
            }
        } else {
            if tool_descs.is_empty() {
                self.config.system_prompt.clone()
            } else {
                format!("{}\n\n## Available Tools\n{}", self.config.system_prompt, tool_descs.join("\n"))
            }
        };

        let with_entry = if self.config.entry_context.is_empty() {
            base
        } else {
            format!("{}\n\n{}", base, self.config.entry_context)
        };

        // Inject active skill context if set
        if let Some(ref skill_id) = *self.active_skill.read().unwrap() {
            if let Some(ref registry) = self.skill_registry {
                if let Some(skill) = registry.get(skill_id) {
                    let skill_ctx = skill.build_context();
                    return format!("{}\n\n{}", with_entry, skill_ctx);
                }
            }
        }

        with_entry
    }

    /// Get tool descriptions from the registry for the system prompt.
    fn get_tool_descriptions(&self) -> Vec<String> {
        // Convert tool schemas to descriptions
        match self.registry.get_tool_schemas() {
            Ok(schemas) => {
                let allowed = self.active_skill_tool_whitelist();
                schemas.as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|f| {
                            let func = f.get("function")?;
                            let name = func.get("name")?.as_str()?;
                            if let Some(ref whitelist) = allowed {
                                if !whitelist.iter().any(|w| w == name) {
                                    return None;
                                }
                            }
                            let description = func.get("description")?.as_str()?;
                            Some(format!("- {}: {}", name, description))
                        })
                        .collect()
                }).unwrap_or_default()
            }
            Err(_) => vec![],
        }
    }

    /// Return the tool whitelist for the active skill, if any.
    fn active_skill_tool_whitelist(&self) -> Option<Vec<String>> {
        let active = self.active_skill.read().unwrap().clone()?;
        let registry = self.skill_registry.as_ref()?;
        let skill = registry.get(&active)?;
        if skill.meta.tools.is_empty() {
            None
        } else {
            Some(skill.meta.tools.clone())
        }
    }

    /// Filter a tools JSON value to only include tools in the active skill whitelist.
    fn filter_tools_value(&self, tools: &Value) -> Value {
        let allowed = match self.active_skill_tool_whitelist() {
            Some(w) => w,
            None => return tools.clone(),
        };
        let allowed_set: std::collections::HashSet<String> = allowed.into_iter().collect();
        match tools.as_array() {
            Some(arr) => {
                let filtered: Vec<Value> = arr
                    .iter()
                    .filter(|v| {
                        v.get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|n| n.as_str())
                            .map(|name| allowed_set.contains(name))
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();
                Value::Array(filtered)
            }
            None => tools.clone(),
        }
    }



    /// Run the agent with a user query
    ///
    /// This is the main entry point that orchestrates the agent loop:
    /// 1. Sends user query to LLM with available tools
    /// 2. Processes any tool calls
    /// 3. Sends tool results back to LLM
    /// 4. Returns final response
    ///
    /// # Arguments
    ///
    /// * `query` - The user's request
    ///
    /// # Returns
    ///
    /// The final response from the agent
    /// Shared core of the non-streaming agent loop.
    ///
    /// Iterates up to `max_iterations`, calling the LLM and executing any tool
    /// calls. Returns `(final_response, completed)`.
    async fn run_sync_loop(
        &self,
        messages: &mut Vec<Message>,
        tools: &serde_json::Value,
        llm: Arc<dyn LlmProvider>,
    ) -> Result<(String, bool), AgentError> {
        let mut final_response = String::new();
        let mut completed = false;

        for iteration in 0..self.config.max_iterations {
            debug!("Iteration {}/{}", iteration + 1, self.config.max_iterations);

            if self.cancel_token.is_cancelled() {
                warn!("Agent run cancelled");
                return Err(AgentError::Cancelled);
            }

            // Proactive compaction via CompactionService
            if let Some(ref service) = self.compaction_service {
                if let Err(e) = service.maybe_compact(messages, llm.as_ref()).await {
                    warn!("Compaction failed: {}", e);
                }
            }

            if self.should_compact(messages).await {
                match self.compact_messages(messages).await {
                    Ok(compacted) => {
                        info!(
                            "Context compacted: {} messages -> {} messages",
                            messages.len(),
                            compacted.len()
                        );
                        *messages = compacted;
                    }
                    Err(e) => {
                        warn!("Failed to compact messages: {}", e);
                    }
                }
            }

            let prompt_tokens = CompactionService::estimate_tokens(messages) as u32;
            let response = tokio::time::timeout(
                tokio::time::Duration::from_secs(45),
                llm.complete(messages, tools),
            )
            .await
            .map_err(|_| AgentError::Llm("LLM request timed out after 45s".into()))??;
            let completion_tokens = response.content.len().div_ceil(4) as u32;
            self.accumulate_usage(prompt_tokens, completion_tokens);
            final_response = response.content.clone();

            if response.tool_calls.is_empty() {
                self.send_wire_message(WireMessage::ContentPart {
                    text: response.content.clone(),
                });
                info!("Agent loop completed after {} iterations", iteration + 1);
                completed = true;
                break;
            }

            if !response.content.is_empty() {
                self.send_wire_message(WireMessage::ContentPart {
                    text: response.content.clone(),
                });
            }

            messages.push(Message {
                role: MessageRole::Assistant,
                content: response.content,
                tool_calls: Some(response.tool_calls.clone()),
                tool_call_id: None,
            });

            for tool_call in &response.tool_calls {
                self.send_wire_message(WireMessage::StepBegin {
                    tool_name: tool_call.function.name.clone(),
                });

                let args: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                    .unwrap_or_else(|_| serde_json::json!({}));
                self.send_wire_message(WireMessage::ToolCall {
                    id: tool_call.id.clone(),
                    name: tool_call.function.name.clone(),
                    arguments: args,
                });

                let result = self.execute_tool_call(tool_call).await;
                let result_content = match result {
                    Ok(value) => value.to_string(),
                    Err(e) => json!({"error": e.to_string()}).to_string(),
                };

                self.send_wire_message(WireMessage::ToolResult {
                    id: tool_call.id.clone(),
                    result: result_content.clone(),
                });

                messages.push(Message::tool(&tool_call.id, result_content));
            }
        }

        Ok((final_response, completed))
    }

    pub async fn run(&self, query: impl AsRef<str>) -> Result<String, AgentError> {
        let llm = self
            .llm
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| AgentError::Llm("No LLM provider configured".to_string()))?;
        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);

        // Build system prompt with optional memory context
        let base_system_prompt = self.build_system_prompt();
        let mut system_prompt = base_system_prompt;

        if let Some(ref store) = self.memory_store {
            match store.search(query.as_ref(), 5).await {
                Ok(memories) => {
                    if !memories.is_empty() {
                        let memory_text = memories
                            .iter()
                            .map(|m| format!("- {}", m.content))
                            .collect::<Vec<_>>()
                            .join("\n");
                        system_prompt
                            .push_str(&format!("\n\n# Relevant Memories\n{}\n", memory_text));
                    }
                }
                Err(e) => {
                    warn!("Failed to retrieve memories: {}", e);
                }
            }
        }

        let mut messages = vec![
            Message::system(system_prompt),
            Message::user(query.as_ref()),
        ];

        info!("Starting agent loop for query: {}", query.as_ref());

        self.send_wire_message(WireMessage::TurnBegin {
            user_input: query.as_ref().to_string(),
        });

        let (final_response, completed) = self.run_sync_loop(&mut messages, &tools, llm).await?;

        self.send_wire_message(WireMessage::TurnEnd);

        let usage = self.get_session_usage();
        self.send_wire_message(WireMessage::Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        });

        // Persist interaction to memory
        let memory_content = if completed {
            format!("User: {}\nAssistant: {}", query.as_ref(), final_response)
        } else {
            format!(
                "User: {}\nAssistant: [max iterations reached] {}",
                query.as_ref(),
                final_response
            )
        };
        self.store_conversation_memory(memory_content).await;

        if let Some(ref ticker) = self.memory_ticker {
            match ticker.tick().await {
                true => info!("Memory ticker triggered"),
                false => debug!("Memory ticker not triggered yet"),
            }
        }

        if completed {
            Ok(final_response)
        } else {
            warn!("Max iterations ({}) reached", self.config.max_iterations);
            Err(AgentError::MaxIterationsExceeded(
                self.config.max_iterations,
            ))
        }
    }

    /// Run a synchronous (non-streaming) agent loop with pre-built messages.
    /// Used by the Gateway for non-streaming chat completion requests.
    pub async fn run_with_messages_sync(
        &self,
        mut messages: Vec<Message>,
    ) -> Result<String, AgentError> {
        let llm = self
            .llm
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| AgentError::Llm("No LLM provider configured".to_string()))?;
        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);

        let (final_response, completed) = self.run_sync_loop(&mut messages, &tools, llm).await?;

        self.send_wire_message(WireMessage::TurnEnd);

        let usage = self.get_session_usage();
        self.send_wire_message(WireMessage::Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        });

        if completed {
            Ok(final_response)
        } else {
            warn!("Max iterations ({}) reached", self.config.max_iterations);
            Err(AgentError::MaxIterationsExceeded(self.config.max_iterations))
        }
    }

    /// Run the agent with streaming response.
    ///
    /// Same as `run()`, but streams the final assistant response via `on_chunk`.
    /// Tool-calling rounds still use `complete()` internally; only the final
    /// text response is streamed.
    pub async fn run_streaming<F>(
        &self,
        query: impl AsRef<str>,
        on_chunk: F,
    ) -> Result<String, AgentError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let llm = self
            .llm
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| AgentError::Llm("No LLM provider configured".to_string()))?;

        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);

        let base_system_prompt = self.build_system_prompt();
        let mut system_prompt = base_system_prompt;

        if let Some(ref store) = self.memory_store {
            match store.search(query.as_ref(), 5).await {
                Ok(memories) => {
                    if !memories.is_empty() {
                        let memory_text = memories
                            .iter()
                            .map(|m| format!("- {}", m.content))
                            .collect::<Vec<_>>()
                            .join("\n");
                        system_prompt
                            .push_str(&format!("\n\n# Relevant Memories\n{}\n", memory_text));
                    }
                }
                Err(e) => {
                    warn!("Failed to retrieve memories: {}", e);
                }
            }
        }

        let messages = vec![
            Message::system(system_prompt),
            Message::user(query.as_ref()),
        ];

        info!(
            "Starting streaming agent loop for query: {}",
            query.as_ref()
        );

        // Send TurnBegin message
        self.send_wire_message(WireMessage::TurnBegin {
            user_input: query.as_ref().to_string(),
        });

        self.run_streaming_loop(messages, query.as_ref(), tools, llm.clone(), on_chunk)
            .await
    }

    /// Run the streaming agent loop with a pre-built message list.
    pub async fn run_streaming_with_messages<F>(
        &self,
        messages: Vec<Message>,
        on_chunk: F,
    ) -> Result<String, AgentError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let llm = self
            .llm
            .read()
            .unwrap()
            .clone()
            .ok_or_else(|| AgentError::Llm("No LLM provider configured".to_string()))?;

        let tools = self.registry.get_tool_schemas()?;

        let query_hint = messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        info!(
            "Starting streaming agent loop with {} messages",
            messages.len()
        );

        // Send TurnBegin message
        self.send_wire_message(WireMessage::TurnBegin {
            user_input: query_hint.clone(),
        });

        self.run_streaming_loop(messages, &query_hint, tools, llm.clone(), on_chunk)
            .await
    }

    async fn run_streaming_loop<F>(
        &self,
        mut messages: Vec<Message>,
        query_hint: &str,
        tools: serde_json::Value,
        llm: std::sync::Arc<dyn LlmProvider>,
        mut on_chunk: F,
    ) -> Result<String, AgentError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let mut final_response = String::new();
        let mut completed = false;

        for iteration in 0..self.config.max_iterations {
            debug!("Iteration {}/{}", iteration + 1, self.config.max_iterations);

            if self.cancel_token.is_cancelled() {
                warn!("Agent run streaming cancelled");
                return Err(AgentError::Cancelled);
            }

            // Proactive compaction via CompactionService
            if let Some(ref service) = self.compaction_service {
                if let Err(e) = service.maybe_compact(&mut messages, llm.as_ref()).await {
                    warn!("Compaction failed: {}", e);
                }
            }

            // Stream-first: try streaming, fall back to complete() if unsupported or errors.
            let mut turn_response: Option<LlmResponse> = None;
            let mut prompt_tokens = 0u32;
            let mut completion_tokens = 0u32;

            match llm.stream(&messages, &tools) {
                Ok(mut stream_rx) => {
                    prompt_tokens = CompactionService::estimate_tokens(&messages) as u32;
                    // Send final content start notification
                    self.send_wire_message(WireMessage::ContentPart {
                        text: String::new(),
                    });
                    let mut accumulated = String::new();
                    let mut tool_calls: Vec<ToolCall> = Vec::new();
                    while let Some(chunk_result) = stream_rx.recv().await {
                        match chunk_result {
                            Ok(delta) => {
                                if let Some(content) = delta.content {
                                    accumulated.push_str(&content);
                                    on_chunk(&content);
                                    self.send_wire_message(WireMessage::ContentPart {
                                        text: content,
                                    });
                                }
                                for call in delta.tool_calls {
                                    tool_calls.push(call);
                                }
                            }
                            Err(e) => {
                                warn!("Stream error: {}, falling back to complete()", e);
                                accumulated.clear();
                                tool_calls.clear();
                                break;
                            }
                        }
                    }
                    completion_tokens = accumulated.len().div_ceil(4) as u32;
                    turn_response = Some(LlmResponse {
                        content: accumulated,
                        tool_calls,
                        is_complete: true,
                    });
                }
                Err(e) => {
                    debug!(
                        "Streaming not supported or failed: {}, falling back to complete()",
                        e
                    );
                }
            }

            let was_streamed = turn_response.is_some();
            let response = match turn_response {
                Some(r) => r,
                None => {
                    prompt_tokens = CompactionService::estimate_tokens(&messages) as u32;
                    let r = llm.complete(&messages, &tools).await?;
                    completion_tokens = r.content.len().div_ceil(4) as u32;
                    r
                }
            };

            self.accumulate_usage(prompt_tokens, completion_tokens);

            if response.tool_calls.is_empty() {
                // No tool calls: final answer.
                // If we arrived here via fallback (turn_response was None), simulate
                // streaming from the complete() response for smooth UI.
                if !was_streamed && !response.content.is_empty() {
                    for c in response.content.chars() {
                        let chunk = c.to_string();
                        on_chunk(&chunk);
                        self.send_wire_message(WireMessage::ContentPart {
                            text: chunk.clone(),
                        });
                    }
                }

                final_response = response.content;
                info!("Agent loop completed after {} iterations", iteration + 1);
                completed = true;
                break;
            }

            // Tool-calling round: send assistant content (if any)
            if !response.content.is_empty() {
                self.send_wire_message(WireMessage::ContentPart {
                    text: response.content.clone(),
                });
            }

            messages.push(Message {
                role: MessageRole::Assistant,
                content: response.content,
                tool_calls: Some(response.tool_calls.clone()),
                tool_call_id: None,
            });

            for tool_call in &response.tool_calls {
                // Send StepBegin message
                self.send_wire_message(WireMessage::StepBegin {
                    tool_name: tool_call.function.name.clone(),
                });

                // Send ToolCall message
                let args: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                    .unwrap_or_else(|_| serde_json::json!({}));
                self.send_wire_message(WireMessage::ToolCall {
                    id: tool_call.id.clone(),
                    name: tool_call.function.name.clone(),
                    arguments: args,
                });

                let result = self.execute_tool_call(tool_call).await;
                let result_content = match result {
                    Ok(value) => value.to_string(),
                    Err(e) => json!({"error": e.to_string()}).to_string(),
                };

                // Send ToolResult message
                self.send_wire_message(WireMessage::ToolResult {
                    id: tool_call.id.clone(),
                    result: result_content.clone(),
                });

                messages.push(Message::tool(&tool_call.id, result_content));
            }
        }

        // Send TurnEnd message
        self.send_wire_message(WireMessage::TurnEnd);

        // Send usage report
        let usage = self.get_session_usage();
        self.send_wire_message(WireMessage::Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        });

        let memory_content = if completed {
            format!("User: {}\nAssistant: {}", query_hint, final_response)
        } else {
            format!(
                "User: {}\nAssistant: [max iterations reached] {}",
                query_hint,
                final_response
            )
        };
        self.store_conversation_memory(memory_content).await;

        if let Some(ref ticker) = self.memory_ticker {
            match ticker.tick().await {
                true => info!("Memory ticker triggered"),
                false => debug!("Memory ticker not triggered yet"),
            }
        }

        if completed {
            Ok(final_response)
        } else {
            warn!("Max iterations ({}) reached", self.config.max_iterations);
            Err(AgentError::MaxIterationsExceeded(
                self.config.max_iterations,
            ))
        }
    }

    /// Detect whether a tool call targets a sensitive file or path.
    fn detect_sensitive_access(&self, tool_name: &str, args: &Value) -> Option<String> {
        use crate::tools::file::is_sensitive_file;
        match tool_name {
            "file_read" | "file_write" | "file_edit" => {
                if let Some(path_str) = args.get("path").and_then(|v| v.as_str()) {
                    let path = std::path::PathBuf::from(path_str);
                    let path = if path.is_absolute() {
                        path
                    } else {
                        self.config.working_dir.join(path)
                    };
                    if is_sensitive_file(&path) {
                        return Some(path.display().to_string());
                    }
                }
            }
            "bash" | "powershell" => {
                if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                    for token in cmd.split_whitespace() {
                        let trimmed = token.trim_matches(|c| c == '"' || c == '\'');
                        if !trimmed.is_empty() {
                            let path = std::path::Path::new(trimmed);
                            if is_sensitive_file(path) {
                                return Some(trimmed.to_string());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    /// Execute a single tool call
    async fn execute_tool_call(&self, tool_call: &ToolCall) -> Result<Value, ToolError> {
        let name = &tool_call.function.name;
        let args: Value = serde_json::from_str(&tool_call.function.arguments)
            .map_err(|e| ToolError::invalid_params(format!("Invalid JSON: {}", e)))?;

        info!("Executing tool '{}' with args: {:?}", name, args);

        let sensitive_path = self.detect_sensitive_access(name, &args);

        // 如果配置了审批运行时，先请求审批
        if let Some(ref runtime) = self.approval_runtime {
            let description = sensitive_path
                .as_ref()
                .map(|p| format!("Sensitive file access: {}", p));

            let tool_call_for_approval = if sensitive_path.is_some() {
                let mut tc = tool_call.clone();
                let mut approval_args = args.clone();
                approval_args["_sensitive_file_warning"] =
                    json!("This operation accesses a sensitive file");
                tc.function.arguments = approval_args.to_string();
                tc
            } else {
                tool_call.clone()
            };

            match self.approval_mode {
                ApprovalMode::Interactive => {
                    // 创建审批请求
                    let turn_id = uuid::Uuid::new_v4().to_string();
                    let request_id = runtime
                        .create_request(
                            &tool_call_for_approval,
                            ApprovalSource::ForegroundTurn { turn_id },
                            description,
                        )
                        .await
                        .map_err(|e| {
                            ToolError::execution_failed(format!("Approval error: {}", e))
                        })?;

                    // 等待审批结果，带超时
                    let approval_result = tokio::time::timeout(
                        tokio::time::Duration::from_secs(300),
                        runtime.wait_for_response(&request_id),
                    )
                    .await;

                    match approval_result {
                        Ok(Ok(ApprovalResponse::Approve)) => {
                            // 继续执行
                        }
                        Ok(Ok(ApprovalResponse::Reject)) => {
                            return Err(ToolError::execution_failed(
                                "Tool call rejected by user".to_string(),
                            ));
                        }
                        Ok(Ok(ApprovalResponse::ApproveForSession)) => {
                            if let Err(e) = runtime
                                .resolve(&request_id, ApprovalResponse::ApproveForSession)
                                .await
                            {
                                return Err(ToolError::execution_failed(format!(
                                    "Approval error: {}",
                                    e
                                )));
                            }
                        }
                        Ok(Err(e)) => {
                            return Err(ToolError::execution_failed(format!(
                                "Approval error: {}",
                                e
                            )));
                        }
                        Err(_) => {
                            return Err(ToolError::execution_failed(
                                "Approval timeout after 300 seconds".to_string(),
                            ));
                        }
                    }
                }
                ApprovalMode::Yolo => {
                    // Yolo 模式跳过审批
                }
                ApprovalMode::Plan => {
                    // Plan 模式下暂时按 Interactive 处理
                    let turn_id = uuid::Uuid::new_v4().to_string();
                    let request_id = runtime
                        .create_request(
                            &tool_call_for_approval,
                            ApprovalSource::ForegroundTurn { turn_id },
                            description,
                        )
                        .await
                        .map_err(|e| {
                            ToolError::execution_failed(format!("Approval error: {}", e))
                        })?;

                    let approval_result = tokio::time::timeout(
                        tokio::time::Duration::from_secs(300),
                        runtime.wait_for_response(&request_id),
                    )
                    .await;

                    match approval_result {
                        Ok(Ok(ApprovalResponse::Approve)) => {
                            // 继续执行
                        }
                        Ok(Ok(ApprovalResponse::ApproveForSession)) => {
                            if let Err(e) = runtime
                                .resolve(&request_id, ApprovalResponse::ApproveForSession)
                                .await
                            {
                                return Err(ToolError::execution_failed(format!(
                                    "Approval error: {}",
                                    e
                                )));
                            }
                        }
                        Ok(Ok(ApprovalResponse::Reject)) => {
                            return Err(ToolError::execution_failed(
                                "Tool call rejected by user".to_string(),
                            ));
                        }
                        Ok(Err(e)) => {
                            return Err(ToolError::execution_failed(format!(
                                "Approval error: {}",
                                e
                            )));
                        }
                        Err(_) => {
                            return Err(ToolError::execution_failed(
                                "Approval timeout after 300 seconds".to_string(),
                            ));
                        }
                    }
                }
            }
        }

        let ctx = ToolContext::new()
            .with_working_dir(&self.config.working_dir)
            .with_read_only(self.config.read_only)
            .with_timeout(self.config.tool_timeout_secs)
            .with_approval_mode(self.approval_mode);

        self.registry.execute(name, args, ctx).await
    }

    /// Execute a tool directly (bypassing the LLM loop)
    ///
    /// Useful for programmatic tool execution
    pub async fn execute_tool(&self, name: &str, args: Value) -> Result<Value, ToolError> {
        let ctx = ToolContext::new()
            .with_working_dir(&self.config.working_dir)
            .with_read_only(self.config.read_only)
            .with_timeout(self.config.tool_timeout_secs)
            .with_approval_mode(self.approval_mode);

        self.registry.execute(name, args, ctx).await
    }

    /// 检查是否需要压缩
    async fn should_compact(&self, messages: &[Message]) -> bool {
        let token_count = estimate_message_tokens(messages);
        self.compaction_config
            .should_compact(token_count, self.max_context_tokens)
    }

    /// 执行压缩
    async fn compact_messages(&self, messages: &[Message]) -> Result<Vec<Message>, AgentError> {
        use crate::compaction::{Compaction, SimpleCompaction};

        let compactor = SimpleCompaction::new();

        // 调用 LLM 压缩 (如果配置了 LLM)
        let llm_opt = self.llm.read().unwrap().clone();
        if let Some(ref llm) = llm_opt {
            let result = compactor.compact(messages, llm.as_ref()).await?;

            // 构建压缩后的消息列表
            let mut new_messages = vec![Message::system(format!(
                "Previous context compacted: {} messages summarized",
                messages.len() - result.messages.len() + 1
            ))];
            new_messages.extend(result.messages);

            Ok(new_messages)
        } else {
            Ok(messages.to_vec())
        }
    }
}

use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::FileReadTool;

    #[test]
    fn test_message_creation() {
        let system = Message::system("You are helpful");
        assert_eq!(system.role, MessageRole::System);

        let user = Message::user("Hello");
        assert_eq!(user.role, MessageRole::User);

        let tool = Message::tool("call_123", "result");
        assert_eq!(tool.role, MessageRole::Tool);
        assert_eq!(tool.tool_call_id, Some("call_123".to_string()));
    }

    #[test]
    fn test_agent_config() {
        let config = AgentConfig::new()
            .with_max_iterations(5)
            .with_read_only(true);

        assert_eq!(config.max_iterations, 5);
        assert!(config.read_only);
    }

    #[tokio::test]
    async fn test_agent_direct_tool_execution() {
        let registry = ToolRegistry::new();
        registry.register(FileReadTool::new()).unwrap();

        let agent = Agent::new(registry);

        // This will fail because file doesn't exist, but tests the path
        let result = agent
            .execute_tool("file_read", json!({"path": "/nonexistent/file.txt"}))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_agent_run_streaming() {
        use std::sync::{Arc, Mutex};

        let registry = ToolRegistry::new();
        let config = AgentConfig::new();
        let agent = Agent::with_config(registry, config).with_llm(Arc::new(MockLlm));

        let chunks = Arc::new(Mutex::new(Vec::new()));
        let chunks_clone = chunks.clone();
        let result = agent
            .run_streaming("Hello", move |chunk| {
                chunks_clone.lock().unwrap().push(chunk.to_string());
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "This is a mock response");
        assert_eq!(*chunks.lock().unwrap(), vec!["This is a mock response"]);
    }

    #[tokio::test]
    async fn test_compaction_triggered_in_agent() {
        use crate::compaction::CompactionConfig;
        use std::sync::atomic::{AtomicUsize, Ordering};

        // 创建一个 Mock LLM 记录调用次数
        struct CountingMockLlm {
            call_count: AtomicUsize,
        }

        #[async_trait::async_trait]
        impl LlmProvider for CountingMockLlm {
            async fn complete(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<LlmResponse, AgentError> {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                Ok(LlmResponse {
                    content: "This is a mock response for compaction test".to_string(),
                    tool_calls: vec![],
                    is_complete: true,
                })
            }

            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
            {
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

        let registry = ToolRegistry::new();
        let config = AgentConfig::new().with_max_iterations(5);

        // 创建一个使用低阈值触发压缩的 Agent
        let agent = Agent::with_config(registry, config)
            .with_llm(Arc::new(CountingMockLlm {
                call_count: AtomicUsize::new(0),
            }))
            .with_max_context_tokens(100) // 设置低阈值触发压缩
            .with_compaction_config(CompactionConfig::default());

        // 运行多次对话
        for i in 0..3 {
            let result = agent
                .run(
                    format!(
                        "test query with some content to increase token count {} ",
                        i
                    )
                    .repeat(10),
                )
                .await;
            assert!(result.is_ok());
        }

        // 验证压缩逻辑被正确配置 (token 估算和压缩配置)
        // 由于 MockLlm 在压缩时也会返回简单响应，测试主要验证代码路径不崩溃
    }

    #[test]
    fn test_should_compact_method() {
        use crate::compaction::CompactionConfig;

        let registry = ToolRegistry::new();
        let config = AgentConfig::new();
        let agent = Agent::with_config(registry, config)
            .with_max_context_tokens(100)
            .with_compaction_config(CompactionConfig::default());

        // 创建足够多的消息以超过阈值
        let messages: Vec<Message> = (0..20)
            .map(|i| {
                Message::user(
                    format!(
                        "This is a test message with enough content to consume tokens {} ",
                        i
                    )
                    .repeat(5),
                )
            })
            .collect();

        // 验证 should_compact 方法存在并且可以调用
        // 注意：由于方法是 async 的，我们主要验证编译通过
        let rt = tokio::runtime::Runtime::new().unwrap();
        let should_compact = rt.block_on(agent.should_compact(&messages));

        // 消息内容应该触发压缩（超过 100 token 的 80% = 80 tokens）
        assert!(
            should_compact,
            "Should detect that compaction is needed with large messages"
        );
    }

    #[tokio::test]
    async fn test_tool_call_approval_flow() {
        use crate::approval::{ApprovalResponse, InMemoryApprovalRuntime};
        use std::time::Duration;

        // 创建一个 Mock LLM 会返回工具调用
        struct MockLlmWithToolCall;

        #[async_trait::async_trait]
        impl LlmProvider for MockLlmWithToolCall {
            async fn complete(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<LlmResponse, AgentError> {
                Ok(LlmResponse {
                    content: "I'll use the mock tool".to_string(),
                    tool_calls: vec![ToolCall {
                        id: "call_123".to_string(),
                        call_type: "function".to_string(),
                        function: FunctionCall {
                            name: "mock_tool".to_string(),
                            arguments: r#"{"param": "value"}"#.to_string(),
                        },
                    }],
                    is_complete: false,
                })
            }

            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
            {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tokio::spawn(async move {
                    let _ = tx
                        .send(Ok(StreamDelta {
                            content: Some("Mock response".to_string()),
                            tool_calls: vec![],
                        }))
                        .await;
                });
                Ok(rx)
            }

            fn set_prompt_cache_key(&mut self, _key: &str) {}
        }

        // 创建注册表并注册一个 Mock 工具
        let registry = ToolRegistry::new();
        // 由于我们没有真正的 mock_tool，我们期望工具执行失败
        // 但审批流程应该被触发

        // 创建内存审批运行时
        let approval_rt = Arc::new(InMemoryApprovalRuntime::new());
        let rt_clone = approval_rt.clone();

        let agent = Agent::with_config(registry, AgentConfig::new().with_max_iterations(1))
            .with_approval_runtime(approval_rt)
            .with_approval_mode(ApprovalMode::Interactive)
            .with_llm(Arc::new(MockLlmWithToolCall));

        // 在后台运行 Agent
        let handle = tokio::spawn(async move { agent.run("use mock tool").await });

        // 等待审批请求出现
        tokio::time::sleep(Duration::from_millis(100)).await;
        let pending = rt_clone.list_pending();
        assert_eq!(pending.len(), 1, "Should have one pending approval request");

        // 批准请求
        rt_clone
            .resolve(&pending[0].id, ApprovalResponse::Approve)
            .await
            .expect("Failed to resolve approval");

        // Agent 应该完成（虽然工具执行会失败，因为 mock_tool 不存在）
        let result = handle.await.unwrap();
        // 结果应该是 Err，因为工具不存在，但审批流程已经测试到了
        assert!(
            result.is_err(),
            "Expected error because mock_tool is not registered"
        );
    }

    #[tokio::test]
    async fn test_tool_call_yolo_mode() {
        use crate::approval::InMemoryApprovalRuntime;

        // 创建一个 Mock LLM 会返回工具调用
        struct MockLlmWithToolCall;

        #[async_trait::async_trait]
        impl LlmProvider for MockLlmWithToolCall {
            async fn complete(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<LlmResponse, AgentError> {
                Ok(LlmResponse {
                    content: "I'll use the mock tool".to_string(),
                    tool_calls: vec![ToolCall {
                        id: "call_456".to_string(),
                        call_type: "function".to_string(),
                        function: FunctionCall {
                            name: "mock_tool".to_string(),
                            arguments: r#"{"param": "value"}"#.to_string(),
                        },
                    }],
                    is_complete: false,
                })
            }

            fn stream(
                &self,
                _messages: &[Message],
                _tools: &Value,
            ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>
            {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                tokio::spawn(async move {
                    let _ = tx
                        .send(Ok(StreamDelta {
                            content: Some("Mock response".to_string()),
                            tool_calls: vec![],
                        }))
                        .await;
                });
                Ok(rx)
            }

            fn set_prompt_cache_key(&mut self, _key: &str) {}
        }

        let registry = ToolRegistry::new();
        let approval_rt = Arc::new(InMemoryApprovalRuntime::new());

        let agent = Agent::with_config(registry, AgentConfig::new().with_max_iterations(1))
            .with_approval_runtime(approval_rt.clone())
            .with_approval_mode(ApprovalMode::Yolo) // Yolo 模式
            .with_llm(Arc::new(MockLlmWithToolCall));

        // 运行 Agent
        let result = agent.run("use mock tool").await;
        // 结果应该是 Err，因为工具不存在，但 Yolo 模式应该跳过审批
        assert!(
            result.is_err(),
            "Expected error because mock_tool is not registered"
        );

        // Yolo 模式下不应有 pending 审批请求
        let pending = approval_rt.list_pending();
        assert!(
            pending.is_empty(),
            "Yolo mode should not create pending approval requests"
        );
    }

    #[tokio::test]
    async fn test_agent_run_with_wire() {
        use clarity_wire::Wire;
        use std::sync::Arc;
        use tokio::time::{timeout, Duration};

        // Create Wire
        let wire = Wire::new();
        let mut ui_side = wire.ui_side(false);

        // Create Agent with Wire
        let registry = ToolRegistry::new();
        let config = AgentConfig::new();
        let agent = Agent::with_config(registry, config)
            .with_llm(Arc::new(MockLlm))
            .with_wire(Arc::new(wire));

        // Run Agent in background
        let handle = tokio::spawn(async move { agent.run("test query").await });

        // Verify UI side receives TurnBegin
        let msg = timeout(Duration::from_millis(1000), ui_side.recv())
            .await
            .expect("timeout waiting for TurnBegin")
            .expect("channel closed");
        assert!(matches!(msg, WireMessage::TurnBegin { user_input } if user_input == "test query"));

        // Verify ContentPart is received
        let msg = timeout(Duration::from_millis(1000), ui_side.recv())
            .await
            .expect("timeout waiting for ContentPart")
            .expect("channel closed");
        assert!(
            matches!(msg, WireMessage::ContentPart { text } if text == "This is a mock response")
        );

        // Verify TurnEnd is received
        let msg = timeout(Duration::from_millis(1000), ui_side.recv())
            .await
            .expect("timeout waiting for TurnEnd")
            .expect("channel closed");
        assert!(matches!(msg, WireMessage::TurnEnd));

        // Wait for agent to complete
        let result = timeout(Duration::from_millis(1000), handle)
            .await
            .expect("timeout waiting for agent")
            .expect("join error");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "This is a mock response");
    }

    #[tokio::test]
    async fn test_agent_run_streaming_with_wire() {
        use clarity_wire::Wire;
        use std::sync::Arc;
        use std::sync::{Arc as StdArc, Mutex};
        use tokio::time::{timeout, Duration};

        // Create Wire
        let wire = Wire::new();
        let mut ui_side = wire.ui_side(false);

        // Create Agent with Wire
        let registry = ToolRegistry::new();
        let config = AgentConfig::new();
        let agent = Agent::with_config(registry, config)
            .with_llm(Arc::new(MockLlm))
            .with_wire(Arc::new(wire));

        // Run Agent in background with streaming
        let chunks = StdArc::new(Mutex::new(Vec::new()));
        let chunks_clone = chunks.clone();
        let handle = tokio::spawn(async move {
            agent
                .run_streaming("streaming test", move |chunk| {
                    chunks_clone.lock().unwrap().push(chunk.to_string());
                })
                .await
        });

        // Verify UI side receives TurnBegin
        let msg = timeout(Duration::from_millis(1000), ui_side.recv())
            .await
            .expect("timeout waiting for TurnBegin")
            .expect("channel closed");
        assert!(
            matches!(msg, WireMessage::TurnBegin { user_input } if user_input == "streaming test")
        );

        // Verify ContentPart is received (empty start marker)
        let msg = timeout(Duration::from_millis(1000), ui_side.recv())
            .await
            .expect("timeout waiting for ContentPart start")
            .expect("channel closed");
        assert!(matches!(msg, WireMessage::ContentPart { .. }));

        // Verify streaming ContentParts are received
        let mut content_received = false;
        loop {
            match timeout(Duration::from_millis(500), ui_side.recv()).await {
                Ok(Some(msg)) => match msg {
                    WireMessage::ContentPart { text } => {
                        if !text.is_empty() {
                            content_received = true;
                        }
                    }
                    WireMessage::TurnEnd => break,
                    _ => {}
                },
                Ok(None) => break,
                Err(_) => break, // Timeout
            }
        }
        assert!(content_received, "Should have received content parts");

        // Wait for agent to complete
        let result = timeout(Duration::from_millis(1000), handle)
            .await
            .expect("timeout waiting for agent")
            .expect("join error");
        assert!(result.is_ok());
    }

}
