//! MCP (Model Context Protocol) Client Interface
//!
//! This module provides a minimal JSON-RPC 2.0 stdio client for MCP servers,
//! allowing the agent to connect to external tool servers that implement the
//! Model Context Protocol.
//!
//! ## Overview
//!
//! MCP is a protocol for connecting AI assistants to external data
//! sources and tools. This module provides:
//!
//! - MCP client implementation over stdio (JSON-RPC 2.0 / NDJSON)
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
//!     // Connect to an MCP server via stdio
//!     let client = McpClient::connect_stdio("npx", &["-y", "@modelcontextprotocol/server-filesystem", "."]).await?;
//!     
//!     // List available tools from the server
//!     let tools = client.list_tools().await?;
//!     
//!     // Execute a tool on the server
//!     let result = client.call_tool("read_file", serde_json::json!({
//!         "path": "example.txt"
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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, RwLock};
use tracing::{debug, error, info, warn};

// =============================================================================
// JSON-RPC 2.0 Types
// =============================================================================

#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: T,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcResponse<T> {
    jsonrpc: String,
    id: u64,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}

// =============================================================================
// MCP Protocol Types
// =============================================================================

/// MCP Tool definition from a server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    /// Standard MCP field for input schema
    #[serde(rename = "inputSchema", default)]
    pub input_schema: Option<Value>,
    /// Fallback field used by some servers
    #[serde(default)]
    pub parameters: Option<Value>,
}

impl McpTool {
    /// Get the parameter schema for this tool
    pub fn schema(&self) -> Value {
        self.input_schema
            .clone()
            .or_else(|| self.parameters.clone())
            .unwrap_or_else(|| serde_json::json!({"type": "object"}))
    }
}

#[derive(Debug, Serialize)]
struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
    capabilities: Value,
    #[serde(rename = "clientInfo")]
    client_info: ClientInfo,
}

#[derive(Debug, Serialize)]
struct ClientInfo {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    _protocol_version: String,
    #[serde(rename = "serverInfo")]
    server_info: ServerInfo,
    capabilities: Option<Value>,
}

#[derive(Debug, Deserialize, Clone)]
struct ServerInfo {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct ListToolsResult {
    tools: Vec<McpTool>,
}

#[derive(Debug, Serialize)]
struct CallToolParams {
    name: String,
    arguments: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CallToolResult {
    content: Vec<ToolContent>,
    #[serde(rename = "isError")]
    is_error: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[allow(dead_code)]
enum ToolContent {
    Text { text: String },
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    Resource { resource: Value },
}

// =============================================================================
// Stdio Transport
// =============================================================================

type PendingRequest = oneshot::Sender<Result<Value, AgentError>>;

struct McpTransport {
    stdin: tokio::sync::Mutex<tokio::io::BufWriter<ChildStdin>>,
    pending: Arc<RwLock<HashMap<u64, PendingRequest>>>,
    next_id: AtomicU64,
    /// Keep the child process alive as long as the transport exists
    _child: Child,
}

impl McpTransport {
    async fn spawn(command: &str, args: &[impl AsRef<str>]) -> Result<Self, AgentError> {
        let args: Vec<String> = args.iter().map(|a| a.as_ref().to_string()).collect();
        info!("Spawning MCP server process: {} {:?}", command, args);
        let args_slice: &[String] = &args;

        let mut cmd = Command::new(command);
        cmd.args(args_slice)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| AgentError::Registry(format!("Failed to spawn MCP server: {}", e)))?;

        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");

        let pending = Arc::new(RwLock::new(HashMap::<u64, PendingRequest>::new()));
        let pending_stdout = pending.clone();

        // stdout reader task
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                debug!("MCP stdout: {}", line);
                match serde_json::from_str::<JsonRpcResponse<Value>>(&line) {
                    Ok(resp) => {
                        let mut map = pending_stdout.write().await;
                        if let Some(tx) = map.remove(&resp.id) {
                            let result = if let Some(err) = resp.error {
                                Err(AgentError::ToolExecutionFailed(
                                    "MCP".to_string(),
                                    format!("JSON-RPC error {}: {}", err.code, err.message),
                                ))
                            } else {
                                Ok(resp.result.unwrap_or(Value::Null))
                            };
                            let _ = tx.send(result);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse MCP response: {}", e);
                    }
                }
            }
            debug!("MCP stdout reader closed");
        });

