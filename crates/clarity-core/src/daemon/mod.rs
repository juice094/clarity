//! Daemon runtime — cross-platform lockfile + signal handling
//!
//! Provides a lightweight daemon runtime for Clarity processes:
//! - PID lockfile to prevent duplicate instances
//! - Graceful shutdown via Ctrl+C / SIGTERM
//! - Process liveness detection (best-effort)
//!
//! # Example
//!
//! ```rust,no_run
//! use clarity_core::daemon::{DaemonLock, DaemonRuntime, DaemonError};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), DaemonError> {
//!     let lock = DaemonLock::new("clarity-gateway");
//!     lock.acquire()?;
//!
//!     let runtime = DaemonRuntime::new();
//!     runtime.run(async {
//!         // Your long-running task here
//!         tokio::time::sleep(std::time::Duration::from_secs(60)).await;
//!         Ok(())
//!     }).await
//! }
//! ```

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{info, warn};

/// Errors that can occur during daemon operations
#[derive(Debug, Error)]
pub enum DaemonError {
    /// Another instance is already running
    #[error("Daemon already running: {0}")]
    AlreadyRunning(String),
    /// Lockfile I/O error
    #[error("Lockfile error: {0}")]
    LockfileError(String),
    /// Signal handling error
    #[error("Signal error: {0}")]
    SignalError(String),
    /// The daemon was interrupted
    #[error("Daemon interrupted")]
    Interrupted,
}

/// PID lockfile manager for preventing duplicate daemon instances
///
/// Writes the current process ID and a timestamp to a lockfile.
/// On `acquire()`, checks whether another instance appears to be alive.
/// On `release()`, deletes the lockfile.
#[derive(Debug, Clone)]
pub struct DaemonLock {
    name: String,
    pid_file: PathBuf,
}

impl DaemonLock {
    /// Create a new lock manager for the given daemon name
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let pid_file = dirs::home_dir()
            .map(|p| {
                p.join(".clarity")
                    .join("daemon")
                    .join(format!("{}.pid", name))
            })
            .unwrap_or_else(|| PathBuf::from(format!("{}.pid", name)));
        Self { name, pid_file }
    }

    /// Attempt to acquire the lock
    ///
    /// Returns `Ok(())` if the lock was acquired.
    /// Returns `Err(DaemonError::AlreadyRunning)` if another instance
    /// appears to be alive.
    pub fn acquire(&self) -> Result<(), DaemonError> {
        if let Some(parent) = self.pid_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                DaemonError::LockfileError(format!("Failed to create daemon directory: {}", e))
            })?;
        }

        if self.pid_file.exists() {
            if let Some((pid, _timestamp)) = self.read_lockfile()? {
                if self.is_process_alive(pid) {
                    return Err(DaemonError::AlreadyRunning(format!(
                        "PID {} is still alive (lockfile: {})",
                        pid,
                        self.pid_file.display()
                    )));
                } else {
                    warn!(
                        "Stale lockfile detected for PID {}. Removing and re-acquiring.",
                        pid
                    );
                    let _ = std::fs::remove_file(&self.pid_file);
                }
            }
        }

        let pid = std::process::id();
        let timestamp = now_secs();
        let content = format!("{}\n{}\n", pid, timestamp);

        std::fs::write(&self.pid_file, content)
            .map_err(|e| DaemonError::LockfileError(format!("Failed to write lockfile: {}", e)))?;

        info!("Daemon lock acquired: {} (PID {})", self.name, pid);
        Ok(())
    }

    /// Release the lock by deleting the PID file
    pub fn release(&self) -> Result<(), DaemonError> {
        if self.pid_file.exists() {
            std::fs::remove_file(&self.pid_file).map_err(|e| {
                DaemonError::LockfileError(format!("Failed to remove lockfile: {}", e))
            })?;
            info!("Daemon lock released: {}", self.name);
        }
        Ok(())
    }

    /// Check whether this daemon currently holds the lock
    pub fn is_holding_lock(&self) -> bool {
        if let Ok(Some((pid, _))) = self.read_lockfile() {
            pid == std::process::id()
        } else {
            false
        }
    }

    /// Return the path to the PID file
    pub fn pid_file_path(&self) -> &Path {
        &self.pid_file
    }

    fn read_lockfile(&self) -> Result<Option<(u32, u64)>, DaemonError> {
        let content = match std::fs::read_to_string(&self.pid_file) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let mut lines = content.lines();
        let pid = lines.next().and_then(|s| s.parse::<u32>().ok());
        let timestamp = lines.next().and_then(|s| s.parse::<u64>().ok());

        match (pid, timestamp) {
            (Some(p), Some(t)) => Ok(Some((p, t))),
            _ => Ok(None),
        }
    }

    /// Best-effort process liveness check
    ///
    /// On Unix: sends signal 0 to the PID.
    /// On Windows: attempts to open the process (non-blocking).
    #[cfg(unix)]
    fn is_process_alive(&self, pid: u32) -> bool {
        use std::process::Command;
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    fn is_process_alive(&self, pid: u32) -> bool {
        use std::process::Command;
        Command::new("tasklist")
            .arg("/FI")
            .arg(format!("PID eq {}", pid))
            .arg("/FO")
            .arg("CSV")
            .arg("/NH")
            .output()
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains(&pid.to_string())
            })
            .unwrap_or(false)
    }

    #[cfg(not(any(unix, windows)))]
    fn is_process_alive(&self, _pid: u32) -> bool {
        // Conservative default: assume alive if lockfile exists
        true
    }
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        // Only release if we hold the lock
        if self.is_holding_lock() {
            let _ = self.release();
        }
    }
}

