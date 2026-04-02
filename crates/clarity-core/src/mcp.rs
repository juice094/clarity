//! MCP (Model Context Protocol) Client Interface
//!
//! This module provides a client interface for MCP servers, allowing
//! the agent to connect to external tool servers that implement the
//! Model Context Protocol.
//!
//! ## Overview
//!
//! MCP is a protocol for connecting AI assistants to external data
//! sources and tools. This module provides:
//!
//! - MCP client implementation
//! - Tool adapter for MCP servers
//! - Connection management
//!
//! ## Example
//!
//! ```rust,no_run
//! use clarity_core::mcp::McpClient;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Connect to an MCP server
//!     let client = McpClient::connect("stdio", "mcp-server-example").await?;
//!     
//!     // List available tools from the server
//!     let tools = client.list_tools().await?;
//!     
//!     // Execute a tool on the server
//!     let result = client.call_tool("search", serde_json::json!({
//!         "query": "example"
//!     })).await?;
//!     
//!     Ok(())
//! }
//! ```

use crate::error::{AgentError, ToolError};
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// MCP Tool definition from a server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// MCP Client for connecting to MCP servers
///
/// This is a placeholder/stub implementation. The full implementation
/// would use the rmcp crate or a custom MCP client implementation.
#[derive(Clone)]
pub struct McpClient {
    inner: Arc<RwLock<McpClientInner>>,
}

#[derive(Debug)]
struct McpClientInner {
    server_name: String,
    connected: bool,
    tools: Vec<McpTool>,
    session: Option<McpSession>,
}

#[derive(Debug)]
struct McpSession;

impl McpClient {
    /// Create a new MCP client connection
    ///
    /// # Arguments
    ///
    /// * `transport` - Transport type ("stdio", "http", "websocket")
    /// * `server" - Server command or URL
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use clarity_core::mcp::McpClient;
    ///
    /// async fn connect() -> anyhow::Result<McpClient> {
    ///     // Connect via stdio to a local MCP server
    ///     let client = McpClient::connect("stdio", "./my-mcp-server").await?;
    ///     Ok(client)
    /// }
    /// ```
    pub async fn connect(
        _transport: &str,
        server: impl Into<String>,
    ) -> Result<Self, AgentError> {
        let server_name = server.into();
        info!("Connecting to MCP server: {}", server_name);
        
        // TODO: Implement actual MCP connection using rmcp or custom protocol
        // For now, this is a placeholder that returns a mock client
        
        let inner = McpClientInner {
            server_name,
            connected: true,
            tools: vec![],
            session: None,
        };
        
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
        })
    }
    
    /// Check if the client is connected
    pub async fn is_connected(&self) -> bool {
        let inner = self.inner.read().await;
        inner.connected
    }
    
    /// List tools available from the MCP server
    pub async fn list_tools(&self) -> Result<Vec<McpTool>, AgentError> {
        let inner = self.inner.read().await;
        
        if !inner.connected {
            return Err(AgentError::Llm("MCP client not connected".to_string()));
        }
        
        // TODO: Implement actual tool listing via MCP protocol
        Ok(inner.tools.clone())
    }
    
    /// Call a tool on the MCP server
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, AgentError> {
        let inner = self.inner.read().await;
        
        if !inner.connected {
            return Err(AgentError::Llm("MCP client not connected".to_string()));
        }
        
        debug!("Calling MCP tool '{}' with args: {:?}", tool_name, arguments);
        
        // TODO: Implement actual tool calling via MCP protocol
        // This is a placeholder
        
        Ok(json!({
            "tool": tool_name,
            "result": "placeholder",
            "note": "MCP implementation pending"
        }))
    }
    
    /// Disconnect from the MCP server
    pub async fn disconnect(&self) -> Result<(), AgentError> {
        let mut inner = self.inner.write().await;
        inner.connected = false;
        inner.session = None;
        info!("Disconnected from MCP server: {}", inner.server_name);
        Ok(())
    }
}

