//! OpenClaw Gateway WebSocket client.
//!
//! Maintains a long-lived WebSocket to a local Kimi Desktop OpenClaw Gateway,
//! handles the `connect.challenge` handshake, routes RPC responses back to
//! callers, and broadcasts server events to subscribers.

use crate::device::DeviceIdentity;
use crate::openclaw_gateway::protocol::{
    ConnectChallenge, ConnectParams, HelloOk, OpenClawDeviceProof, OpenClawFrame,
    build_cli_connect_params, build_device_auth_payload, build_device_connect_params,
};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{interval, timeout};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Default request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Interval for application-level pings when no traffic occurs.
const PING_INTERVAL: Duration = Duration::from_secs(25);
/// Maximum consecutive connection failures before surfacing a terminal error.
const MAX_RETRIES: usize = 5;

/// A high-level event emitted by the OpenClaw Gateway connection.
#[derive(Clone, Debug)]
pub enum OpenClawEvent {
    /// Handshake completed; carries server capabilities and policy.
    Connected(HelloOk),
    /// Connection lost; carries the reason.
    Disconnected(String),
    /// An unsolicited server event (`chat`, `agent`, `health`, `tick`, ...).
    ServerEvent {
        /// Event name.
        event: String,
        /// Event payload.
        payload: Option<serde_json::Value>,
        /// Monotonic sequence number, when present.
        seq: Option<u64>,
    },
    /// A recoverable or terminal error message.
    Error(String),
}

/// Handle to an OpenClaw Gateway connection.
#[derive(Clone)]
pub struct OpenClawGatewayClient {
    /// Channel for issuing outbound RPCs.
    cmd_tx: mpsc::UnboundedSender<ClientCommand>,
    /// Broadcast channel for server events.
    event_tx: broadcast::Sender<OpenClawEvent>,
    /// Latest hello-ok snapshot (for feature/capability checks).
    hello_ok: Arc<Mutex<Option<HelloOk>>>,
}

/// Internal command sent into the connection task.
enum ClientCommand {
    /// Perform an RPC and return the payload or error.
    Call {
        id: String,
        method: String,
        params: Option<serde_json::Value>,
        resp_tx: oneshot::Sender<Result<serde_json::Value, OpenClawClientError>>,
    },
}

/// Errors returned by RPC calls.
#[derive(Debug, thiserror::Error)]
pub enum OpenClawClientError {
    /// The Gateway returned an error response.
    #[error("gateway error {code}: {message}")]
    GatewayError {
        /// Error code.
        code: String,
        /// Error message.
        message: String,
        /// Optional details.
        details: Option<serde_json::Value>,
    },
    /// The request timed out.
    #[error("request timeout")]
    Timeout,
    /// The connection task exited.
    #[error("connection closed")]
    ConnectionClosed,
    /// Wrapped generic error.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<serde_json::Error> for OpenClawClientError {
    fn from(err: serde_json::Error) -> Self {
        Self::Other(err.into())
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for OpenClawClientError {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::ConnectionClosed
    }
}

/// Internal bundle holding the connect template plus an optional device
/// identity that needs the challenge nonce before it can be signed.
struct ConnectionConfig {
    params: ConnectParams,
    device_identity: Option<DeviceIdentity>,
}

impl OpenClawGatewayClient {
    /// Connect without a device identity (CLI mode).
    ///
    /// This is sufficient for local session/chat control, but some methods may
    /// require a paired device for full scopes.
    pub async fn connect(url: &str, token: &str) -> Result<Self> {
        let platform = platform_string();
        let params = build_cli_connect_params(token, &platform, Some("Desktop"));
        Self::connect_with_params(url, params).await
    }

    /// Connect with a device identity proof (paired-device mode).
    pub async fn connect_with_device(
        url: &str,
        token: &str,
        device: &DeviceIdentity,
    ) -> Result<Self> {
        let platform = platform_string();
        // Device proof is generated after the challenge nonce is known, so the
        // params here carry a placeholder device field that gets replaced.
        let params = build_device_connect_params(
            token,
            &platform,
            Some("Desktop"),
            OpenClawDeviceProof {
                id: String::new(),
                public_key: String::new(),
                signature: String::new(),
                signed_at: 0,
                nonce: String::new(),
            },
        );
        Self::connect_with_config(
            url,
            ConnectionConfig {
                params,
                device_identity: Some(device.clone()),
            },
        )
        .await
    }

