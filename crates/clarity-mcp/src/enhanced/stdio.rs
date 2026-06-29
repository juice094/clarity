use super::{
    JsonRpcRequest, JsonRpcResponse, McpClient, McpError, McpServerConfig, McpTransport,
    validate_mcp_command,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{RwLock, oneshot};
use tracing::info;
// =============================================================================
// Stdio Client
// =============================================================================

/// MCP client using a local stdio subprocess transport.
pub struct StdioMcpClient {
    config: McpServerConfig,
    child: Option<Child>,
    stdin: Option<tokio::sync::Mutex<ChildStdin>>,
    request_id: AtomicU64,
    pending: Arc<RwLock<HashMap<u64, oneshot::Sender<JsonRpcResponse<Value>>>>>,
    alive: Arc<std::sync::atomic::AtomicBool>,
    /// Handle for the stdout reader task. Stored so Drop can abort it.
    reader_handle: Option<tokio::task::JoinHandle<()>>,
}

impl StdioMcpClient {
    /// Create a new stdio MCP client from the given configuration.
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            child: None,
            stdin: None,
            request_id: AtomicU64::new(1),
            pending: Arc::new(RwLock::new(HashMap::new())),
            alive: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            reader_handle: None,
        }
    }
}

impl Drop for StdioMcpClient {
    fn drop(&mut self) {
        // Abort the reader task first so it doesn't hold references.
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }
        // Kill the child process synchronously. `start_kill()` sends SIGKILL
        // (Unix) / TerminateProcess (Windows) without requiring an async
        // runtime — safe to call from Drop even during runtime shutdown.
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            // Best-effort wait — don't block the drop path indefinitely.
            if let Ok(Some(status)) = child.try_wait() {
                tracing::debug!("MCP stdio child process exited with status {:?}", status);
            }
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
        self.alive.store(true, std::sync::atomic::Ordering::SeqCst);

        // Give wrapper tools (npx, uvx) a moment to finish setup before
        // sending the initialization handshake.
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Start response reader
        let pending = self.pending.clone();
        let alive = self.alive.clone();
        self.reader_handle = Some(tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(response) = serde_json::from_str::<JsonRpcResponse<Value>>(&line) {
                    let mut pending = pending.write().await;
                    if let Some(sender) = pending.remove(&response.id) {
                        if let Err(_e) = sender.send(response) {
                            tracing::debug!(
                                "MCP stdio: response receiver dropped before delivery (id dropped)"
                            );
                        }
                    }
                }
            }
            alive.store(false, std::sync::atomic::Ordering::SeqCst);
        }));

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
        self.alive.store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    async fn request_raw(&self, method: &str, params: Option<Value>) -> Result<Value, McpError> {
        if !self.alive.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(McpError::ConnectionFailed(
                "MCP server process is not running or has exited".into(),
            ));
        }

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