        // stderr reader task
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                warn!("MCP stderr: {}", line);
            }
        });

        Ok(Self {
            stdin: tokio::sync::Mutex::new(tokio::io::BufWriter::new(stdin)),
            pending,
            next_id: AtomicU64::new(1),
            _child: child,
        })
    }

    async fn request<T: Serialize>(
        &self,
        method: impl Into<String>,
        params: T,
    ) -> Result<Value, AgentError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        };

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.write().await;
            pending.insert(id, tx);
        }

        let json = serde_json::to_string(&req)
            .map_err(|e| AgentError::Registry(format!("JSON serialize error: {}", e)))?;

        debug!("MCP request: {}", json);

        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(json.as_bytes())
                .await
                .map_err(|e| AgentError::Registry(format!("MCP write error: {}", e)))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| AgentError::Registry(format!("MCP write error: {}", e)))?;
            stdin
                .flush()
                .await
                .map_err(|e| AgentError::Registry(format!("MCP flush error: {}", e)))?;
        }

        let timeout = tokio::time::Duration::from_secs(30);
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                let mut pending = self.pending.write().await;
                pending.remove(&id);
                Err(AgentError::Registry("MCP request cancelled".to_string()))
            }
            Err(_) => {
                let mut pending = self.pending.write().await;
                pending.remove(&id);
                Err(AgentError::Registry("MCP request timed out".to_string()))
            }
        }
    }

    async fn notify<T: Serialize>(
        &self,
        method: impl Into<String>,
        params: T,
    ) -> Result<(), AgentError> {
        #[derive(Serialize)]
        struct Notification<T> {
            jsonrpc: &'static str,
            method: String,
            params: T,
        }

        let req = Notification {
            jsonrpc: "2.0",
            method: method.into(),
            params,
        };

        let json = serde_json::to_string(&req)
            .map_err(|e| AgentError::Registry(format!("JSON serialize error: {}", e)))?;

        debug!("MCP notify: {}", json);

        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(json.as_bytes())
                .await
                .map_err(|e| AgentError::Registry(format!("MCP write error: {}", e)))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| AgentError::Registry(format!("MCP write error: {}", e)))?;
            stdin
                .flush()
                .await
                .map_err(|e| AgentError::Registry(format!("MCP flush error: {}", e)))?;
        }

        Ok(())
    }
}

// =============================================================================
// MCP Client
// =============================================================================

/// MCP Client for connecting to MCP servers
#[derive(Clone)]
pub struct McpClient {
    transport: Arc<McpTransport>,
    server_info: Arc<RwLock<Option<ServerInfo>>>,
    tools: Arc<RwLock<Vec<McpTool>>>,
}

impl McpClient {
    /// Connect to an MCP server via stdio transport.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to spawn the MCP server
    /// * `args` - Arguments to pass to the command
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use clarity_core::mcp::McpClient;
    ///
    /// async fn connect() -> anyhow::Result<McpClient> {
    ///     let client = McpClient::connect_stdio("npx", &["-y", "@modelcontextprotocol/server-filesystem", "."]).await?;
    ///     Ok(client)
    /// }
    /// ```
    pub async fn connect_stdio(
        command: impl AsRef<str>,
        args: &[impl AsRef<str>],
    ) -> Result<Self, AgentError> {
        let cmd = command.as_ref();
        info!("Connecting to MCP server via stdio: {}", cmd);

        let transport = Arc::new(McpTransport::spawn(cmd, args).await?);

        // Initialize handshake
        let init_params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: serde_json::json!({}),
            client_info: ClientInfo {
                name: "clarity-core".to_string(),
                version: crate::VERSION.to_string(),
            },
        };

        let init_result: InitializeResult = serde_json::from_value(
            transport.request("initialize", init_params).await?,
        )
        .map_err(|e| AgentError::Registry(format!("Invalid initialize response: {}", e)))?;

        info!(
            "MCP server initialized: {} v{}",
            init_result.server_info.name, init_result.server_info.version
        );

