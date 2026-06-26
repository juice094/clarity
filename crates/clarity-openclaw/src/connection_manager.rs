//! Unified Claw connection manager with protocol auto-detection.
//!
//! Consumers open a single Gateway URL and the manager determines whether the
//! remote speaks the native Clarity Gateway WebSocket protocol or OpenClaw
//! JSON-RPC by reading the server's first message. The detected dialect is then
//! used for the lifetime of the connection.
//!
//! # Protocol strategy
//!
//! - **Gateway WebSocket** is the single canonical protocol for Clarity
//!   internal mesh (`clarity-claw` ↔ `clarity-gateway` ↔ frontends).
//! - **OpenClaw JSON-RPC** is an external-interop fallback for talking to
//!   out-of-process KimiClaw / OpenClaw Gateways. It participates in basic
//!   chat/history/pairing but not in Clarity-internal semantics such as
//!   role-context sync, `WireMessage`, or MCP tool events.
//!
//! # Current implementation
//!
//! The manager performs a short-lived probe handshake to read the server's
//! first frame, then closes the probe and starts a dedicated client for the
//! detected protocol. This keeps the existing `ClawClient` and `GatewayClient`
//! implementations intact while still centralizing protocol selection.
//!
//! ponytail: one extra WebSocket handshake per connection. A future iteration
//! can reuse the probed stream by moving the connection loops of the two
//! clients behind the `ProtocolHandler` trait; the public API stays the same.

use crate::client::{ClawAuth, ClawClient, ClawResponse};
use crate::gateway_client::{GatewayClient, GatewayResponse};
use crate::protocol::{DetectedProtocol, ProtocolCommand, ProtocolEvent, ProtocolHistoryMessage};
use crate::types::OpenClawSendMethod;
use futures_util::StreamExt;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Handle to a managed Claw connection.
#[derive(Clone)]
pub struct ClawConnectionManager {
    tx: Sender<ManagerCommand>,
    rx: Arc<Mutex<Receiver<ProtocolEvent>>>,
}

impl ClawConnectionManager {
    /// Open a connection to `url` and auto-detect the remote protocol.
    ///
    /// The probe read times out after 5 seconds; on failure a
    /// [`ProtocolEvent::Error`] is emitted.
    pub fn connect(url: &str) -> Self {
        Self::connect_with_auth(url, None)
    }

    /// Open a connection with an optional OpenClaw authentication config.
    ///
    /// `auth` is only used when the remote speaks OpenClaw JSON-RPC.
    /// Defaults to `OpenClawSendMethod::SessionsSend` for OpenClaw dialect.
    pub fn connect_with_auth(url: &str, auth: Option<ClawAuth>) -> Self {
        Self::connect_with_options(url, auth, OpenClawSendMethod::SessionsSend)
    }

    /// Open a connection with auth and an explicit OpenClaw send-method choice.
    ///
    /// `send_method` is only used when the remote speaks OpenClaw JSON-RPC.
    pub fn connect_with_options(
        url: &str,
        auth: Option<ClawAuth>,
        send_method: OpenClawSendMethod,
    ) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<ManagerCommand>();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel::<ProtocolEvent>();

        let url = url.to_string();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = resp_tx.send(ProtocolEvent::Error(format!("tokio runtime: {}", e)));
                    return;
                }
            };
            rt.block_on(run_manager(&url, auth, send_method, cmd_rx, resp_tx));
        });

        Self {
            tx: cmd_tx,
            rx: Arc::new(Mutex::new(resp_rx)),
        }
    }

    /// Send a command to the active protocol handler.
    pub fn send(&self, cmd: ProtocolCommand) {
        let _ = self.tx.send(ManagerCommand::Protocol(cmd));
    }

    /// Set or clear the passphrase used to encrypt role-context events at rest.
    pub fn set_role_passphrase(&self, role_id: &str, passphrase: &str) {
        let _ = self.tx.send(ManagerCommand::Protocol(
            ProtocolCommand::SetRolePassphrase {
                role_id: role_id.into(),
                passphrase: passphrase.into(),
            },
        ));
    }

    /// Non-blocking poll for a normalized protocol event.
    pub fn try_recv(&self) -> Option<ProtocolEvent> {
        self.rx.lock().ok()?.try_recv().ok()
    }

    /// Drain all pending normalized protocol events.
    pub fn drain(&self) -> Vec<ProtocolEvent> {
        let mut out = Vec::new();
        if let Ok(rx) = self.rx.lock() {
            while let Ok(r) = rx.try_recv() {
                out.push(r);
            }
        }
        out
    }
}

