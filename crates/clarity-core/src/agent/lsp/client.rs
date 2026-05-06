//! Stdio-based LSP JSON-RPC client.
//!
//! Reuses the transport pattern from `clarity_mcp::McpClientLegacy`:
//! spawn a subprocess, read stdout line-by-line, route responses via oneshot
//! channels, write requests to stdin.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, RwLock};
use tracing::{debug, info};

use crate::agent::lsp::protocol::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, InitializeParams, InitializeResult,
    JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, PublishDiagnosticsParams,
    TextDocumentContentChangeEvent, TextDocumentItem, VersionedTextDocumentIdentifier,
};
use crate::error::AgentError;

pub struct LspClient {
    _child: Option<Child>,
    stdin: Arc<tokio::sync::Mutex<Option<ChildStdin>>>,
    request_id: AtomicU64,
    pending: Arc<RwLock<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    diagnostics: Arc<RwLock<Vec<PublishDiagnosticsParams>>>,
}

impl LspClient {
    /// Spawn an LSP server and perform the initialize handshake.
    pub async fn spawn(
        command: &str,
        args: &[String],
        root_uri: Option<String>,
    ) -> Result<Self, AgentError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            AgentError::ToolExecutionFailed(
                "lsp_spawn".to_string(),
                format!("Failed to spawn LSP server '{}': {}", command, e),
            )
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            AgentError::ToolExecutionFailed(
                "lsp_spawn".to_string(),
                "Failed to open LSP stdin".into(),
            )
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AgentError::ToolExecutionFailed(
                "lsp_spawn".to_string(),
                "Failed to open LSP stdout".into(),
            )
        })?;

        let pending: Arc<RwLock<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let diagnostics: Arc<RwLock<Vec<PublishDiagnosticsParams>>> =
            Arc::new(RwLock::new(Vec::new()));

        // Spawn response / notification reader
        let pending_clone = pending.clone();
        let diagnostics_clone = diagnostics.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                debug!("LSP server line: {}", line);

                let value: Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(e) => {
                        debug!("Failed to parse LSP line as JSON: {}", e);
                        continue;
                    }
                };

                // Response: has "id"
                if value.get("id").is_some() {
                    if let Ok(resp) = serde_json::from_value::<JsonRpcResponse>(value) {
                        let mut pending = pending_clone.write().await;
                        if let Some(sender) = pending.remove(&resp.id) {
                            let _ = sender.send(resp);
                        }
                    }
                }
                // Notification: has "method", no "id"
                else if value.get("method").is_some() {
                    if let Ok(notif) = serde_json::from_value::<JsonRpcNotification>(value) {
                        if notif.method == "textDocument/publishDiagnostics" {
                            if let Some(params) = notif.params {
                                if let Ok(p) =
                                    serde_json::from_value::<PublishDiagnosticsParams>(params)
                                {
                                    let mut buf = diagnostics_clone.write().await;
                                    if let Some(existing) = buf.iter_mut().find(|d| d.uri == p.uri)
                                    {
                                        *existing = p;
                                    } else {
                                        buf.push(p);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let client = Self {
            _child: Some(child),
            stdin: Arc::new(tokio::sync::Mutex::new(Some(stdin))),
            request_id: AtomicU64::new(1),
            pending,
            diagnostics,
        };

        // Initialize handshake
        let init_params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri,
            capabilities: serde_json::json!({}),
        };

        let _: InitializeResult = client
            .request("initialize", init_params)
            .await
            .map_err(|e| {
                AgentError::ToolExecutionFailed(
                    "lsp_initialize".to_string(),
                    format!("LSP initialize failed: {}", e),
                )
            })?;

        client
            .notify("initialized", serde_json::json!({}))
            .await
            .map_err(|e| {
                AgentError::ToolExecutionFailed(
                    "lsp_initialized".to_string(),
                    format!("LSP initialized notification failed: {}", e),
                )
            })?;

        info!("LSP client connected: {} {:?}", command, args);
        Ok(client)
    }

    /// Send a JSON-RPC request and await the response.
    pub async fn request<T: Serialize + Send, R: DeserializeOwned>(
        &self,
        method: &str,
        params: T,
    ) -> Result<R, AgentError> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params: Some(params),
        };

        let json = serde_json::to_string(&request)
            .map_err(|e| AgentError::Llm(format!("Failed to serialize LSP request: {}", e)))?;

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.write().await;
            pending.insert(id, tx);
        }

        {
            let mut stdin_guard = self.stdin.lock().await;
            if let Some(ref mut stdin) = *stdin_guard {
                stdin
                    .write_all(json.as_bytes())
                    .await
                    .map_err(|e| AgentError::Llm(format!("LSP stdin write failed: {}", e)))?;
                stdin
                    .write_all(b"\n")
                    .await
                    .map_err(|e| AgentError::Llm(format!("LSP stdin write failed: {}", e)))?;
                stdin
                    .flush()
                    .await
                    .map_err(|e| AgentError::Llm(format!("LSP stdin flush failed: {}", e)))?;
            } else {
                return Err(AgentError::Llm("LSP stdin closed".into()));
            }
        }

        let response = rx
            .await
            .map_err(|_| AgentError::Llm("LSP request cancelled or timed out".into()))?;

        if let Some(error) = response.error {
            return Err(AgentError::Llm(format!(
                "LSP error {}: {}",
                error.code, error.message
            )));
        }

        let result = response
            .result
            .ok_or_else(|| AgentError::Llm("LSP response missing result".into()))?;

        serde_json::from_value(result)
            .map_err(|e| AgentError::Llm(format!("Failed to deserialize LSP result: {}", e)))
    }

    /// Send a JSON-RPC notification (fire-and-forget).
    pub async fn notify<T: Serialize + Send>(
        &self,
        method: &str,
        params: T,
    ) -> Result<(), AgentError> {
        #[derive(Serialize)]
        struct NotifyEnvelope<T: Serialize> {
            jsonrpc: &'static str,
            method: String,
            params: T,
        }

        let envelope = NotifyEnvelope {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
        };

        let json = serde_json::to_string(&envelope)
            .map_err(|e| AgentError::Llm(format!("Failed to serialize LSP notification: {}", e)))?;

        let mut stdin_guard = self.stdin.lock().await;
        if let Some(ref mut stdin) = *stdin_guard {
            stdin
                .write_all(json.as_bytes())
                .await
                .map_err(|e| AgentError::Llm(format!("LSP stdin write failed: {}", e)))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| AgentError::Llm(format!("LSP stdin write failed: {}", e)))?;
            stdin
                .flush()
                .await
                .map_err(|e| AgentError::Llm(format!("LSP stdin flush failed: {}", e)))?;
        } else {
            return Err(AgentError::Llm("LSP stdin closed".into()));
        }

        Ok(())
    }

    /// Send `textDocument/didOpen`.
    pub async fn did_open(
        &self,
        uri: &str,
        language_id: &str,
        text: &str,
    ) -> Result<(), AgentError> {
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.to_string(),
                language_id: language_id.to_string(),
                version: 1,
                text: text.to_string(),
            },
        };
        self.notify("textDocument/didOpen", params).await
    }

    /// Send `textDocument/didChange` with full-document replacement.
    pub async fn did_change(&self, uri: &str, version: i32, text: &str) -> Result<(), AgentError> {
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.to_string(),
                version,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                text: text.to_string(),
            }],
        };
        self.notify("textDocument/didChange", params).await
    }

    /// Drain all buffered diagnostics and return them.
    pub async fn drain_diagnostics(&self) -> Vec<PublishDiagnosticsParams> {
        let mut buf = self.diagnostics.write().await;
        std::mem::take(&mut *buf)
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        if let Some(mut child) = self._child.take() {
            let _ = child.start_kill();
        }
    }
}
