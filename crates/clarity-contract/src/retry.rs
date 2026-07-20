//! Retry and connection reliability types.
//!
//! This module provides shared types for retry configuration, exponential backoff,
//! and connection state management. These types are used by connection managers
//! (`clarity-claw`), task supervisors (`clarity-core`), and any component that
//! needs structured retry/backoff semantics.
//!
//! Design follows production patterns from syncthing-rust's
//! `syncthing-core/src/types/connection.rs`.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// ConnectionState
// ============================================================================

/// Connection lifecycle state machine.
///
/// Transitions follow a linear progression with error/disconnect as terminal states:
/// ```text
/// Initial → Connecting → Connected → TlsHandshakeComplete
///   → ProtocolHandshakeComplete → ClusterConfigComplete
///   (→ Disconnecting → Disconnected)
///   (→ Error at any stage)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConnectionState {
    /// Initial state before any connection attempt.
    #[default]
    Initial,
    /// Connection attempt in progress.
    Connecting,
    /// TCP/TLS connection established.
    Connected,
    /// TLS handshake completed successfully.
    TlsHandshakeComplete,
    /// Protocol-level handshake completed (Hello exchange).
    ProtocolHandshakeComplete,
    /// Cluster/device configuration exchange completed.
    ClusterConfigComplete,
    /// Graceful disconnect in progress.
    Disconnecting,
    /// Connection terminated.
    Disconnected,
    /// Terminal error state.
    Error,
}

impl ConnectionState {
    /// Whether the connection is in an active state (can carry data).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            ConnectionState::Connected
                | ConnectionState::TlsHandshakeComplete
                | ConnectionState::ProtocolHandshakeComplete
                | ConnectionState::ClusterConfigComplete
        )
    }

    /// Whether messages can be sent over this connection.
    pub fn can_send(&self) -> bool {
        matches!(
            self,
            ConnectionState::ProtocolHandshakeComplete | ConnectionState::ClusterConfigComplete
        )
    }

    /// Whether the connection has reached a terminal state.
    pub fn is_terminated(&self) -> bool {
        matches!(self, ConnectionState::Disconnected | ConnectionState::Error)
    }
}

// ============================================================================
// AddressType
// ============================================================================

/// Network address with transport scheme.
///
/// Serialized as human-readable URL strings:
/// - `tcp://host:port`
/// - `quic://host:port`
/// - `relay://url`
/// - `ws://host:port` (WebSocket)
/// - `wss://host:port` (Secure WebSocket)
/// - `dynamic` (discovered address)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AddressType {
    /// TCP address.
    Tcp(String),
    /// QUIC address.
    Quic(String),
    /// Relay/proxy address.
    Relay(String),
    /// WebSocket address.
    WebSocket(String),
    /// Secure WebSocket address.
    SecureWebSocket(String),
    /// Dynamically discovered address.
    Dynamic,
}

impl AddressType {
    /// Get the address string without the scheme prefix.
    pub fn as_str(&self) -> &str {
        match self {
            AddressType::Tcp(addr) => addr,
            AddressType::Quic(addr) => addr,
            AddressType::Relay(addr) => addr,
            AddressType::WebSocket(addr) => addr,
            AddressType::SecureWebSocket(addr) => addr,
            AddressType::Dynamic => "dynamic",
        }
    }
}

impl fmt::Display for AddressType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddressType::Tcp(addr) => write!(f, "tcp://{}", addr),
            AddressType::Quic(addr) => write!(f, "quic://{}", addr),
            AddressType::Relay(url) => write!(f, "relay://{}", url),
            AddressType::WebSocket(addr) => write!(f, "ws://{}", addr),
            AddressType::SecureWebSocket(addr) => write!(f, "wss://{}", addr),
            AddressType::Dynamic => write!(f, "dynamic"),
        }
    }
}

