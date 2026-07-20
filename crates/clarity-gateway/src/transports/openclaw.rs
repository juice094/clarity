//! Server-side adapter for the OpenClaw-compatible `/openclaw/ws` endpoint.
//!
//! This module implements `clarity_contract::ClawTransport` over Axum's
//! WebSocket split sink/stream, translating between transport-agnostic events
//! and OpenClaw JSON-RPC frames.

use std::sync::Mutex;

use async_trait::async_trait;
use clarity_contract::{
    ClawTransport, HistoryMessage, MessageContext, TransportAuth, TransportCaps, TransportError,
    TransportEvent,
};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures::stream::BoxStream;
use tracing::{debug, error};

use super::common::{ServerTransportContext, session_messages_to_history};
use crate::handlers::AgentHandle;
use crate::session_store::SessionMessage;

/// `ClawTransport` implementation for the OpenClaw-compatible endpoint.
///
/// Unlike the native Gateway adapter, this adapter does **not** stream raw wire
/// messages; it emits transport-agnostic `ChatChunk`/`Done` events that the
/// OpenClaw handler converts into JSON-RPC response frames.
pub struct OpenClawServerTransport {
    ctx: ServerTransportContext,
    caps: TransportCaps,
    event_tx: UnboundedSender<TransportEvent>,
    event_rx: Mutex<Option<UnboundedReceiver<TransportEvent>>>,
}

impl OpenClawServerTransport {
    /// Create a new adapter bound to the given transport context.
    pub fn new(ctx: ServerTransportContext) -> Self {
        let (event_tx, event_rx) = unbounded::<TransportEvent>();
        Self {
            ctx,
            caps: openclaw_caps(),
            event_tx,
            event_rx: Mutex::new(Some(event_rx)),
        }
    }

    /// Send a transport event to the outbound event stream.
    fn emit(&self, ev: TransportEvent) {
        let _ = self.event_tx.unbounded_send(ev);
    }
}

#[async_trait]
impl ClawTransport for OpenClawServerTransport {
    async fn handshake(&self, _auth: TransportAuth) -> Result<TransportCaps, TransportError> {
        self.emit(TransportEvent::Connected {
            gateway_url: self.ctx.state.started_at.to_rfc3339(),
            session_id: Some(self.ctx.session_id.clone()),
        });
        Ok(self.caps.clone())
    }

    async fn send_message(&self, ctx: MessageContext) -> Result<(), TransportError> {
        let state = self.ctx.state.clone();
        let session_id = ctx
            .session_key
            .unwrap_or_else(|| self.ctx.session_id.clone());
        let event_tx = self.event_tx.clone();
        let message = ctx.message;

        tokio::spawn(async move {
            debug!(
                "OpenClawServerTransport sending message for session {}",
                session_id
            );

            // Serialize Agent turns across all WebSocket connections. The shared
            // Agent can only run one turn at a time; acquiring the permit here
            // queues concurrent chat.send requests instead of failing with
            // "Agent is already running a turn".
            let _permit = match state.agent_turn_sem.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    error!("OpenClaw turn semaphore closed");
                    let _ = event_tx.unbounded_send(TransportEvent::Error {
                        message: "OpenClaw turn queue closed".into(),
                    });
                    let _ = event_tx.unbounded_send(TransportEvent::Done);
                    return;
                }
            };

            let user_msg = SessionMessage::new("user", &message);
            if let Err(e) = state
                .session_store
                .append_message(&session_id, &user_msg)
                .await
            {
                error!("Failed to append user message: {}", e);
            }

            let agent = state.clone_agent();
            match agent.run(&message).await {
                Ok(reply) => {
                    let assistant_msg = SessionMessage::new("assistant", &reply);
                    if let Err(e) = state
                        .session_store
                        .append_message(&session_id, &assistant_msg)
                        .await
                    {
                        error!("Failed to append assistant message: {}", e);
                    }
                    let _ = event_tx.unbounded_send(TransportEvent::ChatChunk { content: reply });
                }
                Err(e) => {
                    error!("Agent execution error in OpenClaw transport: {}", e);
                    let _ = event_tx.unbounded_send(TransportEvent::Error {
                        message: format!("Agent execution error: {}", e),
                    });
                }
            }

            let _ = event_tx.unbounded_send(TransportEvent::Done);
        });

        Ok(())
    }

    async fn get_history(
        &self,
        session_key: Option<String>,
    ) -> Result<Vec<HistoryMessage>, TransportError> {
        let session_id = session_key.unwrap_or_else(|| self.ctx.session_id.clone());
        let messages = self
            .ctx
            .state
            .session_store
            .load_session(&session_id)
            .await
            .map_err(|e| TransportError::other(format!("failed to load session history: {}", e)))?;
        Ok(session_messages_to_history(&messages))
    }

    async fn sync_role_context(
        &self,
        role_id: String,
        since_event_id: Option<String>,
    ) -> Result<(), TransportError> {
        let store = self.ctx.state.role_context_store.clone();
        let device_id = self.ctx.session_id.clone();

        let events = store
            .list_events(&role_id, since_event_id.as_deref())
            .await
            .map_err(|e| {
                TransportError::other(format!("failed to list role context events: {}", e))
            })?;
        let online_devices = store.online_devices(&role_id).await.unwrap_or_default();

        if let Err(e) = store.record_device_presence(&role_id, &device_id).await {
            error!("Failed to record device presence: {}", e);
        }

        self.emit(TransportEvent::RoleContextSynced {
            role_id,
            events,
            next_cursor: None,
            online_devices,
        });
        Ok(())
    }

    async fn abort(&self) -> Result<(), TransportError> {
        // ponytail: per-turn abort is not yet plumbed into the server agent.
        Ok(())
    }

    fn events(&self) -> BoxStream<'static, TransportEvent> {
        let rx = self
            .event_rx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .unwrap_or_else(|| {
                let (_tx, rx) = unbounded::<TransportEvent>();
                rx
            });
        Box::pin(rx)
    }

    fn capabilities(&self) -> TransportCaps {
        self.caps.clone()
    }
}

fn openclaw_caps() -> TransportCaps {
    TransportCaps {
        methods: vec![
            "chat.send".into(),
            "chat.history".into(),
            "role_context.sync".into(),
            "chat.abort".into(),
        ],
        events: vec![
            "connected".into(),
            "chat_chunk".into(),
            "done".into(),
            "error".into(),
            "role_context.synced".into(),
        ],
        max_payload: None,
        protocol_version: Some(1),
        extras: std::collections::HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openclaw_caps_advertises_expected_methods() {
        let caps = openclaw_caps();
        assert!(caps.supports_method("chat.send"));
        assert!(caps.supports_method("chat.history"));
        assert!(!caps.supports_event("wire_payload"));
    }
}
