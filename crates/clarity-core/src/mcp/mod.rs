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
//! Moving this module into a standalone `clarity-mcp` crate is **blocked**
//! on `clarity-contract` maturity.  MCP currently depends on core-internal
//! types (`Tool`, `ToolContext`, `ToolResult`, `ToolRegistry`, `AgentError`)
//! which are not yet available in `clarity-contract`.
//!
//! The current `mcp` feature gate (`#[cfg(feature = "mcp")]`) already
//! satisfies the "optional compilation" requirement.  Re-evaluate extraction
//! after `clarity-contract` covers the tool-interface surface.
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

// Re-export from enhanced module
pub use enhanced::{
    HttpClientBuilder, HttpMcpClient, McpClient, McpClientBuilder, McpClientInstance, McpError,
    McpRegistry, McpResource, McpServerConfig, McpTool, McpTransport, OAuthConfig,
    SseClientBuilder, SseMcpClient, StdioClientBuilder, StdioMcpClient, ToolCallResult,
    ToolContent,
};

// Re-export MCP tool bridge
pub use tools::{register_mcp_tools, McpToolWrapper};

// Legacy exports for backward compatibility
use crate::error::{AgentError, ToolError};
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, RwLock};
use tracing::{debug, info};

// =============================================================================
// Legacy Types (for backward compatibility)
// =============================================================================

/// Legacy MCP client for stdio transport
pub struct McpClientLegacy {
    _command: String,
    _args: Vec<String>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    request_id: AtomicU64,
    pending: Arc<RwLock<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest<T: serde::Serialize> {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<T>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    id: u64,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize, Clone)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl McpClientLegacy {
    /// Connect to an MCP server via stdio
    pub async fn connect_stdio(
        command: impl Into<String>,
        args: &[impl AsRef<str>],
    ) -> Result<Self, AgentError> {
        let command = command.into();
        let args: Vec<String> = args.iter().map(|a| a.as_ref().to_string()).collect();

        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            AgentError::Tool(ToolError::ExecutionFailed(format!(
                "Failed to spawn MCP server: {}",
                e
            )))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            AgentError::Tool(ToolError::ExecutionFailed("Failed to open stdin".into()))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AgentError::Tool(ToolError::ExecutionFailed("Failed to open stdout".into()))
        })?;

        let pending: Arc<RwLock<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        // Spawn response reader
        let pending_clone = pending.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                debug!("MCP server response: {}", line);
                if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&line) {
                    let mut pending = pending_clone.write().await;
                    if let Some(sender) = pending.remove(&response.id) {
                        let _ = sender.send(response);
                    }
                }
            }
        });

        info!("Connected to MCP server: {} {:?}", command, args);

        Ok(Self {
            _command: command,
            _args: args,
            child: Some(child),
            stdin: Some(stdin),
            request_id: AtomicU64::new(1),
            pending,
        })
    }

    /// Send a JSON-RPC request
    pub async fn request<T: Serialize + Send>(
        &mut self,
        method: &str,
        params: Option<T>,
    ) -> Result<Value, AgentError> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.write().await;
            pending.insert(id, tx);
        }

        let request_json = serde_json::to_string(&request).map_err(|e| {
            AgentError::Tool(ToolError::ExecutionFailed(format!(
                "Failed to serialize request: {}",
                e
            )))
        })?;

        debug!("MCP request: {}", request_json);

        if let Some(ref mut stdin) = self.stdin {
            stdin
                .write_all(request_json.as_bytes())
                .await
                .map_err(|e| AgentError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| AgentError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
            stdin
                .flush()
                .await
                .map_err(|e| AgentError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
        }

        let response = rx.await.map_err(|_| {
            AgentError::Tool(ToolError::ExecutionFailed(
                "MCP request cancelled or timed out".into(),
            ))
        })?;

        if let Some(error) = response.error {
            return Err(AgentError::Tool(ToolError::ExecutionFailed(format!(
                "MCP error {}: {}",
                error.code, error.message
            ))));
        }

        response.result.ok_or_else(|| {
            AgentError::Tool(ToolError::ExecutionFailed(
                "MCP response missing result".into(),
            ))
        })
    }

    /// List available tools from the server
    pub async fn list_tools(&mut self) -> Result<Vec<McpToolInfo>, AgentError> {
        let result = self.request::<Value>("tools/list", None).await?;
        let tools: Vec<McpToolInfo> = serde_json::from_value(
            result.get("tools").cloned().unwrap_or_default(),
        )
        .map_err(|e| {
            AgentError::Tool(ToolError::ExecutionFailed(format!(
                "Failed to parse tools list: {}",
                e
            )))
        })?;
        Ok(tools)
    }

    /// Call a tool on the server
    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> Result<ToolCallResult, AgentError> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments
        });
        let result = self.request("tools/call", Some(params)).await?;
        serde_json::from_value(result).map_err(|e| {
            AgentError::Tool(ToolError::ExecutionFailed(format!(
                "Failed to parse tool result: {}",
                e
            )))
        })
    }
}