#[derive(Clone, Debug)]
enum ManagerCommand {
    Protocol(ProtocolCommand),
}

async fn run_manager(
    url: &str,
    auth: Option<ClawAuth>,
    send_method: OpenClawSendMethod,
    cmd_rx: Receiver<ManagerCommand>,
    resp_tx: Sender<ProtocolEvent>,
) {
    let detected = match probe_protocol(url).await {
        Ok(d) => d,
        Err(e) => {
            let _ = resp_tx.send(ProtocolEvent::Error(e));
            return;
        }
    };

    match detected {
        DetectedProtocol::GatewayWebSocket => {
            run_gateway_manager(url, cmd_rx, resp_tx).await;
        }
        DetectedProtocol::OpenClawJsonRpc => {
            run_openclaw_manager(url, auth, send_method, cmd_rx, resp_tx).await;
        }
    }
}

/// Probe the server by opening a WebSocket, reading the first text frame, and
/// closing the connection. Returns the detected protocol dialect.
async fn probe_protocol(url: &str) -> Result<DetectedProtocol, String> {
    let (mut ws_stream, _) = connect_async(url)
        .await
        .map_err(|e| format!("WebSocket probe connect: {}", e))?;

    let first = match tokio::time::timeout(Duration::from_secs(5), ws_stream.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => text,
        Ok(Some(Ok(_))) => return Err("probe received non-text first frame".into()),
        Ok(Some(Err(e))) => return Err(format!("probe WebSocket error: {}", e)),
        Ok(None) => return Err("probe connection closed before first frame".into()),
        Err(_) => return Err("probe first frame timeout".into()),
    };

    // Explicitly close the probe stream so the server cleans up before the
    // real connection is opened.
    let _ = ws_stream.close(None).await;

    Ok(DetectedProtocol::from_first_frame(&first))
}

async fn run_gateway_manager(
    url: &str,
    cmd_rx: Receiver<ManagerCommand>,
    resp_tx: Sender<ProtocolEvent>,
) {
    let client = GatewayClient::connect(url);
    let (internal_tx, internal_rx) = std::sync::mpsc::channel::<GatewayResponse>();

    // Bridge GatewayClient events into normalized ProtocolEvents.
    let bridge_resp_tx = resp_tx.clone();
    let client_clone = client.clone();
    std::thread::spawn(move || {
        loop {
            for r in client_clone.drain() {
                let _ = bridge_resp_tx.send(translate_gateway_response(r, &bridge_resp_tx));
            }
            std::thread::sleep(Duration::from_millis(10));
            if internal_rx.try_recv().is_err() {
                // Keep running until the manager handle is dropped.
                continue;
            }
            break;
        }
    });

    for cmd in cmd_rx {
        match cmd {
            ManagerCommand::Protocol(ProtocolCommand::Chat { message, .. }) => {
                // Gateway WebSocket is the single native Clarity protocol;
                // chat.send is the canonical method.
                client.chat(&message, true);
            }
            ManagerCommand::Protocol(ProtocolCommand::GetHistory { .. }) => {
                client.get_history();
            }
            ManagerCommand::Protocol(ProtocolCommand::SubscribeSession { .. }) => {
                // Gateway does not require explicit session subscription.
            }
            ManagerCommand::Protocol(ProtocolCommand::SubscribeMessages { .. }) => {
                // Gateway does not require explicit message subscription.
            }
            ManagerCommand::Protocol(ProtocolCommand::SyncRoleContext {
                role_id,
                since_event_id,
                device_id,
            }) => {
                client.sync_role_context(&role_id, since_event_id.as_deref(), &device_id);
            }
            ManagerCommand::Protocol(ProtocolCommand::SetRolePassphrase { .. }) => {
                // Gateway dialect does not persist role-context at rest.
            }
        }
    }

    let _ = internal_tx.send(GatewayResponse::Error("manager dropped".into()));
}

fn translate_gateway_response(
    resp: GatewayResponse,
    _resp_tx: &Sender<ProtocolEvent>,
) -> ProtocolEvent {
    match resp {
        GatewayResponse::Connected {
            gateway_url,
            session_id,
        } => ProtocolEvent::Connected {
            gateway_url,
            session_id: Some(session_id),
        },
        GatewayResponse::Chat { message, .. } => ProtocolEvent::ChatChunk(message),
        GatewayResponse::WireMessage { payload } => ProtocolEvent::WireMessage(payload),
        GatewayResponse::History { messages } => ProtocolEvent::History(
            messages
                .into_iter()
                .map(|m| ProtocolHistoryMessage {
                    role: m.role,
                    content: m.content,
                })
                .collect(),
        ),
        GatewayResponse::RoleContextSynced {
            role_id,
            events,
            next_cursor,
            online_devices,
        } => ProtocolEvent::RoleContextSynced {
            role_id,
            events,
            next_cursor,
            online_devices,
        },
        GatewayResponse::Error(e) => ProtocolEvent::Error(e),
    }
}

