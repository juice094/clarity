//! Gateway-sync transport for Claw Mesh role contexts.
//!
//! This transport rides the existing `ClawConnectionManager` WebSocket
//! connection and uses the native Gateway `SyncRoleContext` / `RoleContextSynced`
//! message pair to fetch missing events.

use super::transport::{MeshTransportError, Result, RoleContextTransport};
use crate::connection_manager::ClawConnectionManager;
use crate::protocol::{ProtocolCommand, ProtocolEvent};
use clarity_contract::{ClawContextEvent, RoleContextId};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Transport that synchronizes role contexts through a connected Gateway.
pub struct GatewaySyncTransport {
    manager: ClawConnectionManager,
    pending: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<ProtocolEvent>>>>,
    notify_tx: tokio::sync::mpsc::UnboundedSender<RoleContextId>,
}

impl GatewaySyncTransport {
    /// Create a new Gateway sync transport.
    ///
    /// The returned receiver yields role ids when the transport observes a
    /// `RoleContextSynced` event.
    pub fn new(
        manager: ClawConnectionManager,
    ) -> (Self, tokio::sync::mpsc::UnboundedReceiver<RoleContextId>) {
        let (notify_tx, notify_rx) = tokio::sync::mpsc::unbounded_channel();
        let transport = Self {
            manager,
            pending: Arc::new(Mutex::new(HashMap::new())),
            notify_tx,
        };
        transport.spawn_drain_loop();
        (transport, notify_rx)
    }

    fn spawn_drain_loop(&self) {
        let pending = Arc::clone(&self.pending);
        let notify_tx = self.notify_tx.clone();
        let manager = self.manager.clone();
        std::thread::spawn(move || {
            loop {
                for event in manager.drain() {
                    match event {
                        ProtocolEvent::RoleContextSynced {
                            role_id,
                            events,
                            next_cursor,
                            online_devices,
                        } => {
                            let correlated = pending.lock().remove(&role_id);
                            let evt = ProtocolEvent::RoleContextSynced {
                                role_id: role_id.clone(),
                                events,
                                next_cursor,
                                online_devices,
                            };
                            if let Some(tx) = correlated {
                                let _ = tx.send(evt);
                            } else {
                                // Unsolicited sync response: notify the subscriber.
                                let _ = notify_tx.send(RoleContextId::new(role_id));
                            }
                        }
                        ProtocolEvent::Error(e) => {
                            // Surface errors to any pending request and continue.
                            // ponytail: in production, correlate errors by request id.
                            let mut guard = pending.lock();
                            for (_, tx) in guard.drain() {
                                let _ = tx.send(ProtocolEvent::Error(e.clone()));
                            }
                        }
                        _ => {}
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
    }
}

#[async_trait::async_trait]
impl RoleContextTransport for GatewaySyncTransport {
    async fn publish(&self, _role_id: &RoleContextId, _event: &ClawContextEvent) -> Result<()> {
        // The Gateway dialect is currently read-only for role-context sync.
        // Writes propagate through syncthing-rust or a future Gateway API.
        Err(MeshTransportError::Unsupported(
            "GatewaySyncTransport does not support publishing events".into(),
        ))
    }

    async fn collect(&self, role_id: &RoleContextId) -> Result<Vec<ClawContextEvent>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut guard = self.pending.lock();
            guard.insert(role_id.as_ref().to_string(), tx);
        }

        self.manager.send(ProtocolCommand::SyncRoleContext {
            role_id: role_id.as_ref().into(),
            since_event_id: None,
            device_id: String::new(), // ponytail: pass real device id when available
        });

        match tokio::time::timeout(Duration::from_secs(10), rx).await {
            Ok(Ok(ProtocolEvent::RoleContextSynced { events, .. })) => Ok(events),
            Ok(Ok(ProtocolEvent::Error(e))) => Err(MeshTransportError::Remote(e)),
            Ok(Ok(_)) => Err(MeshTransportError::Other(
                "unexpected protocol event".into(),
            )),
            Ok(Err(_)) => Err(MeshTransportError::Other("response channel closed".into())),
            Err(_) => {
                self.pending.lock().remove(role_id.as_ref());
                Err(MeshTransportError::Remote("sync request timeout".into()))
            }
        }
    }

    fn subscribe(&self) -> tokio::sync::mpsc::UnboundedReceiver<RoleContextId> {
        // Notifications are produced by the drain loop via notify_tx; the
        // receiver is owned by the caller. Returning a closed channel here is a
        // safe fallback because callers should use the receiver returned from
        // `new()`.
        let (_, rx) = tokio::sync::mpsc::unbounded_channel();
        rx
    }
}
