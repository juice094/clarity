//! MCP Tool integration: bridge MCP servers into the Clarity ToolRegistry.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::ToolError;
use crate::registry::ToolRegistry;
use crate::tools::{Tool, ToolContext, ToolResult};
use clarity_mcp::{McpClient, McpClientInstance, McpError, McpRegistry, McpTool};

/// A Clarity `Tool` backed by an MCP server.
pub struct McpToolWrapper {
    client: Arc<RwLock<McpClientInstance>>,
    /// Display name in ToolRegistry (prefixed, e.g. "filesystem_list_directory")
    name: String,
    /// Original MCP tool name (unprefixed, e.g. "list_directory")
    mcp_name: String,
    description: String,
    parameters: Value,
}

impl McpToolWrapper {
    /// Create a new wrapper from an MCP tool descriptor and its client.
    /// `tool.name` should be the prefixed display name.
    /// `mcp_name` is the original tool name expected by the MCP server.
    pub fn new(
        client: Arc<RwLock<McpClientInstance>>,
        tool: McpTool,
        mcp_name: impl Into<String>,
    ) -> Self {
        Self {
            client,
            name: tool.name,
            mcp_name: mcp_name.into(),
            description: tool.description.unwrap_or_default(),
            parameters: tool.input_schema,
        }
    }
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> Value {
        self.parameters.clone()
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let client = self.client.read().await;
        let result = client.call_tool(&self.mcp_name, args).await.map_err(|e| {
            let msg = format!("MCP tool error: {}", e);
            // Provide a helpful hint for Git ownership errors from MCP servers.
            if msg.contains("not owned by current user") || msg.contains("safe.directory") {
                ToolError::ExecutionFailed(format!(
                    "{}\n\nHint: This is a Git ownership issue. \
                         Run: git config --global --add safe.directory <repo-path>",
                    msg
                ))
            } else {
                ToolError::ExecutionFailed(msg)
            }
        })?;

        // Flatten text content into a single JSON string value.
        let mut texts = Vec::new();
        for content in result.content {
            match content {
                clarity_mcp::ToolContent::Text { text } => texts.push(text),
                clarity_mcp::ToolContent::Resource { resource } => {
                    if let Some(text) = resource.text {
                        texts.push(text);
                    }
                }
                _ => {}
            }
        }
        Ok(Value::String(texts.join("\n")))
    }
}

