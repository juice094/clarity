//! Network interface monitor.
//!
//! Detects OS network interface changes and emits events so the connection
//! manager can proactively reconnect on network transitions (Wi-Fi ↔ Ethernet,
//! VPN connect/disconnect, etc.).
//!
//! Design follows production patterns from syncthing-rust's
//! `syncthing-net/src/netmon.rs`.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};
use tracing::debug;

/// Network change event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetChangeEvent {
    /// Network interfaces have changed (added, removed, or modified).
    InterfacesChanged,
}

/// Monitors network interface changes and emits events on change.
///
/// Uses `netdev::get_interfaces()` to poll the system's network interface list
/// at a configurable interval. On change, emits `NetChangeEvent::InterfacesChanged`
/// through an mpsc channel.
///
/// Uses `MissedTickBehavior::Skip` to avoid burst processing after system sleep.
pub struct NetMonitor {
    interface_source: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
}

impl NetMonitor {
    /// Create a new monitor using the real `netdev` interface list.
    pub fn new() -> Self {
        Self {
            interface_source: Arc::new(|| {
                netdev::get_interfaces()
                    .into_iter()
                    .map(|iface| iface.name)
                    .collect()
            }),
        }
    }

    /// Subscribe to network change events.
    ///
    /// Polls every 5 seconds. Returns a receiver that yields `NetChangeEvent`
    /// whenever the interface list changes.
    pub fn subscribe(&self) -> mpsc::Receiver<NetChangeEvent> {
        self.subscribe_with_interval(Duration::from_secs(5))
    }

    /// Subscribe with a custom polling interval.
    fn subscribe_with_interval(
        &self,
        interval_duration: Duration,
    ) -> mpsc::Receiver<NetChangeEvent> {
        let (tx, rx) = mpsc::channel(16);
        let source = Arc::clone(&self.interface_source);
        tokio::spawn(async move {
            run_monitor(tx, source, interval_duration).await;
        });
        rx
    }
}

impl Default for NetMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Core monitoring loop.
async fn run_monitor(
    tx: mpsc::Sender<NetChangeEvent>,
    source: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
    interval_duration: Duration,
) {
    let mut ticker = interval(interval_duration);
    // Skip missed ticks to avoid burst after system sleep/hibernate.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_interfaces: Option<Vec<String>> = None;

    loop {
        ticker.tick().await;

        let current = source();
        let changed = match &last_interfaces {
            None => true, // First poll: always emit.
            Some(last) => last != &current,
        };

        if changed {
            debug!("Network interfaces changed, emitting NetChangeEvent");
            last_interfaces = Some(current);
            if tx.send(NetChangeEvent::InterfacesChanged).await.is_err() {
                // Receiver dropped; stop monitoring.
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_netmon_detects_interface_change() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);

        let monitor = NetMonitor {
            interface_source: Arc::new(move || {
                let count = cc.fetch_add(1, Ordering::SeqCst);
                match count {
                    0 => vec!["eth0".to_string()],
                    _ => vec!["eth0".to_string(), "eth1".to_string()],
                }
            }),
        };

        let mut rx = monitor.subscribe_with_interval(Duration::from_millis(50));

        // First tick: last_interfaces is None, should emit.
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
        assert!(event.is_ok());
        assert!(matches!(
            event.unwrap(),
            Some(NetChangeEvent::InterfacesChanged)
        ));

        // Second tick: interfaces changed, should emit again.
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
        assert!(event.is_ok());
        assert!(matches!(
            event.unwrap(),
            Some(NetChangeEvent::InterfacesChanged)
        ));
    }

    #[tokio::test]
    async fn test_netmon_no_event_on_unchanged() {
        let monitor = NetMonitor {
            interface_source: Arc::new(|| vec!["eth0".to_string()]),
        };

        let mut rx = monitor.subscribe_with_interval(Duration::from_millis(50));

        // First tick: emits (last_interfaces is None).
        let event = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
        assert!(event.is_ok());

        // Second tick: no change, should NOT emit within 200ms.
        let event = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await;
        assert!(event.is_err()); // Timeout expected.
    }
}
