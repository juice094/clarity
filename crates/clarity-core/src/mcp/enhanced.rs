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
use tracing::{info, warn};

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

fn default_timeout() -> u64 {
    30
}
fn default_reconnect_delay() -> u64 {
    5000
}

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

    /// Add an argument for stdio transport
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        if let McpTransport::Stdio { args, .. } = &mut self.transport {
            args.push(arg.into());
        }
        self
    }

    /// Add multiple arguments for stdio transport
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        if let McpTransport::Stdio {
            args: ref mut a, ..
        } = &mut self.transport
        {
            a.extend(args);
        }
        self
    }

    /// Add multiple environment variables for stdio transport
    pub fn with_envs(mut self, env: std::collections::HashMap<String, String>) -> Self {
        if let McpTransport::Stdio { env: ref mut e, .. } = &mut self.transport {
            e.extend(env);
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
    async fn request_raw(&self, method: &str, params: Option<Value>) -> Result<Value, McpError>;

    /// List available tools
    async fn list_tools(&self) -> Result<Vec<McpTool>, McpError> {
        let result = self.request_raw("tools/list", None).await?;
        let tools = result
            .get("tools")
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

    /// List available resources
    async fn list_resources(&self) -> Result<ListResourcesResult, McpError> {
        let result = self.request_raw("resources/list", None).await?;
        serde_json::from_value(result).map_err(McpError::Serialization)
    }

    /// Read a resource by URI
    async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult, McpError> {
        let params = serde_json::json!({ "uri": uri });
        let result = self.request_raw("resources/read", Some(params)).await?;
        serde_json::from_value(result).map_err(McpError::Serialization)
    }

    /// List available prompts
    async fn list_prompts(&self) -> Result<ListPromptsResult, McpError> {
        let result = self.request_raw("prompts/list", None).await?;
        serde_json::from_value(result).map_err(McpError::Serialization)
    }

    /// Get a prompt by name with optional arguments
    async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<GetPromptResult, McpError> {
        let mut params = serde_json::json!({ "name": name });
        if let Some(args) = arguments {
            params["arguments"] = args;
        }
        let result = self.request_raw("prompts/get", Some(params)).await?;
        serde_json::from_value(result).map_err(McpError::Serialization)
    }
}

// =============================================================================
// Command validation for MCP stdio transport
// =============================================================================

/// Validate an MCP stdio command to prevent command-injection attacks.
///
/// Rules:
/// 1. If `CLARITY_MCP_ALLOWLIST` is set, the command must match or start with
///    one of the comma-separated entries.
/// 2. Reject shell metacharacters and `..` sequences.
/// 3. Absolute paths are allowed only if they exist and are files.
/// 4. Bare names (no path separators) are allowed — the OS resolves them via PATH.
/// 5. Relative paths are rejected.
fn validate_mcp_command(command: &str) -> Result<(), McpError> {
    let allowlist = std::env::var("CLARITY_MCP_ALLOWLIST").ok();
    validate_mcp_command_with_allowlist(command, allowlist.as_deref())
}

fn validate_mcp_command_with_allowlist(
    command: &str,
    allowlist: Option<&str>,
) -> Result<(), McpError> {
    // 1. Explicit allowlist takes precedence.
    if let Some(allowlist) = allowlist {
        let allowed: Vec<&str> = allowlist
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if !allowed.is_empty() {
            let matched = allowed
                .iter()
                .any(|prefix| command == *prefix || command.starts_with(&format!("{}/", prefix)));
            if !matched {
                return Err(McpError::CommandNotAllowed(format!(
                    "Command '{}' not in CLARITY_MCP_ALLOWLIST",
                    command
                )));
            }
            return Ok(());
        }
    }

    // 2. Default hardening.
    const BAD_CHARS: &[char] = &[
        ';', '|', '&', '$', '`', '<', '>', '(', ')', '{', '}', '*', '?', '~', '\'', '"',
    ];
    if command.contains("..") || command.contains(BAD_CHARS) {
        return Err(McpError::CommandNotAllowed(format!(
            "Command '{}' contains unsafe characters",
            command
        )));
    }

    let path = std::path::Path::new(command);

    // Absolute path: must exist and be a file.
    if path.is_absolute() {
        if path.exists() && path.is_file() {
            return Ok(());
        }
        return Err(McpError::CommandNotAllowed(format!(
            "Absolute command '{}' does not exist or is not a file",
            command
        )));
    }

    // Bare name: allowed, OS resolves via PATH.
    if !command.contains('/') && !command.contains('\\') {
        return Ok(());
    }

    // Anything else is treated as a relative path and rejected.
    Err(McpError::CommandNotAllowed(format!(
        "Relative command paths are not allowed: '{}'",
        command
    )))
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
            return Err(McpError::InvalidTransport(
                "Expected stdio transport".into(),
            ));
        };

        validate_mcp_command(command)?;

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
                    cmd.spawn()
                        .map_err(|e| McpError::ConnectionFailed(e.to_string()))?
                }
                #[cfg(not(windows))]
                return Err(McpError::ConnectionFailed(e.to_string()));
            }
            Err(e) => return Err(McpError::ConnectionFailed(e.to_string())),
        };
        let stdin = child
            .stdin
            .take()
            .ok_or(McpError::ConnectionFailed("Failed to open stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or(McpError::ConnectionFailed("Failed to open stdout".into()))?;

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

    async fn request_raw(&self, method: &str, params: Option<Value>) -> Result<Value, McpError> {
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

        response
            .result
            .ok_or_else(|| McpError::InvalidResponse("No result in response".into()))
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
                headers.insert(
                    "Authorization",
                    format!("Bearer {}", token).parse().unwrap(),
                );
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

// =============================================================================
// SSE Client Stub
// =============================================================================
/// Stub implementation for SSE MCP client.
///
/// **WARNING**: This is not a real SSE client. `connect()` is a no-op and
/// `request_raw()` sends plain HTTP POST requests instead of using the
/// Server-Sent Events protocol (MCP-over-SSE).
///
/// Protocol flow:
/// 1. GET /sse → SSE stream
/// 2. event: endpoint → data: /messages?sid=xxx
/// 3. POST /messages?sid=xxx (JSON-RPC request)
/// 4. Response arrives via SSE event: message → data: {jsonrpc:"2.0", id, result}
pub struct SseMcpClient {
    config: McpServerConfig,
    client: reqwest::Client,
    request_id: AtomicU64,
    pending: Arc<RwLock<HashMap<u64, oneshot::Sender<JsonRpcResponse<Value>>>>>,
    sse_task: Option<tokio::task::JoinHandle<()>>,
    message_endpoint: Arc<RwLock<Option<String>>>,
}

impl SseMcpClient {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
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
                use futures::StreamExt;

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
                                            let resolved = if data.starts_with("http://") || data.starts_with("https://") {
                                                data
                                            } else {
                                                match url::Url::parse(&url).and_then(|base| base.join(&data)) {
                                                    Ok(u) => u.to_string(),
                                                    Err(e) => {
                                                        warn!("Failed to resolve endpoint URL '{}': {}", data, e);
                                                        String::new()
                                                    }
                                                }
                                            };
                                            if !resolved.is_empty() {
                                                let mut ep = message_endpoint.write().await;
                                                *ep = Some(resolved);
                                                info!("SSE message endpoint discovered: {}", ep.as_ref().unwrap());
                                            }
                                        } else if current_event == "message" || current_event.is_empty() {
                                            if let Ok(response) = serde_json::from_str::<JsonRpcResponse<Value>>(&data) {
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
        let _ = self.request_raw("notifications/initialized", None).await;

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

// =============================================================================
// Client Builder
// =============================================================================

pub struct McpClientBuilder;

impl McpClientBuilder {
    pub fn from_config(config: McpServerConfig) -> McpClientInstance {
        match &config.transport {
            McpTransport::Stdio { .. } => {
                McpClientInstance::Stdio(Box::new(StdioMcpClient::new(config)))
            }
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

    /// Build an `McpClientInstance` from an `McpServerEntry` (config-file format).
    /// Automatically selects stdio, http, or sse based on the `transport` field.
    pub fn from_mcp_entry(
        name: impl Into<String>,
        entry: &crate::mcp::config::McpServerEntry,
    ) -> McpClientInstance {
        let name = name.into();
        match entry.transport.as_deref() {
            Some("http") | Some("Http") => {
                let url = entry.url.clone().unwrap_or_default();
                let mut builder = Self::http(&name, url);
                for (k, v) in &entry.headers {
                    builder = builder.header(k, v);
                }
                builder.build()
            }
            Some("sse") | Some("Sse") => {
                let url = entry.url.clone().unwrap_or_default();
                let mut builder = Self::sse(&name, url);
                for (k, v) in &entry.headers {
                    builder = builder.header(k, v);
                }
                builder.build()
            }
            _ => {
                let mut builder = Self::stdio(&name, &entry.command);
                for arg in &entry.args {
                    builder = builder.arg(arg);
                }
                for (k, v) in &entry.env {
                    builder = builder.env(k, v);
                }
                builder.build()
            }
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
// Resource Types
// =============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceMeta {
    pub uri: String,
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesResult {
    pub resources: Vec<McpResourceMeta>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextResourceContents {
    pub uri: String,
    pub mime_type: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobResourceContents {
    pub uri: String,
    pub mime_type: Option<String>,
    pub blob: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ResourceContents {
    Text(TextResourceContents),
    Blob(BlobResourceContents),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceResult {
    pub contents: Vec<ResourceContents>,
}

// =============================================================================
// Prompt Types
// =============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPrompt {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Option<Vec<PromptArgument>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsResult {
    pub prompts: Vec<McpPrompt>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptMessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum PromptContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: McpResource },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMessage {
    pub role: PromptMessageRole,
    pub content: PromptContent,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptResult {
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
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

    #[error("Command not allowed: {0}")]
    CommandNotAllowed(String),

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
    clients: HashMap<String, Arc<RwLock<McpClientInstance>>>,
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: impl Into<String>, client: McpClientInstance) {
        self.clients
            .insert(name.into(), Arc::new(RwLock::new(client)));
    }

    pub fn get(&self, name: &str) -> Option<&Arc<RwLock<McpClientInstance>>> {
        self.clients.get(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<Arc<RwLock<McpClientInstance>>> {
        self.clients.remove(name)
    }

    pub fn list(&self) -> Vec<&str> {
        self.clients.keys().map(|k| k.as_str()).collect()
    }

    pub async fn connect_all(&self) -> Result<(), McpError> {
        for (name, client) in &self.clients {
            info!("Connecting to MCP server: {}", name);
            client.write().await.connect().await?;
        }
        Ok(())
    }

    pub async fn disconnect_all(&self) -> Result<(), McpError> {
        for (name, client) in &self.clients {
            info!("Disconnecting from MCP server: {}", name);
            client.write().await.disconnect().await?;
        }
        Ok(())
    }

    /// Iterate over registered clients.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Arc<RwLock<McpClientInstance>>)> {
        self.clients.iter()
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
        assert!(
            matches!(&config.transport, McpTransport::Stdio { command, .. } if command == "npx")
        );
    }

    #[test]
    fn test_http_config() {
        let config = McpServerConfig::http("api", "https://api.example.com/mcp")
            .with_header("Authorization", "Bearer token");

        if let McpTransport::Http { url, headers, .. } = &config.transport {
            assert_eq!(url, "https://api.example.com/mcp");
            assert_eq!(
                headers.get("Authorization"),
                Some(&"Bearer token".to_string())
            );
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

    #[test]
    fn test_validate_command_bare_name_allowed() {
        assert!(validate_mcp_command_with_allowlist("npx", None).is_ok());
        assert!(validate_mcp_command_with_allowlist("node", None).is_ok());
        assert!(validate_mcp_command_with_allowlist("uvx", None).is_ok());
    }

    #[test]
    fn test_validate_command_rejects_metacharacters() {
        assert!(validate_mcp_command_with_allowlist("bash; rm -rf /", None).is_err());
        assert!(validate_mcp_command_with_allowlist("node | curl", None).is_err());
        assert!(validate_mcp_command_with_allowlist("npx && evil", None).is_err());
        assert!(validate_mcp_command_with_allowlist("`whoami`", None).is_err());
        assert!(validate_mcp_command_with_allowlist("$(id)", None).is_err());
    }

    #[test]
    fn test_validate_command_rejects_relative_paths() {
        assert!(validate_mcp_command_with_allowlist("../evil.exe", None).is_err());
        assert!(validate_mcp_command_with_allowlist("./script.sh", None).is_err());
        assert!(validate_mcp_command_with_allowlist("subdir/binary", None).is_err());
    }

    #[test]
    fn test_validate_command_allowlist_override() {
        let allowlist = "/usr/bin/npx,/opt/bin";
        // Allowed because it matches exactly
        assert!(validate_mcp_command_with_allowlist("/usr/bin/npx", Some(allowlist)).is_ok());
        // Allowed because it starts with /opt/bin/
        assert!(validate_mcp_command_with_allowlist("/opt/bin/tool", Some(allowlist)).is_ok());
        // Blocked
        assert!(validate_mcp_command_with_allowlist("/usr/bin/node", Some(allowlist)).is_err());
        assert!(validate_mcp_command_with_allowlist("npx", Some(allowlist)).is_err());
    }

    #[test]
    fn test_resource_types_deserialize() {
        let json = serde_json::json!({
            "resources": [
                {
                    "uri": "file:///tmp/test.txt",
                    "name": "test.txt",
                    "mimeType": "text/plain",
                    "description": "A test file"
                }
            ]
        });
        let result: ListResourcesResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.resources.len(), 1);
        assert_eq!(result.resources[0].uri, "file:///tmp/test.txt");
        assert_eq!(result.resources[0].name.as_ref().unwrap(), "test.txt");
    }

    #[test]
    fn test_read_resource_result_deserialize() {
        let json = serde_json::json!({
            "contents": [
                {
                    "uri": "file:///tmp/test.txt",
                    "mimeType": "text/plain",
                    "text": "Hello, world!"
                }
            ]
        });
        let result: ReadResourceResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.contents.len(), 1);
        match &result.contents[0] {
            ResourceContents::Text(t) => {
                assert_eq!(t.text, "Hello, world!");
            }
            _ => panic!("Expected text resource"),
        }
    }

    #[test]
    fn test_prompt_types_deserialize() {
        let json = serde_json::json!({
            "prompts": [
                {
                    "name": "code-review",
                    "description": "Review code changes",
                    "arguments": [
                        {
                            "name": "pr_number",
                            "description": "The PR number",
                            "required": true
                        }
                    ]
                }
            ]
        });
        let result: ListPromptsResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.prompts.len(), 1);
        assert_eq!(result.prompts[0].name, "code-review");
        let args = result.prompts[0].arguments.as_ref().unwrap();
        assert_eq!(args[0].name, "pr_number");
        assert_eq!(args[0].required, Some(true));
    }

    #[test]
    fn test_get_prompt_result_deserialize() {
        let json = serde_json::json!({
            "description": "Code review prompt",
            "messages": [
                {
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": "Please review this code."
                    }
                }
            ]
        });
        let result: GetPromptResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.description.as_ref().unwrap(), "Code review prompt");
        assert_eq!(result.messages.len(), 1);
        assert!(matches!(result.messages[0].role, PromptMessageRole::User));
        assert!(matches!(
            result.messages[0].content,
            PromptContent::Text { .. }
        ));
    }
}
