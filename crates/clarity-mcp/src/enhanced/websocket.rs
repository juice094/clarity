use super::{JsonRpcRequest, JsonRpcResponse, McpClient, McpError, McpServerConfig, McpTransport};
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{RwLock, oneshot};
use tracing::warn;
// =============================================================================
// WebSocket Client
// =============================================================================

/// MCP client using a WebSocket transport.
pub struct WebSocketMcpClient {
    config: McpServerConfig,
    request_id: AtomicU64,
    pending: Arc<RwLock<HashMap<u64, oneshot::Sender<JsonRpcResponse<Value>>>>>,
    ws_task: Option<tokio::task::JoinHandle<()>>,
    ws_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
}

impl WebSocketMcpClient {
    /// Create a new WebSocket MCP client from the given configuration.
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            request_id: AtomicU64::new(1),
            pending: Arc::new(RwLock::new(HashMap::new())),
            ws_task: None,
            ws_tx: None,
        }
    }
}

#[async_trait]
impl McpClient for WebSocketMcpClient {
    async fn connect(&mut self) -> Result<(), McpError> {
        let McpTransport::WebSocket {
            url,
            timeout_seconds,
            ..
        } = &self.config.transport
        else {
            return Err(McpError::InvalidTransport(
                "Expected WebSocket transport".into(),
            ));
        };

        let pending = self.pending.clone();
        let url = url.clone();
        let timeout = *timeout_seconds;

        let (ws_tx, mut ws_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        let task = tokio::spawn(async move {
            let result = tokio::time::timeout(
                Duration::from_secs(timeout),
                tokio_tungstenite::connect_async(&url),
            )
            .await;

            let (ws_stream, _) = match result {
                Ok(Ok((s, r))) => (s, r),
                Ok(Err(e)) => {
                    warn!("WebSocket connection failed: {}", e);
                    return;
                }
                Err(_) => {
                    warn!("WebSocket connection timed out");
                    return;
                }
            };

            let (mut write, mut read) = ws_stream.split();

            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                                if let Ok(response) = serde_json::from_str::<JsonRpcResponse<Value>>(&text) {
                                    let mut pending = pending.write().await;
                                    if let Some(sender) = pending.remove(&response.id) {
                                        if let Err(_e) = sender.send(response) {
                                            tracing::debug!("MCP WebSocket: response receiver dropped before delivery");
                                        }
                                    }
                                }
                            }
                            Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) | None => break,
                            Some(Err(e)) => {
                                warn!("WebSocket read error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                    cmd = ws_rx.recv() => {
                        match cmd {
                            Some(text) => {
                                if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Text(text)).await {
                                    warn!("WebSocket write error: {}", e);
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        self.ws_task = Some(task);
        self.ws_tx = Some(ws_tx);

        // Brief delay to let connection establish before handshake
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Perform initialization handshake
        let init_params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "clarity-core",
                "version": env!("CARGO_PKG_VERSION")
            }
        });
        let _ = self.request_raw("initialize", Some(init_params)).await?;
        if let Err(e) = self.request_raw("notifications/initialized", None).await {
            tracing::debug!("MCP WebSocket initialized notification failed: {}", e);
        }

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), McpError> {
        if let Some(task) = self.ws_task.take() {
            task.abort();
        }
        self.ws_tx = None;
        Ok(())
    }

    async fn request_raw(&self, method: &str, params: Option<Value>) -> Result<Value, McpError> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let McpTransport::WebSocket {
            timeout_seconds, ..
        } = &self.config.transport
        else {
            return Err(McpError::InvalidTransport(
                "Expected WebSocket transport".into(),
            ));
        };

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.write().await;
            pending.insert(id, tx);
        }

        let json = match serde_json::to_string(&request) {
            Ok(s) => s,
            Err(e) => {
                let mut pending = self.pending.write().await;
                pending.remove(&id);
                return Err(McpError::RequestFailed(format!(
                    "JSON serialization failed: {}",
                    e
                )));
            }
        };

        if let Some(ws_tx) = &self.ws_tx {
            if let Err(e) = ws_tx.send(json) {
                let mut pending = self.pending.write().await;
                pending.remove(&id);
                return Err(McpError::RequestFailed(format!(
                    "WebSocket send failed: {}",
                    e
                )));
            }
        } else {
            let mut pending = self.pending.write().await;
            pending.remove(&id);
            return Err(McpError::RequestFailed("WebSocket not connected".into()));
        }

        match tokio::time::timeout(Duration::from_secs(*timeout_seconds), rx).await {
            Ok(Ok(response)) => {
                if let Some(error) = response.error {
                    return Err(McpError::RpcError(error.message));
                }
                response.result.ok_or_else(|| {
                    McpError::InvalidResponse("No result in WebSocket response".into())
                })
            }
            Ok(Err(_)) => {
                let mut pending = self.pending.write().await;
                pending.remove(&id);
                Err(McpError::RequestFailed(
                    "WebSocket response channel closed".into(),
                ))
            }
            Err(_) => {
                let mut pending = self.pending.write().await;
                pending.remove(&id);
                Err(McpError::RequestTimeout)
            }
        }
    }
}
