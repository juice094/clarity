//! Minimal MCP (Model Context Protocol) Server infrastructure.
//!
//! Provides a `McpServer` trait and a `StdioMcpServer` runner that
//! communicates over stdin/stdout using JSON-RPC 2.0.
//!
//! This is intentionally lightweight — just enough to expose internal
//! capabilities (e.g. `MeshLlmProvider`) as MCP tools.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, info, warn};

use crate::{McpError, McpTool, ToolCallResult, ToolContent};

// ============================================================================
// McpServer trait
// ============================================================================

#[async_trait]
pub trait McpServer: Send + Sync {
    /// Server name (e.g. "clarity-llm-mesh").
    fn name(&self) -> &str;

    /// Server version (e.g. "0.3.0").
    fn version(&self) -> &str;

    /// List tools exposed by this server.
    async fn list_tools(&self) -> Vec<McpTool>;

    /// Call a tool by name with JSON arguments.
    async fn call_tool(&self, name: &str, args: Value) -> Result<ToolCallResult, McpError>;
}

// ============================================================================
// JSON-RPC types
// ============================================================================

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse<T> {
    jsonrpc: &'static str,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcError {
    fn parse_error(msg: impl Into<String>) -> Self {
        Self {
            code: -32700,
            message: msg.into(),
            data: None,
        }
    }
    fn method_not_found(msg: impl Into<String>) -> Self {
        Self {
            code: -32601,
            message: msg.into(),
            data: None,
        }
    }
    fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: msg.into(),
            data: None,
        }
    }
    #[allow(dead_code)]
    fn internal_error(msg: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: msg.into(),
            data: None,
        }
    }
}

// ============================================================================
// StdioMcpServer runner
// ============================================================================

/// Run an `McpServer` over stdin/stdout.
pub struct StdioMcpServer;

impl StdioMcpServer {
    /// Start the JSON-RPC loop. Blocks until stdin closes.
    pub async fn run<S: McpServer>(server: S) {
        info!("MCP Server '{}' starting on stdio", server.name());
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            debug!("MCP request: {}", line);

            let req: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    let resp = JsonRpcResponse::<Value> {
                        jsonrpc: "2.0",
                        id: None,
                        result: None,
                        error: Some(JsonRpcError::parse_error(e.to_string())),
                    };
                    Self::write_line(&resp);
                    continue;
                }
            };

            let resp = Self::handle_request(&server, req).await;
            Self::write_line(&resp);
        }

        info!("MCP Server '{}' stdin closed, shutting down", server.name());
    }

    async fn handle_request<S: McpServer>(
        server: &S,
        req: JsonRpcRequest,
    ) -> JsonRpcResponse<Value> {
        match req.method.as_str() {
            "initialize" => {
                let result = serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": server.name(),
                        "version": server.version(),
                    },
                    "capabilities": {
                        "tools": {}
                    }
                });
                JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: req.id,
                    result: Some(result),
                    error: None,
                }
            }
            "tools/list" => {
                let tools = server.list_tools().await;
                let result = serde_json::json!({
                    "tools": tools.iter().map(|t| serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema,
                    })).collect::<Vec<_>>()
                });
                JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: req.id,
                    result: Some(result),
                    error: None,
                }
            }
            "tools/call" => {
                let params = match req.params {
                    Some(p) => p,
                    None => {
                        return JsonRpcResponse {
                            jsonrpc: "2.0",
                            id: req.id,
                            result: None,
                            error: Some(JsonRpcError::invalid_params("missing params")),
                        };
                    }
                };
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(Value::Null);

                if name.is_empty() {
                    return JsonRpcResponse {
                        jsonrpc: "2.0",
                        id: req.id,
                        result: None,
                        error: Some(JsonRpcError::invalid_params("missing tool name")),
                    };
                }

                match server.call_tool(name, args).await {
                    Ok(result) => {
                        let texts: Vec<String> = result
                            .content
                            .iter()
                            .filter_map(|c| match c {
                                ToolContent::Text { text } => Some(text.clone()),
                                _ => None,
                            })
                            .collect();
                        let result = serde_json::json!({
                            "content": texts.into_iter().map(|t| {
                                serde_json::json!({ "type": "text", "text": t })
                            }).collect::<Vec<_>>(),
                            "isError": false,
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0",
                            id: req.id,
                            result: Some(result),
                            error: None,
                        }
                    }
                    Err(e) => {
                        warn!("MCP tool call failed: {}", e);
                        let result = serde_json::json!({
                            "content": [serde_json::json!({
                                "type": "text",
                                "text": format!("Error: {}", e),
                            })],
                            "isError": true,
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0",
                            id: req.id,
                            result: Some(result),
                            error: None,
                        }
                    }
                }
            }
            _ => JsonRpcResponse {
                jsonrpc: "2.0",
                id: req.id,
                result: None,
                error: Some(JsonRpcError::method_not_found(format!(
                    "method '{}' not supported",
                    req.method
                ))),
            },
        }
    }

    fn write_line<T: Serialize>(resp: &JsonRpcResponse<T>) {
        if let Ok(json) = serde_json::to_string(resp) {
            let mut stdout = std::io::stdout();
            let _ = writeln!(stdout, "{}", json);
            let _ = stdout.flush();
            debug!("MCP response: {}", json);
        }
    }
}
