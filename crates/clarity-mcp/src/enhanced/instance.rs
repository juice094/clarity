use super::{HttpMcpClient, McpClient, McpError, SseMcpClient, StdioMcpClient, WebSocketMcpClient};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
// =============================================================================
// MCP Registry
// =============================================================================

/// Polymorphic MCP client instance produced by [`McpClientBuilder`].
pub enum McpClientInstance {
    /// Stdio subprocess transport.
    Stdio(Box<StdioMcpClient>),
    /// HTTP POST transport.
    Http(HttpMcpClient),
    /// Server-Sent Events transport.
    Sse(SseMcpClient),
    /// WebSocket transport.
    WebSocket(WebSocketMcpClient),
}

#[async_trait]
impl McpClient for McpClientInstance {
    async fn connect(&mut self) -> Result<(), McpError> {
        match self {
            McpClientInstance::Stdio(c) => c.connect().await,
            McpClientInstance::Http(c) => c.connect().await,
            McpClientInstance::Sse(c) => c.connect().await,
            McpClientInstance::WebSocket(c) => c.connect().await,
        }
    }

    async fn disconnect(&mut self) -> Result<(), McpError> {
        match self {
            McpClientInstance::Stdio(c) => c.disconnect().await,
            McpClientInstance::Http(c) => c.disconnect().await,
            McpClientInstance::Sse(c) => c.disconnect().await,
            McpClientInstance::WebSocket(c) => c.disconnect().await,
        }
    }

    async fn request_raw(&self, method: &str, params: Option<Value>) -> Result<Value, McpError> {
        match self {
            McpClientInstance::Stdio(c) => c.request_raw(method, params).await,
            McpClientInstance::Http(c) => c.request_raw(method, params).await,
            McpClientInstance::Sse(c) => c.request_raw(method, params).await,
            McpClientInstance::WebSocket(c) => c.request_raw(method, params).await,
        }
    }
}

/// Registry holding named MCP client instances.
pub struct McpRegistry {
    clients: HashMap<String, Arc<RwLock<McpClientInstance>>>,
}

impl McpRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// Register a client under the given name.
    pub fn register(&mut self, name: impl Into<String>, client: McpClientInstance) {
        self.clients
            .insert(name.into(), Arc::new(RwLock::new(client)));
    }

    /// Look up a registered client by name.
    pub fn get(&self, name: &str) -> Option<&Arc<RwLock<McpClientInstance>>> {
        self.clients.get(name)
    }

    /// Remove and return a registered client by name.
    pub fn remove(&mut self, name: &str) -> Option<Arc<RwLock<McpClientInstance>>> {
        self.clients.remove(name)
    }

    /// Return the names of all registered clients.
    pub fn list(&self) -> Vec<&str> {
        self.clients.keys().map(|k| k.as_str()).collect()
    }

    /// Connect all registered clients.
    pub async fn connect_all(&self) -> Result<(), McpError> {
        for (name, client) in &self.clients {
            info!("Connecting to MCP server: {}", name);
            client.write().await.connect().await?;
        }
        Ok(())
    }

    /// Disconnect all registered clients.
    pub async fn disconnect_all(&self) -> Result<(), McpError> {
        for (name, client) in &self.clients {
            info!("Disconnecting from MCP server: {}", name);
            client.write().await.disconnect().await?;
        }
        Ok(())
    }

    /// Iterate over registered clients.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Arc<RwLock<McpClientInstance>>)> {
        self.clients.iter()
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}
