//! Tool Registry for discovering and executing tools
//!
//! The `ToolRegistry` manages all available tools and provides:
//! - Tool registration and lookup
//! - LLM-compatible tool discovery (JSON Schema)
//! - Batch execution capabilities

use crate::error::{AgentError, ToolError};
use crate::tools::{SharedTool, Tool, ToolContext, ToolResult};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};

/// A registry of all available tools
///
/// The registry maintains a mapping of tool names to tool instances,
/// allowing dynamic tool discovery and execution.
///
/// # Example
///
/// ```rust
/// use clarity_core::ToolRegistry;
/// use clarity_core::tools::{FileReadTool, BashTool};
///
/// let mut registry = ToolRegistry::new();
/// registry.register(FileReadTool::new()).unwrap();
/// registry.register(BashTool::new()).unwrap();
///
/// // Get tool schemas for LLM
/// let schemas = registry.get_tool_schemas();
/// ```
#[derive(Clone)]
pub struct ToolRegistry {
    tools: Arc<std::sync::RwLock<HashMap<String, SharedTool>>>,
}

impl ToolRegistry {
    /// Create a new empty tool registry
    pub fn new() -> Self {
        Self {
            tools: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Create a registry with all built-in tools pre-registered
    pub fn with_builtin_tools() -> Self {
        use crate::tools::{
            AskUserTool, CancelCronTool, ComputerUseTool, FileEditTool, FileReadTool,
            FileWriteTool, GlobTool, GrepTool, ListCronTool, NotifyTool, PlanTool, PowerShellTool,
            ReadMediaFileTool, ScheduleCronTool, TaskListTool, TaskOutputTool, TaskStopTool,
            ThinkTool, TodoTool, WebBrowserTool, WebFetchTool, WebSearchTool,
        };
        #[cfg(not(target_os = "windows"))]
        use crate::tools::BashTool;

        let registry = Self::new();

        // Register file tools
        let _ = registry.register(FileReadTool::new());
        let _ = registry.register(FileWriteTool::new());
        let _ = registry.register(FileEditTool::new());

        // Register search tools
        let _ = registry.register(GlobTool::new());
        let _ = registry.register(GrepTool::new());

        // Register shell tools — platform-specific
        #[cfg(not(target_os = "windows"))]
        let _ = registry.register(BashTool::new());
        #[cfg(target_os = "windows")]
        let _ = registry.register(PowerShellTool::new());

        // Register web tools
        let _ = registry.register(WebSearchTool::new());
        let _ = registry.register(WebFetchTool::new());
        let _ = registry.register(WebBrowserTool::new());

        // Register think tool
        let _ = registry.register(ThinkTool::new());

        // Register task tools
        let _ = registry.register(TaskListTool::new());
        let _ = registry.register(TaskOutputTool::new());
        let _ = registry.register(TaskStopTool::new());

        // Register ask_user tool
        let _ = registry.register(AskUserTool::new());

        // Register notify tool
        let _ = registry.register(NotifyTool::new());

        // Register todo tool
        let _ = registry.register(TodoTool::new());

        // Register plan tool
        let _ = registry.register(PlanTool::new());

        // Register cron scheduling tools
        let _ = registry.register(ScheduleCronTool::new());
        let _ = registry.register(ListCronTool::new());
        let _ = registry.register(CancelCronTool::new());

        // Register computer use tool
        let _ = registry.register(ComputerUseTool::new());

        // Register media reading tool
        let _ = registry.register(ReadMediaFileTool::new());

        registry
    }

    /// Register a tool in the registry
    ///
    /// # Arguments
    ///
    /// * `tool` - The tool to register
    ///
    /// # Errors
    ///
    /// Returns an error if a tool with the same name is already registered
    pub fn register<T: Tool + 'static>(&self, tool: T) -> Result<(), AgentError> {
        let tool = Arc::new(tool) as SharedTool;
        self.register_shared(tool)
    }

    /// Register a shared tool in the registry
    ///
    /// # Arguments
    ///
    /// * `tool` - The shared tool to register
    ///
    /// # Errors
    ///
    /// Returns an error if a tool with the same name is already registered
    pub fn register_shared(&self, tool: SharedTool) -> Result<(), AgentError> {
        let name = tool.name().to_string();

        let mut tools = self
            .tools
            .write()
            .map_err(|_| AgentError::Registry("Registry lock poisoned".to_string()))?;

        if tools.contains_key(&name) {
            return Err(AgentError::Registry(format!(
                "Tool '{}' is already registered",
                name
            )));
        }

        info!("Registered tool: {}", name);
        tools.insert(name, tool);

        Ok(())
    }

    /// Unregister a tool by name
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to unregister
    ///
    /// # Returns
    ///
    /// `true` if the tool was found and removed, `false` otherwise
    pub fn unregister(&self, name: &str) -> Result<bool, AgentError> {
        let mut tools = self
            .tools
            .write()
            .map_err(|_| AgentError::Registry("Registry lock poisoned".to_string()))?;

        let removed = tools.remove(name).is_some();
        if removed {
            info!("Unregistered tool: {}", name);
        }

        Ok(removed)
    }

    /// Get a tool by name
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to retrieve
    ///
    /// # Returns
    ///
    /// Some(tool) if found, None otherwise
    pub fn get(&self, name: &str) -> Result<Option<SharedTool>, AgentError> {
        let tools = self
            .tools
            .read()
            .map_err(|_| AgentError::Registry("Registry lock poisoned".to_string()))?;

        Ok(tools.get(name).cloned())
    }

    /// Check if a tool is registered
    pub fn contains(&self, name: &str) -> Result<bool, AgentError> {
        let tools = self
            .tools
            .read()
            .map_err(|_| AgentError::Registry("Registry lock poisoned".to_string()))?;

        Ok(tools.contains_key(name))
    }

    /// Get all registered tool names
    pub fn list_tools(&self) -> Result<Vec<String>, AgentError> {
        let tools = self
            .tools
            .read()
            .map_err(|_| AgentError::Registry("Registry lock poisoned".to_string()))?;

        Ok(tools.keys().cloned().collect())
    }

    /// Get tool schemas for LLM function calling
    ///
    /// Returns a JSON array of tool definitions in the format expected by
    /// OpenAI-style function calling APIs.
    pub fn get_tool_schemas(&self) -> Result<Value, AgentError> {
        let tools = self
            .tools
            .read()
            .map_err(|_| AgentError::Registry("Registry lock poisoned".to_string()))?;

        let schemas: Vec<Value> = tools
            .values()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.parameters()
                    }
                })
            })
            .collect();

        Ok(Value::Array(schemas))
    }

    /// Get tool definitions in a simplified format
    ///
    /// Returns a JSON object mapping tool names to their schemas
    pub fn get_tool_definitions(&self) -> Result<Value, AgentError> {
        let tools = self
            .tools
            .read()
            .map_err(|_| AgentError::Registry("Registry lock poisoned".to_string()))?;

        let mut definitions = serde_json::Map::new();

        for tool in tools.values() {
            definitions.insert(
                tool.name().to_string(),
                json!({
                    "description": tool.description(),
                    "parameters": tool.parameters()
                }),
            );
        }

        Ok(Value::Object(definitions))
    }

    /// Execute a tool by name with the given arguments
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to execute
    /// * `args` - The arguments to pass to the tool
    /// * `ctx` - The execution context
    ///
    /// # Returns
    ///
    /// The result of the tool execution as a JSON Value
    pub async fn execute(&self, name: &str, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        let tool = self
            .get(name)
            .map_err(|e| ToolError::execution_failed(format!("Registry error: {}", e)))?
            .ok_or_else(|| ToolError::not_found(name))?;

        // Verify capability token if present
        if let Some(ref token) = ctx.capability_token {
            if let Err(e) = token.verify(name, &ctx.working_dir) {
                return Err(ToolError::PermissionDenied(e.to_string()));
            }
        }

        debug!("Executing tool '{}' with args: {:?}", name, args);

        let result = tool.execute(args, ctx).await;

        match &result {
            Ok(_) => debug!("Tool '{}' executed successfully", name),
            Err(e) => error!("Tool '{}' failed: {}", name, e),
        }

        result
    }

    /// Get the number of registered tools
    pub fn len(&self) -> Result<usize, AgentError> {
        let tools = self
            .tools
            .read()
            .map_err(|_| AgentError::Registry("Registry lock poisoned".to_string()))?;

        Ok(tools.len())
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> Result<bool, AgentError> {
        Ok(self.len()? == 0)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::FileReadTool;

    #[test]
    fn test_register_and_get() {
        let registry = ToolRegistry::new();

        assert!(registry.register(FileReadTool::new()).is_ok());
        assert!(registry.contains("file_read").unwrap());

        let tool = registry.get("file_read").unwrap();
        assert!(tool.is_some());

        let tool = registry.get("nonexistent").unwrap();
        assert!(tool.is_none());
    }

    #[test]
    fn test_duplicate_registration() {
        let registry = ToolRegistry::new();

        assert!(registry.register(FileReadTool::new()).is_ok());
        assert!(registry.register(FileReadTool::new()).is_err());
    }

    #[test]
    fn test_list_tools() {
        let registry = ToolRegistry::new();

        registry.register(FileReadTool::new()).unwrap();

        let tools = registry.list_tools().unwrap();
        assert_eq!(tools.len(), 1);
        assert!(tools.contains(&"file_read".to_string()));
    }

    #[test]
    fn test_get_schemas() {
        let registry = ToolRegistry::with_builtin_tools();

        let schemas = registry.get_tool_schemas().unwrap();
        assert!(schemas.is_array());

        let defs = registry.get_tool_definitions().unwrap();
        assert!(defs.is_object());
    }

    #[test]
    fn test_unregister() {
        let registry = ToolRegistry::new();

        registry.register(FileReadTool::new()).unwrap();
        assert!(registry.contains("file_read").unwrap());

        assert!(registry.unregister("file_read").unwrap());
        assert!(!registry.contains("file_read").unwrap());
        assert!(!registry.unregister("file_read").unwrap());
    }
}