/// Register all tools exposed by the MCP registry into a Clarity `ToolRegistry`.
///
/// Each tool is prefixed with its server name to avoid collisions
/// (e.g. `filesystem_read_file`).
pub async fn register_mcp_tools(
    mcp_registry: &McpRegistry,
    tool_registry: &ToolRegistry,
) -> Result<(), McpError> {
    for (server_name, client) in mcp_registry.iter() {
        let tools = client.read().await.list_tools().await?;
        for tool in tools {
            let mcp_name = tool.name.clone();
            let name = format!("{}_{}", server_name, tool.name);
            let wrapper = McpToolWrapper::new(
                client.clone(),
                McpTool {
                    name: name.clone(),
                    description: tool.description,
                    input_schema: tool.input_schema,
                },
                mcp_name,
            );
            tool_registry
                .register(wrapper)
                .map_err(|e| McpError::RpcError(e.to_string()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    #[ignore = "requires external MCP filesystem server via npx"]
    async fn test_mcp_filesystem_tool_e2e() {
        // Skip if npx is unavailable (CI environments without Node.js).
        if std::process::Command::new("npx")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("Skipping MCP filesystem test: npx not available");
            return;
        }

        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("hello.txt");
        {
            let mut f = std::fs::File::create(&file_path).unwrap();
            f.write_all(b"Hello from MCP filesystem server!").unwrap();
        }

        let server_path = tmp.path().to_str().unwrap();
        let mut client = clarity_mcp::McpClientBuilder::stdio("filesystem", "npx")
            .arg("-y")
            .arg("@modelcontextprotocol/server-filesystem")
            .arg(server_path)
            .build();
        client
            .connect()
            .await
            .expect("Failed to connect to MCP filesystem server");

        // Verify tool listing works
        let tools = client.list_tools().await.expect("Failed to list tools");
        let read_file_tool = tools.iter().find(|t| t.name == "read_file");
        assert!(
            read_file_tool.is_some(),
            "Expected filesystem server to expose 'read_file' tool, got: {:?}",
            tools.iter().map(|t| &t.name).collect::<Vec<_>>()
        );

        // Call read_file directly via MCP client
        let result = client
            .call_tool(
                "read_file",
                serde_json::json!({ "path": file_path.to_str().unwrap() }),
            )
            .await
            .expect("Failed to call read_file tool");
        assert!(!result.is_error);

        let content: String = result
            .content
            .into_iter()
            .filter_map(|c| match c {
                clarity_mcp::ToolContent::Text { text } => Some(text),
                _ => None,
            })
            .collect();
        assert!(content.contains("Hello from MCP filesystem server!"));

        // Now verify ToolRegistry integration
        let mut mcp_registry = McpRegistry::new();
        mcp_registry.register("fs", client);

        let tool_registry = ToolRegistry::new();
        register_mcp_tools(&mcp_registry, &tool_registry)
            .await
            .expect("Failed to register MCP tools");

        let registered_names = tool_registry.list_tools().unwrap();
        assert!(
            registered_names.contains(&"fs_read_file".to_string()),
            "Expected 'fs_read_file' in registry, got: {:?}",
            registered_names
        );

        let wrapper = tool_registry.get("fs_read_file").unwrap().unwrap();
        let ctx = ToolContext::new();
        let output = wrapper
            .execute(
                serde_json::json!({ "path": file_path.to_str().unwrap() }),
                ctx,
            )
            .await
            .expect("Wrapper execution failed");
        assert!(
            output
                .as_str()
                .unwrap()
                .contains("Hello from MCP filesystem server!")
        );
    }

    use axum::{Router, extract::Json, routing::post};
    use clarity_mcp::{HttpMcpClient, McpServerConfig};
    use std::net::SocketAddr;
    use tokio::net::TcpListener;

    async fn mock_mcp_handler(Json(req): Json<serde_json::Value>) -> Json<serde_json::Value> {
        let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);

        if method == "tools/call" {
            let params = req
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name == "error_tool" {
                return Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32600, "message": "not owned by current user" }
                }));
            }
            return Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": format!("Hello from {}", name) }],
                    "isError": false
                }
            }));
        }

        Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [
                    {
                        "name": "tool1",
                        "description": "First mock tool",
                        "inputSchema": { "type": "object" }
                    },
                    {
                        "name": "tool2",
                        "description": "Second mock tool",
                        "inputSchema": { "type": "object" }
                    }
                ]
            }
        }))
    }

    async fn start_mock_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let app = Router::new().route("/", post(mock_mcp_handler));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = axum::serve(listener, app);
        let handle = tokio::spawn(async move { server.await.unwrap() });
        (addr, handle)
    }

    #[tokio::test]
    async fn test_mcp_tool_wrapper_metadata() {
        let client = Arc::new(RwLock::new(McpClientInstance::Http(HttpMcpClient::new(
            McpServerConfig::http("dummy", "http://127.0.0.1:1"),
        ))));
        let tool = McpTool {
            name: "srv_hello".to_string(),
            description: Some("Hello tool".to_string()),
            input_schema: serde_json::json!({"type": "object"}),
        };
        let wrapper = McpToolWrapper::new(client, tool, "hello");

        assert_eq!(wrapper.name(), "srv_hello");
        assert_eq!(wrapper.description(), "Hello tool");
        assert_eq!(wrapper.parameters(), serde_json::json!({"type": "object"}));
    }

    #[tokio::test]
    async fn test_mcp_tool_wrapper_execute_success() {
        let (addr, _server) = start_mock_server().await;
        let client = Arc::new(RwLock::new(McpClientInstance::Http(HttpMcpClient::new(
            McpServerConfig::http("mock", format!("http://{}", addr)),
        ))));
        let tool = McpTool {
            name: "mock_hello".to_string(),
            description: None,
            input_schema: serde_json::json!({}),
        };
        let wrapper = McpToolWrapper::new(client, tool, "hello");

        let result = wrapper
            .execute(serde_json::json!({}), ToolContext::new())
            .await
            .unwrap();
        assert_eq!(result, serde_json::json!("Hello from hello"));
    }

    #[tokio::test]
    async fn test_mcp_tool_wrapper_execute_git_ownership_hint() {
        let (addr, _server) = start_mock_server().await;
        let client = Arc::new(RwLock::new(McpClientInstance::Http(HttpMcpClient::new(
            McpServerConfig::http("mock", format!("http://{}", addr)),
        ))));
        let tool = McpTool {
            name: "mock_error".to_string(),
            description: None,
            input_schema: serde_json::json!({}),
        };
        let wrapper = McpToolWrapper::new(client, tool, "error_tool");

        let err = wrapper
            .execute(serde_json::json!({}), ToolContext::new())
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not owned by current user"));
        assert!(msg.contains("safe.directory"));
    }

    #[tokio::test]
    async fn test_register_mcp_tools() {
        let (addr, _server) = start_mock_server().await;
        let client = McpClientInstance::Http(HttpMcpClient::new(McpServerConfig::http(
            "mock",
            format!("http://{}", addr),
        )));

        let mut mcp_registry = McpRegistry::new();
        mcp_registry.register("mock", client);

        let tool_registry = ToolRegistry::new();
        register_mcp_tools(&mcp_registry, &tool_registry)
            .await
            .unwrap();

        let names = tool_registry.list_tools().unwrap();
        assert!(names.contains(&"mock_tool1".to_string()));
        assert!(names.contains(&"mock_tool2".to_string()));
    }

    #[tokio::test]
    async fn test_register_mcp_tools_empty_registry() {
        let mcp_registry = McpRegistry::new();
        let tool_registry = ToolRegistry::new();
        register_mcp_tools(&mcp_registry, &tool_registry)
            .await
            .unwrap();
        assert!(tool_registry.list_tools().unwrap().is_empty());
    }
}