impl Serialize for AddressType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for AddressType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AddressTypeVisitor;

        impl<'de> serde::de::Visitor<'de> for AddressTypeVisitor {
            type Value = AddressType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    formatter,
                    "a URL string (e.g. \"tcp://host:port\") or an object like {{\"Tcp\": \"host:port\"}}"
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "Dynamic" | "dynamic" => Ok(AddressType::Dynamic),
                    _ => parse_address_str(value).map_err(serde::de::Error::custom),
                }
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let key: String = map
                    .next_key()?
                    .ok_or_else(|| serde::de::Error::custom("empty address object"))?;
                match key.as_str() {
                    "Tcp" => {
                        let value: String = map.next_value()?;
                        Ok(AddressType::Tcp(value))
                    }
                    "Quic" => {
                        let value: String = map.next_value()?;
                        Ok(AddressType::Quic(value))
                    }
                    "Relay" => {
                        let value: String = map.next_value()?;
                        Ok(AddressType::Relay(value))
                    }
                    "WebSocket" => {
                        let value: String = map.next_value()?;
                        Ok(AddressType::WebSocket(value))
                    }
                    "SecureWebSocket" => {
                        let value: String = map.next_value()?;
                        Ok(AddressType::SecureWebSocket(value))
                    }
                    "Dynamic" => {
                        let (): () = map.next_value()?;
                        Ok(AddressType::Dynamic)
                    }
                    _ => Err(serde::de::Error::custom(format!(
                        "unknown address type variant: {}",
                        key
                    ))),
                }
            }
        }

        deserializer.deserialize_any(AddressTypeVisitor)
    }
}

/// Parse a URL-style address string into an `AddressType`.
fn parse_address_str(s: &str) -> Result<AddressType, String> {
    if let Some(addr) = s.strip_prefix("tcp://") {
        Ok(AddressType::Tcp(addr.to_string()))
    } else if let Some(addr) = s.strip_prefix("quic://") {
        Ok(AddressType::Quic(addr.to_string()))
    } else if let Some(url) = s.strip_prefix("relay://") {
        Ok(AddressType::Relay(url.to_string()))
    } else if let Some(addr) = s.strip_prefix("ws://") {
        Ok(AddressType::WebSocket(addr.to_string()))
    } else if let Some(addr) = s.strip_prefix("wss://") {
        Ok(AddressType::SecureWebSocket(addr.to_string()))
    } else {
        Err(format!(
            "Invalid address format '{}'. Expected one of: tcp://host:port, quic://host:port, \
             relay://url, ws://host:port, wss://host:port, dynamic",
            s
        ))
    }
}

// ============================================================================
// ConnectionStats
// ============================================================================

/// Per-connection statistics.
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// When the connection was established.
    pub connected_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Timestamp of last activity (send or receive).
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
    /// Total bytes sent.
    pub bytes_sent: u64,
    /// Total bytes received.
    pub bytes_received: u64,
    /// Total messages sent.
    pub messages_sent: u64,
    /// Total messages received.
    pub messages_received: u64,
    /// Number of reconnection attempts.
    pub retry_count: u32,
}

// ============================================================================
// ConnectionPriority
// ============================================================================

/// Priority level for connections.
///
/// Higher-priority connections get preferential scheduling:
/// - Faster heartbeat intervals
/// - Halved reconnection jitter
/// - Preferred during connection competition
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ConnectionPriority {
    /// Lowest priority (background sync, archival).
    Lowest = 0,
    /// Low priority.
    Low = 1,
    /// Normal priority (default).
    #[default]
    Normal = 2,
    /// High priority (active work).
    High = 3,
    /// Highest priority (critical control channel).
    Highest = 4,
}

// ============================================================================
// ConnectionType
// ============================================================================

/// Direction of a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionType {
    /// Connection initiated by the remote peer.
    Incoming,
    /// Connection initiated locally (dialed).
    Outgoing,
}

// ============================================================================
// RetryConfig
// ============================================================================

