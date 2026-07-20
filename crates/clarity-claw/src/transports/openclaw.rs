//! OpenClaw / KimiClaw JSON-RPC transport adapter.
//!
//! Wraps the existing `crate::client::ClawClient` so it implements
//! `clarity_contract::ClawTransport`. The adapter converts between the
//! transport-agnostic `MessageContext`/`TransportEvent` types and the
//! OpenClaw-specific command/response enums.

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

use crate::client::{ClawAuth, ClawClient, ClawResponse};
use crate::device::DeviceIdentity;
use crate::types::OpenClawSendMethod;

/// OpenClaw JSON-RPC transport adapter.
#[derive(Clone)]
pub struct OpenClawTransport {
    client: ClawClient,
    rx: Arc<Mutex<UnboundedReceiver<TransportEvent>>>,
    caps: TransportCaps,
    send_method: OpenClawSendMethod,
}

impl OpenClawTransport {
    /// Open an OpenClaw transport with the given auth and send method.
    pub fn new(url: &str, auth: TransportAuth, send_method: OpenClawSendMethod) -> Self {
        Self::new_with_device(url, auth, send_method, None)
    }

    /// Open an OpenClaw transport with an optional device identity for
    /// `TokenWithDevice` authentication.
    pub fn new_with_device(
        url: &str,
        auth: TransportAuth,
        send_method: OpenClawSendMethod,
        device_identity: Option<DeviceIdentity>,
    ) -> Self {
        let claw_auth = map_auth(auth, device_identity);
        let client = ClawClient::connect_with_auth(url, claw_auth);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<TransportEvent>();

        let client_clone = client.clone();
        tokio::spawn(async move {
            loop {
                for resp in client_clone.drain() {
                    for ev in translate_openclaw_response(resp) {
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
                "sessions.send".into(),
                "chat.history".into(),
                "device.pair.request".into(),
            ],
            events: vec![
                "connected".into(),
                "chat.chunk".into(),
                "done".into(),
                "history".into(),
                "device.paired".into(),
                "reconnecting".into(),
                "error".into(),
            ],
            max_payload: Some(8 * 1024 * 1024),
            protocol_version: Some(3),
            extras: HashMap::new(),
        };

        Self {
            client,
            rx: Arc::new(Mutex::new(rx)),
            caps,
            send_method,
        }
    }
}

#[async_trait]
impl ClawTransport for OpenClawTransport {
    async fn handshake(&self, _auth: TransportAuth) -> Result<TransportCaps, TransportError> {
        // The OpenClaw handshake is performed during connection construction.
        Ok(self.caps.clone())
    }

    async fn send_message(&self, ctx: MessageContext) -> Result<(), TransportError> {
        let key = ctx.session_key.unwrap_or_default();
        match self.send_method {
            OpenClawSendMethod::ChatSend => {
                self.client.send_message(&key, &ctx.message);
            }
            OpenClawSendMethod::SessionsSend => {
                self.client.send_session_message(&key, &ctx.message);
            }
        }
        Ok(())
    }

    async fn get_history(
        &self,
        session_key: Option<String>,
    ) -> Result<Vec<HistoryMessage>, TransportError> {
        let key = session_key.unwrap_or_default();
        self.client.fetch_history(&key);
        Ok(Vec::new())
    }

    async fn sync_role_context(
        &self,
        _role_id: String,
        _since_event_id: Option<String>,
    ) -> Result<(), TransportError> {
        // OpenClaw uses syncthing-rust for role-context sync, not the WebSocket
        // transport. Report unsupported so the caller can fall back.
        Err(TransportError::Unsupported(
            "OpenClaw dialect uses syncthing-rust for role-context sync".into(),
        ))
    }

    async fn abort(&self) -> Result<(), TransportError> {
        // OpenClaw does not expose a per-turn abort RPC.
        Err(TransportError::Unsupported(
            "OpenClaw abort not supported".into(),
        ))
    }

    async fn request_pairing(
        &self,
        device_id: String,
        public_key: String,
        client_id: String,
        client_mode: String,
        platform: String,
        role: String,
        scopes: Vec<String>,
    ) -> Result<(), TransportError> {
        self.client.request_pairing(
            &device_id,
            &public_key,
            &client_id,
            &client_mode,
            &platform,
            &role,
            &scopes,
        );
        Ok(())
    }

    fn events(&self) -> BoxStream<'static, TransportEvent> {
        let rx = self.rx.clone();
        Box::pin(OpenClawEventStream { rx })
    }

    fn capabilities(&self) -> TransportCaps {
        self.caps.clone()
    }
}

struct OpenClawEventStream {
    rx: Arc<Mutex<UnboundedReceiver<TransportEvent>>>,
}

impl Stream for OpenClawEventStream {
    type Item = TransportEvent;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut rx = self.rx.lock();
        rx.poll_recv(cx)
    }
}

fn map_auth(auth: TransportAuth, device_identity: Option<DeviceIdentity>) -> ClawAuth {
    let device_token = auth.device_token.clone();
    if let (Some(token), Some(device)) = (device_token, device_identity) {
        ClawAuth::TokenWithDevice {
            token,
            device: Box::new(device),
        }
    } else if let Some(token) = auth.device_token {
        ClawAuth::TokenOnly { token }
    } else if let Some(token) = auth.token {
        ClawAuth::TokenOnly { token }
    } else if let Some(token) = auth.bootstrap_token {
        ClawAuth::TokenOnly { token }
    } else if let Some(password) = auth.password {
        // OpenClawAuth uses token field for passwords too in some configs.
        ClawAuth::TokenOnly { token: password }
    } else {
        ClawAuth::TokenOnly {
            token: String::new(),
        }
    }
}

fn translate_openclaw_response(resp: ClawResponse) -> Vec<TransportEvent> {
    let mut out = Vec::new();
    match resp {
        ClawResponse::Connected { gateway_url } => {
            out.push(TransportEvent::Connected {
                gateway_url,
                session_id: None,
            });
        }
        ClawResponse::HistoryLoaded { messages, .. } => {
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
        ClawResponse::SessionMessage {
            role,
            content,
            finished,
        } => {
            if role != "user" && !content.trim().is_empty() {
                out.push(TransportEvent::ChatChunk { content });
            }
            if finished {
                out.push(TransportEvent::Done);
            }
        }
        ClawResponse::PairingResult {
            device_id,
            approved,
            token,
            scopes,
        } => {
            out.push(TransportEvent::DevicePaired {
                device_id,
                approved,
                token,
                scopes,
            });
        }
        ClawResponse::Event {
            event_type,
            payload,
        } => {
            if event_type == "openclaw.reconnect_pending" {
                let reason = payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let seconds = payload.get("seconds").and_then(|v| v.as_u64()).unwrap_or(0);
                out.push(TransportEvent::Reconnecting { reason, seconds });
            } else if matches!(
                event_type.as_str(),
                "done" | "finished" | "turn_end" | "message_end"
            ) {
                out.push(TransportEvent::Done);
            } else if let Some(text) = extract_claw_text(&payload)
                && !text.trim().is_empty()
            {
                out.push(TransportEvent::ChatChunk { content: text });
            }
        }
        ClawResponse::Reply {
            ok,
            method,
            payload,
            ..
        } => {
            if !ok {
                let method_str = method.as_deref().unwrap_or("unknown");
                tracing::info!(
                    method = %method_str,
                    payload = %payload,
                    "OpenClaw translating error Reply"
                );
                let detail = if payload.is_null()
                    || payload.as_object().map(|o| o.is_empty()).unwrap_or(false)
                {
                    "empty error payload".to_string()
                } else {
                    payload.to_string()
                };
                let err = extract_openclaw_error_message(&payload).unwrap_or(detail);
                out.push(TransportEvent::Error {
                    message: format!("OpenClaw {} failed: {}", method_str, err),
                });
            }
        }
        ClawResponse::Error(e) => {
            out.push(TransportEvent::Error { message: e });
        }
    }
    out
}

fn extract_openclaw_error_message(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("error")
        .and_then(|v| v.as_str().map(String::from))
        .or_else(|| {
            payload
                .get("error")
                .and_then(|v| v.get("message"))
                .and_then(|m| m.as_str())
                .map(String::from)
        })
        .or_else(|| {
            payload
                .get("message")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
}

fn extract_claw_text(payload: &serde_json::Value) -> Option<String> {
    for key in ["text", "content", "message", "delta", "answer", "output"] {
        if let Some(text) = payload.get(key).and_then(|v| v.as_str()) {
            return Some(text.into());
        }
    }
    if let Some(content) = payload
        .get("message")
        .or_else(|| payload.get("choices"))
        .and_then(|v| v.get("content"))
        .and_then(|v| v.as_str())
    {
        return Some(content.into());
    }
    if let Some(choices) = payload.get("choices").and_then(|v| v.as_array())
        && let Some(choice) = choices.first()
        && let Some(text) = choice
            .get("message")
            .and_then(|v| v.get("content"))
            .and_then(|v| v.as_str())
            .or_else(|| choice.get("text").and_then(|v| v.as_str()))
            .or_else(|| {
                choice
                    .get("delta")
                    .and_then(|v| v.get("content"))
                    .and_then(|v| v.as_str())
            })
    {
        return Some(text.into());
    }
    if let Some(delta) = payload.get("delta").and_then(|v| v.as_object())
        && let Some(text) = delta.get("content").and_then(|v| v.as_str())
    {
        return Some(text.into());
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_session_message_to_chunk() {
        let events = translate_openclaw_response(ClawResponse::SessionMessage {
            role: "assistant".into(),
            content: "hello".into(),
            finished: true,
        });
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], TransportEvent::ChatChunk { content } if content == "hello"));
        assert!(matches!(&events[1], TransportEvent::Done));
    }

    #[test]
    fn translate_user_message_is_suppressed() {
        let events = translate_openclaw_response(ClawResponse::SessionMessage {
            role: "user".into(),
            content: "hi".into(),
            finished: true,
        });
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], TransportEvent::Done));
    }

    #[test]
    fn translate_pairing_result() {
        let events = translate_openclaw_response(ClawResponse::PairingResult {
            device_id: "d1".into(),
            approved: true,
            token: Some("tok".into()),
            scopes: vec!["read".into()],
        });
        assert!(
            matches!(&events[0], TransportEvent::DevicePaired { device_id, approved, token, scopes }
            if device_id == "d1" && *approved && token == &Some("tok".to_string()) && scopes == &["read".to_string()])
        );
    }

    #[test]
    fn translate_error_reply() {
        let events = translate_openclaw_response(ClawResponse::Reply {
            id: "1".into(),
            ok: false,
            method: Some("sessions.send".into()),
            payload: serde_json::json!({ "error": "not found" }),
        });
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], TransportEvent::Error { message } if message == "OpenClaw sessions.send failed: not found")
        );
    }

    #[test]
    fn translate_reconnect_event() {
        let events = translate_openclaw_response(ClawResponse::Event {
            event_type: "openclaw.reconnect_pending".into(),
            payload: serde_json::json!({"reason":"network","seconds":5}),
        });
        assert!(
            matches!(&events[0], TransportEvent::Reconnecting { reason, seconds }
            if reason == "network" && *seconds == 5)
        );
    }
}
