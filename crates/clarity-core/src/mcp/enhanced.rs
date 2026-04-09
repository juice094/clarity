//! Enhanced MCP (Model Context Protocol) Client
//!
//! Supports multiple transports: stdio, HTTP, SSE
//! Reference: Kimi CLI's fastmcp implementation

use async_trait::async_trait;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, RwLock};
use tracing::info;

// =============================================================================
// Transport Types
// =============================================================================

/// MCP transport types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "lowercase")]
pub enum McpTransport {
    /// Stdio transport (local process)
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    /// HTTP transport (POST requests)
    Http {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
    /// SSE transport (Server-Sent Events)
    Sse {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
        #[serde(default = "default_reconnect_delay")]
        reconnect_delay_ms: u64,
    },
}

fn default_timeout() -> u64 { 30 }
fn default_reconnect_delay() -> u64 { 5000 }

/// OAuth 2.0 configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub token_url: String,
    pub auth_url: Option<String>,
    pub scope: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(flatten)]
    pub transport: McpTransport,
    #[serde(default)]
    pub oauth: Option<OAuthConfig>,
}

impl McpServerConfig {
    /// Create a new stdio server config
    pub fn stdio(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: McpTransport::Stdio {
                command: command.into(),
                args: Vec::new(),
                env: HashMap::new(),
            },
            oauth: None,
        }
    }

    /// Create a new HTTP server config
    pub fn http(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: McpTransport::Http {
                url: url.into(),
                headers: HashMap::new(),
                timeout_seconds: 30,
            },
            oauth: None,
        }
    }

    /// Create a new SSE server config
    pub fn sse(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: McpTransport::Sse {
                url: url.into(),
                headers: HashMap::new(),
                timeout_seconds: 30,
                reconnect_delay_ms: 5000,
            },
            oauth: None,
        }
    }

    /// Add header for HTTP/SSE transports
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        match &mut self.transport {
            McpTransport::Http { headers, .. } | McpTransport::Sse { headers, .. } => {
                headers.insert(key, value);
            }
            _ => {}
        }
        self
    }
}

// =============================================================================
// JSON-RPC 2.0 Types
// =============================================================================

#[derive(Debug, Serialize)]
struct JsonRpcRequest<T: serde::Serialize> {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<T>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcResponse<T> {
    jsonrpc: String,
    id: u64,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}

// =============================================================================
// MCP Client Trait
// =============================================================================

#[async_trait]
pub trait McpClient: Send + Sync {
    /// Connect to the server
    async fn connect(&mut self) -> Result<(), McpError>;

    /// Disconnect from the server
    async fn disconnect(&mut self) -> Result<(), McpError>;

    /// Send a raw JSON-RPC request
    async fn request_raw(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, McpError>;

    /// List available tools
    async fn list_tools(&self) -> Result<Vec<McpTool>, McpError> {
        let result = self.request_raw("tools/list", None).await?;
        let tools = result.get("tools")
            .cloned()
            .unwrap_or_else(|| Value::Array(vec![]));
        serde_json::from_value(tools).map_err(McpError::Serialization)
    }

    /// Call a tool
    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments
        });
        let result = self.request_raw("tools/call", Some(params)).await?;
        serde_json::from_value(result).map_err(McpError::Serialization)
    }
}

// =============================================================================
// Stdio Client
// =============================================================================

pub struct StdioMcpClient {
    config: McpServerConfig,
    child: Option<Child>,
    stdin: Option<tokio::sync::Mutex<ChildStdin>>,
    request_id: AtomicU64,
    pending: Arc<RwLock<HashMap<u64, oneshot::Sender<JsonRpcResponse<Value>>>>>,
}

impl StdioMcpClient {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            child: None,
            stdin: None,
            request_id: AtomicU64::new(1),
            pending: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Drop for StdioMcpClient {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            tokio::spawn(async move {
                let _ = child.kill().await;
            });
        }
    }
}