/// Configuration for retry with exponential backoff and jitter.
///
/// Backoff formula: `initial_backoff_ms * multiplier^attempt`, capped at
/// `max_backoff_ms`, then jittered by ±25%.
///
/// # Example
///
/// ```rust
/// use clarity_contract::retry::RetryConfig;
/// use std::time::Duration;
///
/// let config = RetryConfig::default();
/// // Attempt 0: ~1s (±250ms jitter)
/// // Attempt 1: ~2s
/// // Attempt 2: ~4s
/// // ...
/// // Attempt 10+: capped at 5min
/// let delay = config.backoff_duration(3);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts before giving up.
    pub max_retries: u32,
    /// Initial backoff duration in milliseconds.
    pub initial_backoff_ms: u64,
    /// Maximum backoff duration in milliseconds (hard cap).
    pub max_backoff_ms: u64,
    /// Backoff multiplier per attempt (typically 2.0 for exponential).
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 10,
            initial_backoff_ms: 1000,
            max_backoff_ms: 300_000, // 5 minutes
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Create a conservative retry config for low-resource or high-latency environments.
    pub fn conservative() -> Self {
        Self {
            max_retries: 5,
            initial_backoff_ms: 2_000,
            max_backoff_ms: 600_000, // 10 minutes
            backoff_multiplier: 2.0,
        }
    }

    /// Create an aggressive retry config for fast-recovery scenarios.
    pub fn aggressive() -> Self {
        Self {
            max_retries: 20,
            initial_backoff_ms: 200,
            max_backoff_ms: 30_000, // 30 seconds
            backoff_multiplier: 1.5,
        }
    }

    /// Compute the backoff duration for the given attempt number (0-based).
    ///
    /// Includes ±25% random jitter to prevent thundering herd.
    pub fn backoff_duration(&self, attempt: u32) -> std::time::Duration {
        if attempt == 0 {
            let jittered = self.jitter(self.initial_backoff_ms as f64);
            return std::time::Duration::from_millis(jittered.max(100));
        }

        let multiplier = self.backoff_multiplier.powi(attempt as i32);
        let backoff_ms = (self.initial_backoff_ms as f64 * multiplier) as u64;
        let backoff_ms = backoff_ms.min(self.max_backoff_ms);

        let jittered = self.jitter(backoff_ms as f64);
        std::time::Duration::from_millis(jittered.max(100))
    }

    /// Returns true if the given attempt exceeds the max retry limit.
    pub fn is_exhausted(&self, attempt: u32) -> bool {
        attempt >= self.max_retries
    }

    /// Apply ±25% random jitter to a duration value.
    fn jitter(&self, ms: f64) -> u64 {
        // SAFE: uses deterministic fallback for wasm targets where RNG may not be available
        #[cfg(not(target_arch = "wasm32"))]
        {
            let jitter_factor = rand::random::<f64>() * 0.5 - 0.25; // -0.25 .. +0.25
            (ms * (1.0 + jitter_factor)) as u64
        }
        #[cfg(target_arch = "wasm32")]
        {
            // Fallback: no jitter on WASM (no rand::random)
            ms as u64
        }
    }
}

// ============================================================================
// HeartbeatConfig
// ============================================================================

/// Transport-aware heartbeat configuration.
///
/// Different transport types have different idle-timeout characteristics:
/// - TCP direct connections can tolerate longer intervals.
/// - Relay/proxy paths typically have ~90s idle timeouts (e.g., Tailscale DERP).
/// - WebSocket connections may have intermediate timeouts.
/// - In-memory connections have no timeout but benefit from frequent liveness checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    /// Base heartbeat interval (used for direct TCP connections).
    pub interval_secs: u64,
    /// Heartbeat interval for relay/proxy paths (typically capped at 30s).
    pub relay_interval_secs: u64,
    /// Heartbeat interval for WebSocket connections.
    pub websocket_interval_secs: u64,
    /// Heartbeat interval for in-memory/local connections.
    pub memory_interval_secs: u64,
    /// Maximum consecutive heartbeat timeouts before declaring dead.
    pub max_missed_heartbeats: u32,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval_secs: 60,
            relay_interval_secs: 30,
            websocket_interval_secs: 45,
            memory_interval_secs: 10,
            max_missed_heartbeats: 3,
        }
    }
}

