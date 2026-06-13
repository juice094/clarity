#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
mod common;

use std::net::SocketAddr;

use axum::{Json, Router, routing::post};
use clarity_core::mcp::{McpRegistry, register_mcp_tools};
use clarity_core::registry::ToolRegistry;
use clarity_mcp::McpClient;
use serde_json::Value;

/// Start a mock MCP HTTP server on a random local port.
/// Returns the server address and a shutdown signal.
async fn start_mock_mcp_server() -> SocketAddr {
    let app = Router::new().route(
        "/",
        post(|Json(body): Json<Value>| async move {
            let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
            match method {
                "tools/list" => Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": body.get("id"),
                    "result": {
                        "tools": [{
                            "name": "greet",
                            "description": "Returns a greeting",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "name": {"type": "string"}
                                },
                                "required": ["name"]
                            }
                        }]
                    }
                })),
                "tools/call" => {
                    let args = body
                        .get("params")
                        .and_then(|p| p.get("arguments"))
                        .cloned()
                        .unwrap_or_default();
                    let name = args.get("name").and_then(|n| n.as_str()).unwrap_or("world");
                    Json(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id"),
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": format!("Hello, {}!", name)
                            }],
                            "isError": false
                        }
                    }))
                }
                _ => Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": body.get("id"),
                    "error": {"code": -32601, "message": "Method not found"}
                })),
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server a moment to start.
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    addr
}

/// Scenario: MCP tool registration and execution end-to-end.
/// 1. Start mock MCP HTTP server.
/// 2. Create HttpMcpClient, connect, register in McpRegistry.
/// 3. Register MCP tools into Clarity ToolRegistry.
/// 4. Verify tool is accessible via Agent execution.
#[tokio::test]
async fn test_mcp_tool_registration_and_execution() {
    let addr = start_mock_mcp_server().await;
    let url = format!("http://{}/", addr);

    // Build HTTP MCP client.
    let config = clarity_mcp::McpServerConfig::http("mock", &url);
    let mut client = clarity_mcp::HttpMcpClient::new(config);
    client
        .connect()
        .await
        .expect("Failed to connect to mock MCP server");

    // Register client in McpRegistry.
    let mut mcp_registry = McpRegistry::new();
    mcp_registry.register("mock", clarity_mcp::McpClientInstance::Http(client));

    // Register MCP tools into Clarity ToolRegistry.
    let tool_registry = ToolRegistry::new();
    register_mcp_tools(&mcp_registry, &tool_registry)
        .await
        .expect("Failed to register MCP tools");

    // Verify tool was registered with prefixed name.
    let registered = tool_registry.list_tools().unwrap();
    assert!(
        registered.contains(&"mock_greet".to_string()),
        "Expected 'mock_greet' in registry, got: {:?}",
        registered
    );

    // Verify tool schema is present.
    let schemas = tool_registry.get_tool_schemas().unwrap();
    let tools = schemas.as_array().unwrap();
    let greet_schema = tools.iter().find(|s| {
        s.get("function")
            .and_then(|f| f.get("name"))
            .and_then(|n| n.as_str())
            == Some("mock_greet")
    });
    assert!(greet_schema.is_some(), "Expected schema for 'mock_greet'");
}

/// Scenario: Execute an MCP-backed tool directly via ToolRegistry.
/// Verifies that the MCP tool wrapper correctly forwards the call to the
/// mock HTTP server and returns the result.
#[tokio::test]
async fn test_mcp_tool_direct_execution() {
    let addr = start_mock_mcp_server().await;
    let url = format!("http://{}/", addr);

    let config = clarity_mcp::McpServerConfig::http("mock", &url);
    let mut client = clarity_mcp::HttpMcpClient::new(config);
    client.connect().await.unwrap();

    let mut mcp_registry = McpRegistry::new();
    mcp_registry.register("mock", clarity_mcp::McpClientInstance::Http(client));

    let tool_registry = ToolRegistry::new();
    register_mcp_tools(&mcp_registry, &tool_registry)
        .await
        .unwrap();

    // Execute the tool directly through the registry.
    let result = tool_registry
        .execute(
            "mock_greet",
            serde_json::json!({"name": "IntegrationTest"}),
            clarity_core::tools::ToolContext::default(),
        )
        .await;

    assert!(result.is_ok(), "Tool execution failed: {:?}", result);
    let output = result.unwrap();
    assert_eq!(
        output,
        serde_json::Value::String("Hello, IntegrationTest!".to_string()),
        "Expected MCP tool result, got: {:?}",
        output
    );
}
