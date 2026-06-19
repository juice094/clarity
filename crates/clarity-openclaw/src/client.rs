//! OpenClaw Gateway JSON-RPC WebSocket client.
//!
//! Maintains a persistent WebSocket connection to the OpenClaw Gateway
//! in a background thread. Communication with the UI thread is via
//! std::sync::mpsc channels — the UI sends commands and polls for
//! responses without blocking the frame loop.
//!
//! # Protocol (confirmed against remote OpenClaw Gateways)
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

#![allow(missing_docs)]

use crate::device::DeviceIdentity;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

/// Authentication mode for an OpenClaw connection.
#[derive(Clone)]
pub enum ClawAuth {
    /// Plain token authentication (used by remote OpenClaw Gateways and
    /// unpaired local Gateways that do not enforce device scopes).
    TokenOnly { token: String },
    /// Device-paired authentication. The device token authorizes the session;
    /// the Ed25519 private key is used to sign the connect challenge.
    #[allow(dead_code)] // Wired in main.rs once device-token config is exposed.
    DevicePaired {
        device: Box<DeviceIdentity>,
        device_token: String,
    },
    /// Remote token + device attestation. Some Gateways clear scopes for
    /// non-loopback token-only connections unless the client proves device
    /// identity by signing the `connect.challenge` nonce.
    TokenWithDevice {
        token: String,
        device: Box<DeviceIdentity>,
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
    /// Send a message via `sessions.send` (remote OpenClaw path).
    SendSessionMessage { key: String, message: String },
    /// Fetch message history for a session.
    FetchHistory { session_key: String },
    /// Subscribe to session-level events for a session.
    SubscribeSession { key: String },
    /// Subscribe to message-level events for a session.
    SubscribeMessages { key: String },
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
        /// The method of the original request, if it was tracked.
        method: Option<String>,
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

    /// Start a remote-token connection with a device attestation.
    ///
    /// The admin token is used for both authentication and as the signed
    /// payload token. The Gateway must support the `connect.challenge` flow.
    pub fn connect_with_remote_device(
        gateway_url: &str,
        token: &str,
        device: DeviceIdentity,
    ) -> Self {
        Self::connect_with_auth(
            gateway_url,
            ClawAuth::TokenWithDevice {
                token: token.into(),
                device: Box::new(device),
            },
        )
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

    /// Subscribe to session-level events for a session.
    pub fn subscribe_session(&self, key: &str) {
        let _ = self
            .tx
            .send(ClawCommand::SubscribeSession { key: key.into() });
    }

    /// Subscribe to message-level events for a session.
    pub fn subscribe_messages(&self, key: &str) {
        let _ = self
            .tx
            .send(ClawCommand::SubscribeMessages { key: key.into() });
    }

    /// Send a message to a session using the Gateway's `sessions.send` method.
    ///
    /// This is the remote-OpenClaw path; local KimiClaw typically expects
    /// `chat.send` via [`Self::send_message`].
    pub fn send_session_message(&self, key: &str, message: &str) {
        let _ = self.tx.send(ClawCommand::SendSessionMessage {
            key: key.into(),
            message: message.into(),
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

/// Classification of why a single connection attempt ended.
enum ConnectionExit {
    /// Configuration or authentication error: do not retry.
    Terminal(String),
    /// Network or transient error: retry with exponential backoff.
    Transient(String),
}

/// Compute the next exponential-backoff delay, capping at 30 seconds.
///
/// Sequence: 1s, 2s, 4s, 8s, 16s, 30s, 30s, ...
fn next_backoff(current: Duration) -> Duration {
    let next = current.saturating_mul(2);
    let cap = Duration::from_secs(30);
    if next > cap { cap } else { next }
}

async fn run_connection(
    gateway_url: &str,
    auth: ClawAuth,
    cmd_rx: Receiver<ClawCommand>,
    resp_tx: Sender<ClawResponse>,
) {
    let (async_tx, mut async_rx) = tokio::sync::mpsc::unbounded_channel::<ClawCommand>();
    tokio::task::spawn_blocking(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            if async_tx.send(cmd).is_err() {
                break;
            }
        }
    });

    let mut delay = Duration::from_secs(1);
    loop {
        match run_single_connection(gateway_url, &auth, &mut async_rx, &resp_tx).await {
            Ok(()) => break,
            Err(ConnectionExit::Terminal(reason)) => {
                let _ = resp_tx.send(ClawResponse::Error(reason));
                break;
            }
            Err(ConnectionExit::Transient(reason)) => {
                tracing::warn!(
                    reason = %reason,
                    delay = %delay.as_secs(),
                    "OpenClaw connection transient failure; retrying"
                );
                let _ = resp_tx.send(ClawResponse::Event {
                    event_type: "openclaw.reconnect_pending".to_string(),
                    payload: serde_json::json!({
                        "reason": reason,
                        "seconds": delay.as_secs(),
                    }),
                });
                tokio::time::sleep(delay).await;
                delay = next_backoff(delay);
            }
        }
    }
}

async fn run_single_connection(
    gateway_url: &str,
    auth: &ClawAuth,
    async_rx: &mut tokio::sync::mpsc::UnboundedReceiver<ClawCommand>,
    resp_tx: &Sender<ClawResponse>,
) -> Result<(), ConnectionExit> {
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
            return Err(ConnectionExit::Terminal(format!(
                "parse gateway url: {}",
                e
            )));
        }
    };
    let host = uri.authority().map(|a| a.as_str()).unwrap_or(gateway_url);
    let _scheme = uri.scheme_str().unwrap_or("ws");
    let origin = if host.starts_with("127.0.0.1:")
        || host.starts_with("localhost:")
        || host == "127.0.0.1"
        || host == "localhost"
    {
        // Local KimiClaw validates the Origin and expects the desktop app URI.
        Some("app://kimi-desktop".to_string())
    } else {
        // Remote Gateways reject synthetic non-local Origins like
        // app://kimi-desktop, but accept a missing Origin header.
        None
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
        Err(e) => return Err(ConnectionExit::Terminal(format!("build ws request: {}", e))),
    };
    let ws_stream = match connect_async(request).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            return Err(ConnectionExit::Transient(format!(
                "WebSocket connect: {}",
                e
            )));
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // ── Step 1: Authenticate ──────────────────────────────────────
    // Token-only connections (remote Gateway admin token or local gateway
    // token) use the generic gateway client identity. Device-paired control-ui
    // connections must use the whitelisted control-ui identity.
    let (client_id, client_mode) = match auth {
        ClawAuth::TokenOnly { .. } | ClawAuth::TokenWithDevice { .. } => ("gateway-client", "cli"),
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
            ClawAuth::TokenOnly { token }
            | ClawAuth::TokenWithDevice { token, .. }
            | ClawAuth::DevicePaired {
                device_token: token,
                ..
            } => token.clone(),
        };

        let device_block = match auth {
            ClawAuth::TokenOnly { .. } => None,
            ClawAuth::TokenWithDevice { device, .. } | ClawAuth::DevicePaired { device, .. }
                if with_device =>
            {
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
            ClawAuth::TokenWithDevice { .. } | ClawAuth::DevicePaired { .. } => None,
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
    if matches!(
        auth,
        ClawAuth::DevicePaired { .. } | ClawAuth::TokenWithDevice { .. }
    ) {
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
                return Err(ConnectionExit::Terminal(format!(
                    "Unexpected pre-auth message: {:?}",
                    other
                )));
            }
            Ok(Some(Err(e))) => {
                return Err(ConnectionExit::Terminal(format!("WebSocket error: {}", e)));
            }
            Ok(None) => {
                return Err(ConnectionExit::Terminal(
                    "Connection closed before challenge".into(),
                ));
            }
            Err(_) => {
                return Err(ConnectionExit::Terminal(
                    "Pre-auth challenge timeout".into(),
                ));
            }
        }
    }

    let with_device = matches!(
        auth,
        ClawAuth::DevicePaired { .. } | ClawAuth::TokenWithDevice { .. }
    );
    let connect_req = build_connect_req(
        auth,
        client_id,
        client_mode,
        role,
        &scopes,
        "1",
        challenge_nonce.as_deref(),
        with_device,
    );
    if let Err(e) = write.send(Message::Text(connect_req.to_string())).await {
        return Err(ConnectionExit::Transient(format!("send connect: {}", e)));
    }

    // Wait for auth response. The Gateway may emit a `connect.challenge`
    // event before the final hello-ok/ok response. Device-paired clients
    // must re-send a connect request with a v2 signature that includes the
    // challenge nonce; token-only clients ignore it.
    let mut challenge_answered = challenge_nonce.is_some();
    let auth_ok = loop {
        // Token-only clients never read a pre-auth challenge, so there is
        // nothing to answer. Device-attested clients set this to true once
        // they have resent connect with the signed nonce.
        match tokio::time::timeout(Duration::from_secs(10), read.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&text) {
                    let msg_type = resp.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let event = resp.get("event").and_then(|v| v.as_str()).unwrap_or("");

                    if msg_type == "event" && event == "connect.challenge" {
                        if matches!(
                            auth,
                            ClawAuth::DevicePaired { .. } | ClawAuth::TokenWithDevice { .. }
                        ) && !challenge_answered
                        {
                            if let Some(nonce) = resp
                                .get("payload")
                                .and_then(|p| p.get("nonce"))
                                .and_then(|v| v.as_str())
                            {
                                let req = build_connect_req(
                                    auth,
                                    client_id,
                                    client_mode,
                                    role,
                                    &scopes,
                                    "2",
                                    Some(nonce),
                                    true,
                                );
                                if let Err(e) = write.send(Message::Text(req.to_string())).await {
                                    return Err(ConnectionExit::Transient(format!(
                                        "send challenge connect: {}",
                                        e
                                    )));
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

                    // For device-attested auth, if we already answered a
                    // challenge, require a positive ok/hello-ok before
                    // declaring success.
                    if matches!(
                        auth,
                        ClawAuth::DevicePaired { .. } | ClawAuth::TokenWithDevice { .. }
                    ) && challenge_answered
                        && !ok
                    {
                        continue;
                    }
                    break ok;
                }
                break false;
            }
            Ok(Some(Ok(other))) => {
                return Err(ConnectionExit::Terminal(format!(
                    "Unexpected auth response: {:?}",
                    other
                )));
            }
            Ok(Some(Err(e))) => {
                return Err(ConnectionExit::Terminal(format!("WebSocket error: {}", e)));
            }
            Ok(None) => {
                return Err(ConnectionExit::Terminal(
                    "Connection closed during auth".into(),
                ));
            }
            Err(_) => {
                return Err(ConnectionExit::Terminal("Auth timeout".into()));
            }
        }
    };

    if !auth_ok {
        return Err(ConnectionExit::Terminal("Auth failed".into()));
    }

    let _ = resp_tx.send(ClawResponse::Connected {
        gateway_url: gateway_url.into(),
    });

    // ── Step 2: Main loop ─────────────────────────────────────────
    let mut req_id: u64 = 0;
    // Track pending request IDs so replies can be attributed to their original
    // method. This lets the UI decide whether an ok=false response should be
    // surfaced to the user or only logged.
    let mut pending_methods: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    // KimiClaw/OpenClaw assistant streams send the cumulative message text
    // on every chunk. Track the previous cumulative text so the UI receives
    // deltas and does not duplicate content.
    let mut last_assistant_text = String::new();

    let start = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut ping_interval = tokio::time::interval_at(start, Duration::from_secs(30));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                if let Err(e) = write.send(Message::Ping(Vec::new())).await {
                    return Err(ConnectionExit::Transient(format!("ping send: {}", e)));
                }
            }

            cmd = async_rx.recv() => {
                match cmd {
                    Some(ClawCommand::SendMessage { session_key, message }) => {
                        req_id += 1;
                        let id = req_id.to_string();
                        let method = "chat.send".to_string();
                        pending_methods.insert(id.clone(), method.clone());
                        let idempotency_key = format!(
                            "clarity-{}-{}",
                            id,
                            base64::engine::general_purpose::URL_SAFE_NO_PAD
                                .encode(rand::random::<[u8; 8]>())
                        );
                        let req = serde_json::json!({
                            "type": "req",
                            "id": &id,
                            "method": &method,
                            "params": {
                                "sessionKey": &session_key,
                                "message": &message,
                                "deliver": false,
                                "idempotencyKey": idempotency_key
                            }
                        });
                        if let Err(e) = write.send(Message::Text(req.to_string())).await {
                            return Err(ConnectionExit::Transient(format!("send: {}", e)));
                        }
                    }
                    Some(ClawCommand::FetchHistory { session_key }) => {
                        // Try threads.list first (common in OpenClaw/ACP),
                        // fall back to sessions.list.
                        req_id += 1;
                        let id = req_id.to_string();
                        let method = "threads.list".to_string();
                        pending_methods.insert(id.clone(), method.clone());
                        let req = serde_json::json!({
                            "type": "req",
                            "id": &id,
                            "method": &method,
                            "params": {
                                "sessionKey": &session_key,
                                "limit": 50
                            }
                        });
                        if let Err(e) = write.send(Message::Text(req.to_string())).await {
                            return Err(ConnectionExit::Transient(format!("send history: {}", e)));
                        }
                    }
                    Some(ClawCommand::SubscribeSession { key }) => {
                        req_id += 1;
                        let id = req_id.to_string();
                        let method = "sessions.subscribe".to_string();
                        pending_methods.insert(id.clone(), method.clone());
                        let req = serde_json::json!({
                            "type": "req",
                            "id": &id,
                            "method": &method,
                            "params": { "key": &key }
                        });
                        if let Err(e) = write.send(Message::Text(req.to_string())).await {
                            return Err(ConnectionExit::Transient(format!("subscribe session: {}", e)));
                        }
                    }
                    Some(ClawCommand::SubscribeMessages { key }) => {
                        req_id += 1;
                        let id = req_id.to_string();
                        let method = "sessions.messages.subscribe".to_string();
                        pending_methods.insert(id.clone(), method.clone());
                        let req = serde_json::json!({
                            "type": "req",
                            "id": &id,
                            "method": &method,
                            "params": { "key": &key }
                        });
                        if let Err(e) = write.send(Message::Text(req.to_string())).await {
                            return Err(ConnectionExit::Transient(format!("subscribe messages: {}", e)));
                        }
                    }
                    Some(ClawCommand::SendSessionMessage { key, message }) => {
                        req_id += 1;
                        let id = req_id.to_string();
                        let method = "sessions.send".to_string();
                        pending_methods.insert(id.clone(), method.clone());
                        let idempotency_key = format!(
                            "clarity-{}-{}",
                            id,
                            base64::engine::general_purpose::URL_SAFE_NO_PAD
                                .encode(rand::random::<[u8; 8]>())
                        );
                        let req = serde_json::json!({
                            "type": "req",
                            "id": &id,
                            "method": &method,
                            "params": {
                                "key": &key,
                                "message": &message,
                                "idempotencyKey": idempotency_key
                            }
                        });
                        if let Err(e) = write.send(Message::Text(req.to_string())).await {
                            return Err(ConnectionExit::Transient(format!("send session message: {}", e)));
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
                        let method = "device.pair.request".to_string();
                        pending_methods.insert(id.clone(), method.clone());
                        let req = serde_json::json!({
                            "type": "req",
                            "id": &id,
                            "method": &method,
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
                            return Err(ConnectionExit::Transient(format!("send pairing: {}", e)));
                        }
                    }
                    Some(ClawCommand::RawRequest { id, method, params }) => {
                        pending_methods.insert(id.clone(), method.clone());
                        let req = serde_json::json!({
                            "type": "req",
                            "id": &id,
                            "method": &method,
                            "params": &params
                        });
                        if let Err(e) = write.send(Message::Text(req.to_string())).await {
                            return Err(ConnectionExit::Transient(format!("send: {}", e)));
                        }
                    }
                    None => {
                        // Channel closed — UI dropped the handle.
                        return Ok(());
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
                                        let id = val["id"].as_str().unwrap_or("").to_string();
                                        let method = pending_methods.remove(&id);
                                        let _ = resp_tx.send(ClawResponse::Reply {
                                            id,
                                            method,
                                            ok: val["ok"].as_bool().unwrap_or(false),
                                            payload,
                                        });
                                    }
                                }
                                "event" | "evt" => {
                                    let event_type = val["event"].as_str().unwrap_or("").to_string();
                                    let payload = val.get("payload").cloned().unwrap_or_default();

                                    // Session messages, chat events, and OpenClaw/KimiClaw
                                    // agent streaming events carry content for the main chat
                                    // stream.
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
                                    } else if event_type == "agent" {
                                        if let Some((role, content, finished)) =
                                            extract_agent_event(&payload)
                                        {
                                            // Convert cumulative assistant text into deltas.
                                            let delta = if role == "assistant" && !finished {
                                                compute_stream_delta(&last_assistant_text, &content)
                                            } else {
                                                content.clone()
                                            };
                                            if finished {
                                                last_assistant_text.clear();
                                            } else if role == "assistant" {
                                                last_assistant_text.clone_from(&content);
                                            }
                                            if !delta.is_empty() || finished {
                                                let _ = resp_tx.send(ClawResponse::SessionMessage {
                                                    role,
                                                    content: delta,
                                                    finished,
                                                });
                                            }
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
                        return Err(ConnectionExit::Transient("Gateway closed connection".into()));
                    }
                    Some(Ok(_)) => {
                        // Binary, Ping, Pong, Frame — ignored.
                    }
                    Some(Err(e)) => {
                        return Err(ConnectionExit::Transient(format!("WebSocket: {}", e)));
                    }
                    None => {
                        return Err(ConnectionExit::Transient("WebSocket stream ended".into()));
                    }
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

/// Compute the incremental delta between the previous cumulative stream text
/// and the current cumulative stream text.
///
/// OpenClaw/KimiClaw assistant streams send the full cumulative text on every
/// chunk. This helper returns only the newly appended suffix, or the full text
/// when the stream backtracks or starts fresh.
fn compute_stream_delta(prev: &str, current: &str) -> String {
    if current.starts_with(prev) {
        current.chars().skip(prev.chars().count()).collect()
    } else {
        current.to_string()
    }
}

/// Extract a streaming agent event from OpenClaw/KimiClaw.
///
/// Assistant events have `"stream": "assistant"` with `"data": { "text": "..." }`.
/// Lifecycle end events have `"stream": "lifecycle"` and `"data": { "phase": "end" }`.
fn extract_agent_event(payload: &serde_json::Value) -> Option<(String, String, bool)> {
    let stream = payload.get("stream").and_then(|v| v.as_str())?;
    match stream {
        "assistant" => {
            let data = payload.get("data")?;
            // Prefer the cumulative text; fall back to delta.
            let text = data
                .get("text")
                .and_then(|v| v.as_str())
                .or_else(|| data.get("delta").and_then(|v| v.as_str()))?;
            Some(("assistant".into(), text.into(), false))
        }
        "lifecycle" => {
            let phase = payload
                .get("data")
                .and_then(|d| d.get("phase"))
                .and_then(|v| v.as_str());
            if phase == Some("end") {
                Some(("assistant".into(), String::new(), true))
            } else {
                None
            }
        }
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{extract_agent_event, next_backoff};

    #[test]
    fn extract_agent_event_assistant_chunk() {
        let payload = serde_json::json!({
            "stream": "assistant",
            "data": { "text": "hello" }
        });
        assert!(matches!(
            extract_agent_event(&payload),
            Some((ref role, ref text, false)) if role == "assistant" && text == "hello"
        ));
    }

    #[test]
    fn extract_agent_event_lifecycle_end() {
        let payload = serde_json::json!({
            "stream": "lifecycle",
            "data": { "phase": "end" }
        });
        assert!(matches!(
            extract_agent_event(&payload),
            Some((ref role, ref text, true)) if role == "assistant" && text.is_empty()
        ));
    }

    #[test]
    fn extract_agent_event_ignores_unknown_stream() {
        let payload = serde_json::json!({ "stream": "heartbeat" });
        assert!(extract_agent_event(&payload).is_none());
    }

    #[test]
    fn backoff_progression_capped_at_thirty_seconds() {
        let mut delay = Duration::from_secs(1);
        let expected = [1, 2, 4, 8, 16, 30, 30];
        for &secs in &expected {
            assert_eq!(delay.as_secs(), secs);
            delay = next_backoff(delay);
        }
    }
}