impl HeartbeatConfig {
    /// Select the appropriate heartbeat interval for the given address type.
    pub fn interval_for(&self, addr_type: &AddressType) -> std::time::Duration {
        let secs = match addr_type {
            AddressType::Relay(_) => self.relay_interval_secs.min(self.interval_secs),
            AddressType::WebSocket(_) | AddressType::SecureWebSocket(_) => {
                self.websocket_interval_secs.min(self.interval_secs)
            }
            AddressType::Dynamic => self.relay_interval_secs.min(self.interval_secs),
            AddressType::Tcp(_) | AddressType::Quic(_) => self.interval_secs,
        };
        std::time::Duration::from_secs(secs)
    }

    /// Heartbeat interval for in-memory connections.
    pub fn memory_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.memory_interval_secs.min(self.interval_secs))
    }

    /// Maximum time to wait without a heartbeat response before declaring dead.
    pub fn dead_after(&self, addr_type: &AddressType) -> std::time::Duration {
        self.interval_for(addr_type) * self.max_missed_heartbeats
    }
}

// ============================================================================
// Supervisor types — task supervision with restart policy and backoff
// ============================================================================

/// Restart policy for a supervised task.
///
/// Controls whether a task is automatically restarted after it completes or fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    /// Always restart the task when it finishes, regardless of result.
    Always,
    /// Only restart when the task returns an error or panics.
    OnFailure,
    /// Never restart the task.
    Never,
}

/// Exponential backoff configuration for task restarts.
///
/// Unlike `RetryConfig`, this computes backoff deterministically without jitter
/// (jitter is handled at the caller level or in `RetryConfig`).
/// The formula: `initial_delay * 2^attempt`, capped at `max_delay`.
#[derive(Debug, Clone, Copy)]
pub struct ExponentialBackoff {
    /// Initial delay before the first restart.
    pub initial_delay: std::time::Duration,
    /// Maximum delay between restarts.
    pub max_delay: std::time::Duration,
    /// Duration after which the restart counter resets to zero.
    pub reset_after: std::time::Duration,
}

impl ExponentialBackoff {
    /// Compute the next backoff delay for the given attempt number (0-based).
    ///
    /// Returns `initial_delay * 2^attempt` capped at `max_delay`.
    pub fn next_delay(&self, attempt: u32) -> std::time::Duration {
        let multiplier = 2u32.saturating_pow(attempt.min(31));
        self.initial_delay
            .saturating_mul(multiplier)
            .min(self.max_delay)
    }
}

/// Configuration controlling how a supervised task is restarted.
#[derive(Debug, Clone)]
pub struct RestartConfig {
    /// When to restart the task.
    pub restart_policy: RestartPolicy,
    /// Backoff parameters for restart delays.
    pub backoff: ExponentialBackoff,
    /// Maximum number of restarts allowed within the `backoff.reset_after` window.
    /// Once exceeded, the task is considered permanently failed.
    pub max_restarts: u32,
}

impl Default for RestartConfig {
    fn default() -> Self {
        Self {
            restart_policy: RestartPolicy::OnFailure,
            backoff: ExponentialBackoff {
                initial_delay: std::time::Duration::from_millis(100),
                max_delay: std::time::Duration::from_secs(60),
                reset_after: std::time::Duration::from_secs(60),
            },
            max_restarts: 5,
        }
    }
}

// ============================================================================
// ConnectionMetrics — per-connection atomic counters for hot-path monitoring
// ============================================================================

/// Per-connection atomic counters for hot-path throughput monitoring.
///
/// All counters use `AtomicU64` with `Relaxed` ordering for minimal overhead
/// on every send/receive operation. Aggregated snapshots can be reported
/// through `clarity-telemetry` at a lower frequency.
///
/// Design follows production patterns from syncthing-rust's
/// `syncthing-net/src/session/mod.rs` (`BepSessionMetrics`).
#[derive(Debug, Default)]
pub struct ConnectionMetrics {
    /// Total bytes sent over this connection.
    pub bytes_sent: AtomicU64,
    /// Total bytes received over this connection.
    pub bytes_received: AtomicU64,
    /// Total messages sent.
    pub messages_sent: AtomicU64,
    /// Total messages received.
    pub messages_received: AtomicU64,
    /// Number of reconnection attempts.
    pub reconnects: AtomicU64,
    /// Number of heartbeat timeouts detected.
    pub heartbeat_timeouts: AtomicU64,
    /// Number of errors encountered (any kind).
    pub errors: AtomicU64,
    /// Number of successful probe handshakes.
    pub successful_probes: AtomicU64,
}