/// Adapter that wraps an MCP client tool as a Clarity Tool
///
/// This allows MCP tools to be registered in the ToolRegistry
/// and used by the Agent just like native tools.
pub struct McpToolAdapter {
    client: McpClient,
    tool: McpTool,
}

impl McpToolAdapter {
    /// Create a new adapter for an MCP tool
    pub fn new(client: McpClient, tool: McpTool) -> Self {
        Self { client, tool }
    }
    
    /// Get the underlying MCP tool definition
    pub fn tool_def(&self) -> &McpTool {
        &self.tool
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.tool.name
    }
    
    fn description(&self) -> &str {
        &self.tool.description
    }
    
    fn parameters(&self) -> Value {
        self.tool.parameters.clone()
    }
    
    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        self.client
            .call_tool(&self.tool.name, args)
            .await
            .map_err(|e| ToolError::execution_failed(format!("MCP error: {}", e)))
    }
}

/// Manager for multiple MCP connections
///
/// Provides centralized management of MCP client connections.
pub struct McpManager {
    clients: Arc<RwLock<HashMap<String, McpClient>>>,
}

impl McpManager {
    /// Create a new MCP manager
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Add an MCP connection
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this connection
    /// * `client" - The MCP client
    pub async fn add_client(
        &self,
        name: impl Into<String>,
        client: McpClient,
    ) -> Result<(), AgentError> {
        let name = name.into();
        let mut clients = self.clients.write().await;
        
        if clients.contains_key(&name) {
            return Err(AgentError::Registry(
                format!("MCP client '{}' already exists", name)
            ));
        }
        
        clients.insert(name, client);
        Ok(())
    }
    
    /// Get an MCP client by name
    pub async fn get_client(&self, name: &str) -> Option<McpClient> {
        let clients = self.clients.read().await;
        clients.get(name).cloned()
    }
    
    /// Remove an MCP client
    pub async fn remove_client(&self, name: &str) -> Result<(), AgentError> {
        let mut clients = self.clients.write().await;
        
        if let Some(client) = clients.remove(name) {
            client.disconnect().await?;
            info!("Removed MCP client: {}", name);
        }
        
        Ok(())
    }
    
    /// List all connected MCP clients
    pub async fn list_clients(&self) -> Vec<String> {
        let clients = self.clients.read().await;
        clients.keys().cloned().collect()
    }
    
    /// Get all tools from all MCP connections as Tool adapters
    pub async fn get_all_tools(&self) -> Vec<McpToolAdapter> {
        let clients = self.clients.read().await;
        let mut adapters = vec![];
        
        for (name, client) in clients.iter() {
            match client.list_tools().await {
                Ok(tools) => {
                    for tool in tools {
                        adapters.push(McpToolAdapter::new(client.clone(), tool));
                    }
                }
                Err(e) => {
                    warn!("Failed to list tools from MCP client '{}': {}", name, e);
                }
            }
        }
        
        adapters
    }
    
    /// Disconnect all MCP clients
    pub async fn disconnect_all(&self) -> Result<(), AgentError> {
        let mut clients = self.clients.write().await;
        
        for (name, client) in clients.iter() {
            if let Err(e) = client.disconnect().await {
                error!("Error disconnecting MCP client '{}': {}", name, e);
            }
        }
        
        clients.clear();
        Ok(())
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export types for convenience
pub use crate::tools::SharedTool;

use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mcp_tool_creation() {
        let tool = McpTool {
            name: "search".to_string(),
            description: "Search for documents".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            }),
        };
        
        assert_eq!(tool.name, "search");
    }
    
    #[tokio::test]
    async fn test_mcp_manager() {
        let manager = McpManager::new();
        
        // Initially empty
        let clients = manager.list_clients().await;
        assert!(clients.is_empty());
        
        // Note: Can't test add_client without an actual MCP server connection
    }
}
