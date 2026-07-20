//! Native Gateway WebSocket transport adapter.
//!
//! Wraps the existing `crate::gateway_client::GatewayClient` so it implements
//! `clarity_contract::ClawTransport`. The adapter is intentionally thin: all
//! protocol parsing and reconnection logic stays in `gateway_client.rs`, while
//! this layer only normalizes command/event shapes.

use std::collections::HashMap;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use async_trait::async_trait;
use clarity_contract::{
    ClawTransport, HistoryMessage, MessageContext, MessageRole, TransportAuth, TransportCaps,
    TransportError, TransportEvent,
};
use futures::stream::{BoxStream, Stream};
use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::gateway_client::{GatewayClient, GatewayResponse};

/// Native Gateway WebSocket transport.
#[derive(Clone)]
pub struct GatewayWebSocketTransport {
    client: GatewayClient,
    rx: Arc<Mutex<UnboundedReceiver<TransportEvent>>>,
    caps: TransportCaps,
}

impl GatewayWebSocketTransport {
    /// Open a native Gateway WebSocket transport.
    pub fn new(url: &str) -> Self {
        let client = GatewayClient::connect(url);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<TransportEvent>();

        let client_clone = client.clone();
        tokio::spawn(async move {
            loop {
                for resp in client_clone.drain() {
                    for ev in translate_gateway_response(resp) {
                        if tx.send(ev).is_err() {
                            return;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });

        let caps = TransportCaps {
            methods: vec![
                "chat.send".into(),
                "chat.history".into(),
                "role_context.sync".into(),
            ],
            events: vec![
                "connected".into(),
                "chat.chunk".into(),
                "done".into(),
                "history".into(),
                "role_context.synced".into(),
                "error".into(),
            ],
            max_payload: Some(8 * 1024 * 1024),
            protocol_version: Some(1),
            extras: HashMap::new(),
        };

        Self {
            client,
            rx: Arc::new(Mutex::new(rx)),
            caps,
        }
    }
}

#[async_trait]
impl ClawTransport for GatewayWebSocketTransport {
    async fn handshake(&self, _auth: TransportAuth) -> Result<TransportCaps, TransportError> {
        // The native Gateway does not require an explicit handshake; the
        // welcome frame is consumed during construction. We return static caps
        // here and rely on the event stream for the negotiated session id.
        Ok(self.caps.clone())
    }

    async fn send_message(&self, ctx: MessageContext) -> Result<(), TransportError> {
        // ponytail: session_key is ignored because the native Gateway assigns
        // a fresh session per connection. If multi-session support is needed,
        // extend GatewayClient to accept a session id.
        let _ = ctx.session_key;
        self.client.chat(&ctx.message, true);
        Ok(())
    }

    async fn get_history(
        &self,
        _session_key: Option<String>,
    ) -> Result<Vec<HistoryMessage>, TransportError> {
        // History arrives asynchronously via the event stream. Trigger the
        // request and return an empty immediate result to keep the trait
        // non-blocking.
        self.client.get_history();
        Ok(Vec::new())
    }

    async fn sync_role_context(
        &self,
        role_id: String,
        since_event_id: Option<String>,
    ) -> Result<(), TransportError> {
        // ponytail: device_id is synthesized from the hostname. When the
        // Gateway supports explicit device identity, thread it through here.
        let device_id = synth_device_id();
        self.client
            .sync_role_context(&role_id, since_event_id.as_deref(), &device_id);
        Ok(())
    }

    async fn abort(&self) -> Result<(), TransportError> {
        // The native Gateway does not expose a per-turn abort over WebSocket.
        Err(TransportError::Unsupported(
            "native Gateway WebSocket abort not supported".into(),
        ))
    }

    fn events(&self) -> BoxStream<'static, TransportEvent> {
        let rx = self.rx.clone();
        Box::pin(GatewayEventStream { rx })
    }

    fn capabilities(&self) -> TransportCaps {
        self.caps.clone()
    }
}

struct GatewayEventStream {
    rx: Arc<Mutex<UnboundedReceiver<TransportEvent>>>,
}

impl Stream for GatewayEventStream {
    type Item = TransportEvent;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut rx = self.rx.lock();
        rx.poll_recv(cx)
    }
}

fn translate_gateway_response(resp: GatewayResponse) -> Vec<TransportEvent> {
    let mut out = Vec::new();
    match resp {
        GatewayResponse::Connected {
            gateway_url,
            session_id,
        } => {
            out.push(TransportEvent::Connected {
                gateway_url,
                session_id: Some(session_id),
            });
        }
        GatewayResponse::Chat { message, .. } => {
            out.push(TransportEvent::ChatChunk { content: message });
        }
        GatewayResponse::Done => {
            out.push(TransportEvent::Done);
        }
        GatewayResponse::WireMessage { payload } => {
            out.push(TransportEvent::WirePayload { payload });
        }
        GatewayResponse::History { messages } => {
            out.push(TransportEvent::History {
                messages: messages
                    .into_iter()
                    .map(|m| HistoryMessage {
                        role: parse_role(&m.role),
                        content: m.content,
                        tool_calls: None,
                        tool_call_id: None,
                    })
                    .collect(),
            });
        }
        GatewayResponse::RoleContextSynced {
            role_id,
            events,
            next_cursor,
            online_devices,
        } => {
            out.push(TransportEvent::RoleContextSynced {
                role_id,
                events,
                next_cursor,
                online_devices,
            });
        }
        GatewayResponse::Error(e) => {
            out.push(TransportEvent::Error { message: e });
        }
    }
    out
}

fn parse_role(role: &str) -> MessageRole {
    match role.to_ascii_lowercase().as_str() {
        "system" => MessageRole::System,
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "tool" => MessageRole::Tool,
        _ => MessageRole::User,
    }
}

fn synth_device_id() -> String {
    if cfg!(target_os = "windows") {
        format!(
            "claw-{}",
            std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".into())
        )
    } else {
        format!(
            "claw-{}",
            std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".into())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_connected_event() {
        let events = translate_gateway_response(GatewayResponse::Connected {
            gateway_url: "ws://localhost".into(),
            session_id: "s1".into(),
        });
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], TransportEvent::Connected { gateway_url, session_id }
                if gateway_url == "ws://localhost" && session_id == &Some("s1".to_string()))
        );
    }

    #[test]
    fn translate_chat_event_is_chunk_only() {
        let events = translate_gateway_response(GatewayResponse::Chat {
            message: "hello".into(),
            tool_calls: None,
        });
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], TransportEvent::ChatChunk { content } if content == "hello"));
    }

    #[test]
    fn translate_done_event() {
        let events = translate_gateway_response(GatewayResponse::Done);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], TransportEvent::Done));
    }

    #[test]
    fn translate_history_parses_roles() {
        let events = translate_gateway_response(GatewayResponse::History {
            messages: vec![crate::gateway_client::GatewayMessage {
                role: "assistant".into(),
                content: "hi".into(),
                timestamp: "2026-01-01T00:00:00Z".into(),
            }],
        });
        assert!(matches!(&events[0], TransportEvent::History { messages }
            if messages.len() == 1 && messages[0].role == MessageRole::Assistant));
    }

    #[test]
    fn parse_unknown_role_defaults_to_user() {
        assert_eq!(parse_role("bot"), MessageRole::User);
    }
}