#[async_trait]
impl McpClient for StdioMcpClient {
    async fn connect(&mut self) -> Result<(), McpError> {
        let McpTransport::Stdio { command, args, env } = &self.config.transport else {
            return Err(McpError::InvalidTransport("Expected stdio transport".into()));
        };

        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        for (key, value) in env {
            cmd.env(key, value);
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                #[cfg(windows)]
                {
                    // On Windows, commands like `npx` are `.cmd` scripts which
                    // CreateProcessW cannot launch directly. Retry via `cmd /c`.
                    let mut cmd = Command::new("cmd");
                    cmd.arg("/c").arg(command);
                    cmd.args(args)
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped());
                    for (key, value) in env {
                        cmd.env(key, value);
                    }
                    cmd.spawn().map_err(|e| McpError::ConnectionFailed(e.to_string()))?
                }
                #[cfg(not(windows))]
                return Err(McpError::ConnectionFailed(e.to_string()));
            }
            Err(e) => return Err(McpError::ConnectionFailed(e.to_string())),
        };
        let stdin = child.stdin.take().ok_or(McpError::ConnectionFailed("Failed to open stdin".into()))?;
        let stdout = child.stdout.take().ok_or(McpError::ConnectionFailed("Failed to open stdout".into()))?;

        self.child = Some(child);
        self.stdin = Some(tokio::sync::Mutex::new(stdin));

        // Give wrapper tools (npx, uvx) a moment to finish setup before
        // sending the initialization handshake.
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Start response reader
        let pending = self.pending.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(response) = serde_json::from_str::<JsonRpcResponse<Value>>(&line) {
                    let mut pending = pending.write().await;
                    if let Some(sender) = pending.remove(&response.id) {
                        let _ = sender.send(response);
                    }
                }
            }
        });

        // Perform MCP initialization handshake
        let init_params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "clarity-core",
                "version": env!("CARGO_PKG_VERSION")
            }
        });
        let _ = self.request_raw("initialize", Some(init_params)).await?;

        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let notification_json = serde_json::to_string(&notification)?;
        if let Some(ref stdin_mutex) = self.stdin {
            let mut stdin = stdin_mutex.lock().await;
            stdin.write_all(notification_json.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }

        // Some MCP servers need a brief moment after the initialization
        // handshake before they accept further requests.
        tokio::time::sleep(Duration::from_millis(100)).await;

        info!("Connected to MCP server via stdio: {}", self.config.name);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), McpError> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
        self.stdin = None;
        Ok(())
    }

    async fn request_raw(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, McpError> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.write().await;
            pending.insert(id, tx);
        }

        let request_json = serde_json::to_string(&request)?;
        if let Some(ref stdin_mutex) = self.stdin {
            let mut stdin = stdin_mutex.lock().await;
            stdin.write_all(request_json.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }

        let response = tokio::time::timeout(Duration::from_secs(120), rx)
            .await
            .map_err(|_| McpError::RequestTimeout)?
            .map_err(|_| McpError::RequestTimeout)?;
        
        if let Some(error) = response.error {
            return Err(McpError::RpcError(error.message));
        }

        response.result.ok_or_else(|| McpError::InvalidResponse("No result in response".into()))
    }
}

// =============================================================================
// HTTP Client
// =============================================================================

pub struct HttpMcpClient {
    config: McpServerConfig,
    client: reqwest::Client,
    request_id: AtomicU64,
}

