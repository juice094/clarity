use super::{
    GetPromptResult, ListPromptsResult, ListResourcesResult, McpError, McpTool, ReadResourceResult,
    ToolCallResult,
};
use async_trait::async_trait;
use serde_json::Value;
/// Asynchronous MCP client interface.
#[async_trait]
pub trait McpClient: Send + Sync {
    /// Connect to the server
    async fn connect(&mut self) -> Result<(), McpError>;

    /// Disconnect from the server
    async fn disconnect(&mut self) -> Result<(), McpError>;

    /// Send a raw JSON-RPC request
    async fn request_raw(&self, method: &str, params: Option<Value>) -> Result<Value, McpError>;

    /// List available tools
    async fn list_tools(&self) -> Result<Vec<McpTool>, McpError> {
        let result = self.request_raw("tools/list", None).await?;
        let tools = result
            .get("tools")
            .cloned()
            .unwrap_or_else(|| Value::Array(vec![]));
        serde_json::from_value(tools).map_err(McpError::Serialization)
    }

    /// Call a tool
    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments
        });
        let result = self.request_raw("tools/call", Some(params)).await?;
        serde_json::from_value(result).map_err(McpError::Serialization)
    }

    /// List available resources
    async fn list_resources(&self) -> Result<ListResourcesResult, McpError> {
        let result = self.request_raw("resources/list", None).await?;
        serde_json::from_value(result).map_err(McpError::Serialization)
    }

    /// Read a resource by URI
    async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult, McpError> {
        let params = serde_json::json!({ "uri": uri });
        let result = self.request_raw("resources/read", Some(params)).await?;
        serde_json::from_value(result).map_err(McpError::Serialization)
    }

    /// List available prompts
    async fn list_prompts(&self) -> Result<ListPromptsResult, McpError> {
        let result = self.request_raw("prompts/list", None).await?;
        serde_json::from_value(result).map_err(McpError::Serialization)
    }

    /// Get a prompt by name with optional arguments
    async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<GetPromptResult, McpError> {
        let mut params = serde_json::json!({ "name": name });
        if let Some(args) = arguments {
            params["arguments"] = args;
        }
        let result = self.request_raw("prompts/get", Some(params)).await?;
        serde_json::from_value(result).map_err(McpError::Serialization)
    }
}