impl ConnectionMetrics {
    /// Take a point-in-time snapshot of all counters.
    ///
    /// Uses `Relaxed` ordering — values across different counters may not
    /// be perfectly consistent with each other, but each individual value
    /// is accurate at the time of the load.
    pub fn snapshot(&self) -> ConnectionMetricsSnapshot {
        ConnectionMetricsSnapshot {
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            messages_sent: self.messages_sent.load(Ordering::Relaxed),
            messages_received: self.messages_received.load(Ordering::Relaxed),
            reconnects: self.reconnects.load(Ordering::Relaxed),
            heartbeat_timeouts: self.heartbeat_timeouts.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            successful_probes: self.successful_probes.load(Ordering::Relaxed),
        }
    }
}

/// A point-in-time snapshot of [`ConnectionMetrics`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionMetricsSnapshot {
    /// Total bytes sent.
    pub bytes_sent: u64,
    /// Total bytes received.
    pub bytes_received: u64,
    /// Total messages sent.
    pub messages_sent: u64,
    /// Total messages received.
    pub messages_received: u64,
    /// Number of reconnection attempts.
    pub reconnects: u64,
    /// Number of heartbeat timeouts detected.
    pub heartbeat_timeouts: u64,
    /// Number of errors encountered.
    pub errors: u64,
    /// Number of successful probe handshakes.
    pub successful_probes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // RetryConfig tests
    // ==========================================================================

    #[test]
    fn test_retry_config_default() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_retries, 10);
        assert_eq!(cfg.initial_backoff_ms, 1000);
        assert_eq!(cfg.max_backoff_ms, 300_000);
        assert_eq!(cfg.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_retry_config_conservative() {
        let cfg = RetryConfig::conservative();
        assert_eq!(cfg.max_retries, 5);
        assert_eq!(cfg.initial_backoff_ms, 2000);
        assert_eq!(cfg.max_backoff_ms, 600_000);
    }

    #[test]
    fn test_retry_config_aggressive() {
        let cfg = RetryConfig::aggressive();
        assert_eq!(cfg.max_retries, 20);
        assert_eq!(cfg.initial_backoff_ms, 200);
        assert_eq!(cfg.max_backoff_ms, 30_000);
    }

    #[test]
    fn test_backoff_duration_increases() {
        let cfg = RetryConfig::default();
        let d0 = cfg.backoff_duration(0).as_millis();
        let d1 = cfg.backoff_duration(1).as_millis();
        let d2 = cfg.backoff_duration(2).as_millis();

        // With jitter, d2 should generally be >= d1, but due to jitter this
        // isn't guaranteed. We verify that the maximum possible values increase.
        // Without jitter: d0≈1000, d1≈2000, d2≈4000.
        // Minimum (after jitter): d0≥75, d1≥150, d2≥300.
        assert!(d0 >= 75, "d0={} too small", d0); // 1000 * 0.75 = 750, min capped at 100
        assert!(d1 >= 150, "d1={} too small", d1); // 2000 * 0.75 = 1500, min capped at 100
        assert!(d2 >= 300, "d2={} too small", d2); // 4000 * 0.75 = 3000, min capped at 100
    }

    #[test]
    fn test_backoff_duration_capped_at_max() {
        let cfg = RetryConfig {
            max_retries: 10,
            initial_backoff_ms: 1000,
            max_backoff_ms: 5000,
            backoff_multiplier: 2.0,
        };
        // Attempt 10: 1000 * 2^10 = 1,024,000 ms, but cap is 5000.
        let d = cfg.backoff_duration(10).as_millis();
        // Max with jitter: 5000 * 1.25 = 6250
        assert!(d <= 6250, "backoff {} exceeds cap with jitter", d);
    }

    #[test]
    fn test_is_exhausted() {
        let cfg = RetryConfig::default(); // max_retries = 10
        assert!(!cfg.is_exhausted(0));
        assert!(!cfg.is_exhausted(9));
        assert!(cfg.is_exhausted(10));
        assert!(cfg.is_exhausted(11));
    }

    // ==========================================================================
    // ConnectionState tests
    // ==========================================================================

    #[test]
    fn test_connection_state_is_active() {
        assert!(!ConnectionState::Initial.is_active());
        assert!(!ConnectionState::Connecting.is_active());
        assert!(ConnectionState::Connected.is_active());
        assert!(ConnectionState::TlsHandshakeComplete.is_active());
        assert!(ConnectionState::ProtocolHandshakeComplete.is_active());
        assert!(ConnectionState::ClusterConfigComplete.is_active());
        assert!(!ConnectionState::Disconnecting.is_active());
        assert!(!ConnectionState::Disconnected.is_active());
        assert!(!ConnectionState::Error.is_active());
    }

    #[test]
    fn test_connection_state_can_send() {
        assert!(!ConnectionState::Connected.can_send());
        assert!(ConnectionState::ProtocolHandshakeComplete.can_send());
        assert!(ConnectionState::ClusterConfigComplete.can_send());
        assert!(!ConnectionState::Disconnected.can_send());
    }

    #[test]
    fn test_connection_state_is_terminated() {
        assert!(!ConnectionState::Initial.is_terminated());
        assert!(!ConnectionState::Connected.is_terminated());
        assert!(ConnectionState::Disconnected.is_terminated());
        assert!(ConnectionState::Error.is_terminated());
    }

    #[test]
    fn test_connection_state_default_is_initial() {
        assert_eq!(ConnectionState::default(), ConnectionState::Initial);
    }

    // ==========================================================================
    // AddressType tests
    // ==========================================================================

    #[test]
    fn test_address_type_display() {
        assert_eq!(
            AddressType::Tcp("127.0.0.1:8080".into()).to_string(),
            "tcp://127.0.0.1:8080"
        );
        assert_eq!(
            AddressType::Quic("0.0.0.0:22000".into()).to_string(),
            "quic://0.0.0.0:22000"
        );
        assert_eq!(
            AddressType::Relay("relay.example.com".into()).to_string(),
            "relay://relay.example.com"
        );
        assert_eq!(
            AddressType::WebSocket("localhost:9000".into()).to_string(),
            "ws://localhost:9000"
        );
        assert_eq!(
            AddressType::SecureWebSocket("example.com".into()).to_string(),
            "wss://example.com"
        );
        assert_eq!(AddressType::Dynamic.to_string(), "dynamic");
    }

    #[test]
    fn test_address_type_serde_roundtrip() {
        let cases = vec![
            AddressType::Tcp("127.0.0.1:8080".into()),
            AddressType::Quic("0.0.0.0:22000".into()),
            AddressType::Relay("relay://example.com".into()),
            AddressType::WebSocket("localhost:9000".into()),
            AddressType::SecureWebSocket("example.com:443".into()),
            AddressType::Dynamic,
        ];

        for addr in cases {
            let json = serde_json::to_string(&addr).unwrap();
            let restored: AddressType = serde_json::from_str(&json).unwrap();
            assert_eq!(addr, restored, "roundtrip failed for {:?}", json);
        }
    }

    #[test]
    fn test_address_type_as_str() {
        assert_eq!(AddressType::Tcp("host:123".into()).as_str(), "host:123");
        assert_eq!(
            AddressType::WebSocket("host:456".into()).as_str(),
            "host:456"
        );
        assert_eq!(AddressType::Dynamic.as_str(), "dynamic");
    }

    // ==========================================================================
    // HeartbeatConfig tests
    // ==========================================================================

    #[test]
    fn test_heartbeat_interval_for_relay_is_capped() {
        let cfg = HeartbeatConfig::default();
        let interval = cfg.interval_for(&AddressType::Relay("proxy:443".into()));
        assert_eq!(interval, std::time::Duration::from_secs(30));
    }

    #[test]
    fn test_heartbeat_interval_for_tcp_is_full() {
        let cfg = HeartbeatConfig::default();
        let interval = cfg.interval_for(&AddressType::Tcp("host:123".into()));
        assert_eq!(interval, std::time::Duration::from_secs(60));
    }

    #[test]
    fn test_heartbeat_dead_after() {
        let cfg = HeartbeatConfig::default();
        let dead = cfg.dead_after(&AddressType::Tcp("host:123".into()));
        assert_eq!(dead, std::time::Duration::from_secs(180)); // 60s * 3
    }

    // ==========================================================================
    // ConnectionPriority tests
    // ==========================================================================

    #[test]
    fn test_connection_priority_ordering() {
        assert!(ConnectionPriority::Highest > ConnectionPriority::High);
        assert!(ConnectionPriority::High > ConnectionPriority::Normal);
        assert!(ConnectionPriority::Normal > ConnectionPriority::Low);
        assert!(ConnectionPriority::Low > ConnectionPriority::Lowest);
        assert_eq!(ConnectionPriority::default(), ConnectionPriority::Normal);
    }

    // ==========================================================================
    // ExponentialBackoff tests
    // ==========================================================================

    #[test]
    fn test_exponential_backoff_next_delay_increases() {
        let backoff = ExponentialBackoff {
            initial_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(60),
            reset_after: std::time::Duration::from_secs(60),
        };

        let d0 = backoff.next_delay(0); // 100 * 1 = 100ms
        let d1 = backoff.next_delay(1); // 100 * 2 = 200ms
        let d2 = backoff.next_delay(2); // 100 * 4 = 400ms
        let d3 = backoff.next_delay(3); // 100 * 8 = 800ms

        assert_eq!(d0, std::time::Duration::from_millis(100));
        assert_eq!(d1, std::time::Duration::from_millis(200));
        assert_eq!(d2, std::time::Duration::from_millis(400));
        assert_eq!(d3, std::time::Duration::from_millis(800));
    }

    #[test]
    fn test_exponential_backoff_capped_at_max() {
        let backoff = ExponentialBackoff {
            initial_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(1),
            reset_after: std::time::Duration::from_secs(60),
        };

        let d10 = backoff.next_delay(10); // 100 * 1024 = 102400ms, cap at 1000ms
        assert_eq!(d10, std::time::Duration::from_secs(1));
    }

    #[test]
    fn test_exponential_backoff_saturating_mul() {
        // Verify saturating_mul prevents overflow at high attempt counts
        let backoff = ExponentialBackoff {
            initial_delay: std::time::Duration::from_secs(1),
            max_delay: std::time::Duration::from_secs(3600),
            reset_after: std::time::Duration::from_secs(3600),
        };

        let d31 = backoff.next_delay(31); // Attempt 31 should not panic
        assert_eq!(d31, std::time::Duration::from_secs(3600)); // Capped at max
    }

    // ==========================================================================
    // RestartConfig tests
    // ==========================================================================

    #[test]
    fn test_restart_config_default() {
        let cfg = RestartConfig::default();
        assert_eq!(cfg.restart_policy, RestartPolicy::OnFailure);
        assert_eq!(cfg.max_restarts, 5);
        assert_eq!(
            cfg.backoff.initial_delay,
            std::time::Duration::from_millis(100)
        );
        assert_eq!(cfg.backoff.max_delay, std::time::Duration::from_secs(60));
        assert_eq!(cfg.backoff.reset_after, std::time::Duration::from_secs(60));
    }

    #[test]
    fn test_restart_policy_eq() {
        assert_eq!(RestartPolicy::Always, RestartPolicy::Always);
        assert_eq!(RestartPolicy::OnFailure, RestartPolicy::OnFailure);
        assert_eq!(RestartPolicy::Never, RestartPolicy::Never);
        assert_ne!(RestartPolicy::Always, RestartPolicy::Never);
    }
}