impl Drop for McpClientLegacy {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            tokio::spawn(async move {
                let _ = child.kill().await;
            });
        }
    }
}

/// Tool information from MCP server
#[derive(Debug, Clone, Deserialize)]
pub struct McpToolInfo {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
}

/// Result of calling an MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCallResultLegacy {
    pub content: Vec<ToolContentLegacy>,
    #[serde(default)]
    pub is_error: bool,
}

/// Content from tool execution
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContentLegacy {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: McpResourceLegacy },
}

/// Resource reference from tool execution
#[derive(Debug, Clone, Deserialize)]
pub struct McpResourceLegacy {
    pub uri: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub blob: Option<String>,
}

/// Map an [`McpError`] to a human-readable [`ToolError`].
fn map_mcp_error(err: McpError) -> ToolError {
    match err {
        McpError::ConnectionFailed(msg)
        | McpError::InvalidTransport(msg)
        | McpError::CommandNotAllowed(msg)
        | McpError::RequestFailed(msg) => {
            ToolError::Unavailable(format!("MCP transport error: {}", msg))
        }
        McpError::RequestTimeout => ToolError::Unavailable("MCP request timed out".to_string()),
        McpError::Io(io_err) => ToolError::Unavailable(format!("MCP I/O error: {}", io_err)),
        McpError::RpcError(msg) => {
            ToolError::execution_failed(format!("MCP server error: {}", msg))
        }
        McpError::InvalidResponse(msg) => {
            ToolError::execution_failed(format!("MCP invalid response: {}", msg))
        }
        McpError::Serialization(err) => {
            ToolError::execution_failed(format!("MCP serialization error: {}", err))
        }
    }
}

/// Adapter to use MCP tools as Clarity tools
#[derive(Clone)]
pub struct McpToolAdapter {
    client: Arc<tokio::sync::Mutex<McpClientInstance>>,
    tool_info: McpTool,
}

impl McpToolAdapter {
    pub fn new(client: Arc<tokio::sync::Mutex<McpClientInstance>>, tool_info: McpTool) -> Self {
        Self { client, tool_info }
    }

    pub fn name(&self) -> &str {
        &self.tool_info.name
    }

    pub fn description(&self) -> Option<&str> {
        self.tool_info.description.as_deref()
    }

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

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        tracing::debug!("Executing MCP tool: {}", self.name());

        let client = self.client.lock().await;
        let result = client
            .call_tool(self.name(), args)
            .await
            .map_err(map_mcp_error)?;

        let mut texts = Vec::new();
        for content in result.content {
            match content {
                ToolContent::Text { text } => texts.push(text),
                ToolContent::Image { data: _, mime_type } => {
                    texts.push(format!("[Image: {}]", mime_type));
                }
                ToolContent::Resource { resource } => {
                    if let Some(text) = resource.text {
                        texts.push(text);
                    } else {
                        texts.push(format!("[Resource: {}]", resource.uri));
                    }
                }
            }
        }

        let joined = texts.join("\n");

        if result.is_error {
            return Err(ToolError::execution_failed(joined));
        }

        // Detect application-level errors in JSON payloads
        // (e.g. devbase returning {"success":false,"error":"..."})
        if let Ok(parsed) = serde_json::from_str::<Value>(&joined) {
            let has_error_field = parsed.get("error").is_some();
            let success_false = parsed.get("success").and_then(|v| v.as_bool()) == Some(false);
            if has_error_field || success_false {
                return Err(ToolError::execution_failed(joined));
            }
        }

        Ok(Value::String(joined))
    }
}

/// Manager for multiple MCP servers
pub struct McpManager {
    clients: HashMap<String, Arc<tokio::sync::Mutex<McpClientInstance>>>,
    tools: Vec<McpToolAdapter>,
}

impl McpManager {
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
            .map_err(|e| AgentError::Tool(map_mcp_error(e)))
    }

    /// Get all discovered tools as Clarity tools
    pub fn tools(&self) -> &[McpToolAdapter] {
        &self.tools
    }

    /// Register all discovered tools into a `ToolRegistry`.
    /// Duplicate names are logged as warnings and skipped.
    pub fn register_all(&self, registry: &crate::registry::ToolRegistry) {
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
