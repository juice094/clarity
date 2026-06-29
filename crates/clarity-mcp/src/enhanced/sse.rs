use super::{JsonRpcRequest, JsonRpcResponse, McpClient, McpError, McpServerConfig, McpTransport};
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{RwLock, oneshot};
use tracing::{info, warn};

// =============================================================================
// SSE Client
// =============================================================================
/// Full SSE MCP client implementing the MCP-over-SSE protocol.
///
/// Protocol flow:
/// 1. GET /sse → SSE stream
/// 2. event: endpoint → data: /messages?sid=xxx
/// 3. POST /messages?sid=xxx (JSON-RPC request)
/// 4. Response arrives via SSE event: message → data: {jsonrpc:"2.0", id, result}
///
/// Features:
/// - Automatic endpoint discovery from SSE stream
/// - JSON-RPC response correlation via pending request map
/// - Automatic reconnection with configurable delay
/// - Full initialization handshake (initialize + notifications/initialized)
pub struct SseMcpClient {
    config: McpServerConfig,
    client: reqwest::Client,
    request_id: AtomicU64,
    pending: Arc<RwLock<HashMap<u64, oneshot::Sender<JsonRpcResponse<Value>>>>>,
    sse_task: Option<tokio::task::JoinHandle<()>>,
    message_endpoint: Arc<RwLock<Option<String>>>,
}

impl SseMcpClient {
    /// Create a new SSE MCP client from the given configuration.
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            request_id: AtomicU64::new(1),
            pending: Arc::new(RwLock::new(HashMap::new())),
            sse_task: None,
            message_endpoint: Arc::new(RwLock::new(None)),
        }
    }

    /// Test-only constructor that accepts a pre-built HTTP client.
    ///
    /// ponytail: used by mock tests to bypass system HTTP proxies that
    /// would otherwise intercept 127.0.0.1 requests.
    #[cfg(test)]
    pub(crate) fn with_client(config: McpServerConfig, client: reqwest::Client) -> Self {
        Self {
            config,
            client,
            request_id: AtomicU64::new(1),
            pending: Arc::new(RwLock::new(HashMap::new())),
            sse_task: None,
            message_endpoint: Arc::new(RwLock::new(None)),
        }
    }
}

