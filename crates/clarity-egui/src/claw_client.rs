//! OpenClaw Gateway JSON-RPC WebSocket client.
//!
//! Maintains a persistent WebSocket connection to the OpenClaw Gateway
//! in a background thread. Communication with the UI thread is via
//! std::sync::mpsc channels — the UI sends commands and polls for
//! responses without blocking the frame loop.
//!
//! # Protocol (confirmed by Gray-Cloud)
//!
//! 1. Connect:
//!    `{"type":"req","id":"1","method":"connect","params":{
//!       "minProtocol":3,"maxProtocol":3,
//!       "client":{"id":"gateway-client","version":"0.1.0","platform":"windows","mode":"cli"},
//!       "role":"operator","scopes":["operator.read","operator.write"],
//!       "auth":{"token":"..."}}}`
//! 2. The Gateway may emit a routine `connect.challenge` event first; token-only
//!    clients ignore it and proceed with the authenticated connect request.
//! 3. Send:   `{"type":"req","id":"uuid","method":"sessions.send","params":{"sessionKey":"agent:main:main","message":"..."}}`
//! 4. Reply:  `{"type":"res","id":"uuid","ok":true,"payload":{...}}` or
//!    `{"type":"event","event":"hello-ok","payload":{...}}`

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

/// A command queued from the UI thread to the WebSocket thread.
#[derive(Debug)]
pub enum ClawCommand {
    /// Send a chat message to the target session.
    SendMessage {
        session_key: String,
        message: String,
    },
    /// Fetch message history for a session.
    FetchHistory { session_key: String },
    /// Send raw JSON-RPC request (for future extensibility).
    #[allow(dead_code)]
    RawRequest {
        method: String,
        params: serde_json::Value,
    },
}

/// A response from the WebSocket thread back to the UI thread.
#[derive(Debug, Clone)]
pub enum ClawResponse {
    /// Successfully connected and authenticated.
    Connected { gateway_url: String },
    /// Received a reply to a previously-sent request.
    #[allow(dead_code)]
    Reply {
        id: String,
        ok: bool,
        payload: serde_json::Value,
    },
    /// Server pushed an event (e.g. session update).
    #[allow(dead_code)]
    Event {
        event_type: String,
        payload: serde_json::Value,
    },
    /// Connection or authentication error.
    Error(String),
    /// Session message history loaded.
    HistoryLoaded {
        #[allow(dead_code)]
        session_key: String,
        messages: Vec<GatewayMessage>,
    },
}

/// A single message in a Gateway session.
#[derive(Debug, Clone)]
pub struct GatewayMessage {
    pub role: String,
    pub content: String,
}

/// Handle for communicating with the Claw Gateway WebSocket thread.
///
/// `tx` can be cloned to send commands from multiple threads.
/// `rx` is behind `Arc<Mutex<>>` so `ClawClient` itself can be cloned.
#[derive(Clone)]
pub struct ClawClient {
    tx: Sender<ClawCommand>,
    rx: Arc<parking_lot::Mutex<Receiver<ClawResponse>>>,
}

impl ClawClient {
    /// Start the WebSocket connection in a background thread.
    ///
    /// Returns a `ClawClient` handle immediately. The connection
    /// handshake runs asynchronously; `Connected` or `Error` will
    /// arrive on the response channel when it completes.
    pub fn connect(gateway_url: &str, token: &str) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<ClawCommand>();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel::<ClawResponse>();

        let gw = gateway_url.to_string();
        let tok = token.to_string();
        let resp = resp_tx.clone();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = resp.send(ClawResponse::Error(format!("tokio runtime: {}", e)));
                    return;
                }
            };
            rt.block_on(run_connection(&gw, &tok, cmd_rx, resp));
        });

        Self {
            tx: cmd_tx,
            rx: Arc::new(parking_lot::Mutex::new(resp_rx)),
        }
    }

    /// Send a message to the target agent session.
    pub fn send_message(&self, session_key: &str, message: &str) {
        let _ = self.tx.send(ClawCommand::SendMessage {
            session_key: session_key.into(),
            message: message.into(),
        });
    }

    /// Request message history for a session.
    pub fn fetch_history(&self, session_key: &str) {
        let _ = self.tx.send(ClawCommand::FetchHistory {
            session_key: session_key.into(),
        });
    }

    /// Non-blocking poll for responses from the Gateway.
    #[allow(dead_code)]
    pub fn try_recv(&self) -> Option<ClawResponse> {
        self.rx.lock().try_recv().ok()
    }

    /// Drain all pending responses.
    pub fn drain(&self) -> Vec<ClawResponse> {
        let mut out = Vec::new();
        let rx = self.rx.lock();
        while let Ok(r) = rx.try_recv() {
            out.push(r);
        }
        out
    }
}