impl HttpMcpClient {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            request_id: AtomicU64::new(1),
        }
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse().unwrap());
        
        if let McpTransport::Http { headers: custom_headers, .. } = &self.config.transport {
            for (key, value) in custom_headers {
                if let (Ok(header_name), Ok(header_value)) = 
                    (key.parse::<reqwest::header::HeaderName>(), value.parse::<reqwest::header::HeaderValue>()) {
                    headers.insert(header_name, header_value);
                }
            }
        }

        // Add OAuth token if available
        if let Some(oauth) = &self.config.oauth {
            if let Some(token) = &oauth.access_token {
                headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
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

    async fn request_raw(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, McpError> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let McpTransport::Http { url, timeout_seconds, .. } = &self.config.transport else {
            return Err(McpError::InvalidTransport("Expected HTTP transport".into()));
        };

        let response = self.client
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

        rpc_response.result.ok_or_else(|| McpError::InvalidResponse("No result in response".into()))
    }
}

// =============================================================================
// SSE Client
// =============================================================================

pub struct SseMcpClient {
    config: McpServerConfig,
    client: reqwest::Client,
    request_id: AtomicU64,
}

impl SseMcpClient {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            request_id: AtomicU64::new(1),
        }
    }
}

#[async_trait]
impl McpClient for SseMcpClient {
    async fn connect(&mut self) -> Result<(), McpError> {
        info!("SSE MCP client configured: {}", self.config.name);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), McpError> {
        Ok(())
    }

    async fn request_raw(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, McpError> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let McpTransport::Sse { url, timeout_seconds, .. } = &self.config.transport else {
            return Err(McpError::InvalidTransport("Expected SSE transport".into()));
        };

        let response = self.client
            .post(url)
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

        rpc_response.result.ok_or_else(|| McpError::InvalidResponse("No result in response".into()))
    }
}

// =============================================================================
// Client Builder
// =============================================================================

pub struct McpClientBuilder;

impl McpClientBuilder {
    pub fn from_config(config: McpServerConfig) -> McpClientInstance {
        match &config.transport {
            McpTransport::Stdio { .. } => McpClientInstance::Stdio(Box::new(StdioMcpClient::new(config))),
            McpTransport::Http { .. } => McpClientInstance::Http(HttpMcpClient::new(config)),
            McpTransport::Sse { .. } => McpClientInstance::Sse(SseMcpClient::new(config)),
        }
    }

    pub fn stdio(name: impl Into<String>, command: impl Into<String>) -> StdioClientBuilder {
        StdioClientBuilder {
            config: McpServerConfig::stdio(name, command),
        }
    }

    pub fn http(name: impl Into<String>, url: impl Into<String>) -> HttpClientBuilder {
        HttpClientBuilder {
            config: McpServerConfig::http(name, url),
        }
    }

    pub fn sse(name: impl Into<String>, url: impl Into<String>) -> SseClientBuilder {
        SseClientBuilder {
            config: McpServerConfig::sse(name, url),
        }
    }
}

pub struct StdioClientBuilder {
    config: McpServerConfig,
}

impl StdioClientBuilder {
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        if let McpTransport::Stdio { args, .. } = &mut self.config.transport {
            args.push(arg.into());
        }
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let McpTransport::Stdio { env, .. } = &mut self.config.transport {
            env.insert(key.into(), value.into());
        }
        self
    }

    pub fn build(self) -> McpClientInstance {
        McpClientInstance::Stdio(Box::new(StdioMcpClient::new(self.config)))
    }
}

pub struct HttpClientBuilder {
    config: McpServerConfig,
}

impl HttpClientBuilder {
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config = self.config.with_header(key, value);
        self
    }

    pub fn oauth(mut self, oauth: OAuthConfig) -> Self {
        self.config.oauth = Some(oauth);
        self
    }

    pub fn build(self) -> McpClientInstance {
        McpClientInstance::Http(HttpMcpClient::new(self.config))
    }
}

pub struct SseClientBuilder {
    config: McpServerConfig,
}

impl SseClientBuilder {
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config = self.config.with_header(key, value);
        self
    }

    pub fn oauth(mut self, oauth: OAuthConfig) -> Self {
        self.config.oauth = Some(oauth);
        self
    }

    pub fn build(self) -> McpClientInstance {
        McpClientInstance::Sse(SseMcpClient::new(self.config))
    }
}

