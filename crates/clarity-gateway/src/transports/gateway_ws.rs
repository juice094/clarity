//! Server-side adapter for the native Gateway `/ws` WebSocket endpoint.
//!
//! This module implements `clarity_contract::ClawTransport` over Axum's
//! WebSocket split sink/stream so the native Gateway endpoint can share the
//! same chat/history/sync path as the OpenClaw endpoint.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use clarity_contract::{
    ClawTransport, HistoryMessage, MessageContext, TransportAuth, TransportCaps, TransportError,
    TransportEvent,
};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures::stream::BoxStream;
use tracing::{debug, error};

use super::common::{
    ServerTransportContext, session_messages_to_contract_messages, session_messages_to_history,
};
use crate::handlers::AgentHandle;
use crate::session_store::SessionMessage;

/// `ClawTransport` implementation for the native Gateway WebSocket.
///
/// The adapter turns a single WebSocket connection into a transport-agnostic
/// chat session: `send_message` runs the shared `Agent` and emits wire events
/// plus a final assistant chunk; `get_history` reads from the persistent
/// session store; `sync_role_context` queries the role-context store.
pub struct GatewayWebSocketTransport {
    ctx: ServerTransportContext,
    caps: TransportCaps,
    event_tx: UnboundedSender<TransportEvent>,
    event_rx: Mutex<Option<UnboundedReceiver<TransportEvent>>>,
}

