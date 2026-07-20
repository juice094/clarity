//! Gateway watchdog — external health probe that triggers process restart.
//!
//! Closes the self-healing loop: `clarity-gateway::health::GatewayHealthMonitor`
//! writes a `gateway-restart-intent.json` file after N consecutive probe failures.
//! This watchdog polls the gateway health endpoint, reads the intent file when
//! present, and restarts the gateway process.
//!
//! Design follows production patterns from syncthing-rust's
//! `cmd/syncthing/src/tui/watchdog.rs` (AD-008 mobile network resilience).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{error, info, warn};

// ============================================================================
// GatewayWatchdog
// ============================================================================

/// Monitors gateway health via TCP probe and triggers restart on persistent failure.
///
/// # How it works
///
/// 1. Every `poll_interval` (default 30s), performs a TCP connect to the gateway's
///    health probe port.
/// 2. On failure, increments a consecutive failure counter.
/// 3. When failures exceed `failure_threshold` (default 3), reads the
///    `gateway-restart-intent.json` file written by the gateway's
///    `GatewayHealthMonitor`.
/// 4. Kills the old gateway process and spawns a replacement.
/// 5. On successful probe, resets the failure counter.
///
/// This watchdog runs as an **external** process (typically the `clarity-claw`
/// system-tray binary), decoupled from the gateway process it monitors.
pub struct GatewayWatchdog {
    /// Gateway health probe address (TCP).
    probe_addr: SocketAddr,
    /// Path where gateway writes restart intent on failure.
    intent_path: PathBuf,
    /// Polling interval between health probes.
    poll_interval: Duration,
    /// Consecutive failure threshold before triggering restart.
    failure_threshold: u32,
    /// Current consecutive failure count.
    consecutive_failures: AtomicU32,
    /// Command to restart the gateway.
    restart_command: Vec<String>,
}

impl GatewayWatchdog {
    /// Create a new watchdog.
    ///
    /// # Arguments
    ///
    /// * `probe_addr` — Gateway health probe TCP address (e.g., `127.0.0.1:18791`).
    /// * `intent_path` — Path to `gateway-restart-intent.json`.
    /// * `restart_command` — Shell command to restart the gateway (e.g.,
    ///   `["clarity-gateway", "--port", "18790"]`).
    pub fn new(
        probe_addr: SocketAddr,
        intent_path: impl Into<PathBuf>,
        restart_command: Vec<String>,
    ) -> Self {
        Self {
            probe_addr,
            intent_path: intent_path.into(),
            poll_interval: Duration::from_secs(30),
            failure_threshold: 3,
            consecutive_failures: AtomicU32::new(0),
            restart_command,
        }
    }

    /// Create a watchdog with default paths for a local development setup.
    ///
    /// - Probe port: 18791
    /// - Intent path: `~/.clarity/gateway-restart-intent.json`
    /// - Restart command: `["clarity-gateway"]`
    pub fn local_default() -> Self {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let intent_path = home.join(".clarity").join("gateway-restart-intent.json");
        Self::new(
            std::net::SocketAddr::from(([127, 0, 0, 1], 18791)),
            intent_path,
            vec!["clarity-gateway".to_string()],
        )
    }

    /// Set a custom polling interval.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Set a custom failure threshold.
    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Start the watchdog loop.
    ///
    /// Runs forever until the process is terminated. Spawns on the current
    /// Tokio runtime.
    pub async fn run(&self) {
        let mut ticker = interval(self.poll_interval);
        // Skip missed ticks after system sleep to avoid burst restart.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        info!(
            probe_addr = %self.probe_addr,
            poll_interval_secs = self.poll_interval.as_secs(),
            failure_threshold = self.failure_threshold,
            "Gateway watchdog started"
        );

        loop {
            ticker.tick().await;

            match self.probe().await {
                ProbeResult::Healthy => {
                    let prev = self.consecutive_failures.swap(0, Ordering::Relaxed);
                    if prev > 0 {
                        info!(previous_failures = prev, "Gateway health restored");
                    }
                }
                ProbeResult::Unhealthy => {
                    let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
                    warn!(
                        consecutive_failures = failures,
                        threshold = self.failure_threshold,
                        "Gateway health probe failed"
                    );

                    if failures >= self.failure_threshold {
                        self.handle_persistent_failure(failures).await;
                        // Reset after handling — if restart failed, we'll count up again.
                        self.consecutive_failures.store(0, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    /// Perform a single TCP health probe.
    async fn probe(&self) -> ProbeResult {
        match tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(self.probe_addr))
            .await
        {
            Ok(Ok(_)) => ProbeResult::Healthy,
            Ok(Err(_)) | Err(_) => ProbeResult::Unhealthy,
        }
    }

    /// Handle persistent failure: read intent, kill old process, spawn replacement.
    async fn handle_persistent_failure(&self, failures: u32) {
        error!(
            consecutive_failures = failures,
            "Gateway is unresponsive — triggering restart"
        );

        // Read the restart intent for diagnostic context.
        if let Ok(content) = std::fs::read_to_string(&self.intent_path) {
            if let Ok(intent) = serde_json::from_str::<serde_json::Value>(&content) {
                warn!(
                    reason = %intent.get("reason").and_then(|v| v.as_str()).unwrap_or("unknown"),
                    timestamp = %intent.get("timestamp").and_then(|v| v.as_str()).unwrap_or("unknown"),
                    "Gateway restart intent detected"
                );
            }
            // Clean up the intent file after reading.
            let _ = std::fs::remove_file(&self.intent_path);
        }

        // Spawn a new gateway process.
        self.restart_gateway();
    }

    /// Spawn a new gateway process as a detached child.
    fn restart_gateway(&self) {
        if self.restart_command.is_empty() {
            error!("No restart command configured — cannot restart gateway");
            return;
        }

        let program = &self.restart_command[0];
        let args = &self.restart_command[1..];

        match std::process::Command::new(program)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => {
                info!(
                    pid = child.id(),
                    program = %program,
                    "Gateway restarted successfully"
                );
            }
            Err(e) => {
                error!(
                    error = %e,
                    program = %program,
                    "Failed to restart gateway"
                );
            }
        }
    }
}

// ============================================================================
// ProbeResult
// ============================================================================

/// Result of a single health probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeResult {
    /// Probe succeeded (TCP connection accepted).
    Healthy,
    /// Probe failed (connection refused or timed out).
    Unhealthy,
}

// ============================================================================
// Quick probe helper (for one-shot checks)
// ============================================================================

/// Quick one-shot probe to a gateway's health port.
///
/// Returns `true` if the gateway responded, `false` otherwise.
///
/// # Example
///
/// ```rust,no_run
/// use clarity_claw::watchdog::quick_probe;
///
/// # async fn example() {
/// let addr = "127.0.0.1:18791".parse().unwrap();
/// let healthy = quick_probe(addr).await;
/// # }
/// ```
pub async fn quick_probe(addr: SocketAddr) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(addr)).await,
        Ok(Ok(_))
    )
}
