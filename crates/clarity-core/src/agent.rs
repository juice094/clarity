//! Agent Loop - Core orchestration component
//!
//! The `Agent` manages the interaction loop between the LLM and tools.
//! It handles:
//! - Tool discovery for LLM
//! - Request routing to appropriate tools
//! - Context management
//! - Iteration limits and safety

use crate::error::{AgentError, ToolError};
use crate::memory::{Memory, MemoryStore, MemoryTicker};
use crate::personality::{Personality, PersonalityConfig, PersonalityLoader, SystemPromptBuilder};
use crate::registry::ToolRegistry;
use crate::tools::ToolContext;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// LLM message role
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a tool response message
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// A tool call from the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}

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
    /// System prompt (legacy, use personality instead)
    pub system_prompt: String,
    /// Personality configuration
    pub personality_config: Option<PersonalityConfig>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            tool_timeout_secs: 60,
            working_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            read_only: false,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            personality_config: None,
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

    /// Set custom system prompt (legacy, use with_personality instead)
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Set personality configuration
    pub fn with_personality(mut self, config: PersonalityConfig) -> Self {
        self.personality_config = Some(config);
        self
    }
}

/// Default system prompt for the agent
const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a helpful AI assistant with access to various tools.
You can use these tools to help users with their tasks.

When you need to use a tool, respond with a tool call in the appropriate format.
After receiving the tool result, provide a helpful response to the user.

Available tools will be provided at the start of each conversation.
"#;

/// LLM Provider trait - implement this to integrate with different LLMs
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Generate a response from the LLM
    ///
    /// # Arguments
    ///
    /// * `messages` - Conversation history
    /// * `tools` - Available tools as JSON Schema
    ///
    /// # Returns
    ///
    /// The LLM's response, potentially including tool calls
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError>;
}

/// Response from an LLM
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// The text content of the response
    pub content: String,
    /// Tool calls to execute (if any)
    pub tool_calls: Vec<ToolCall>,
    /// Whether this is the final response
    pub is_complete: bool,
}

/// Simple mock LLM for testing
pub struct MockLlm;

#[async_trait::async_trait]
impl LlmProvider for MockLlm {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        // Mock implementation - returns a simple response
        Ok(LlmResponse {
            content: "This is a mock response".to_string(),
            tool_calls: vec![],
            is_complete: true,
        })
    }
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
/// use clarity_core::personality::{PersonalityConfig, YuanType};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let registry = ToolRegistry::with_builtin_tools();
///     
///     // Configure with personality
///     let personality_config = PersonalityConfig::new()
///         .with_agent_name("Clarity")
///         .with_user_name("User")
///         .with_yuan_type(YuanType::Hanako);
///     
///     let config = AgentConfig::new()
///         .with_max_iterations(10)
///         .with_read_only(false)
///         .with_personality(personality_config);
///     
///     let agent = Agent::with_config(registry, config);
///     
///     // This would need an actual LLM provider
///     // let response = agent.run("List all Rust files").await?;
///     
///     Ok(())
/// }
/// ```
pub struct Agent {
    registry: ToolRegistry,
    config: AgentConfig,
    llm: Option<Arc<dyn LlmProvider>>,
    personality: Option<Personality>,
    system_prompt_builder: Option<SystemPromptBuilder>,
    memory_store: Option<Arc<dyn MemoryStore>>,
    memory_ticker: Option<MemoryTicker>,
}

impl Agent {
    /// Create a new Agent with the given registry and default config
    pub fn new(registry: ToolRegistry) -> Self {
        Self::with_config(registry, AgentConfig::default())
    }

    /// Create a new Agent with custom configuration
    pub fn with_config(registry: ToolRegistry, config: AgentConfig) -> Self {
        let (personality, system_prompt_builder) =
            if let Some(ref personality_config) = config.personality_config {
                let loader = PersonalityLoader::new();
                match loader.load(personality_config) {
                    Ok(personality) => {
                        let builder = SystemPromptBuilder::new(personality.clone());
                        (Some(personality), Some(builder))
                    }
                    Err(e) => {
                        warn!("Failed to load personality: {}. Using fallback.", e);
                        (None, None)
                    }
                }
            } else {
                (None, None)
            };

        Self {
            registry,
            config,
            llm: None,
            personality,
            system_prompt_builder,
            memory_store: None,
            memory_ticker: None,
        }
    }

