//! LSP Server process manager.
//!
//! Manages external LSP server processes via stdio, handling
//! JSON-RPC message encoding/decoding with Content-Length headers.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Information about a running LSP server.
#[derive(Clone, Debug, serde::Serialize)]
pub struct LspServerInfo {
    pub id: String,
    pub server_path: String,
    pub root_path: String,
    pub status: String,
}

/// A single LSP server process handle.
struct LspProcess {
    server_path: String,
    root_path: String,
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    status: String,
}

/// Manages multiple LSP server processes.
#[derive(Default)]
pub struct LspManager {
    processes: Arc<Mutex<HashMap<String, LspProcess>>>,
    next_id: Arc<Mutex<u64>>,
}

impl LspManager {
    /// Create a new LspManager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start an LSP server process.
    pub async fn start(
        &self,
        server_path: String,
        args: Vec<String>,
        root_path: String,
    ) -> Result<String, String> {
        let mut child = tokio::process::Command::new(&server_path)
            .args(&args)
            .current_dir(&root_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn LSP server: {e}"))?;

        // Reap stderr in background to avoid blocking the pipe.
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let trimmed = line.trim_end();
                            if !trimmed.is_empty() {
                                info!(target: "lsp::stderr", "{trimmed}");
                            }
                            line.clear();
                        }
                        Err(e) => {
                            warn!("LSP stderr read error: {e}");
                            break;
                        }
                    }
                }
            });
        }

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture LSP stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture LSP stdout".to_string())?;

        // Spawn a background task to wait on the child process.
        let processes = self.processes.clone();
        let id = self.alloc_id().await;
        let id_clone = id.clone();
        tokio::spawn(async move {
            let status = child.wait().await;
            match status {
                Ok(code) => {
                    info!("LSP process {id_clone} exited with {code:?}");
                }
                Err(e) => {
                    warn!("LSP process {id_clone} wait error: {e}");
                }
            }
            // Mark as stopped in the map.
            let mut map = processes.lock().await;
            if let Some(proc) = map.get_mut(&id_clone) {
                proc.status = "stopped".to_string();
            }
        });

        let process = LspProcess {
            server_path: server_path.clone(),
            root_path: root_path.clone(),
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
            status: "running".to_string(),
        };

        self.processes.lock().await.insert(id.clone(), process);
        info!("LSP server started: id={id}, path={server_path}, root={root_path}");
        Ok(id)
    }

    /// Send a raw JSON-RPC message to the LSP server.
    pub async fn send(&self, process_id: String, message: String) -> Result<(), String> {
        let map = self.processes.lock().await;
        let proc = map
            .get(&process_id)
            .ok_or_else(|| format!("LSP process {process_id} not found"))?;

        if proc.status != "running" {
            return Err(format!("LSP process {process_id} is not running"));
        }

        let payload = encode_lsp_message(&message);
        let mut stdin = proc.stdin.lock().await;
        stdin
            .write_all(&payload)
            .await
            .map_err(|e| format!("Failed to write to LSP stdin: {e}"))?;
        stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush LSP stdin: {e}"))?;
        Ok(())
    }

    /// Read the next JSON-RPC message from the LSP server (non-blocking).
    pub async fn recv(&self, process_id: String) -> Result<Option<String>, String> {
        let map = self.processes.lock().await;
        let proc = map
            .get(&process_id)
            .ok_or_else(|| format!("LSP process {process_id} not found"))?;

        if proc.status != "running" {
            return Err(format!("LSP process {process_id} is not running"));
        }

        let mut stdout = proc.stdout.lock().await;
        read_lsp_message(&mut *stdout).await
    }

    /// Stop an LSP server process.
    pub async fn stop(&self, process_id: String) -> Result<(), String> {
        let mut map = self.processes.lock().await;
        let proc = map
            .get_mut(&process_id)
            .ok_or_else(|| format!("LSP process {process_id} not found"))?;

        if proc.status != "running" {
            return Ok(());
        }

        // Signal EOF by closing stdin; then mark stopped.
        let mut stdin = proc.stdin.lock().await;
        let _ = stdin.shutdown().await;
        drop(stdin);

        proc.status = "stopped".to_string();
        info!("LSP server stopped: id={process_id}");
        Ok(())
    }

    /// List all known LSP servers and their status.
    pub async fn list(&self) -> Vec<LspServerInfo> {
        let map = self.processes.lock().await;
        map.iter()
            .map(|(id, p)| LspServerInfo {
                id: id.clone(),
                server_path: p.server_path.clone(),
                root_path: p.root_path.clone(),
                status: p.status.clone(),
            })
            .collect()
    }

    /// Stop all running LSP servers. Should be called on app shutdown.
    pub async fn shutdown_all(&self) {
        let mut map = self.processes.lock().await;
        for (id, proc) in map.iter_mut() {
            if proc.status == "running" {
                let mut stdin = proc.stdin.lock().await;
                let _ = stdin.shutdown().await;
                proc.status = "stopped".to_string();
                info!("LSP server auto-stopped on shutdown: id={id}");
            }
        }
    }

    async fn alloc_id(&self) -> String {
        let mut id = self.next_id.lock().await;
        *id += 1;
        format!("lsp-{id}")
    }

}

/// Encode a JSON body into an LSP message with Content-Length header.
fn encode_lsp_message(json: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{json}", json.len()).into_bytes()
}

/// Read a single LSP message from the given buffered reader.
async fn read_lsp_message<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
) -> Result<Option<String>, String> {
    // Read the header line by line until we get an empty line.
    let mut content_length: Option<usize> = None;
    let mut header_buf = String::new();

    loop {
        header_buf.clear();
        let bytes_read = reader
            .read_line(&mut header_buf)
            .await
            .map_err(|e| format!("Failed to read LSP header: {e}"))?;
        if bytes_read == 0 {
            // EOF
            return Ok(None);
        }
        let line = header_buf.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(val) = line.strip_prefix("Content-Length: ") {
            content_length = Some(
                val.parse()
                    .map_err(|e| format!("Invalid Content-Length value: {e}"))?,
            );
        }
        // Other headers (e.g., Content-Type) are ignored for now.
    }

    let len = content_length.ok_or("Missing Content-Length header in LSP message")?;

    // Read exactly `len` bytes.
    let mut body_buf = vec![0u8; len];
    reader
        .read_exact(&mut body_buf)
        .await
        .map_err(|e| format!("Failed to read LSP body: {e}"))?;

    let body = String::from_utf8(body_buf)
        .map_err(|e| format!("LSP body is not valid UTF-8: {e}"))?;
    Ok(Some(body))
}