impl GatewayWebSocketTransport {
    /// Create a new adapter bound to the given transport context.
    pub fn new(ctx: ServerTransportContext) -> Self {
        let (event_tx, event_rx) = unbounded::<TransportEvent>();
        Self {
            ctx,
            caps: gateway_caps(),
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
impl ClawTransport for GatewayWebSocketTransport {
    async fn handshake(&self, _auth: TransportAuth) -> Result<TransportCaps, TransportError> {
        // ponytail: do not emit TransportEvent::Connected here. The native
        // Gateway /ws endpoint already sent a WsResponse::Welcome during the
        // WebSocket handshake; emitting it again would cause clients to receive
        // a second welcome and error out with "Unexpected welcome".
        Ok(self.caps.clone())
    }

    async fn send_message(&self, ctx: MessageContext) -> Result<(), TransportError> {
        let state = self.ctx.state.clone();
        let session_id = self.ctx.session_id.clone();
        let event_tx = self.event_tx.clone();
        let message = ctx.message;

        tokio::spawn(async move {
            debug!(
                "GatewayWebSocketTransport sending message for session {}",
                session_id
            );

            // Serialize Agent turns across all WebSocket connections. The shared
            // Agent can only run one turn at a time; acquiring the permit here
            // queues concurrent chat.send requests instead of failing with
            // "Agent is already running a turn".
            let _permit = match state.agent_turn_sem.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    error!("Agent turn semaphore closed");
                    let _ = event_tx.unbounded_send(TransportEvent::Error {
                        message: "Agent turn queue closed".into(),
                    });
                    let _ = event_tx.unbounded_send(TransportEvent::Done);
                    return;
                }
            };

            // Reset any provider-side conversation context before each turn. The
            // Gateway WebSocket transport builds the full conversation history in
            // the prompt via ConversationChatDriver, so stateful providers (e.g.
            // DeepSeek device-login) do not need to reuse their native session.
            // Resetting prevents shared-provider state from leaking across
            // independent WebSocket sessions and causing empty completions.
            if let Some(ref llm) = state.agent.llm() {
                llm.reset_conversation_context();
            }

            let user_msg = SessionMessage::new("user", &message);
            if let Err(e) = state
                .session_store
                .append_message(&session_id, &user_msg)
                .await
            {
                error!("Failed to append user message: {}", e);
            }

            // Build a chat driver from the persisted session history (including the
            // user message just appended). Using the driver avoids the
            // memory-search path in `agent.run_streaming` that has been observed to
            // hang in the Gateway WebSocket context, while keeping the same
            // streaming loop that the HTTP endpoint uses.
            let history = match state.session_store.load_session(&session_id).await {
                Ok(msgs) => session_messages_to_contract_messages(&msgs),
                Err(e) => {
                    error!("Failed to load session history for driver: {}", e);
                    vec![]
                }
            };
            let driver = Arc::new(clarity_core::agent::driver::ConversationChatDriver { history });

            let (ctrl_event_tx, mut ctrl_event_rx) =
                tokio::sync::mpsc::unbounded_channel::<clarity_core::agent::ControllerEvent>();
            let agent = state.clone_agent();
            let (controller, op_tx) = clarity_core::agent::AgentController::new_with_events(
                agent,
                ctrl_event_tx,
                Some(driver),
            );
            let controller_handle = tokio::spawn(controller.run());

            if let Err(e) = op_tx.send(clarity_core::agent::Op::user_turn(message)) {
                error!("Failed to submit user turn to AgentController: {}", e);
                controller_handle.abort();
                let _ = event_tx.unbounded_send(TransportEvent::Error {
                    message: "Failed to start agent turn".into(),
                });
                let _ = event_tx.unbounded_send(TransportEvent::Done);
                return;
            }

            let mut final_text = String::new();
            let mut sent_any_chunk = false;
            let timeout_result =
                tokio::time::timeout(tokio::time::Duration::from_secs(60), async {
                    while let Some(ev) = ctrl_event_rx.recv().await {
                        match ev {
                            clarity_core::agent::ControllerEvent::Chunk(chunk) => {
                                sent_any_chunk = true;
                                final_text.push_str(&chunk);
                                state
                                    .metrics
                                    .messages_sent
                                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                if event_tx
                                    .unbounded_send(TransportEvent::ChatChunk { content: chunk })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            clarity_core::agent::ControllerEvent::Complete(reply) => {
                                final_text = reply;
                                break;
                            }
                            clarity_core::agent::ControllerEvent::Error(e) => {
                                error!("AgentController error in WebSocket transport: {}", e);
                                let _ = event_tx.unbounded_send(TransportEvent::Error {
                                    message: format!("Agent execution error: {}", e),
                                });
                                return;
                            }
                            // Tool-calling lifecycle events are forwarded by the
                            // controller's internal wire bridge; we do not need to
                            // handle them explicitly here.
                            clarity_core::agent::ControllerEvent::ToolCallStart { .. }
                            | clarity_core::agent::ControllerEvent::ToolResult { .. }
                            | clarity_core::agent::ControllerEvent::StepBegin { .. } => {}
                        }
                    }
                })
                .await;

            match timeout_result {
                Ok(()) => {
                    if !sent_any_chunk && !final_text.is_empty() {
                        // The turn completed without streaming any chunks (e.g. a
                        // provider that returns the full response at once). Emit the
                        // final text so the client still gets a reply.
                        let _ = event_tx.unbounded_send(TransportEvent::ChatChunk {
                            content: final_text.clone(),
                        });
                    }
                    if !final_text.is_empty() {
                        let assistant_msg = SessionMessage::new("assistant", &final_text);
                        if let Err(e) = state
                            .session_store
                            .append_message(&session_id, &assistant_msg)
                            .await
                        {
                            error!("Failed to append assistant message: {}", e);
                        }
                    }
                }
                Err(_) => {
                    error!(
                        "Agent turn timed out after 60s in WebSocket transport for session {}",
                        session_id
                    );
                    controller_handle.abort();
                    let _ = event_tx.unbounded_send(TransportEvent::Error {
                        message: "Agent turn timed out after 60s".into(),
                    });
                }
            }

            controller_handle.abort();
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
        // ponytail: per-turn abort is not yet plumbed into the server agent;
        // returning ok keeps the protocol contract honest.
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

fn gateway_caps() -> TransportCaps {
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
            "wire_payload".into(),
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
    fn gateway_caps_advertises_expected_methods() {
        let caps = gateway_caps();
        assert!(caps.supports_method("chat.send"));
        assert!(caps.supports_method("chat.history"));
        assert!(caps.supports_event("wire_payload"));
    }
}
