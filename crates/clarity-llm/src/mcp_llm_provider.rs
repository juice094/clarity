//! MCP-native LLM provider.
//!
//! Implements `LlmProvider` by calling an MCP server's `chat_completion` tool.
//! This allows any MCP-compliant LLM server (including the mesh server example)
//! to be used as a backend for the Clarity Agent.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;

use crate::{LlmProvider, LlmResponse, ProviderCapabilities, StreamDelta};
use clarity_contract::{AgentError, Message};
use clarity_mcp::McpClient;

/// LLM provider backed by an MCP server.
pub struct McpLlmProvider {
    client: Arc<tokio::sync::Mutex<clarity_mcp::McpClientInstance>>,
    tool_name: String,
}

impl McpLlmProvider {
    /// Connect to an MCP LLM server via stdio transport.
    ///
    /// `command` is the executable path (e.g. `cargo` or a compiled binary).
    /// `args` are passed to the command (e.g. `["run", "--example", "mcp_llm_stdio_server"]`).
    pub async fn connect_stdio(command: &str, args: &[String]) -> Result<Self, AgentError> {
        let mut builder = clarity_mcp::McpClientBuilder::stdio("llm-mcp", command);
        for arg in args {
            builder = builder.arg(arg);
        }
        let mut client = builder.build();
        client
            .connect()
            .await
            .map_err(|e| AgentError::Llm(format!("MCP connect failed: {}", e)))?;
        Ok(Self {
            client: Arc::new(tokio::sync::Mutex::new(client)),
            tool_name: "chat_completion".into(),
        })
    }

    /// Set the MCP tool name to call (default: "chat_completion").
    pub fn with_tool_name(mut self, name: impl Into<String>) -> Self {
        self.tool_name = name.into();
        self
    }
}

#[async_trait]
impl LlmProvider for McpLlmProvider {
    async fn complete(
        &self,
        messages: &[Message],
        _tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let client = self.client.lock().await;
        let args = serde_json::json!({ "messages": messages });
        let result = client
            .call_tool(&self.tool_name, args)
            .await
            .map_err(|e| AgentError::Llm(format!("MCP tool error: {}", e)))?;

        let text = result
            .content
            .iter()
            .filter_map(|c| match c {
                clarity_mcp::ToolContent::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(LlmResponse {
            content: text,
            tool_calls: vec![],
            is_complete: true,
        })
    }

    fn stream(
        &self,
        _messages: &[Message],
        _tools: &Value,
    ) -> Result<Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        Err(AgentError::Llm(
            "McpLlmProvider does not support streaming yet".into(),
        ))
    }

    fn set_prompt_cache_key(&self, _key: &str) {
        // MCP server may or may not support prompt caching;
        // nothing to do on the client side.
    }

    fn capabilities(&self) -> ProviderCapabilities {
        // Conservative defaults — actual capabilities depend on the remote server.
        ProviderCapabilities {
            native_tool_calling: true,
            vision: false,
            prompt_caching: false,
            pricing: None,
        }
    }
}
