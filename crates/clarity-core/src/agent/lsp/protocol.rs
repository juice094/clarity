//! Minimal LSP JSON-RPC protocol types.
//!
//! No `lsp-types` crate — just the structs needed for:
//! - initialize / initialized
//! - textDocument/didOpen / didChange
//! - textDocument/publishDiagnostics (notification)

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ------------------------------------------------------------------
// JSON-RPC 2.0 base
// ------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcRequest<T: Serialize> {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<T>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcResponse {
    pub id: u64,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

/// A JSON-RPC notification has no `id` field.
#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcNotification {
    pub method: String,
    pub params: Option<Value>,
}

// ------------------------------------------------------------------
// LSP-specific types
// ------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `InitializeParams` configuration/state.
pub struct InitializeParams {
    pub process_id: Option<u32>,
    pub root_uri: Option<String>,
    pub capabilities: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `InitializeResult` configuration/state.
pub struct InitializeResult {
    // Intentionally retained: deserialized from LSP initialize responses; kept for
    // forward compatibility even when not actively inspected by current code.
    #[allow(dead_code)]
    pub capabilities: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `TextDocumentItem` configuration/state.
pub struct TextDocumentItem {
    pub uri: String,
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `DidOpenTextDocumentParams` configuration/state.
pub struct DidOpenTextDocumentParams {
    pub text_document: TextDocumentItem,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `VersionedTextDocumentIdentifier` configuration/state.
pub struct VersionedTextDocumentIdentifier {
    pub uri: String,
    pub version: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `TextDocumentContentChangeEvent` configuration/state.
pub struct TextDocumentContentChangeEvent {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// `DidChangeTextDocumentParams` configuration/state.
pub struct DidChangeTextDocumentParams {
    pub text_document: VersionedTextDocumentIdentifier,
    pub content_changes: Vec<TextDocumentContentChangeEvent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// `Diagnostic` configuration/state.
pub struct Diagnostic {
    pub range: Option<Value>,
    pub severity: Option<u8>,
    pub code: Option<Value>,
    pub source: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
/// `PublishDiagnosticsParams` configuration/state.
pub struct PublishDiagnosticsParams {
    pub uri: String,
    pub diagnostics: Vec<Diagnostic>,
    pub version: Option<i32>,
}
