use super::{JsonRpcRequest, JsonRpcResponse, McpClient, McpError, McpServerConfig, McpTransport};
use async_trait::async_trait;
use reqwest::header::HeaderMap;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::info;
// =============================================================================
// HTTP Client
// =============================================================================

/// MCP client using HTTP POST requests as transport.
pub struct HttpMcpClient {
    config: McpServerConfig,
    client: reqwest::Client,
    request_id: AtomicU64,
}

impl HttpMcpClient {
    /// Create a new HTTP MCP client from the given configuration.
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            request_id: AtomicU64::new(1),
        }
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Content-Type",
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        if let McpTransport::Http {
            headers: custom_headers,
            ..
        } = &self.config.transport
        {
            for (key, value) in custom_headers {
                if let (Ok(header_name), Ok(header_value)) = (
                    key.parse::<reqwest::header::HeaderName>(),
                    value.parse::<reqwest::header::HeaderValue>(),
                ) {
                    headers.insert(header_name, header_value);
                }
            }
        }

        // Add OAuth token if available
        if let Some(oauth) = &self.config.oauth {
            if let Some(token) = &oauth.access_token {
                if let Ok(header_value) =
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token))
                {
                    headers.insert("Authorization", header_value);
                }
            }
        }

        headers
    }
}

#[async_trait]
impl McpClient for HttpMcpClient {
    async fn connect(&mut self) -> Result<(), McpError> {
        info!("HTTP MCP client configured: {}", self.config.name);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), McpError> {
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

        let McpTransport::Http {
            url,
            timeout_seconds,
            ..
        } = &self.config.transport
        else {
            return Err(McpError::InvalidTransport("Expected HTTP transport".into()));
        };

        let response = self
            .client
            .post(url)
            .headers(self.build_headers())
            .json(&request)
            .timeout(Duration::from_secs(*timeout_seconds))
            .send()
            .await
            .map_err(|e| McpError::RequestFailed(e.to_string()))?;

        let rpc_response: JsonRpcResponse<Value> = response
            .json()
            .await
            .map_err(|e| McpError::InvalidResponse(e.to_string()))?;

        if let Some(error) = rpc_response.error {
            return Err(McpError::RpcError(error.message));
        }

        rpc_response
            .result
            .ok_or_else(|| McpError::InvalidResponse("No result in response".into()))
    }
}
