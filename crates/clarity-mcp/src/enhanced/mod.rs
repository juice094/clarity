//! Enhanced MCP (Model Context Protocol) Client
//!
//! Supports multiple transports: stdio, HTTP, SSE, WebSocket.
//! Reference: Kimi CLI's fastmcp implementation.
//!
//! This module was split from a single large `enhanced.rs` file into
//! transport-specific submodules for maintainability.

pub(crate) mod builder;
pub(crate) mod client;
pub(crate) mod error;
pub(crate) mod http;
pub(crate) mod instance;
pub(crate) mod result_types;
pub(crate) mod rpc;
pub(crate) mod sse;
pub(crate) mod stdio;
pub(crate) mod types;
pub(crate) mod validate;
pub(crate) mod websocket;

#[cfg(test)]
pub(crate) mod tests;

// Re-export the most commonly used types at the `enhanced` level so that
// existing `use clarity_mcp::enhanced::...` imports continue to work.
pub use builder::{
    HttpClientBuilder, McpClientBuilder, SseClientBuilder, StdioClientBuilder,
    WebSocketClientBuilder,
};
pub use client::McpClient;
pub use error::McpError;
pub use http::HttpMcpClient;
pub use instance::{McpClientInstance, McpRegistry};
pub use result_types::{
    BlobResourceContents, GetPromptResult, ListPromptsResult, ListResourcesResult, McpPrompt,
    McpResource, McpResourceMeta, McpTool, PromptArgument, PromptContent, PromptMessage,
    PromptMessageRole, ReadResourceResult, ResourceContents, TextResourceContents, ToolCallResult,
    ToolContent,
};
pub(crate) use rpc::{JsonRpcRequest, JsonRpcResponse};
pub use sse::SseMcpClient;
pub use stdio::StdioMcpClient;
pub use types::{McpServerConfig, McpTransport, OAuthConfig};
pub use validate::{validate_mcp_command, validate_mcp_command_with_allowlist};
pub use websocket::WebSocketMcpClient;
