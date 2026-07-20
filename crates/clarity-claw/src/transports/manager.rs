//! Transport manager built on top of `ClawTransport` adapters.
//!
//! `TransportManager` owns a `Box<dyn ClawTransport>` and exposes a
//! synchronous, UI-thread-friendly poll interface similar to the legacy
//! `ClawConnectionManager`, but backed by the new trait abstraction.

use std::sync::Arc;

use clarity_contract::{
    ClawTransport, MessageContext, TransportAuth, TransportCaps, TransportError, TransportEvent,
};
use futures::StreamExt;
use parking_lot::Mutex;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::device::DeviceIdentity;
use crate::transports::gateway_ws::GatewayWebSocketTransport;
use crate::transports::openclaw::OpenClawTransport;
use crate::types::OpenClawSendMethod;

/// A managed Claw transport handle.
#[derive(Clone)]
pub struct TransportManager {
    transport: Arc<dyn ClawTransport>,
    cmd_tx: UnboundedSender<ManagerCommand>,
    event_rx: Arc<Mutex<UnboundedReceiver<TransportEvent>>>,
}

impl TransportManager {
    /// Create a manager from an existing transport.
    pub fn new<T>(transport: T) -> Self
    where
        T: ClawTransport + 'static,
    {
        let transport: Arc<dyn ClawTransport> = Arc::new(transport);
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<ManagerCommand>();
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<TransportEvent>();

        let transport_clone = transport.clone();
        tokio::spawn(async move {
            let mut stream = transport_clone.events();
            loop {
                tokio::select! {
                    biased;
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(ManagerCommand::Send(ctx, reply)) => {
                                let result = transport_clone.send_message(ctx).await;
                                let _ = reply.send(result);
                            }
                            Some(ManagerCommand::History { session_key, reply }) => {
                                let result = transport_clone.get_history(session_key).await;
                                let _ = reply.send(result);
                            }
                            Some(ManagerCommand::SyncRoleContext { role_id, since_event_id, reply }) => {
                                let result = transport_clone.sync_role_context(role_id, since_event_id).await;
                                let _ = reply.send(result);
                            }
                            Some(ManagerCommand::Abort(reply)) => {
                                let result = transport_clone.abort().await;
                                let _ = reply.send(result);
                            }
                            Some(ManagerCommand::RequestPairing {
                                device_id,
                                public_key,
                                client_id,
                                client_mode,
                                platform,
                                role,
                                scopes,
                                reply,
                            }) => {
                                let result = transport_clone
                                    .request_pairing(
                                        device_id,
                                        public_key,
                                        client_id,
                                        client_mode,
                                        platform,
                                        role,
                                        scopes,
                                    )
                                    .await;
                                let _ = reply.send(result);
                            }
                            None => break,
                        }
                    }
                    event = stream.next() => {
                        match event {
                            Some(ev) => {
                                if event_tx.send(ev).is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        Self {
            transport,
            cmd_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
        }
    }

    /// Convenience constructor for the native Gateway WebSocket transport.
    pub fn gateway(url: &str) -> Self {
        Self::new(GatewayWebSocketTransport::new(url))
    }

    /// Convenience constructor for the OpenClaw transport.
    pub fn openclaw(url: &str, auth: TransportAuth, send_method: OpenClawSendMethod) -> Self {
        Self::new(OpenClawTransport::new(url, auth, send_method))
    }

    /// Convenience constructor for the OpenClaw transport with a device identity.
    pub fn openclaw_with_device(
        url: &str,
        auth: TransportAuth,
        send_method: OpenClawSendMethod,
        device_identity: DeviceIdentity,
    ) -> Self {
        Self::new(OpenClawTransport::new_with_device(
            url,
            auth,
            send_method,
            Some(device_identity),
        ))
    }

    /// Perform the transport handshake (async).
    pub async fn handshake(&self, auth: TransportAuth) -> Result<TransportCaps, TransportError> {
        self.transport.handshake(auth).await
    }

    /// Send a chat message.
    pub async fn send_message(&self, ctx: MessageContext) -> Result<(), TransportError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(ManagerCommand::Send(ctx, tx))
            .map_err(|_| TransportError::other("manager command channel closed"))?;
        rx.await
            .map_err(|_| TransportError::other("manager result channel closed"))?
    }

    /// Fetch history.
    pub async fn get_history(
        &self,
        session_key: Option<String>,
    ) -> Result<Vec<clarity_contract::HistoryMessage>, TransportError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(ManagerCommand::History {
                session_key,
                reply: tx,
            })
            .map_err(|_| TransportError::other("manager command channel closed"))?;
        rx.await
            .map_err(|_| TransportError::other("manager result channel closed"))?
    }

    /// Sync role context.
    pub async fn sync_role_context(
        &self,
        role_id: String,
        since_event_id: Option<String>,
    ) -> Result<(), TransportError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(ManagerCommand::SyncRoleContext {
                role_id,
                since_event_id,
                reply: tx,
            })
            .map_err(|_| TransportError::other("manager command channel closed"))?;
        rx.await
            .map_err(|_| TransportError::other("manager result channel closed"))?
    }

    /// Abort the current turn.
    pub async fn abort(&self) -> Result<(), TransportError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(ManagerCommand::Abort(tx))
            .map_err(|_| TransportError::other("manager command channel closed"))?;
        rx.await
            .map_err(|_| TransportError::other("manager result channel closed"))?
    }

    /// Request device pairing with the remote gateway.
    #[allow(clippy::too_many_arguments)]
    pub async fn request_pairing(
        &self,
        device_id: String,
        public_key: String,
        client_id: String,
        client_mode: String,
        platform: String,
        role: String,
        scopes: Vec<String>,
    ) -> Result<(), TransportError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(ManagerCommand::RequestPairing {
                device_id,
                public_key,
                client_id,
                client_mode,
                platform,
                role,
                scopes,
                reply: tx,
            })
            .map_err(|_| TransportError::other("manager command channel closed"))?;
        rx.await
            .map_err(|_| TransportError::other("manager result channel closed"))?
    }