    /// Connect with explicit connect parameters.
    pub async fn connect_with_params(url: &str, params: ConnectParams) -> Result<Self> {
        Self::connect_with_config(
            url,
            ConnectionConfig {
                params,
                device_identity: None,
            },
        )
        .await
    }

    async fn connect_with_config(url: &str, config: ConnectionConfig) -> Result<Self> {
        let url = url.to_string();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<ClientCommand>();
        let (event_tx, _event_rx) = broadcast::channel::<OpenClawEvent>(256);
        let hello_ok = Arc::new(Mutex::new(None));

        let event_tx_clone = event_tx.clone();
        let hello_ok_clone = hello_ok.clone();
        tokio::spawn(async move {
            run_connection_loop(&url, config, cmd_rx, event_tx_clone, hello_ok_clone).await;
        });

        Ok(Self {
            cmd_tx,
            event_tx,
            hello_ok,
        })
    }

    /// Perform an RPC and await the payload.
    pub async fn call(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, OpenClawClientError> {
        let id = uuid::Uuid::new_v4().to_string();
        let (resp_tx, resp_rx) = oneshot::channel();
        self.cmd_tx
            .send(ClientCommand::Call {
                id,
                method: method.to_string(),
                params,
                resp_tx,
            })
            .map_err(|_| OpenClawClientError::ConnectionClosed)?;

        match timeout(REQUEST_TIMEOUT, resp_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(OpenClawClientError::ConnectionClosed),
            Err(_) => Err(OpenClawClientError::Timeout),
        }
    }

    /// Subscribe to server events.
    pub fn subscribe(&self) -> broadcast::Receiver<OpenClawEvent> {
        self.event_tx.subscribe()
    }

    /// Latest hello-ok snapshot, if handshake has completed.
    pub fn hello_ok(&self) -> Option<HelloOk> {
        self.hello_ok.lock().clone()
    }

    /// Check whether the server advertises a given method.
    pub fn supports_method(&self, method: &str) -> bool {
        self.hello_ok
            .lock()
            .as_ref()
            .map(|h| h.features.methods.iter().any(|m| m == method))
            .unwrap_or(false)
    }
}

/// Build a device proof with the given nonce.
///
/// `token` is the signature token used in the v3 payload.
fn build_device_proof(device: &DeviceIdentity, token: &str, nonce: &str) -> OpenClawDeviceProof {
    let signed_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let payload = build_device_auth_payload(
        &device.device_id(),
        "gateway-client",
        "backend",
        "operator",
        &[
            "operator.admin".to_string(),
            "operator.read".to_string(),
            "operator.write".to_string(),
            "operator.approvals".to_string(),
            "operator.pairing".to_string(),
        ],
        signed_at,
        token,
        nonce,
        &platform_string(),
        Some("Desktop"),
    );
    let signature = device.sign_payload(&payload);
    OpenClawDeviceProof {
        id: device.device_id(),
        public_key: device.public_key(),
        signature,
        signed_at,
        nonce: nonce.to_string(),
    }
}

async fn run_connection_loop(
    url: &str,
    config: ConnectionConfig,
    mut cmd_rx: mpsc::UnboundedReceiver<ClientCommand>,
    event_tx: broadcast::Sender<OpenClawEvent>,
    hello_ok: Arc<Mutex<Option<HelloOk>>>,
) {
    let mut failures = 0usize;

    loop {
        match run_single_connection(url, &config, &mut cmd_rx, &event_tx, &hello_ok).await {
            Ok(()) => {
                failures = 0;
            }
            Err(e) => {
                failures += 1;
                let reason = e.to_string();
                tracing::warn!(
                    failures,
                    error = %reason,
                    "OpenClaw Gateway connection error"
                );
                let _ = event_tx.send(OpenClawEvent::Disconnected(reason.clone()));
                if failures >= MAX_RETRIES {
                    let _ = event_tx.send(OpenClawEvent::Error(format!(
                        "OpenClaw Gateway connection exhausted {} retries: {}",
                        MAX_RETRIES, reason
                    )));
                    break;
                }
                tokio::time::sleep(crate::util::next_backoff(failures)).await;
            }
        }
    }
}

async fn run_single_connection(
    url: &str,
    config: &ConnectionConfig,
    cmd_rx: &mut mpsc::UnboundedReceiver<ClientCommand>,
    event_tx: &broadcast::Sender<OpenClawEvent>,
    hello_ok: &Arc<Mutex<Option<HelloOk>>>,
) -> Result<()> {
    let (ws_stream, _) = connect_async(url)
        .await
        .with_context(|| format!("connect to {}", url))?;
    let (mut write, mut read) = ws_stream.split();

    // 1. Expect connect.challenge
    let challenge = match read.next().await {
        Some(Ok(Message::Text(text))) => parse_challenge(&text)?,
        Some(Ok(other)) => anyhow::bail!("unexpected first frame: {:?}", other),
        Some(Err(e)) => anyhow::bail!("websocket error: {}", e),
        None => anyhow::bail!("connection closed before challenge"),
    };

    // 2. Build connect params. If a device identity is configured, sign the
    //    proof using the actual challenge nonce now.
    let mut params = config.params.clone();
    if let Some(ref device) = config.device_identity {
        let token = params
            .auth
            .as_ref()
            .and_then(|a| a.token.as_deref())
            .unwrap_or("");
        params.device = Some(build_device_proof(device, token, &challenge.nonce));
    }

    // 3. Send connect request.
    let connect_req = build_connect_frame(&params)?;
    write
        .send(Message::Text(serde_json::to_string(&connect_req)?))
        .await?;

    // 3. Expect hello-ok response.
    let hello = match read.next().await {
        Some(Ok(Message::Text(text))) => parse_hello_ok(&text)?,
        Some(Ok(other)) => anyhow::bail!("unexpected handshake response: {:?}", other),
        Some(Err(e)) => anyhow::bail!("websocket error: {}", e),
        None => anyhow::bail!("connection closed before hello-ok"),
    };

    *hello_ok.lock() = Some(hello.clone());
    let _ = event_tx.send(OpenClawEvent::Connected(hello));

    // 4. Main loop: pump commands and read frames.
    let mut pending: HashMap<
        String,
        oneshot::Sender<Result<serde_json::Value, OpenClawClientError>>,
    > = HashMap::new();
    let mut ping_interval = interval(PING_INTERVAL);

    loop {
        tokio::select! {
            biased;
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(ClientCommand::Call { id, method, params, resp_tx }) => {
                        let frame = OpenClawFrame::Req {
                            id: id.clone(),
                            method,
                            params,
                        };
                        pending.insert(id, resp_tx);
                        let text = serde_json::to_string(&frame)?;
                        if let Err(e) = write.send(Message::Text(text)).await {
                            // Drain pending with errors.
                            for (_, tx) in pending.drain() {
                                let _ = tx.send(Err(OpenClawClientError::Other(anyhow::anyhow!("{}", e))));
                            }
                            anyhow::bail!("send failed: {}", e);
                        }
                    }
                    None => {
                        // Command channel closed; exit cleanly.
                        let _ = write.close().await;
                        return Ok(());
                    }
                }
            }

            _ = ping_interval.tick() => {
                if write.send(Message::Text(r#"{"type":"req","id":"ping","method":"ping"}"#.into())).await.is_err() {
                    anyhow::bail!("ping send failed");
                }
            }

            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_frame(&text, &mut pending, event_tx)?;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        anyhow::bail!("connection closed");
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => anyhow::bail!("websocket error: {}", e),
                }
            }
        }
    }
}