        // Send initialized notification
        transport
            .notify("notifications/initialized", serde_json::Value::Null)
            .await?;

        let client = Self {
            transport,
            server_info: Arc::new(RwLock::new(Some(init_result.server_info))),
            tools: Arc::new(RwLock::new(vec![])),
        };

        // Discover tools
        let tools = client.discover_tools().await?;
        {
            let mut t = client.tools.write().await;
            *t = tools;
        }

        Ok(client)
    }

    /// Check if the client is connected
    pub async fn is_connected(&self) -> bool {
        self.server_info.read().await.is_some()
    }

    async fn discover_tools(&self) -> Result<Vec<McpTool>, AgentError> {
        let result: ListToolsResult = serde_json::from_value(
            self.transport.request("tools/list", serde_json::json!({})).await?,
        )
        .map_err(|e| AgentError::Registry(format!("Invalid tools/list response: {}", e)))?;
        info!("Discovered {} MCP tools", result.tools.len());
        Ok(result.tools)
    }

    /// List tools available from the MCP server
    pub async fn list_tools(&self) -> Result<Vec<McpTool>, AgentError> {
        let tools = self.tools.read().await.clone();
        Ok(tools)
    }

    /// Call a tool on the MCP server
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, AgentError> {
        let params = CallToolParams {
            name: tool_name.to_string(),
            arguments: if arguments.is_null() {
                None
            } else {
                Some(arguments)
            },
        };

        let result: CallToolResult = serde_json::from_value(
            self.transport.request("tools/call", params).await?,
        )
        .map_err(|e| AgentError::Registry(format!("Invalid tools/call response: {}", e)))?;

        let is_error = result.is_error.unwrap_or(false);
        let text: String = result
            .content
            .iter()
            .filter_map(|c| match c {
                ToolContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        if is_error {
            return Err(AgentError::ToolExecutionFailed(
                tool_name.to_string(),
                text,
            ));
        }

        Ok(Value::String(text))
    }

    /// Disconnect from the MCP server
    pub async fn disconnect(&self) -> Result<(), AgentError> {
        let mut info = self.server_info.write().await;
        *info = None;
        info!("Disconnected from MCP server");
        Ok(())
    }
}

// =============================================================================
// MCP Tool Adapter
// =============================================================================

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
        self.tool.schema()
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        self.client
            .call_tool(&self.tool.name, args)
            .await
            .map_err(|e| ToolError::execution_failed(format!("MCP error: {}", e)))
    }
}

// =============================================================================
// MCP Manager
// =============================================================================

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

    /// Add an existing MCP client
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this connection
    /// * `client` - The MCP client
    pub async fn add_client(
        &self,
        name: impl Into<String>,
        client: McpClient,
    ) -> Result<(), AgentError> {
        let name = name.into();
        let mut clients = self.clients.write().await;

        if clients.contains_key(&name) {
            return Err(AgentError::Registry(format!(
                "MCP client '{}' already exists",
                name
            )));
        }

        clients.insert(name, client);
        Ok(())
    }

    /// Connect to an MCP server via stdio and add it to the manager
    pub async fn connect_stdio(
        &self,
        name: impl Into<String>,
        command: impl AsRef<str>,
        args: &[impl AsRef<str>],
    ) -> Result<(), AgentError> {
        let name = name.into();
        let client = McpClient::connect_stdio(command, args).await?;
        self.add_client(name, client).await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_tool_schema_fallback() {
        let tool = McpTool {
            name: "search".to_string(),
            description: "Search for documents".to_string(),
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            })),
            parameters: None,
        };

        assert_eq!(tool.name, "search");
        let schema = tool.schema();
        assert!(schema.get("type").is_some());
    }

    #[test]
    fn test_mcp_tool_parameters_fallback() {
        let tool = McpTool {
            name: "calc".to_string(),
            description: "Calculator".to_string(),
            input_schema: None,
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "expr": {"type": "string"}
                }
            })),
        };

        let schema = tool.schema();
        assert!(schema.get("properties").is_some());
    }

    #[tokio::test]
    async fn test_mcp_manager() {
        let manager = McpManager::new();

        // Initially empty
        let clients = manager.list_clients().await;
        assert!(clients.is_empty());
    }
}
