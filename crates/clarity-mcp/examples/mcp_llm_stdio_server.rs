//! MCP LLM Server Example
//!
//! Exposes `MeshLlmProvider` as an MCP server over stdio.
//!
//! ## Run
//! ```bash
//! CLARITY_MESH_PROVIDERS=openai,kimi cargo run --example mcp_llm_stdio_server
//! ```

use clarity_contract::{LlmProvider, Message};
use clarity_llm::mesh::MeshLlmProvider;
use clarity_mcp::{McpServer, McpTool, StdioMcpServer, ToolCallResult, ToolContent};
use serde_json::Value;

struct LlmMcpServer {
    mesh: MeshLlmProvider,
}

#[async_trait::async_trait]
impl McpServer for LlmMcpServer {
    fn name(&self) -> &str {
        "clarity-llm-mesh"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    async fn list_tools(&self) -> Vec<McpTool> {
        vec![McpTool {
            name: "chat_completion".into(),
            description: Some("Chat with the LLM mesh".into()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "messages": {
                        "type": "array",
                        "description": "Array of {role, content} messages"
                    },
                    "model": {
                        "type": "string",
                        "description": "Optional model override (ignored by mesh)"
                    }
                },
                "required": ["messages"]
            }),
        }]
    }

    async fn call_tool(
        &self,
        name: &str,
        args: Value,
    ) -> Result<ToolCallResult, clarity_mcp::McpError> {
        if name != "chat_completion" {
            return Err(clarity_mcp::McpError::RequestFailed(format!(
                "Unknown tool: {}",
                name
            )));
        }

        let messages: Vec<Message> = serde_json::from_value(
            args.get("messages")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
        )
        .map_err(|e| clarity_mcp::McpError::Serialization(e))?;

        match self.mesh.complete(&messages, &Value::Null).await {
            Ok(resp) => Ok(ToolCallResult {
                content: vec![ToolContent::Text { text: resp.content }],
                is_error: false,
            }),
            Err(e) => Ok(ToolCallResult {
                content: vec![ToolContent::Text {
                    text: format!("LLM error: {}", e),
                }],
                is_error: true,
            }),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mesh = MeshLlmProvider::from_env()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load mesh: {}", e))?;

    if mesh.provider_names().is_empty() {
        eprintln!("No mesh providers loaded. Set CLARITY_MESH_PROVIDERS.");
        return Ok(());
    }

    tracing::info!(
        "MCP LLM Server starting with providers: {:?}",
        mesh.provider_names()
    );

    let server = LlmMcpServer { mesh };
    StdioMcpServer::run(server).await;
    Ok(())
}
