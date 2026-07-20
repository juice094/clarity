use serde::{Deserialize, Serialize};
use serde_json::Value;
// =============================================================================
// JSON-RPC 2.0 Types
// =============================================================================

#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcRequest<T: serde::Serialize> {
    pub(crate) jsonrpc: &'static str,
    pub(crate) id: u64,
    pub(crate) method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) params: Option<T>,
}

#[derive(Debug, Deserialize)]
// Intentionally retained because `jsonrpc` is part of the JSON-RPC 2.0 wire
// format and is validated by serde even though it is not read directly.
#[allow(dead_code)]
pub(crate) struct JsonRpcResponse<T> {
    jsonrpc: String,
    pub(crate) id: u64,
    pub(crate) result: Option<T>,
    pub(crate) error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize, Clone)]
// Intentionally retained because `code` and `data` are deserialized for
// completeness and may be inspected by future error-handling logic.
#[allow(dead_code)]
pub(crate) struct JsonRpcError {
    pub(crate) code: i32,
    pub(crate) message: String,
    pub(crate) data: Option<Value>,
}

// =============================================================================
// MCP Client Trait
// =============================================================================