/// Daemon runtime with graceful shutdown support
///
/// Wraps a long-running async task and listens for shutdown signals.
/// When a signal is received, the task is given a chance to clean up.
#[derive(Clone)]
pub struct DaemonRuntime {
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl Default for DaemonRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl DaemonRuntime {
    /// Create a new daemon runtime
    pub fn new() -> Self {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::broadcast::channel(1);
        Self { shutdown_tx }
    }

    /// Run the given future until completion or shutdown signal
    ///
    /// # Arguments
    ///
    /// * `task` — The long-running async work to execute. Receives a shutdown receiver.
    /// * `timeout` — Maximum time to wait for graceful shutdown after signal
    pub async fn run<F, Fut>(&self, task: F, timeout: Option<Duration>) -> Result<(), DaemonError>
    where
        F: FnOnce(tokio::sync::broadcast::Receiver<()>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), DaemonError>> + Send + 'static,
    {
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let work_handle = tokio::spawn(task(shutdown_rx.resubscribe()));

        // Wait for either OS signal or internal shutdown broadcast
        let signal_received = tokio::select! {
            _ = Self::wait_for_shutdown_signal() => true,
            _ = shutdown_rx.recv() => true,
        };

        if signal_received {
            info!("Shutdown signal received, initiating graceful shutdown...");
            let _ = self.shutdown_tx.send(());

            let timeout = timeout.unwrap_or(Duration::from_secs(30));
            match tokio::time::timeout(timeout, work_handle).await {
                Ok(Ok(result)) => {
                    info!("Daemon task exited gracefully");
                    result
                }
                Ok(Err(join_err)) => {
                    warn!("Daemon task panicked: {}", join_err);
                    Err(DaemonError::SignalError(format!(
                        "Task panicked: {}",
                        join_err
                    )))
                }
                Err(_) => {
                    warn!("Graceful shutdown timed out after {:?}", timeout);
                    Err(DaemonError::SignalError("Shutdown timed out".to_string()))
                }
            }
        } else {
            // Should not reach here because both branches return true
            work_handle
                .await
                .map_err(|e| DaemonError::SignalError(format!("Task join error: {}", e)))?
        }
    }

    /// Trigger a manual shutdown
    pub fn shutdown(&self) -> Result<(), DaemonError> {
        self.shutdown_tx
            .send(())
            .map_err(|e| DaemonError::SignalError(format!("Failed to send shutdown: {}", e)))?;
        Ok(())
    }

    /// Wait for OS shutdown signal (Ctrl+C / SIGTERM)
    #[cfg(unix)]
    async fn wait_for_shutdown_signal() -> Result<(), DaemonError> {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|e| DaemonError::SignalError(format!("SIGTERM setup failed: {}", e)))?;
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .map_err(|e| DaemonError::SignalError(format!("SIGINT setup failed: {}", e)))?;

        tokio::select! {
            _ = sigterm.recv() => Ok(()),
            _ = sigint.recv() => Ok(()),
        }
    }

    #[cfg(windows)]
    async fn wait_for_shutdown_signal() -> Result<(), DaemonError> {
        tokio::signal::ctrl_c()
            .await
            .map_err(|e| DaemonError::SignalError(format!("Ctrl+C setup failed: {}", e)))
    }

    #[cfg(not(any(unix, windows)))]
    async fn wait_for_shutdown_signal() -> Result<(), DaemonError> {
        // Fallback: never return, run until completion
        std::future::pending().await
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_daemon_lock_acquire_and_release() {
        let lock = DaemonLock::new("test-daemon-1");
        // Clean up from previous runs
        let _ = lock.release();

        assert!(lock.acquire().is_ok());
        assert!(lock.pid_file_path().exists());
        assert!(lock.is_holding_lock());

        // Second acquire should fail
        let lock2 = DaemonLock::new("test-daemon-1");
        let result = lock2.acquire();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DaemonError::AlreadyRunning(_)
        ));

        // Release and re-acquire should succeed
        lock.release().unwrap();
        assert!(!lock.pid_file_path().exists());
        assert!(lock2.acquire().is_ok());
        lock2.release().unwrap();
    }

    #[test]
    fn test_daemon_lock_stale_cleanup() {
        let lock = DaemonLock::new("test-daemon-stale");
        let _ = lock.release();

        // Write a stale lockfile with a very old timestamp and non-existent PID
        if let Some(parent) = lock.pid_file_path().parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(lock.pid_file_path(), "99999\n1\n").unwrap();

        // Should detect stale and re-acquire
        assert!(lock.acquire().is_ok());
        assert!(lock.is_holding_lock());
        lock.release().unwrap();
    }

    #[tokio::test]
    async fn test_daemon_runtime_shutdown() {
        let runtime = DaemonRuntime::new();
        let runtime_clone = runtime.clone();

        let handle = tokio::spawn(async move {
            runtime
                .run(
                    |mut rx| async move {
                        tokio::select! {
                            _ = rx.recv() => {
                                info!("Received shutdown in task");
                                Ok(())
                            }
                            _ = tokio::time::sleep(Duration::from_secs(60)) => Ok(()),
                        }
                    },
                    Some(Duration::from_secs(5)),
                )
                .await
        });

        // Give runtime a moment to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Trigger shutdown
        runtime_clone.shutdown().unwrap();

        let result = tokio::time::timeout(Duration::from_secs(3), handle)
            .await
            .expect("Test timed out")
            .expect("Join error");

        assert!(result.is_ok());
    }
}