fn parse_challenge(text: &str) -> Result<ConnectChallenge> {
    let frame: OpenClawFrame =
        serde_json::from_str(text).with_context(|| format!("parse challenge frame: {}", text))?;
    match frame {
        OpenClawFrame::Event {
            event,
            payload: Some(payload),
            ..
        } if event == "connect.challenge" => {
            serde_json::from_value(payload).context("parse connect.challenge payload")
        }
        _ => anyhow::bail!("expected connect.challenge event, got: {}", text),
    }
}

fn build_connect_frame(params: &ConnectParams) -> Result<OpenClawFrame> {
    Ok(OpenClawFrame::Req {
        id: uuid::Uuid::new_v4().to_string(),
        method: "connect".to_string(),
        params: Some(serde_json::to_value(params)?),
    })
}

fn parse_hello_ok(text: &str) -> Result<HelloOk> {
    let frame: OpenClawFrame =
        serde_json::from_str(text).with_context(|| format!("parse hello-ok frame: {}", text))?;
    match frame {
        OpenClawFrame::Res {
            ok: true,
            payload: Some(payload),
            ..
        } => {
            let hello: HelloOk =
                serde_json::from_value(payload).context("parse hello-ok payload")?;
            Ok(hello)
        }
        OpenClawFrame::Res {
            ok: false,
            error: Some(err),
            ..
        } => anyhow::bail!("connect failed {}: {}", err.code, err.message),
        _ => anyhow::bail!("expected hello-ok response, got: {}", text),
    }
}