// ── Connection loop ────────────────────────────────────────────────────

async fn run_connection(
    gateway_url: &str,
    token: &str,
    cmd_rx: Receiver<ClawCommand>,
    resp_tx: Sender<ClawResponse>,
) {
    use futures_util::SinkExt;
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    // Connect.
    let ws_stream = match connect_async(gateway_url).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            let _ = resp_tx.send(ClawResponse::Error(format!("WebSocket connect: {}", e)));
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // ── Step 1: Authenticate ──────────────────────────────────────
    let connect_req = serde_json::json!({
        "type": "req",
        "id": "1",
        "method": "connect",
        "params": {
            "minProtocol": 3,
            "maxProtocol": 3,
            "client": {
                "id": "gateway-client",
                "version": "0.1.0",
                "platform": "windows",
                "mode": "cli"
            },
            "role": "operator",
            "scopes": ["operator.read", "operator.write"],
            "auth": { "token": token }
        }
    });

    if let Err(e) = write.send(Message::Text(connect_req.to_string())).await {
        let _ = resp_tx.send(ClawResponse::Error(format!("send connect: {}", e)));
        return;
    }

    // Wait for auth response. The Gateway emits a routine connect.challenge
    // event on every connection; token-only clients ignore it.
    let auth_ok = loop {
        match tokio::time::timeout(Duration::from_secs(10), read.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&text) {
                    let msg_type = resp.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let event = resp.get("event").and_then(|v| v.as_str()).unwrap_or("");
                    if msg_type == "event" && event == "connect.challenge" {
                        tracing::debug!("Ignoring routine OpenClaw connect.challenge");
                        continue;
                    }
                    let ok = resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false)
                        || (msg_type == "event" && event == "hello-ok");
                    break ok;
                }
                break false;
            }
            Ok(Some(Ok(other))) => {
                let _ = resp_tx.send(ClawResponse::Error(format!(
                    "Unexpected auth response: {:?}",
                    other
                )));
                return;
            }
            Ok(Some(Err(e))) => {
                let _ = resp_tx.send(ClawResponse::Error(format!("WebSocket error: {}", e)));
                return;
            }
            Ok(None) => {
                let _ = resp_tx.send(ClawResponse::Error("Connection closed during auth".into()));
                return;
            }
            Err(_) => {
                let _ = resp_tx.send(ClawResponse::Error("Auth timeout".into()));
                return;
            }
        }
    };

    if auth_ok {
        let _ = resp_tx.send(ClawResponse::Connected {
            gateway_url: gateway_url.into(),
        });
    } else {
        let _ = resp_tx.send(ClawResponse::Error("Auth failed".into()));
        return;
    }

    // ── Step 2: Bridge sync mpsc to async ─────────────────────────
    let (async_tx, mut async_rx) = tokio::sync::mpsc::channel::<ClawCommand>(32);
    tokio::task::spawn_blocking(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            if async_tx.blocking_send(cmd).is_err() {
                break;
            }
        }
    });

    // ── Step 3: Main loop — send commands, receive responses ─────
    let mut req_id: u64 = 0;

    loop {
        tokio::select! {
            cmd = async_rx.recv() => {
                match cmd {
                    Some(ClawCommand::SendMessage { session_key, message }) => {
                        req_id += 1;
                        let id = req_id.to_string();
                        let req = serde_json::json!({
                            "type": "req",
                            "id": &id,
                            "method": "sessions.send",
                            "params": {
                                "sessionKey": &session_key,
                                "message": &message
                            }
                        });
                        if let Err(e) = write.send(Message::Text(req.to_string())).await {
                            let _ = resp_tx.send(ClawResponse::Error(format!("send: {}", e)));
                            break;
                        }
                    }
                    Some(ClawCommand::FetchHistory { session_key }) => {
                        // Try threads.list first (common in OpenClaw/ACP),
                        // fall back to sessions.list.
                        req_id += 1;
                        let id = req_id.to_string();
                        let req = serde_json::json!({
                            "type": "req",
                            "id": &id,
                            "method": "threads.list",
                            "params": {
                                "sessionKey": &session_key,
                                "limit": 50
                            }
                        });
                        if let Err(e) = write.send(Message::Text(req.to_string())).await {
                            let _ = resp_tx.send(ClawResponse::Error(format!("send history: {}", e)));
                            break;
                        }
                    }
                    Some(ClawCommand::RawRequest { method, params }) => {
                        req_id += 1;
                        let id = req_id.to_string();
                        let req = serde_json::json!({
                            "type": "req",
                            "id": &id,
                            "method": &method,
                            "params": &params
                        });
                        if let Err(e) = write.send(Message::Text(req.to_string())).await {
                            let _ = resp_tx.send(ClawResponse::Error(format!("send: {}", e)));
                            break;
                        }
                    }
                    None => {
                        // Channel closed — UI dropped the handle.
                        break;
                    }
                }
            }

            // Incoming messages from the Gateway.
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                            let msg_type = val["type"].as_str().unwrap_or("");
                            match msg_type {
                                "res" => {
                                    let payload = val.get("payload").cloned().unwrap_or_default();
                                    // Detect history responses: threads or messages arrays.
                                    if let Some(msgs) = extract_messages(&payload) {
                                        let _ = resp_tx.send(ClawResponse::HistoryLoaded {
                                            session_key: String::new(),
                                            messages: msgs,
                                        });
                                    } else {
                                        let _ = resp_tx.send(ClawResponse::Reply {
                                            id: val["id"].as_str().unwrap_or("").into(),
                                            ok: val["ok"].as_bool().unwrap_or(false),
                                            payload,
                                        });
                                    }
                                }
                                "evt" => {
                                    let _ = resp_tx.send(ClawResponse::Event {
                                        event_type: val["event"].as_str().unwrap_or("").into(),
                                        payload: val.get("payload").cloned().unwrap_or_default(),
                                    });
                                }
                                _ => {
                                    tracing::debug!(?val, "Unexpected Gateway message");
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        let _ = resp_tx.send(ClawResponse::Error("Gateway closed connection".into()));
                        break;
                    }
                    Some(Ok(_)) => {
                        // Binary, Ping, Pong, Frame — ignored.
                    }
                    Some(Err(e)) => {
                        let _ = resp_tx.send(ClawResponse::Error(format!("WebSocket: {}", e)));
                        break;
                    }
                    None => break,
                }
            }
        }
    }
}

