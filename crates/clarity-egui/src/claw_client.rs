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
//!       "role":"operator",
//!       "scopes":["operator.admin","operator.read","operator.write","operator.approvals","operator.pairing","operator.talk.secrets"],
//!       "auth":{"token":"..."}}}`
//! 2. The Gateway may emit a routine `connect.challenge` event first; token-only
//!    clients ignore it and proceed with the authenticated connect request.
//!    Device-paired clients sign the challenge nonce with their Ed25519 key and
//!    include a `device` block in the connect request.
//! 3. Send:   `{"type":"req","id":"uuid","method":"sessions.send","params":{"sessionKey":"agent:main:main","message":"..."}}`
//! 4. Reply:  `{"type":"res","id":"uuid","ok":true,"payload":{...}}` or
//!    `{"type":"event","event":"hello-ok","payload":{...}}`

use crate::claw_device::DeviceIdentity;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

/// Authentication mode for an OpenClaw connection.
#[derive(Clone)]
pub enum ClawAuth {
    /// Plain token authentication (used by Gray-Cloud and unpaired local
    /// Gateways that do not enforce device scopes).
    TokenOnly { token: String },
    /// Device-paired authentication. The device token authorizes the session;
    /// the Ed25519 private key is used to sign the connect challenge.
    #[allow(dead_code)] // Wired in main.rs once device-token config is exposed.
    DevicePaired {
        device: Box<DeviceIdentity>,
        device_token: String,
    },
}

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
    /// Request pairing for a new device. Used when the Gateway enforces
    /// device scopes (e.g. local KimiClaw).
    #[allow(dead_code)] // Pairing UI will send this once device pairing UX lands.
    RequestPairing {
        device_id: String,
        public_key: String,
        client_id: String,
        client_mode: String,
        platform: String,
        role: String,
        scopes: Vec<String>,
    },
    /// Send raw JSON-RPC request (for future extensibility).
    #[allow(dead_code)]
    RawRequest {
        id: String,
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
    /// A message belonging to the active session (from `session.message` or
    /// `chat` events). These should be rendered in the main chat stream.
    SessionMessage {
        role: String,
        content: String,
        finished: bool,
    },
    /// Device pairing result (approval or pending status).
    PairingResult {
        device_id: String,
        approved: bool,
        token: Option<String>,
        scopes: Vec<String>,
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
    /// Start a token-only WebSocket connection in a background thread.
    pub fn connect(gateway_url: &str, token: &str) -> Self {
        Self::connect_with_auth(
            gateway_url,
            ClawAuth::TokenOnly {
                token: token.into(),
            },
        )
    }

    /// Start a device-paired WebSocket connection in a background thread.
    #[allow(dead_code)] // Used once device-token config is exposed in the UI.
    pub fn connect_with_device(
        gateway_url: &str,
        device: DeviceIdentity,
        device_token: &str,
    ) -> Self {
        Self::connect_with_auth(gateway_url, Self::device_auth(device, device_token.into()))
    }

    fn connect_with_auth(gateway_url: &str, auth: ClawAuth) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<ClawCommand>();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel::<ClawResponse>();

        let gw = gateway_url.to_string();
        let resp = resp_tx.clone();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = resp.send(ClawResponse::Error(format!("tokio runtime: {}", e)));
                    return;
                }
            };
            rt.block_on(run_connection(&gw, auth, cmd_rx, resp));
        });

        Self {
            tx: cmd_tx,
            rx: Arc::new(parking_lot::Mutex::new(resp_rx)),
        }
    }

    /// Box a device identity for the [`ClawAuth::DevicePaired`] variant.
    fn device_auth(device: DeviceIdentity, device_token: String) -> ClawAuth {
        ClawAuth::DevicePaired {
            device: Box::new(device),
            device_token,
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

    /// Request pairing for this device. Should be sent over the temporary
    /// gateway-token connection before switching to device-token auth.
    #[allow(dead_code)] // Used once the pairing UI is wired.
    #[allow(clippy::too_many_arguments)] // Mirrors the OpenClaw protocol fields.
    pub fn request_pairing(
        &self,
        device_id: &str,
        public_key: &str,
        client_id: &str,
        client_mode: &str,
        platform: &str,
        role: &str,
        scopes: &[String],
    ) {
        let _ = self.tx.send(ClawCommand::RequestPairing {
            device_id: device_id.into(),
            public_key: public_key.into(),
            client_id: client_id.into(),
            client_mode: client_mode.into(),
            platform: platform.into(),
            role: role.into(),
            scopes: scopes.to_vec(),
        });
    }

    /// Send a raw JSON-RPC request to the Gateway.
    #[allow(dead_code)] // Used by helper binaries until the UI wires pairing.
    pub fn send_raw_request(&self, id: &str, method: &str, params: serde_json::Value) {
        let _ = self.tx.send(ClawCommand::RawRequest {
            id: id.into(),
            method: method.into(),
            params,
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
    auth: ClawAuth,
    cmd_rx: Receiver<ClawCommand>,
    resp_tx: Sender<ClawResponse>,
) {
    use base64::Engine as _;
    use futures_util::SinkExt;
    use futures_util::StreamExt;
    use rand::Rng as _;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::tungstenite::http::Request;

    // Connect. Local KimiClaw validates the Origin header, so derive it from
    // the Gateway URL (ws -> http, wss -> https).
    let uri: tokio_tungstenite::tungstenite::http::Uri = match gateway_url.parse() {
        Ok(u) => u,
        Err(e) => {
            let _ = resp_tx.send(ClawResponse::Error(format!("parse gateway url: {}", e)));
            return;
        }
    };
    let host = uri.authority().map(|a| a.as_str()).unwrap_or(gateway_url);
    let scheme = uri.scheme_str().unwrap_or("ws");
    let origin = if host.starts_with("127.0.0.1:")
        || host.starts_with("localhost:")
        || host == "127.0.0.1"
        || host == "localhost"
    {
        Some("app://kimi-desktop".to_string())
    } else {
        Some(format!(
            "http{}://{}",
            if scheme == "wss" { "s" } else { "" },
            host
        ))
    };
    let mut key_bytes = [0u8; 16];
    rand::rngs::OsRng.fill(&mut key_bytes);
    let sec_key = base64::engine::general_purpose::STANDARD.encode(key_bytes);
    let mut builder = Request::builder()
        .method("GET")
        .uri(gateway_url)
        .header("Host", host)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", sec_key);
    if let Some(origin) = origin {
        builder = builder.header("Origin", origin);
    }
    let request = match builder.body(()) {
        Ok(req) => req,
        Err(e) => {
            let _ = resp_tx.send(ClawResponse::Error(format!("build ws request: {}", e)));
            return;
        }
    };
    let ws_stream = match connect_async(request).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            let _ = resp_tx.send(ClawResponse::Error(format!("WebSocket connect: {}", e)));
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // ── Step 1: Authenticate ──────────────────────────────────────
    // Token-only connections (Gray-Cloud or local gateway admin) use the
    // generic gateway client identity. Device-paired control-ui connections
    // must use the whitelisted control-ui identity.
    let (client_id, client_mode) = match &auth {
        ClawAuth::TokenOnly { .. } => ("gateway-client", "cli"),
        ClawAuth::DevicePaired { .. } => ("openclaw-control-ui", "webchat"),
    };
    let role = "operator";
    let scopes = vec![
        "operator.admin",
        "operator.read",
        "operator.write",
        "operator.approvals",
        "operator.pairing",
        "operator.talk.secrets",
    ];

    /// Build the signed device attestation payload used by KimiClaw.
    #[allow(clippy::too_many_arguments)] // Mirrors the OpenClaw protocol fields.
    fn build_device_signature_payload(
        version: &str,
        device_id: &str,
        client_id: &str,
        client_mode: &str,
        role: &str,
        scopes: &[&str],
        signed_at_ms: i64,
        token: &str,
        nonce: Option<&str>,
    ) -> String {
        let mut parts = vec![
            version.to_string(),
            device_id.to_string(),
            client_id.to_string(),
            client_mode.to_string(),
            role.to_string(),
            scopes.join(","),
            signed_at_ms.to_string(),
            token.to_string(),
        ];
        if version == "v2" {
            parts.push(nonce.unwrap_or("").to_string());
        }
        parts.join("|")
    }

    /// Build the JSON-RPC `connect` request. For device-paired auth this
    /// includes a signed `device` attestation when `with_device` is true;
    /// for token-only auth it is always omitted.
    #[allow(clippy::too_many_arguments)] // Build helper fed by local constants.
    fn build_connect_req(
        auth: &ClawAuth,
        client_id: &str,
        client_mode: &str,
        role: &str,
        scopes: &[&str],
        id: &str,
        nonce: Option<&str>,
        with_device: bool,
    ) -> serde_json::Value {
        let token = match auth {
            ClawAuth::TokenOnly { token } => token.clone(),
            ClawAuth::DevicePaired { device_token, .. } => device_token.clone(),
        };

        let device_block = match auth {
            ClawAuth::TokenOnly { .. } => None,
            ClawAuth::DevicePaired { device, .. } if with_device => {
                let signed_at_ms = chrono::Utc::now().timestamp_millis();
                let version = if nonce.is_some() { "v2" } else { "v1" };
                let payload = build_device_signature_payload(
                    version,
                    &device.device_id(),
                    client_id,
                    client_mode,
                    role,
                    scopes,
                    signed_at_ms,
                    &token,
                    nonce,
                );
                let signature = device.sign_payload(&payload);
                let mut block = serde_json::json!({
                    "id": device.device_id(),
                    "publicKey": device.public_key(),
                    "signature": signature,
                    "signedAt": signed_at_ms,
                });
                if let Some(n) = nonce {
                    if let Some(obj) = block.as_object_mut() {
                        obj.insert("nonce".to_string(), serde_json::json!(n));
                    }
                }
                Some(block)
            }
            ClawAuth::DevicePaired { .. } => None,
        };

        let mut params = serde_json::json!({
            "minProtocol": 3,
            "maxProtocol": 3,
            "client": {
                "id": client_id,
                "version": env!("CARGO_PKG_VERSION"),
                "platform": "windows",
                "mode": client_mode,
            },
            "role": role,
            "scopes": scopes.to_vec(),
            "auth": { "token": token },
            "caps": ["tool-events"],
        });
        if let Some(device) = device_block {
            if let Some(obj) = params.as_object_mut() {
                obj.insert("device".to_string(), device);
            }
        }

        serde_json::json!({
            "type": "req",
            "id": id,
            "method": "connect",
            "params": params
        })
    }

    // Local KimiClaw sends `connect.challenge` as the first message for
    // device-paired control-ui clients. Read it before sending connect so the
    // v2 device attestation can include the server-provided nonce.
    let mut challenge_nonce: Option<String> = None;
    if matches!(auth, ClawAuth::DevicePaired { .. }) {
        match tokio::time::timeout(Duration::from_secs(5), read.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&text) {
                    let msg_type = resp.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let event = resp.get("event").and_then(|v| v.as_str()).unwrap_or("");
                    if msg_type == "event" && event == "connect.challenge" {
                        challenge_nonce = resp
                            .get("payload")
                            .and_then(|p| p.get("nonce"))
                            .and_then(|v| v.as_str())
                            .map(String::from);
                    }
                }
            }
            Ok(Some(Ok(other))) => {
                let _ = resp_tx.send(ClawResponse::Error(format!(
                    "Unexpected pre-auth message: {:?}",
                    other
                )));
                return;
            }
            Ok(Some(Err(e))) => {
                let _ = resp_tx.send(ClawResponse::Error(format!("WebSocket error: {}", e)));
                return;
            }
            Ok(None) => {
                let _ = resp_tx.send(ClawResponse::Error(
                    "Connection closed before challenge".into(),
                ));
                return;
            }
            Err(_) => {
                // No challenge received; fall through and try a token-only
                // connect. Gray-Cloud and older gateways may not send one.
            }
        }
    }

    let with_device = matches!(auth, ClawAuth::DevicePaired { .. });
    let connect_req = build_connect_req(
        &auth,
        client_id,
        client_mode,
        role,
        &scopes,
        "1",
        challenge_nonce.as_deref(),
        with_device,
    );
    if let Err(e) = write.send(Message::Text(connect_req.to_string())).await {
        let _ = resp_tx.send(ClawResponse::Error(format!("send connect: {}", e)));
        return;
    }

    // Wait for auth response. The Gateway may emit a `connect.challenge`
    // event before the final hello-ok/ok response. Device-paired clients
    // must re-send a connect request with a v2 signature that includes the
    // challenge nonce; token-only clients ignore it.
    let mut challenge_answered = challenge_nonce.is_some();
    let auth_ok = loop {
        match tokio::time::timeout(Duration::from_secs(10), read.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&text) {
                    let msg_type = resp.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let event = resp.get("event").and_then(|v| v.as_str()).unwrap_or("");

                    if msg_type == "event" && event == "connect.challenge" {
                        if matches!(auth, ClawAuth::DevicePaired { .. }) && !challenge_answered {
                            if let Some(nonce) = resp
                                .get("payload")
                                .and_then(|p| p.get("nonce"))
                                .and_then(|v| v.as_str())
                            {
                                let req = build_connect_req(
                                    &auth,
                                    client_id,
                                    client_mode,
                                    role,
                                    &scopes,
                                    "2",
                                    Some(nonce),
                                    true,
                                );
                                if let Err(e) = write.send(Message::Text(req.to_string())).await {
                                    let _ = resp_tx.send(ClawResponse::Error(format!(
                                        "send challenge connect: {}",
                                        e
                                    )));
                                    return;
                                }
                                challenge_answered = true;
                            }
                        } else {
                            tracing::debug!("Ignoring routine OpenClaw connect.challenge");
                        }
                        continue;
                    }

                    let ok = resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false)
                        || (msg_type == "event" && event == "hello-ok");

                    // For device-paired auth, if we already answered a
                    // challenge, require a positive ok/hello-ok before
                    // declaring success.
                    if matches!(auth, ClawAuth::DevicePaired { .. }) && challenge_answered && !ok {
                        continue;
                    }
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
                    Some(ClawCommand::RequestPairing {
                        device_id,
                        public_key,
                        client_id,
                        client_mode,
                        platform,
                        role,
                        scopes,
                    }) => {
                        req_id += 1;
                        let id = req_id.to_string();
                        let req = serde_json::json!({
                            "type": "req",
                            "id": &id,
                            "method": "device.pair.request",
                            "params": {
                                "deviceId": &device_id,
                                "publicKey": &public_key,
                                "clientId": &client_id,
                                "clientMode": &client_mode,
                                "platform": &platform,
                                "role": &role,
                                "scopes": &scopes
                            }
                        });
                        if let Err(e) = write.send(Message::Text(req.to_string())).await {
                            let _ = resp_tx.send(ClawResponse::Error(format!("send pairing: {}", e)));
                            break;
                        }
                    }
                    Some(ClawCommand::RawRequest { id, method, params }) => {
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
                        tracing::debug!(text = %text, "OpenClaw received text");
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
                                    } else if let Some(pairing) = extract_pairing_result(&payload) {
                                        let _ = resp_tx.send(pairing);
                                    } else {
                                        let _ = resp_tx.send(ClawResponse::Reply {
                                            id: val["id"].as_str().unwrap_or("").into(),
                                            ok: val["ok"].as_bool().unwrap_or(false),
                                            payload,
                                        });
                                    }
                                }
                                "event" | "evt" => {
                                    let event_type = val["event"].as_str().unwrap_or("").to_string();
                                    let payload = val.get("payload").cloned().unwrap_or_default();

                                    // Session messages and chat events carry actual
                                    // assistant/user content for the main chat stream.
                                    if event_type == "session.message" || event_type == "chat" {
                                        if let Some((role, content, finished)) =
                                            extract_session_message(&payload)
                                        {
                                            let _ = resp_tx.send(ClawResponse::SessionMessage {
                                                role,
                                                content,
                                                finished,
                                            });
                                        }
                                    } else if event_type == "device.pair.resolved"
                                        || event_type == "device.pair.requested"
                                    {
                                        if let Some(pairing) = extract_pairing_result(&payload) {
                                            let _ = resp_tx.send(pairing);
                                        }
                                    } else {
                                        let _ = resp_tx.send(ClawResponse::Event {
                                            event_type,
                                            payload,
                                        });
                                    }
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

/// Extract a single session/chat message from an event payload.
///
/// Returns `(role, content, finished)`. The `finished` flag is true when the
/// event signals the end of a streaming turn (e.g. `done: true`).
fn extract_session_message(payload: &serde_json::Value) -> Option<(String, String, bool)> {
    // Direct role/content object.
    if let (Some(role), Some(content)) = (
        payload.get("role").and_then(|v| v.as_str()),
        payload.get("content").and_then(|v| v.as_str()),
    ) {
        return Some((role.into(), content.into(), false));
    }
    // Nested message object.
    if let Some(msg) = payload.get("message").or_else(|| payload.get("delta")) {
        if let (Some(role), Some(content)) = (
            msg.get("role").and_then(|v| v.as_str()),
            msg.get("content").and_then(|v| v.as_str()),
        ) {
            return Some((role.into(), content.into(), false));
        }
        if let Some(text) = msg.get("text").and_then(|v| v.as_str()) {
            return Some(("assistant".into(), text.into(), false));
        }
    }
    // Plain text / answer fields without role.
    for key in ["text", "answer", "output", "message"] {
        if let Some(text) = payload.get(key).and_then(|v| v.as_str()) {
            return Some(("assistant".into(), text.into(), false));
        }
    }
    // Done marker.
    if payload
        .get("done")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Some(("assistant".into(), String::new(), true));
    }
    None
}

/// Try to extract a device pairing result from a Gateway payload.
///
/// Handles both the `device.pair.request` response and the
/// `device.pair.resolved` / `device.pair.requested` event payloads.
fn extract_pairing_result(payload: &serde_json::Value) -> Option<ClawResponse> {
    let device_id = payload
        .get("deviceId")
        .or_else(|| payload.get("device_id"))
        .and_then(|v| v.as_str())?
        .to_string();
    let approved = payload
        .get("approved")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let token = payload
        .get("token")
        .and_then(|v| v.as_str())
        .map(String::from);
    let scopes = payload
        .get("scopes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Some(ClawResponse::PairingResult {
        device_id,
        approved,
        token,
        scopes,
    })
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
