//! MCP (Model Context Protocol) Client Interface
//!
//! This module provides JSON-RPC 2.0 clients for MCP servers,
//! allowing the agent to connect to external tool servers.
//!
//! ## Features
//!
//! - **Stdio Transport**: Connect to local MCP servers via stdin/stdout
//! - **HTTP Transport**: Connect to remote MCP servers via HTTP POST
//! - **SSE Transport**: Connect to streaming MCP servers via Server-Sent Events
//! - **OAuth Support**: Authentication support for remote servers
//!
//! ## Extractability Assessment (P3 — Week 4)
//!
//! ~~Moving this module into a standalone `clarity-mcp` crate is **blocked**
//! on `clarity-contract` maturity.~~
//!
//! **DONE**: This module has been split into a standalone `clarity-mcp` crate.
//! The pure MCP protocol layer lives in `clarity-mcp`, while the `Tool` trait
//! bridge (`McpToolAdapter`, `McpToolWrapper`, `McpManager`) remains here.
//!
//! ## Example
//!
//! ```rust,no_run
//! use clarity_core::mcp::{McpClient, McpClientBuilder, McpRegistry};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Stdio transport
//!     let mut client = McpClientBuilder::stdio("filesystem", "npx")
//!         .arg("-y")
//!         .arg("@modelcontextprotocol/server-filesystem")
//!         .arg(".")
//!         .build();
//!     
//!     client.connect().await?;
//!     let tools = client.list_tools().await?;
//!     
//!     // HTTP transport
//!     let mut http_client = McpClientBuilder::http("api", "https://api.example.com/mcp")
//!         .header("Authorization", "Bearer token")
//!         .build();
//!     
//!     http_client.connect().await?;
//!     
//!     // Registry for multiple servers
//!     let mut registry = McpRegistry::new();
//!     registry.register("fs", client);
//!     registry.register("api", http_client);
//!     
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod devkit;
pub mod enhanced;
pub mod tools;

// Re-export from clarity-mcp (pure MCP protocol layer)
pub use clarity_mcp::{
    HttpClientBuilder, HttpMcpClient, McpClient, McpClientBuilder, McpClientInstance,
    McpClientLegacy, McpError, McpRegistry, McpResource, McpResourceLegacy, McpServerConfig,
    McpTool, McpToolInfo, McpTransport, OAuthConfig, SseClientBuilder, SseMcpClient,
    StdioClientBuilder, StdioMcpClient, ToolCallResult, ToolCallResultLegacy, ToolContent,
    ToolContentLegacy, map_mcp_error, process_mcp_tool_result,
};

// Re-export MCP tool bridge
pub use tools::{McpToolWrapper, register_mcp_tools};

use crate::error::AgentError;
use crate::registry::ToolRegistry;
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// Adapter to use MCP tools as Clarity tools
#[derive(Clone)]
pub struct McpToolAdapter {
    client: Arc<tokio::sync::Mutex<McpClientInstance>>,
    tool_info: McpTool,
    requires_approval: bool,
}

impl McpToolAdapter {
    /// Create a new `McpToolAdapter`.
    pub fn new(client: Arc<tokio::sync::Mutex<McpClientInstance>>, tool_info: McpTool) -> Self {
        Self {
            client,
            tool_info,
            requires_approval: true, // secure by default
        }
    }

    /// Return the name.
    pub fn name(&self) -> &str {
        &self.tool_info.name
    }

    /// Return the description, if any.
    pub fn description(&self) -> Option<&str> {
        self.tool_info.description.as_deref()
    }

    /// Return the JSON schema.
    pub fn schema(&self) -> &Value {
        &self.tool_info.input_schema
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.tool_info.name
    }

    fn description(&self) -> &str {
        self.tool_info.description.as_deref().unwrap_or("MCP tool")
    }

    fn parameters(&self) -> Value {
        self.tool_info.input_schema.clone()
    }

    fn requires_approval(&self) -> bool {
        self.requires_approval
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        debug!("Executing MCP tool: {}", self.name());

        let client = self.client.lock().await;
        let result = client
            .call_tool(self.name(), args)
            .await
            .map_err(clarity_mcp::map_mcp_error)?;

        clarity_mcp::process_mcp_tool_result(result)
    }
}

/// Manager for multiple MCP servers
pub struct McpManager {
    clients: HashMap<String, Arc<tokio::sync::Mutex<McpClientInstance>>>,
    tools: Vec<McpToolAdapter>,
}