// =============================================================================
// Types
// =============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: McpResource },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResource {
    pub uri: String,
    pub mime_type: Option<String>,
    pub text: Option<String>,
    pub blob: Option<String>,
}

// =============================================================================
// Error Types
// =============================================================================

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Invalid transport: {0}")]
    InvalidTransport(String),
    
    #[error("Request failed: {0}")]
    RequestFailed(String),
    
    #[error("Request timeout")]
    RequestTimeout,
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("RPC error: {0}")]
    RpcError(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// =============================================================================
// MCP Registry
// =============================================================================

pub enum McpClientInstance {
    Stdio(Box<StdioMcpClient>),
    Http(HttpMcpClient),
    Sse(SseMcpClient),
}

#[async_trait]
impl McpClient for McpClientInstance {
    async fn connect(&mut self) -> Result<(), McpError> {
        match self {
            McpClientInstance::Stdio(c) => c.connect().await,
            McpClientInstance::Http(c) => c.connect().await,
            McpClientInstance::Sse(c) => c.connect().await,
        }
    }

    async fn disconnect(&mut self) -> Result<(), McpError> {
        match self {
            McpClientInstance::Stdio(c) => c.disconnect().await,
            McpClientInstance::Http(c) => c.disconnect().await,
            McpClientInstance::Sse(c) => c.disconnect().await,
        }
    }

    async fn request_raw(&self, method: &str, params: Option<Value>) -> Result<Value, McpError> {
        match self {
            McpClientInstance::Stdio(c) => c.request_raw(method, params).await,
            McpClientInstance::Http(c) => c.request_raw(method, params).await,
            McpClientInstance::Sse(c) => c.request_raw(method, params).await,
        }
    }
}

pub struct McpRegistry {
    clients: HashMap<String, McpClientInstance>,
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: impl Into<String>, client: McpClientInstance) {
        self.clients.insert(name.into(), client);
    }

    pub fn get(&self, name: &str) -> Option<&McpClientInstance> {
        self.clients.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut McpClientInstance> {
        self.clients.get_mut(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<McpClientInstance> {
        self.clients.remove(name)
    }

    pub fn list(&self) -> Vec<&str> {
        self.clients.keys().map(|k| k.as_str()).collect()
    }

    pub async fn connect_all(&mut self) -> Result<(), McpError> {
        for (name, client) in &mut self.clients {
            info!("Connecting to MCP server: {}", name);
            client.connect().await?;
        }
        Ok(())
    }

    pub async fn disconnect_all(&mut self) -> Result<(), McpError> {
        for (name, client) in &mut self.clients {
            info!("Disconnecting from MCP server: {}", name);
            client.disconnect().await?;
        }
        Ok(())
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdio_config() {
        let config = McpServerConfig::stdio("test", "npx");
        assert!(matches!(&config.transport, McpTransport::Stdio { command, .. } if command == "npx"));
    }

    #[test]
    fn test_http_config() {
        let config = McpServerConfig::http("api", "https://api.example.com/mcp")
            .with_header("Authorization", "Bearer token");
        
        if let McpTransport::Http { url, headers, .. } = &config.transport {
            assert_eq!(url, "https://api.example.com/mcp");
            assert_eq!(headers.get("Authorization"), Some(&"Bearer token".to_string()));
        } else {
            panic!("Expected HTTP transport");
        }
    }

    #[test]
    fn test_mcp_registry() {
        let mut registry = McpRegistry::new();
        let client = McpClientBuilder::stdio("test", "echo").build();
        registry.register("test", client);
        
        assert_eq!(registry.list(), vec!["test"]);
        assert!(registry.get("test").is_some());
        assert!(registry.get("missing").is_none());
    }
    
    #[test]
    fn test_mcp_client_instance() {
        let instance = McpClientBuilder::stdio("test", "echo").build();
        assert!(matches!(instance, McpClientInstance::Stdio(_)));
    }
}
