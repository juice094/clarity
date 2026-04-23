//! E2E test for MCP HTTP transport
//!
//! Spawns a minimal Axum-based HTTP MCP server, connects via `McpManager::from_config()`,
//! then exercises `list_tools()` and `call_tool()` end-to-end.

use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use clarity_core::mcp::config::{McpConfig, McpServerEntry};
use clarity_core::mcp::{McpClient, McpManager, ToolContent};
use clarity_core::tools::{Tool, ToolContext};

// =============================================================================
// Minimal MCP HTTP server
// =============================================================================

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcReq {
    jsonrpc: String,
    id: Option<u64>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResp {
    jsonrpc: &'static str,
    id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

#[derive(Clone)]
struct ServerState {
    // Could hold shared counters, etc.
}

async fn mcp_handler(State(_state): State<ServerState>, Json(req): Json<JsonRpcReq>) -> Json<JsonRpcResp> {
    let result = match req.method.as_str() {
        "initialize" => Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": { "name": "test-http-server", "version": "1.0" },
            "capabilities": {}
        })),
        "tools/list" => Some(serde_json::json!({
            "tools": [
                {
                    "name": "echo",
                    "description": "Echo back the input",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "message": { "type": "string" }
                        },
                        "required": ["message"]
                    }
                }
            ]
        })),
        "tools/call" => {
            let params = req.params.unwrap_or_default();
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_default();
            match name {
                "echo" => {
                    let msg = args.get("message").and_then(|v| v.as_str()).unwrap_or("");
                    Some(serde_json::json!({
                        "content": [
                            { "type": "text", "text": format!("echo: {}", msg) }
                        ],
                        "isError": false
                    }))
                }
                other => {
                    return Json(JsonRpcResp {
                        jsonrpc: "2.0",
                        id: req.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("Tool '{}' not found", other),
                        }),
                    });
                }
            }
        }
        _ => {
            return Json(JsonRpcResp {
                jsonrpc: "2.0",
                id: req.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method '{}' not found", req.method),
                }),
            });
        }
    };

    Json(JsonRpcResp {
        jsonrpc: "2.0",
        id: req.id,
        result,
        error: None,
    })
}

async fn spawn_test_server() -> (String, oneshot::Sender<()>) {
    let app = Router::new()
        .route("/mcp", post(mcp_handler))
        .with_state(ServerState {});

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{}/mcp", port);

    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let server = axum::serve(listener, app);
        let _ = server.with_graceful_shutdown(async {
            let _ = rx.await;
        }).await;
    });

    // Give the server a moment to start accepting connections
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    (url, tx)
}

// =============================================================================
// E2E test
// =============================================================================

#[tokio::test]
async fn test_http_mcp_transport_e2e() {
    let (url, _shutdown) = spawn_test_server().await;

    let mut config = McpConfig::default();
    config.servers.insert(
        "http-test".to_string(),
        McpServerEntry {
            transport: Some("http".to_string()),
            url: Some(url),
            disabled: false,
            ..Default::default()
        },
    );

    let manager = McpManager::from_config(&config).await;

    // Verify tools were discovered
    let tools = manager.tools();
    assert_eq!(tools.len(), 1, "Expected exactly one tool from HTTP MCP server");
    assert_eq!(tools[0].name(), "echo");

    // Exercise tool via McpToolAdapter (executes call_tool internally)
    let result = tools[0]
        .execute(serde_json::json!({ "message": "hello" }), ToolContext::new())
        .await;
    assert!(result.is_ok(), "Tool execution failed: {:?}", result.err());
    assert_eq!(result.unwrap(), "echo: hello");

    // Exercise direct client call_tool
    let client = manager
        .get_client("http-test")
        .expect("HTTP test client should be registered");
    let guard = client.lock().await;
    let direct = guard
        .call_tool("echo", serde_json::json!({ "message": "world" }))
        .await;
    assert!(direct.is_ok(), "Direct call_tool failed: {:?}", direct.err());
    let direct_result = direct.unwrap();
    assert_eq!(direct_result.content.len(), 1);
    match &direct_result.content[0] {
        ToolContent::Text { text } => {
            assert_eq!(text, "echo: world");
        }
        other => panic!("Expected text content, got {:?}", other),
    }
}