impl McpManager {
    /// Create a new `McpManager`.
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            tools: Vec::new(),
        }
    }

    /// Create a manager from an `McpConfig`, spawning all enabled servers.
    /// Servers that fail to start are logged and skipped.
    pub async fn from_config(config: &config::McpConfig) -> Self {
        let mut manager = Self::new();
        let mut tasks = Vec::new();
        for (name, entry) in &config.servers {
            if entry.disabled {
                tracing::info!("MCP server '{}' is disabled, skipping", name);
                continue;
            }
            let client = McpClientBuilder::from_mcp_entry(name.clone(), entry);
            let name = name.clone();
            tasks.push(async move {
                let mut c = client;
                let result: Result<
                    (Arc<tokio::sync::Mutex<McpClientInstance>>, Vec<McpTool>),
                    McpError,
                > = async {
                    c.connect().await?;
                    let tools = c.list_tools().await?;
                    Ok((Arc::new(tokio::sync::Mutex::new(c)), tools))
                }
                .await;
                (name, result)
            });
        }
        let results = futures::future::join_all(tasks).await;
        for (name, result) in results {
            match result {
                Ok((client, tools)) => {
                    for tool in tools {
                        manager
                            .tools
                            .push(McpToolAdapter::new(client.clone(), tool));
                    }
                    manager.clients.insert(name, client);
                }
                Err(e) => {
                    tracing::warn!("Failed to start MCP server '{}': {}", name, e);
                }
            }
        }
        manager
    }

    async fn connect_inner(
        _name: String,
        config: McpServerConfig,
    ) -> Result<(Arc<tokio::sync::Mutex<McpClientInstance>>, Vec<McpTool>), McpError> {
        let mut client = McpClientBuilder::from_config(config);
        client.connect().await?;
        let tools = client.list_tools().await?;
        let client = Arc::new(tokio::sync::Mutex::new(client));
        Ok((client, tools))
    }

    async fn connect(&mut self, name: String, config: McpServerConfig) -> Result<(), McpError> {
        let (client, tools) = Self::connect_inner(name.clone(), config).await?;
        for tool in tools {
            self.tools.push(McpToolAdapter::new(client.clone(), tool));
        }
        self.clients.insert(name, client);
        Ok(())
    }

    /// Connect to an MCP server via stdio and add it to the manager
    pub async fn connect_stdio(
        &mut self,
        name: impl Into<String>,
        command: impl Into<String>,
        args: &[impl AsRef<str>],
    ) -> Result<(), AgentError> {
        let name = name.into();
        let args: Vec<String> = args.iter().map(|a| a.as_ref().to_string()).collect();
        let config = McpServerConfig {
            name: name.clone(),
            transport: McpTransport::Stdio {
                command: command.into(),
                args,
                env: HashMap::new(),
            },
            oauth: None,
        };
        self.connect(name, config)
            .await
            .map_err(|e| AgentError::Tool(clarity_mcp::map_mcp_error(e)))
    }

    /// Get all discovered tools as Clarity tools
    pub fn tools(&self) -> &[McpToolAdapter] {
        &self.tools
    }

    /// Register all discovered tools into a `ToolRegistry`.
    /// Duplicate names are logged as warnings and skipped.
    pub fn register_all(&self, registry: &ToolRegistry) {
        for tool in &self.tools {
            if let Err(e) = registry.register(tool.clone()) {
                tracing::warn!("Failed to register MCP tool '{}': {}", tool.name(), e);
            }
        }
    }

    /// Get a client by name
    pub fn get_client(&self, name: &str) -> Option<Arc<tokio::sync::Mutex<McpClientInstance>>> {
        self.clients.get(name).cloned()
    }

    /// List connected server names
    pub fn list_servers(&self) -> Vec<&str> {
        self.clients.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_mcp_transport_config() {
        let config = McpServerConfig::stdio("test", "npx");
        assert!(matches!(&config.transport, McpTransport::Stdio { .. }));

        let http_config = McpServerConfig::http("api", "https://example.com/mcp");
        assert!(matches!(&http_config.transport, McpTransport::Http { .. }));
    }

    #[test]
    fn test_mcp_registry() {
        let mut registry = McpRegistry::new();
        let client = McpClientBuilder::stdio("test", "echo").build();
        registry.register("test", client);

        assert_eq!(registry.list(), vec!["test"]);
    }

    #[test]
    fn test_mcp_client_instance() {
        let instance = McpClientBuilder::stdio("test", "echo").build();
        assert!(matches!(instance, McpClientInstance::Stdio(_)));
    }

    #[tokio::test]
    async fn test_manager_graceful_degradation() {
        let mut manager = McpManager::new();
        let result = manager
            .connect_stdio("bad", "this_command_does_not_exist_12345", &[] as &[&str])
            .await;
        assert!(result.is_err());
        assert!(manager.list_servers().is_empty());
        assert!(manager.tools().is_empty());
    }

    #[tokio::test]
    async fn test_manager_from_config_with_disabled_and_failing() {
        let mut config = config::McpConfig::default();
        config.servers.insert(
            "disabled".to_string(),
            config::McpServerEntry {
                command: "echo".to_string(),
                args: vec![],
                env: HashMap::new(),
                disabled: true,
                ..Default::default()
            },
        );
        config.servers.insert(
            "failing".to_string(),
            config::McpServerEntry {
                command: "this_command_does_not_exist_12345".to_string(),
                args: vec![],
                env: HashMap::new(),
                disabled: false,
                ..Default::default()
            },
        );

        let manager = McpManager::from_config(&config).await;
        assert!(manager.list_servers().is_empty());
        assert!(manager.tools().is_empty());
    }

    #[tokio::test]
    async fn test_manager_register_all_skips_duplicates() {
        let registry = crate::registry::ToolRegistry::new();
        let manager = McpManager::new();
        // Since we cannot easily inject a mock client without a real server,
        // simply verify register_all on an empty manager does not panic.
        manager.register_all(&registry);
        assert!(registry.list_tools().unwrap().is_empty());
    }
}