#[async_trait]
impl McpClient for SseMcpClient {
    async fn connect(&mut self) -> Result<(), McpError> {
        let McpTransport::Sse {
            url,
            timeout_seconds,
            headers,
            reconnect_delay_ms,
        } = &self.config.transport
        else {
            return Err(McpError::InvalidTransport("Expected SSE transport".into()));
        };

        let client = self.client.clone();
        let pending = self.pending.clone();
        let message_endpoint = self.message_endpoint.clone();
        let url = url.clone();
        let headers = headers.clone();
        let timeout = *timeout_seconds;
        let reconnect_delay = *reconnect_delay_ms;

        let task = tokio::spawn(async move {
            loop {
                let mut req = client.get(&url).timeout(Duration::from_secs(timeout));
                for (k, v) in &headers {
                    req = req.header(k, v);
                }
                let response = match req.send().await {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("SSE connection failed: {}", e);
                        tokio::time::sleep(Duration::from_millis(reconnect_delay)).await;
                        continue;
                    }
                };

                let mut stream = response.bytes_stream();
                let mut buffer = String::new();
                let mut current_event = String::new();
                let mut current_data = String::new();

                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            buffer.push_str(&String::from_utf8_lossy(&bytes));
                            while let Some(pos) = buffer.find('\n') {
                                let line = buffer[..pos].trim_end_matches('\r').to_string();
                                buffer = buffer[pos + 1..].to_string();

                                if line.is_empty() {
                                    // Event boundary — dispatch accumulated event
                                    if !current_data.is_empty() {
                                        let data = current_data.trim_start().to_string();
                                        if current_event == "endpoint" {
                                            let resolved = if data.starts_with("http://")
                                                || data.starts_with("https://")
                                            {
                                                data
                                            } else {
                                                match url::Url::parse(&url)
                                                    .and_then(|base| base.join(&data))
                                                {
                                                    Ok(u) => u.to_string(),
                                                    Err(e) => {
                                                        warn!(
                                                            "Failed to resolve endpoint URL '{}': {}",
                                                            data, e
                                                        );
                                                        String::new()
                                                    }
                                                }
                                            };
                                            if !resolved.is_empty() {
                                                let mut ep = message_endpoint.write().await;
                                                *ep = Some(resolved);
                                                if let Some(endpoint) = ep.as_ref() {
                                                    info!(
                                                        "SSE message endpoint discovered: {}",
                                                        endpoint
                                                    );
                                                }
                                            }
                                        } else if current_event == "message"
                                            || current_event.is_empty()
                                        {
                                            if let Ok(response) =
                                                serde_json::from_str::<JsonRpcResponse<Value>>(
                                                    &data,
                                                )
                                            {
                                                let mut pending = pending.write().await;
                                                if let Some(sender) = pending.remove(&response.id) {
                                                    let _ = sender.send(response);
                                                }
                                            }
                                        }
                                    }
                                    current_event.clear();
                                    current_data.clear();
                                } else if let Some(evt) = line.strip_prefix("event:") {
                                    current_event = evt.trim_start().to_string();
                                } else if let Some(data) = line.strip_prefix("data:") {
                                    if !current_data.is_empty() {
                                        current_data.push('\n');
                                    }
                                    current_data.push_str(data);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("SSE stream error: {}", e);
                            break;
                        }
                    }
                }

                warn!("SSE stream ended, reconnecting after {}ms", reconnect_delay);
                tokio::time::sleep(Duration::from_millis(reconnect_delay)).await;
            }
        });

        self.sse_task = Some(task);

        // Wait for endpoint to be discovered before proceeding
        let endpoint_timeout = Duration::from_secs(*timeout_seconds);
        let start = std::time::Instant::now();
        loop {
            {
                let ep = self.message_endpoint.read().await;
                if ep.is_some() {
                    break;
                }
            }
            if start.elapsed() > endpoint_timeout {
                return Err(McpError::RequestTimeout);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

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
            tracing::debug!("MCP SSE initialized notification failed: {}", e);
        }

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), McpError> {
        if let Some(task) = self.sse_task.take() {
            task.abort();
        }
        let mut ep = self.message_endpoint.write().await;
        *ep = None;
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

        let McpTransport::Sse {
            timeout_seconds,
            headers,
            ..
        } = &self.config.transport
        else {
            return Err(McpError::InvalidTransport("Expected SSE transport".into()));
        };

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.write().await;
            pending.insert(id, tx);
        }

        // Wait for message endpoint to be discovered
        let post_url = {
            let start = std::time::Instant::now();
            let timeout = Duration::from_secs(*timeout_seconds);
            loop {
                {
                    let ep = self.message_endpoint.read().await;
                    if let Some(url) = ep.as_ref() {
                        break url.clone();
                    }
                }
                if start.elapsed() > timeout {
                    let mut pending = self.pending.write().await;
                    pending.remove(&id);
                    return Err(McpError::RequestTimeout);
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        };

        // POST the JSON-RPC request to the message endpoint
        let mut req = self
            .client
            .post(&post_url)
            .json(&request)
            .timeout(Duration::from_secs(*timeout_seconds));
        for (k, v) in headers {
            req = req.header(k, v);
        }

        if let Err(e) = req.send().await {
            let mut pending = self.pending.write().await;
            pending.remove(&id);
            return Err(McpError::RequestFailed(e.to_string()));
        }

        // Wait for response via SSE stream
        match tokio::time::timeout(Duration::from_secs(*timeout_seconds), rx).await {
            Ok(Ok(response)) => {
                if let Some(error) = response.error {
                    return Err(McpError::RpcError(error.message));
                }
                response
                    .result
                    .ok_or_else(|| McpError::InvalidResponse("No result in SSE response".into()))
            }
            Ok(Err(_)) => {
                let mut pending = self.pending.write().await;
                pending.remove(&id);
                Err(McpError::RequestFailed(
                    "SSE response channel closed".into(),
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