    /// Non-blocking poll for the next transport event.
    pub fn try_recv(&self) -> Option<TransportEvent> {
        self.event_rx.lock().try_recv().ok()
    }

    /// Drain all pending transport events.
    pub fn drain(&self) -> Vec<TransportEvent> {
        let mut out = Vec::new();
        let mut rx = self.event_rx.lock();
        while let Ok(ev) = rx.try_recv() {
            out.push(ev);
        }
        out
    }

    /// Return the transport capabilities.
    pub fn capabilities(&self) -> TransportCaps {
        self.transport.capabilities()
    }
}

enum ManagerCommand {
    Send(
        MessageContext,
        tokio::sync::oneshot::Sender<Result<(), TransportError>>,
    ),
    History {
        session_key: Option<String>,
        reply: tokio::sync::oneshot::Sender<
            Result<Vec<clarity_contract::HistoryMessage>, TransportError>,
        >,
    },
    SyncRoleContext {
        role_id: String,
        since_event_id: Option<String>,
        reply: tokio::sync::oneshot::Sender<Result<(), TransportError>>,
    },
    Abort(tokio::sync::oneshot::Sender<Result<(), TransportError>>),
    RequestPairing {
        device_id: String,
        public_key: String,
        client_id: String,
        client_mode: String,
        platform: String,
        role: String,
        scopes: Vec<String>,
        reply: tokio::sync::oneshot::Sender<Result<(), TransportError>>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::{
        ClawTransport, MessageContext, TransportAuth, TransportCaps, TransportError, TransportEvent,
    };
    use futures::stream::{self, BoxStream};

    struct DummyTransport {
        caps: TransportCaps,
    }

    #[async_trait::async_trait]
    impl ClawTransport for DummyTransport {
        async fn handshake(&self, _auth: TransportAuth) -> Result<TransportCaps, TransportError> {
            Ok(self.caps.clone())
        }

        async fn send_message(&self, _ctx: MessageContext) -> Result<(), TransportError> {
            Ok(())
        }

        async fn get_history(
            &self,
            _session_key: Option<String>,
        ) -> Result<Vec<clarity_contract::HistoryMessage>, TransportError> {
            Ok(vec![])
        }

        async fn sync_role_context(
            &self,
            _role_id: String,
            _since_event_id: Option<String>,
        ) -> Result<(), TransportError> {
            Ok(())
        }

        async fn abort(&self) -> Result<(), TransportError> {
            Ok(())
        }

        fn events(&self) -> BoxStream<'static, TransportEvent> {
            Box::pin(stream::empty())
        }

        fn capabilities(&self) -> TransportCaps {
            self.caps.clone()
        }
    }

    #[tokio::test]
    async fn manager_roundtrips_caps() {
        let caps = TransportCaps {
            methods: vec!["chat.send".into()],
            ..Default::default()
        };
        let manager = TransportManager::new(DummyTransport { caps: caps.clone() });
        let got = manager.handshake(TransportAuth::default()).await.unwrap();
        assert_eq!(got.methods, caps.methods);
    }

    #[tokio::test]
    async fn manager_send_message_ok() {
        let manager = TransportManager::new(DummyTransport {
            caps: TransportCaps::default(),
        });
        manager
            .send_message(MessageContext {
                message: "hi".into(),
                ..Default::default()
            })
            .await
            .unwrap();
    }
}
