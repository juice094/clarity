//! MCP (Model Context Protocol) Client Interface
//!
//! This crate provides JSON-RPC 2.0 clients for MCP servers,
//! allowing the agent to connect to external tool servers.
//!
//! ## Features
//!
//! - **Stdio Transport**: Connect to local MCP servers via stdin/stdout
//! - **HTTP Transport**: Connect to remote MCP servers via HTTP POST
//! - **SSE Transport**: Connect to streaming MCP servers via Server-Sent Events
//! - **OAuth Support**: Authentication support for remote servers
//!
//! ## Example
//!
//! ```rust,no_run
//! use clarity_mcp::{McpClient, McpClientBuilder, McpRegistry};
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

// Re-export from enhanced module
pub use enhanced::{
    HttpClientBuilder, HttpMcpClient, McpClient, McpClientBuilder, McpClientInstance, McpError,
    McpRegistry, McpResource, McpServerConfig, McpTool, McpTransport, OAuthConfig,
    SseClientBuilder, SseMcpClient, StdioClientBuilder, StdioMcpClient, ToolCallResult,
    ToolContent,
};

// Re-export from config module
pub use config::{default_config_path, McpConfig, McpServerEntry};

// Re-export from devkit module
pub use devkit::{DevkitAsset, DevkitProjectContextResult, DevkitRepo, DevkitVaultNote};

// =============================================================================
// Legacy Types (for backward compatibility)
// =============================================================================

use clarity_contract::{AgentError, ToolError, ToolResult};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, RwLock};
use tracing::{debug, info};

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
pub fn map_mcp_error(err: McpError) -> ToolError {
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

/// Pre-compiled regex patterns for credential scrubbing.
static CREDENTIAL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    let patterns = [
        r#"(?i)api[_-]?key\s*[:=]\s*["']?[a-zA-Z0-9_\-]{16,}["']?"#,
        r#"(?i)token\s*[:=]\s*["']?[a-zA-Z0-9_\-]{16,}["']?"#,
        r#"(?i)password\s*[:=]\s*["']?[^"'\s]{8,}["']?"#,
        r"sk-[a-zA-Z0-9]{20,}",
        r"AIza[a-zA-Z0-9_\-]{30,}",
    ];
    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
});

/// Scrub sensitive credentials from text before sending to LLM context.
fn scrub_credentials(text: &str) -> String {
    let mut result = text.to_string();
    for re in CREDENTIAL_PATTERNS.iter() {
        result = re.replace_all(&result, "[REDACTED]").to_string();
    }
    result
}

/// Process an MCP tool call result, extracting text content and detecting
/// application-level errors in JSON payloads.
///
/// This logic was extracted from [`McpToolAdapter::execute`] so it can be
/// unit-tested without spinning up a real MCP server.
pub fn process_mcp_tool_result(result: ToolCallResult) -> ToolResult<Value> {
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
        return Err(ToolError::execution_failed(scrub_credentials(&joined)));
    }

    // Detect application-level errors in JSON payloads
    // (e.g. devbase returning {"success":false,"error":"..."})
    if let Ok(parsed) = serde_json::from_str::<Value>(&joined) {
        let has_error_field = parsed.get("error").is_some();
        let success_false = parsed.get("success").and_then(|v| v.as_bool()) == Some(false);
        if has_error_field || success_false {
            return Err(ToolError::execution_failed(scrub_credentials(&joined)));
        }
    }

    Ok(Value::String(scrub_credentials(&joined)))
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

    #[test]
    fn test_scrub_credentials_sk_pattern() {
        let input = "Error: token sk-abc12345678901234567890 is invalid";
        let expected = "Error: token [REDACTED] is invalid";
        assert_eq!(scrub_credentials(input), expected);
    }

    #[test]
    fn test_scrub_credentials_api_key_pattern() {
        let input = "config: api_key=supersecret1234567890abcdef";
        let expected = "config: [REDACTED]";
        assert_eq!(scrub_credentials(input), expected);
    }

    #[test]
    fn test_scrub_credentials_no_match() {
        let input = "Hello world, nothing sensitive here.";
        assert_eq!(scrub_credentials(input), input);
    }
}
