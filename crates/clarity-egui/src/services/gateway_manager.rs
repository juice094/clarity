//! Gateway process lifecycle manager.
//!
//! Provides auto-start on egui launch and manual start/stop controls.
//! Gateway runs as an independent process — egui closing does not kill it.

use parking_lot::Mutex;
use std::process::{Child, Command, Stdio};

/// Manages the Gateway child process lifecycle.
pub struct GatewayManager {
    child: Mutex<Option<Child>>,
}

impl GatewayManager {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }

    /// Probe whether Gateway is already listening on 127.0.0.1:18790.
    pub fn is_running() -> bool {
        std::net::TcpStream::connect("127.0.0.1:18790").is_ok()
    }

    /// Start Gateway if it is not already running.
    ///
    /// Returns `Ok(true)` if we started it, `Ok(false)` if it was already up.
    pub fn start_if_needed(&self) -> Result<bool, String> {
        if Self::is_running() {
            tracing::info!("Gateway already running on 127.0.0.1:18790");
            return Ok(false);
        }

        let exe = find_gateway_binary()?;
        tracing::info!("Starting Gateway from {:?}", exe);

        let child = Command::new(&exe)
            .current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn Gateway: {}", e))?;

        *self.child.lock() = Some(child);

        // Wait up to 5s for port to come alive
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if Self::is_running() {
                tracing::info!("Gateway started successfully");
                return Ok(true);
            }
        }

        Err("Gateway did not start within 5 seconds".to_string())
    }

    /// Stop the Gateway process that we started.
    ///
    /// Returns `Ok(true)` if we stopped it, `Ok(false)` if we did not own it.
    #[allow(dead_code)]
    pub fn stop(&self) -> Result<bool, String> {
        let mut guard = self.child.lock();
        if let Some(mut child) = guard.take() {
            tracing::info!("Stopping Gateway (pid {:?})", child.id());
            let _ = child.kill();
            let _ = child.wait();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Default for GatewayManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Search for the Gateway binary in common build directories.
fn find_gateway_binary() -> Result<std::path::PathBuf, String> {
    let candidates = if cfg!(target_os = "windows") {
        vec![
            "target/release/clarity-gateway.exe",
            "target/debug/clarity-gateway.exe",
        ]
    } else {
        vec![
            "target/release/clarity-gateway",
            "target/debug/clarity-gateway",
        ]
    };

    for c in &candidates {
        let path = std::path::PathBuf::from(c);
        if path.exists() {
            return Ok(path);
        }
    }

    // Fallback: try CARGO_MANIFEST_DIR / workspace root
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let root = std::path::PathBuf::from(manifest);
        for c in &candidates {
            let path = root.parent().unwrap_or(&root).join(c);
            if path.exists() {
                return Ok(path);
            }
        }
    }

    Err("clarity-gateway binary not found in target/release or target/debug".to_string())
}
