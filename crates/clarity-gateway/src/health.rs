//! Gateway health monitor — external TCP probe for watchdog integration.
//!
//! Inspired by Daimon's gateway watchdog (`~/.kimi_openclaw/logs/gateway-watchdog.jsonl`):
//! an external process (e.g. `clarity-claw` system tray) probes the gateway
//! via a lightweight TCP port. After N consecutive failures, a restart intent
//! is persisted so the watchdog can restart the gateway without coupling to
//! the HTTP/WebSocket protocol stack.
//!
//! # Design
//!
//! ```text
//! Watchdog (clarity-claw)          Gateway (clarity-gateway)
//!        │                                │
//!        │  TCP connect probe_port        │
//!        ├───────────────────────────────▶│
//!        │          ACK / RST             │
//!        │◄───────────────────────────────┤
//!        │                                │
//!        │  (3 failures)                  │
//!        │  write gateway-restart-intent.json
//!        │                                │
//! ```
//!
//! The probe port is **separate** from the main HTTP port so that a hung
//! Axum/Tower stack does not falsely report healthy.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

// ============================================================================
// GatewayHealthMonitor
// ============================================================================

/// Monitors gateway health via an independent TCP probe port.
pub struct GatewayHealthMonitor {
    /// Address to bind the probe listener.
    bind_addr: SocketAddr,
    /// Path where restart intent is written on failure.
    restart_intent_path: PathBuf,
    /// Consecutive failure threshold before writing restart intent.
    failure_threshold: u32,
    /// Current consecutive failure count.
    consecutive_failures: AtomicU32,
    /// Whether the monitor is currently running.
    running: AtomicU32,
}

/// Health status returned by a probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Probe succeeded.
    Healthy,
    /// Probe failed (connection refused / timeout).
    Unhealthy,
    /// Circuit breaker is open — too many consecutive failures.
    CircuitOpen,
}

/// Persisted restart intent file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RestartIntent {
    /// When the intent was written.
    pub timestamp: String,
    /// Number of consecutive failures observed.
    pub consecutive_failures: u32,
    /// Human-readable reason.
    pub reason: String,
}