async fn run_openclaw_manager(
    url: &str,
    auth: Option<ClawAuth>,
    send_method: OpenClawSendMethod,
    cmd_rx: Receiver<ManagerCommand>,
    resp_tx: Sender<ProtocolEvent>,
) {
    let auth = auth.unwrap_or_else(|| ClawAuth::TokenOnly {
        token: String::new(),
    });
    let client = ClawClient::connect_with_auth(url, auth);
    let (internal_tx, internal_rx) = std::sync::mpsc::channel::<ClawResponse>();

    let bridge_resp_tx = resp_tx.clone();
    let client_clone = client.clone();
    std::thread::spawn(move || {
        loop {
            for r in client_clone.drain() {
                for e in translate_openclaw_response(r) {
                    let _ = bridge_resp_tx.send(e);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
            if internal_rx.try_recv().is_err() {
                continue;
            }
            break;
        }
    });

    for cmd in cmd_rx {
        match cmd {
            ManagerCommand::Protocol(ProtocolCommand::Chat {
                session_key,
                message,
            }) => {
                // OpenClaw JSON-RPC is the external-fallback dialect. Some
                // Gateways expect `sessions.send`, others (KimiClaw/ACP-style)
                // expect `chat.send`. The send_method is configured per
                // connection; Gateway WebSocket uses chat.send in its own branch.
                match send_method {
                    OpenClawSendMethod::ChatSend => {
                        client.send_message(&session_key, &message);
                    }
                    OpenClawSendMethod::SessionsSend => {
                        client.send_session_message(&session_key, &message);
                    }
                }
            }
            ManagerCommand::Protocol(ProtocolCommand::GetHistory { session_key }) => {
                client.fetch_history(&session_key);
            }
            ManagerCommand::Protocol(ProtocolCommand::SubscribeSession { key }) => {
                client.subscribe_session(&key);
            }
            ManagerCommand::Protocol(ProtocolCommand::SubscribeMessages { key }) => {
                client.subscribe_messages(&key);
            }
            ManagerCommand::Protocol(ProtocolCommand::SyncRoleContext { .. }) => {
                let _ = resp_tx.send(ProtocolEvent::Unsupported {
                    reason: "OpenClaw dialect uses syncthing-rust for role-context sync".into(),
                });
            }
            ManagerCommand::Protocol(ProtocolCommand::SetRolePassphrase {
                role_id,
                passphrase,
            }) => {
                client.set_role_passphrase(&role_id, &passphrase);
            }
        }
    }

    let _ = internal_tx.send(ClawResponse::Error("manager dropped".into()));
}

fn translate_openclaw_response(resp: ClawResponse) -> Vec<ProtocolEvent> {
    let mut out = Vec::new();
    match resp {
        ClawResponse::Connected { gateway_url } => {
            out.push(ProtocolEvent::Connected {
                gateway_url,
                session_id: None,
            });
        }
        ClawResponse::HistoryLoaded { messages, .. } => {
            out.push(ProtocolEvent::History(
                messages
                    .into_iter()
                    .map(|m| ProtocolHistoryMessage {
                        role: m.role,
                        content: m.content,
                    })
                    .collect(),
            ));
        }
        ClawResponse::Reply {
            ok,
            method,
            payload,
            ..
        } => {
            if ok {
                // OpenClaw command replies are acknowledgments, not chat
                // streams. Chat content arrives via SessionMessage / Event.
                // We intentionally do not map ok replies to ChatChunk to keep
                // the external fallback dialect from leaking into Gateway
                // semantics.
                tracing::debug!(method = ?method, payload = ?payload, "OpenClaw ok reply ignored");
            } else {
                let method_str = method.as_deref().unwrap_or("unknown");
                let err = extract_openclaw_error_message(&payload);
                let detail = if payload.is_null()
                    || payload.as_object().map(|o| o.is_empty()).unwrap_or(false)
                {
                    "empty error payload".to_string()
                } else {
                    payload.to_string()
                };
                tracing::debug!(
                    method = method_str,
                    payload = %detail,
                    "OpenClaw error reply"
                );
                out.push(ProtocolEvent::Error(format!(
                    "OpenClaw {} failed: {}",
                    method_str,
                    err.as_deref().unwrap_or(&detail)
                )));
            }
        }
        ClawResponse::SessionMessage {
            role,
            content,
            finished,
        } => {
            if role != "user" && !content.trim().is_empty() {
                out.push(ProtocolEvent::ChatChunk(content));
            }
            if finished {
                out.push(ProtocolEvent::Done);
            }
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
                out.push(ProtocolEvent::ReconnectPending { reason, seconds });
            } else if matches!(
                event_type.as_str(),
                "done" | "finished" | "turn_end" | "message_end"
            ) {
                out.push(ProtocolEvent::Done);
            } else if let Some(text) = extract_claw_text(&payload) {
                if !text.trim().is_empty() {
                    out.push(ProtocolEvent::ChatChunk(text));
                }
            }
        }
        ClawResponse::PairingResult {
            device_id,
            approved,
            token,
            scopes,
        } => {
            out.push(ProtocolEvent::PairingResult {
                device_id,
                approved,
                token,
                scopes,
            });
        }
        ClawResponse::Error(e) => {
            out.push(ProtocolEvent::Error(e));
        }
    }
    out
}

/// Try to extract a human-readable error message from an OpenClaw error payload.
///
/// Handles several common shapes:
/// - `{ "error": "string" }`
/// - `{ "error": { "message": "string" } }`
/// - `{ "message": "string" }`
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

/// Try to extract human-readable text from an OpenClaw Gateway payload.
///
/// Different Gateway implementations emit responses under different keys, so
/// this helper checks the common shapes without being tied to one schema.
fn extract_claw_text(payload: &serde_json::Value) -> Option<String> {
    // Direct string fields.
    for key in ["text", "content", "message", "delta", "answer", "output"] {
        if let Some(text) = payload.get(key).and_then(|v| v.as_str()) {
            return Some(text.into());
        }
    }
    // Nested message object.
    if let Some(content) = payload
        .get("message")
        .or_else(|| payload.get("choices"))
        .and_then(|v| v.get("content"))
        .and_then(|v| v.as_str())
    {
        return Some(content.into());
    }
    // OpenAI-style choices array.
    if let Some(choices) = payload.get("choices").and_then(|v| v.as_array()) {
        if let Some(choice) = choices.first() {
            if let Some(text) = choice
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
        }
    }
    // Nested delta object.
    if let Some(delta) = payload.get("delta").and_then(|v| v.as_object()) {
        if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
            return Some(text.into());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_gateway_connected() {
        let (tx, rx) = std::sync::mpsc::channel();
        let resp = GatewayResponse::Connected {
            gateway_url: "ws://localhost".into(),
            session_id: "s1".into(),
        };
        let ev = translate_gateway_response(resp, &tx);
        match ev {
            ProtocolEvent::Connected {
                gateway_url,
                session_id,
            } => {
                assert_eq!(gateway_url, "ws://localhost");
                assert_eq!(session_id, Some("s1".to_string()));
            }
            _ => panic!("expected connected"),
        }
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn translate_openclaw_session_messages() {
        let events = translate_openclaw_response(ClawResponse::SessionMessage {
            role: "assistant".into(),
            content: "hello".into(),
            finished: true,
        });
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], ProtocolEvent::ChatChunk(c) if c == "hello"));
        assert!(matches!(&events[1], ProtocolEvent::Done));
    }

    #[test]
    fn translate_openclaw_error() {
        let events = translate_openclaw_response(ClawResponse::Error("boom".into()));
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ProtocolEvent::Error(e) if e == "boom"));
    }

    #[test]
    fn translate_openclaw_reply_error_extracts_string_error() {
        let events = translate_openclaw_response(ClawResponse::Reply {
            id: "1".into(),
            ok: false,
            method: Some("sessions.send".into()),
            payload: serde_json::json!({ "error": "session not found" }),
        });
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], ProtocolEvent::Error(e) if e == "OpenClaw sessions.send failed: session not found")
        );
    }

    #[test]
    fn translate_openclaw_reply_error_extracts_nested_message() {
        let events = translate_openclaw_response(ClawResponse::Reply {
            id: "2".into(),
            ok: false,
            method: Some("sessions.send".into()),
            payload: serde_json::json!({
                "error": { "code": -32000, "message": "invalid session key" }
            }),
        });
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], ProtocolEvent::Error(e) if e == "OpenClaw sessions.send failed: invalid session key")
        );
    }

    #[test]
    fn translate_openclaw_reply_error_falls_back_to_payload() {
        let events = translate_openclaw_response(ClawResponse::Reply {
            id: "3".into(),
            ok: false,
            method: Some("sessions.send".into()),
            payload: serde_json::Value::Null,
        });
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ProtocolEvent::Error(e) if e.contains("empty error payload")));
    }
}