fn handle_frame(
    text: &str,
    pending: &mut HashMap<String, oneshot::Sender<Result<serde_json::Value, OpenClawClientError>>>,
    event_tx: &broadcast::Sender<OpenClawEvent>,
) -> Result<()> {
    let frame: OpenClawFrame =
        serde_json::from_str(text).with_context(|| format!("parse frame: {}", text))?;

    match frame {
        OpenClawFrame::Res {
            id,
            ok,
            payload,
            error,
        } => {
            if let Some(tx) = pending.remove(&id) {
                let result = if ok {
                    Ok(payload.unwrap_or(serde_json::Value::Null))
                } else {
                    Err(error
                        .map(|e| OpenClawClientError::GatewayError {
                            code: e.code,
                            message: e.message,
                            details: e.details,
                        })
                        .unwrap_or(OpenClawClientError::Other(anyhow::anyhow!(
                            "unknown gateway error"
                        ))))
                };
                let _ = tx.send(result);
            }
        }
        OpenClawFrame::Event {
            event,
            payload,
            seq,
        } => {
            if event == "pong" {
                // Application-level pong; no action needed.
                return Ok(());
            }
            let _ = event_tx.send(OpenClawEvent::ServerEvent {
                event,
                payload,
                seq,
            });
        }
        _ => {}
    }
    Ok(())
}

fn platform_string() -> String {
    #[cfg(target_os = "windows")]
    {
        "win32".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "darwin".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "linux".to_string()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "unknown".to_string()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    pub async fn mock_openclaw_server() -> (
        std::net::SocketAddr,
        tokio::sync::mpsc::UnboundedReceiver<String>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::unbounded_channel::<String>();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (mut write, mut read) = accept_async(stream).await.unwrap().split();

            // Send challenge
            let challenge = serde_json::json!({
                "type": "event",
                "event": "connect.challenge",
                "payload": { "nonce": "test-nonce", "ts": 1234 }
            });
            write
                .send(Message::Text(challenge.to_string()))
                .await
                .unwrap();

            // Read connect
            let mut connected = false;
            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                if !connected {
                                    // Reply hello-ok
                                    let hello = serde_json::json!({
                                        "type": "res",
                                        "id": serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"].as_str().unwrap(),
                                        "ok": true,
                                        "payload": {
                                            "type": "hello-ok",
                                            "protocol": 3,
                                            "server": { "version": "test", "connId": "c1" },
                                            "features": { "methods": ["chat.send"], "events": ["chat"] },
                                            "policy": { "maxPayload": 1000, "maxBufferedBytes": 2000, "tickIntervalMs": 30000 }
                                        }
                                    });
                                    write.send(Message::Text(hello.to_string())).await.unwrap();
                                    connected = true;
                                } else {
                                    // Ignore application-level pings in tests.
                                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                                        if value.get("method").and_then(|m| m.as_str()) == Some("ping") {
                                            continue;
                                        }
                                    }
                                    let _ = tx.send(text);
                                }
                            }
                            _ => break,
                        }
                    }
                }
            }
        });

        (addr, rx)
    }

    #[tokio::test]
    async fn client_handshakes_and_calls_method() {
        let (addr, _rx) = mock_openclaw_server().await;
        let url = format!("ws://{}", addr);
        let client = OpenClawGatewayClient::connect(&url, "test-token")
            .await
            .unwrap();

        // Wait for the handshake to complete by polling hello_ok.
        let mut hello = None;
        for _ in 0..50 {
            if let Some(h) = client.hello_ok() {
                hello = Some(h);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(hello.is_some(), "handshake did not complete");

        assert!(client.supports_method("chat.send"));

        // Call chat.send; mock server will echo it back but not reply.
        let result = client
            .call("chat.send", Some(serde_json::json!({ "text": "hi" })))
            .await;
        // Mock doesn't reply to the call, so it should time out.
        assert!(matches!(result, Err(OpenClawClientError::Timeout)));
    }

    #[test]
    fn platform_string_matches_os() {
        let s = platform_string();
        assert!(["win32", "darwin", "linux", "unknown"].contains(&s.as_str()));
    }
}