impl GatewayHealthMonitor {
    /// Create a new health monitor.
    ///
    /// # Arguments
    ///
    /// * `probe_port` — TCP port for the standalone probe listener.
    /// * `restart_intent_path` — Where to write `gateway-restart-intent.json`.
    /// * `failure_threshold` — Failures before triggering restart intent.
    pub fn new(
        probe_port: u16,
        restart_intent_path: impl Into<PathBuf>,
        failure_threshold: u32,
    ) -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], probe_port)),
            restart_intent_path: restart_intent_path.into(),
            failure_threshold,
            consecutive_failures: AtomicU32::new(0),
            running: AtomicU32::new(0),
        }
    }

    /// Convenience constructor with defaults.
    pub fn default_with_probe_port(probe_port: u16) -> Self {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let intent_path = home.join(".clarity").join("gateway-restart-intent.json");
        Self::new(probe_port, intent_path, 3)
    }

    /// Start the TCP probe listener.
    ///
    /// This spawns a background Tokio task that accepts TCP connections
    /// and immediately closes them (zero-protocol health check).
    pub async fn start(self: Arc<Self>) -> std::io::Result<()> {
        if self
            .running
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            warn!("GatewayHealthMonitor already running");
            return Ok(());
        }

        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("Gateway health probe listening on {}", self.bind_addr);

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        // Reset failures on any successful connection.
                        self.consecutive_failures.store(0, Ordering::Relaxed);
                        info!("Health probe connected from {}", addr);
                        // Immediately close — the connect itself is the health signal.
                        drop(stream);
                    }
                    Err(e) => {
                        error!("Health probe accept error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Perform a single external probe (for watchdog use).
    ///
    /// Connects to the probe port, returns immediately.
    pub async fn probe(&self) -> HealthStatus {
        if self.is_circuit_open() {
            return HealthStatus::CircuitOpen;
        }

        match TcpStream::connect(self.bind_addr).await {
            Ok(_) => {
                self.consecutive_failures.store(0, Ordering::Relaxed);
                HealthStatus::Healthy
            }
            Err(_) => {
                let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
                warn!(
                    "Health probe failed ({} / {} consecutive)",
                    failures, self.failure_threshold
                );

                if failures >= self.failure_threshold {
                    self.write_restart_intent(failures);
                }

                HealthStatus::Unhealthy
            }
        }
    }

    /// Check if the circuit breaker is open.
    pub fn is_circuit_open(&self) -> bool {
        self.consecutive_failures.load(Ordering::Relaxed) >= self.failure_threshold
    }

    /// Reset the failure counter (e.g. after a successful restart).
    pub fn reset(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        // Remove any stale restart intent.
        let _ = std::fs::remove_file(&self.restart_intent_path);
    }

    fn write_restart_intent(&self, failures: u32) {
        let intent = RestartIntent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            consecutive_failures: failures,
            reason: format!("Gateway health probe failed {} consecutive times", failures),
        };

        match serde_json::to_string_pretty(&intent) {
            Ok(json) => {
                if let Some(parent) = self.restart_intent_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&self.restart_intent_path, json) {
                    Ok(()) => {
                        error!("Wrote restart intent to {:?}", self.restart_intent_path);
                    }
                    Err(e) => {
                        error!("Failed to write restart intent: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to serialize restart intent: {}", e);
            }
        }
    }
}

// ============================================================================
// Standalone probe helper (for external watchdogs)
// ============================================================================

/// Quick one-shot probe to a gateway's health port.
///
/// Returns `true` if the gateway responded, `false` otherwise.
///
/// # Example
///
/// ```rust,no_run
/// use clarity_gateway::health::quick_probe;
///
/// # async fn example() {
/// let addr = "127.0.0.1:18791".parse().unwrap();
/// let healthy = quick_probe(addr).await;
/// # }
/// ```
pub async fn quick_probe(addr: std::net::SocketAddr) -> bool {
    matches!(
        tokio::time::timeout(std::time::Duration::from_secs(2), TcpStream::connect(addr)).await,
        Ok(Ok(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_monitor_probe_success() {
        // Start the listener on an ephemeral port.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Update the monitor's bind address.
        let _monitor = Arc::new(GatewayHealthMonitor {
            bind_addr: addr,
            restart_intent_path: std::env::temp_dir().join("clarity-test-restart-intent.json"),
            failure_threshold: 3,
            consecutive_failures: AtomicU32::new(0),
            running: AtomicU32::new(1),
        });

        // Spawn a dummy acceptor.
        let m = Arc::clone(&_monitor);
        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    m.consecutive_failures.store(0, Ordering::Relaxed);
                    drop(stream);
                }
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let status = _monitor.probe().await;
        assert_eq!(status, HealthStatus::Healthy);
        assert_eq!(_monitor.consecutive_failures.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_health_monitor_probe_failure_triggers_intent() {
        let intent_path = std::env::temp_dir().join("clarity-test-restart-intent-fail.json");
        let _ = std::fs::remove_file(&intent_path);

        // Bind to a port but immediately drop the listener so connects fail.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let monitor = Arc::new(GatewayHealthMonitor {
            bind_addr: addr,
            restart_intent_path: intent_path.clone(),
            failure_threshold: 2,
            consecutive_failures: AtomicU32::new(0),
            running: AtomicU32::new(1),
        });

        // First failure.
        let s1 = monitor.probe().await;
        assert_eq!(s1, HealthStatus::Unhealthy);

        // Second failure should trigger intent.
        let s2 = monitor.probe().await;
        assert_eq!(s2, HealthStatus::Unhealthy);

        assert!(intent_path.exists());
        let content = std::fs::read_to_string(&intent_path).unwrap();
        let intent: RestartIntent = serde_json::from_str(&content).unwrap();
        assert_eq!(intent.consecutive_failures, 2);

        let _ = std::fs::remove_file(&intent_path);
    }

    #[test]
    fn test_restart_intent_serde() {
        let intent = RestartIntent {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            consecutive_failures: 3,
            reason: "test".to_string(),
        };
        let json = serde_json::to_string(&intent).unwrap();
        let restored: RestartIntent = serde_json::from_str(&json).unwrap();
        assert_eq!(intent, restored);
    }
}