/// Try to extract messages from a Gateway response payload.
///
/// Handles multiple response shapes:
/// - `{ threads: [{ messages: [...] }] }`  (threads.list)
/// - `{ messages: [...] }`                 (sessions.get)
/// - `{ data: [...] }`                     (generic list)
fn extract_messages(payload: &serde_json::Value) -> Option<Vec<GatewayMessage>> {
    // Try threads.list format: threads[0].messages
    if let Some(threads) = payload.get("threads").and_then(|v| v.as_array()) {
        if let Some(thread) = threads.first() {
            if let Some(msgs) = thread.get("messages").and_then(|v| v.as_array()) {
                return Some(parse_message_list(msgs));
            }
        }
    }
    // Try direct messages array.
    if let Some(msgs) = payload.get("messages").and_then(|v| v.as_array()) {
        return Some(parse_message_list(msgs));
    }
    // Try data array.
    if let Some(items) = payload.get("data").and_then(|v| v.as_array()) {
        return Some(parse_message_list(items));
    }
    None
}

fn parse_message_list(items: &[serde_json::Value]) -> Vec<GatewayMessage> {
    items
        .iter()
        .filter_map(|m| {
            let role = m
                .get("role")
                .or_else(|| m.get("from"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let content = m
                .get("content")
                .or_else(|| m.get("text"))
                .or_else(|| m.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if content.is_empty() {
                None
            } else {
                Some(GatewayMessage {
                    role: role.into(),
                    content: content.into(),
                })
            }
        })
        .collect()
}
