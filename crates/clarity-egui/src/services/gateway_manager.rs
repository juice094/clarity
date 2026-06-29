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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_manager_default_has_no_child() {
        let gm = GatewayManager::default();
        // Default state: no child process.
        assert!(gm.child.lock().is_none());
    }

    #[test]
    fn gateway_manager_new_consistent_with_default() {
        let gm1 = GatewayManager::new();
        let gm2 = GatewayManager::default();
        assert!(gm1.child.lock().is_none());
        assert!(gm2.child.lock().is_none());
    }

    #[test]
    fn stop_without_child_returns_false() {
        let gm = GatewayManager::new();
        let result = gm.stop().unwrap();
        assert!(!result, "stop() without a child should return Ok(false)");
    }

    #[test]
    fn find_gateway_binary_returns_err_for_nonexistent_path() {
        // Verify the search function handles missing binaries gracefully.
        // In test environments, clarity-gateway may or may not exist.
        let result = find_gateway_binary();
        // The function should not panic — it should return either Ok or Err.
        match result {
            Ok(path) => {
                // If found, it must point to a real file.
                assert!(
                    path.exists(),
                    "find_gateway_binary returned a path that doesn't exist: {:?}",
                    path
                );
            }
            Err(e) => {
                assert!(
                    e.contains("not found"),
                    "Error message should mention binary not found, got: {}",
                    e
                );
            }
        }
    }

    #[test]
    fn is_running_does_not_panic() {
        // is_running is a TCP probe — it may or may not connect, but must not panic.
        let _ = GatewayManager::is_running();
    }
}