    /// Set the LLM provider
    pub fn with_llm(mut self, llm: Arc<dyn LlmProvider>) -> Self {
        self.llm = Some(llm);
        self
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

    /// Get the tool registry
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Get the agent configuration
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Get the personality (if configured)
    pub fn personality(&self) -> Option<&Personality> {
        self.personality.as_ref()
    }

    /// Build the system prompt
    /// Uses personality if available, otherwise falls back to config.system_prompt
    fn build_system_prompt(&self) -> String {
        if let Some(ref builder) = self.system_prompt_builder {
            // Build with personality and optional skill definitions
            let skills = self.get_skill_definitions();
            builder
                .clone()
                .with_skills(skills)
                .build()
        } else {
            // Fallback to legacy system prompt
            self.config.system_prompt.clone()
        }
    }

    /// Get skill definitions from the tool registry
    fn get_skill_definitions(&self) -> Vec<String> {
        // Convert tool schemas to skill descriptions
        match self.registry.get_tool_schemas() {
            Ok(schemas) => {
                if let Some(functions) = schemas.get("functions").and_then(|f| f.as_array()) {
                    functions
                        .iter()
                        .filter_map(|f| {
                            let name = f.get("name")?.as_str()?;
                            let description = f.get("description")?.as_str()?;
                            Some(format!("- {}: {}", name, description))
                        })
                        .collect()
                } else {
                    vec![]
                }
            }
            Err(_) => vec![],
        }
    }

    /// Update personality configuration (hot reload)
    ///
    /// This allows changing the agent's personality without recreating the agent.
    pub fn update_personality(&mut self, config: PersonalityConfig) -> anyhow::Result<()> {
        info!("Updating personality configuration");

        let loader = PersonalityLoader::new();
        let personality = loader.load(&config)?;

        self.personality = Some(personality.clone());
        self.system_prompt_builder = Some(SystemPromptBuilder::new(personality));
        self.config.personality_config = Some(config);

        info!("Personality updated successfully");
        Ok(())
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
    pub async fn run(&self, query: impl AsRef<str>) -> Result<String, AgentError> {
        let llm = self
            .llm
            .as_ref()
            .ok_or_else(|| AgentError::Llm("No LLM provider configured".to_string()))?;

        let tools = self.registry.get_tool_schemas()?;

        // Build system prompt
        let system_prompt = self.build_system_prompt();

        // Initialize conversation
        let mut messages = vec![
            Message::system(system_prompt),
            Message::user(query.as_ref()),
        ];

        info!("Starting agent loop for query: {}", query.as_ref());

        // Agent loop
        for iteration in 0..self.config.max_iterations {
            debug!("Iteration {}/{}", iteration + 1, self.config.max_iterations);

            // Get LLM response
            let response = llm.complete(&messages, &tools).await?;

            // If no tool calls, we're done
            if response.tool_calls.is_empty() {
                info!("Agent loop completed after {} iterations", iteration + 1);
                return Ok(response.content);
            }

            // Add assistant message with tool calls
            messages.push(Message {
                role: MessageRole::Assistant,
                content: response.content,
                tool_calls: Some(response.tool_calls.clone()),
                tool_call_id: None,
            });

            // Execute tool calls
            for tool_call in &response.tool_calls {
                let result = self.execute_tool_call(tool_call).await;

                let result_content = match result {
                    Ok(value) => value.to_string(),
                    Err(e) => json!({"error": e.to_string()}).to_string(),
                };

                messages.push(Message::tool(&tool_call.id, result_content));
            }
        }

        warn!("Max iterations ({}) reached", self.config.max_iterations);
        Err(AgentError::MaxIterationsExceeded(self.config.max_iterations))
    }

    /// Execute a single tool call
    async fn execute_tool_call(&self, tool_call: &ToolCall) -> Result<Value, ToolError> {
        let name = &tool_call.function.name;
        let args: Value = serde_json::from_str(&tool_call.function.arguments)
            .map_err(|e| ToolError::invalid_params(format!("Invalid JSON: {}", e)))?;

        info!("Executing tool '{}' with args: {:?}", name, args);

        let ctx = ToolContext::new()
            .with_working_dir(&self.config.working_dir)
            .with_read_only(self.config.read_only)
            .with_timeout(self.config.tool_timeout_secs);

        self.registry.execute(name, args, ctx).await
    }

    /// Execute a tool directly (bypassing the LLM loop)
    ///
    /// Useful for programmatic tool execution
    pub async fn execute_tool(
        &self,
        name: &str,
        args: Value,
    ) -> Result<Value, ToolError> {
        let ctx = ToolContext::new()
            .with_working_dir(&self.config.working_dir)
            .with_read_only(self.config.read_only)
            .with_timeout(self.config.tool_timeout_secs);

        self.registry.execute(name, args, ctx).await
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

    #[test]
    fn test_agent_with_personality() {
        let personality_config = PersonalityConfig::new()
            .with_agent_name("TestAgent")
            .with_user_name("TestUser")
            .with_yuan_type(crate::personality::YuanType::Hanako);

        let config = AgentConfig::new().with_personality(personality_config);
        let registry = ToolRegistry::new();
        let agent = Agent::with_config(registry, config);

        // Should have personality loaded
        assert!(agent.personality().is_some());
    }

    #[test]
    fn test_system_prompt_building() {
        let personality_config = PersonalityConfig::new()
            .with_agent_name("TestAgent")
            .with_user_name("TestUser")
            .with_yuan_type(crate::personality::YuanType::Hanako);

        let config = AgentConfig::new().with_personality(personality_config);
        let registry = ToolRegistry::new();
        let agent = Agent::with_config(registry, config);

        let system_prompt = agent.build_system_prompt();

        // Should contain personality content
        assert!(system_prompt.contains("TestAgent"));
        assert!(system_prompt.contains("人格"));
    }
}
