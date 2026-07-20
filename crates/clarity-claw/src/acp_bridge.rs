//! KimiClaw ACP (Agent Cloud Proxy) bridge client skeleton.
//!
//! The ACP bridge is the WebSocket tunnel that connects a local KimiClaw Gateway
//! to Kimi's cloud bot infrastructure (`wss://www.kimi.com/api-claw/bots/agent-ws`).
//! This module provides a UI-agnostic backend client that can:
//!
//! - load the bridge configuration from `~/.kimi_openclaw/openclaw.json`
//! - open an authenticated WebSocket to the ACP endpoint
//! - keep the connection alive with periodic pings
//! - forward `subscribe` / `send_message_stream` envelopes
//! - relay cloud user messages to the local Clarity Gateway
//!
//! ponytail: this is a protocol skeleton. The exact cloud message schema is
//! inferred from local logs and may drift when Kimi updates the server. Keep
//! the envelope layer thin so schema changes only require updating
//! `AcpMessage`/`AcpEvent`.

use clarity_contract::retry::RetryConfig;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

/// ACP bridge configuration as it appears inside
/// `plugins.entries.kimi-claw.config.bridge` in `openclaw.json`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct AcpBridgeConfig {
    /// Bridge mode, expected to be `acp`.
    pub mode: String,
    /// WebSocket URL of the ACP endpoint.
    pub url: String,
    /// REST API host for file/resource operations.
    #[serde(rename = "kimiapiHost")]
    pub kimi_api_host: String,
    /// Local directory where downloaded Kimi files are stored.
    #[serde(rename = "kimiFileDownloadDir")]
    pub kimi_file_download_dir: PathBuf,
    /// Bot token used to authenticate the ACP session.
    pub token: String,
    /// Optional local OpenClaw Gateway token.
    ///
    /// When bridging to a local Kimi Desktop OpenClaw Gateway, this token is
    /// sent during the `connect` handshake. Falls back to [`Self::token`] and
    /// then to an empty string if unset.
    #[serde(skip)]
    pub local_token: Option<String>,
}

/// Which local backend the ACP bridge should forward cloud messages to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LocalBackend {
    /// The original Clarity Gateway WebSocket (`ws://.../ws`).
    #[default]
    ClarityGateway,
    /// Kimi Desktop's local OpenClaw Gateway WebSocket.
    OpenClawGateway,
}

/// Persisted ACP bridge state.
///
/// Kept minimal intentionally: only the cloud `chat_id` is needed so the bridge
/// can resume forwarding Gateway responses after a process restart.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AcpBridgeState {
    /// Last known chat/session id from the cloud.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
}

impl AcpBridgeState {
    /// Load state from disk, returning default if missing or unreadable.
    pub fn load<P: AsRef<Path>>(path: P) -> Self {
        match std::fs::read_to_string(path.as_ref()) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save state to disk, creating parent directories as needed.
    ///
    /// Failures are logged and ignored so a transient disk issue does not
    /// break the live relay.
    pub fn save<P: AsRef<Path>>(&self, path: P) {
        if let Some(parent) = path.as_ref().parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("failed to create bridge state dir: {}", e);
                return;
            }
        }
        match serde_json::to_string_pretty(self) {
            Ok(raw) => {
                if let Err(e) = std::fs::write(path.as_ref(), raw) {
                    tracing::warn!("failed to write bridge state: {}", e);
                }
            }
            Err(e) => tracing::warn!("failed to serialize bridge state: {}", e),
        }
    }
}

/// Runtime status snapshot of the ACP bridge.
///
/// ponytail: kept in memory only; reconnect_count resets on process restart.
/// Persist to disk next to `acp-bridge-state.json` if cross-run metrics are
/// needed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AcpBridgeStatus {
    /// Whether both ACP and Gateway WebSockets are currently connected.
    pub connected: bool,
    /// Last known chat/session id from the cloud.
    pub chat_id: Option<String>,
    /// Number of reconnect attempts since the process started.
    pub reconnect_count: u32,
    /// Last fatal error that triggered a reconnect, if any.
    pub last_error: Option<String>,
}

fn emit_status(tx: &Option<tokio::sync::watch::Sender<AcpBridgeStatus>>, status: AcpBridgeStatus) {
    if let Some(tx) = tx {
        let _ = tx.send(status);
    }
}

/// High-level ACP message sent toward the cloud.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AcpMessage {
    /// Subscribe to events for a chat/session.
    Subscribe {
        /// Chat id to subscribe to.
        chat_id: String,
    },
    /// Send a user message stream to the cloud Agent.
    SendMessageStream {
        /// Target chat id.
        chat_id: String,
        /// Message payload.
        #[serde(flatten)]
        payload: serde_json::Value,
    },
    /// Application-level ping.
    Ping,
}

/// High-level event received from the cloud.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AcpEvent {
    /// A ping was acknowledged.
    Pong,
    /// Raw cloud payload that the higher layer must interpret.
    ///
    /// ponytail: keep this escape hatch until the cloud schema stabilizes.
    Raw(serde_json::Value),
    /// The connection closed or encountered a fatal error.
    Error(String),
}

/// Backend handle to the ACP bridge.
///
/// The client is intentionally thin: it owns the WebSocket task and exposes
/// `send` / `try_recv` methods so a local Gateway or headless relay can pump
/// messages without touching UI state.
#[derive(Clone)]
pub struct AcpBridgeClient {
    tx: tokio::sync::mpsc::UnboundedSender<AcpMessage>,
}

impl AcpBridgeClient {
    /// Open a new ACP bridge connection from a config.
    pub async fn connect(config: &AcpBridgeConfig) -> anyhow::Result<(Self, AcpEventReceiver)> {
        Self::connect_url(&config.url, &config.token).await
    }

    /// Open a new ACP bridge connection from explicit URL and token.
    pub async fn connect_url(url: &str, token: &str) -> anyhow::Result<(Self, AcpEventReceiver)> {
        let request = build_ws_request(url, token)?;
        let (ws_stream, _) = tokio_tungstenite::connect_async(request).await?;
        let (mut write, mut read) = ws_stream.split();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AcpMessage>();
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<AcpEvent>();

        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            let ping_interval = tokio::time::interval(Duration::from_secs(30));
            tokio::pin!(ping_interval);

            loop {
                tokio::select! {
                    _ = ping_interval.tick() => {
                        if write.send(Message::Text(r#"{"method":"ping"}"#.into())).await.is_err() {
                            break;
                        }
                    }
                    Some(msg) = rx.recv() => {
                        let text = match serde_json::to_string(&msg) {
                            Ok(t) => t,
                            Err(e) => {
                                let _ = event_tx_clone.send(AcpEvent::Error(format!("serialize: {}", e)));
                                continue;
                            }
                        };
                        if write.send(Message::Text(text)).await.is_err() {
                            break;
                        }
                    }
                    next = read.next() => {
                        match next {
                            Some(Ok(Message::Text(text))) => {
                                let event = parse_acp_event(&text);
                                if event_tx_clone.send(event).is_err() {
                                    break;
                                }
                            }
                            Some(Ok(Message::Close(_))) | None => break,
                            Some(Ok(_)) => {}
                            Some(Err(e)) => {
                                let _ = event_tx_clone.send(AcpEvent::Error(format!("websocket: {}", e)));
                                break;
                            }
                        }
                    }
                }
            }

            let _ = event_tx_clone.send(AcpEvent::Error("ACP bridge connection closed".into()));
        });

        Ok((Self { tx }, AcpEventReceiver { rx: event_rx }))
    }

    /// Send a message toward the cloud.
    pub fn send(&self, msg: AcpMessage) -> anyhow::Result<()> {
        self.tx
            .send(msg)
            .map_err(|_| anyhow::anyhow!("ACP bridge task has exited"))
    }

    /// Subscribe to a chat/session on the cloud side.
    pub fn subscribe(&self, chat_id: &str) -> anyhow::Result<()> {
        self.send(AcpMessage::Subscribe {
            chat_id: chat_id.to_string(),
        })
    }
}

/// Result of one relay session between ACP and Gateway.
enum BridgeExit {
    /// Caller requested shutdown (SIGINT / explicit stop).
    CleanShutdown,
    /// Connection dropped or hit a fatal error; outer loop should reconnect.
    FatalError(String),
}

/// Run a bidirectional relay between the ACP cloud bridge and the local
/// backend with automatic reconnection.
///
/// This is the production entry point used by `clarity-headless acp-bridge`.
/// It keeps trying to maintain both WebSockets until the retry budget is
/// exhausted. For graceful shutdown use [`run_acp_gateway_bridge_with_options`].
pub async fn run_acp_gateway_bridge(
    acp_config: &AcpBridgeConfig,
    gateway_url: &str,
    local_backend: LocalBackend,
) -> anyhow::Result<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let result = run_acp_gateway_bridge_with_options(
        acp_config,
        gateway_url,
        local_backend,
        shutdown_rx,
        RetryConfig::default(),
        None,
        None,
    )
    .await;
    drop(shutdown_tx);
    result
}

/// Run the bridge with explicit shutdown and retry controls.
///
/// `shutdown` is a broadcast receiver; sending a message on the corresponding
/// sender requests a clean exit. Any subsequent messages are ignored.
pub async fn run_acp_gateway_bridge_with_options(
    acp_config: &AcpBridgeConfig,
    gateway_url: &str,
    local_backend: LocalBackend,
    mut shutdown: tokio::sync::broadcast::Receiver<()>,
    retry_config: RetryConfig,
    state_path: Option<&std::path::Path>,
    status_tx: Option<tokio::sync::watch::Sender<AcpBridgeStatus>>,
) -> anyhow::Result<()> {
    let mut chat_id: Option<String> = state_path.map(AcpBridgeState::load).and_then(|s| s.chat_id);
    let mut attempt: u32 = 0;

    emit_status(
        &status_tx,
        AcpBridgeStatus {
            chat_id: chat_id.clone(),
            ..AcpBridgeStatus::default()
        },
    );

    loop {
        if attempt > 0 {
            let delay = retry_config.backoff_duration(attempt - 1);
            tracing::info!(
                attempt,
                delay_ms = delay.as_millis(),
                "ACP bridge reconnecting after failure"
            );
            let sleep = tokio::time::sleep(delay);
            tokio::pin!(sleep);
            tokio::select! {
                biased;
                result = shutdown.recv() => {
                    match result {
                        Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            tracing::info!("shutdown requested during ACP bridge reconnect backoff");
                            return Ok(());
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            // All shutdown senders dropped; keep waiting for backoff.
                            (&mut sleep).await;
                        }
                    }
                }
                _ = &mut sleep => {}
            }
        }

        let mut shutdown_once = shutdown.resubscribe();
        match run_acp_gateway_bridge_once(
            acp_config,
            gateway_url,
            &local_backend,
            &mut chat_id,
            &mut shutdown_once,
            state_path,
            &status_tx,
            attempt,
        )
        .await
        {
            BridgeExit::CleanShutdown => {
                emit_status(
                    &status_tx,
                    AcpBridgeStatus {
                        connected: false,
                        chat_id: chat_id.clone(),
                        reconnect_count: attempt,
                        last_error: None,
                    },
                );
                return Ok(());
            }
            BridgeExit::FatalError(err) => {
                tracing::error!(attempt, error = %err, "ACP bridge relay failed");
                emit_status(
                    &status_tx,
                    AcpBridgeStatus {
                        connected: false,
                        chat_id: chat_id.clone(),
                        reconnect_count: attempt + 1,
                        last_error: Some(err.clone()),
                    },
                );
                if retry_config.is_exhausted(attempt) {
                    return Err(anyhow::anyhow!(
                        "ACP bridge exhausted {} retries: {}",
                        retry_config.max_retries,
                        err
                    ));
                }
                attempt += 1;
            }
        }
    }
}

/// Run one bidirectional relay session between ACP and the chosen local backend.
///
/// Returns `BridgeExit::CleanShutdown` when `shutdown` fires, otherwise
/// `FatalError` so the caller can reconnect.
#[allow(clippy::too_many_arguments)]
async fn run_acp_gateway_bridge_once(
    acp_config: &AcpBridgeConfig,
    gateway_url: &str,
    local_backend: &LocalBackend,
    chat_id: &mut Option<String>,
    shutdown: &mut tokio::sync::broadcast::Receiver<()>,
    state_path: Option<&std::path::Path>,
    status_tx: &Option<tokio::sync::watch::Sender<AcpBridgeStatus>>,
    reconnect_count: u32,
) -> BridgeExit {
    let (acp_client, mut acp_events) = match AcpBridgeClient::connect(acp_config).await {
        Ok(v) => v,
        Err(e) => return BridgeExit::FatalError(format!("ACP connect: {}", e)),
    };

    // Auto-subscribe to the last known chat so cloud events keep flowing
    // after a reconnect or process restart.
    if let Some(cid) = chat_id.as_ref() {
        if let Err(e) = acp_client.subscribe(cid) {
            return BridgeExit::FatalError(format!("ACP subscribe: {}", e));
        }
    }

    tracing::info!(
        acp_url = %acp_config.url,
        gateway_url = %gateway_url,
        backend = ?local_backend,
        "ACP bridge relay started"
    );

    match local_backend {
        LocalBackend::ClarityGateway => {
            run_acp_clarity_gateway_once(
                acp_config,
                gateway_url,
                acp_client,
                &mut acp_events,
                chat_id,
                shutdown,
                state_path,
                status_tx,
                reconnect_count,
            )
            .await
        }
        LocalBackend::OpenClawGateway => {
            run_acp_openclaw_gateway_once(
                acp_config,
                gateway_url,
                acp_client,
                &mut acp_events,
                chat_id,
                shutdown,
                state_path,
                status_tx,
                reconnect_count,
            )
            .await
        }
    }
}

/// Build a local OpenClaw Gateway `session_key` from the cloud `chat_id`.
///
/// ponytail: heuristic mapping. KimiClaw appears to route cloud chats into the
/// `agent:main:{chat_id}` session namespace; when no chat id is known we fall
/// back to the default `agent:main:main` session. Adjust once the exact
/// namespace is confirmed.
fn openclaw_session_key(chat_id: Option<&str>) -> String {
    chat_id
        .filter(|s| !s.is_empty())
        .map(|cid| format!("agent:main:{}", cid))
        .unwrap_or_else(|| "agent:main:main".to_string())
}

/// Persist a newly observed `chat_id` and update the in-memory value.
fn update_chat_id(
    value: &serde_json::Value,
    chat_id: &mut Option<String>,
    state_path: Option<&std::path::Path>,
) {
    if let Some(cid) = value.get("chat_id").and_then(|v| v.as_str()) {
        *chat_id = Some(cid.to_string());
        if let Some(path) = state_path {
            AcpBridgeState {
                chat_id: chat_id.clone(),
            }
            .save(path);
        }
    }
}

/// Relay one session over the original Clarity Gateway WebSocket.
#[allow(clippy::too_many_arguments)]
async fn run_acp_clarity_gateway_once(
    _acp_config: &AcpBridgeConfig,
    gateway_url: &str,
    acp_client: AcpBridgeClient,
    acp_events: &mut AcpEventReceiver,
    chat_id: &mut Option<String>,
    shutdown: &mut tokio::sync::broadcast::Receiver<()>,
    state_path: Option<&std::path::Path>,
    status_tx: &Option<tokio::sync::watch::Sender<AcpBridgeStatus>>,
    reconnect_count: u32,
) -> BridgeExit {
    let ws_url = crate::gateway_ws_url(gateway_url);
    let (ws_stream, _) = match tokio_tungstenite::connect_async(&ws_url).await {
        Ok(v) => v,
        Err(e) => return BridgeExit::FatalError(format!("Gateway connect: {}", e)),
    };
    let (mut gw_write, mut gw_read) = ws_stream.split();

    // Consume the Gateway welcome frame.
    match gw_read.next().await {
        Some(Ok(Message::Text(text))) => {
            let welcome: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => return BridgeExit::FatalError(format!("invalid welcome frame: {}", e)),
            };
            if welcome.get("type").and_then(|v| v.as_str()) != Some("welcome") {
                return BridgeExit::FatalError(format!("expected welcome, got: {}", text));
            }
        }
        other => {
            return BridgeExit::FatalError(format!("expected welcome frame, got {:?}", other));
        }
    }

    emit_status(
        status_tx,
        AcpBridgeStatus {
            connected: true,
            chat_id: chat_id.clone(),
            reconnect_count,
            last_error: None,
        },
    );

    loop {
        tokio::select! {
            biased;
            result = shutdown.recv() => {
                match result {
                    Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        tracing::info!("shutdown requested, closing ACP bridge relay");
                        return BridgeExit::CleanShutdown;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {}
                }
            }
            msg = gw_read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // B2: forward Gateway responses back to the cloud.
                        if let Some(cid) = chat_id.as_ref() {
                            let payload = serde_json::json!({
                                "method": "send_message_stream",
                                "chat_id": cid,
                                "gateway_response": text,
                            });
                            if let Err(e) = acp_client.send(AcpMessage::SendMessageStream {
                                chat_id: cid.clone(),
                                payload,
                            }) {
                                return BridgeExit::FatalError(format!("forward to ACP: {}", e));
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => {
                        return BridgeExit::FatalError("Gateway connection closed".into());
                    }
                    _ => {}
                }
            }
            event = acp_events.recv() => {
                match event {
                    Some(AcpEvent::Raw(value)) => {
                        update_chat_id(&value, chat_id, state_path);
                        if let Some(text) = extract_cloud_message_text(&value) {
                            let request = serde_json::json!({
                                "type": "chat",
                                "message": text,
                                "use_wire": false,
                            });
                            let json = request.to_string();
                            if let Err(e) = gw_write.send(Message::Text(json)).await {
                                return BridgeExit::FatalError(format!("forward to Gateway: {}", e));
                            }
                        }
                    }
                    Some(AcpEvent::Error(e)) => {
                        tracing::warn!("ACP bridge error: {}", e);
                    }
                    Some(AcpEvent::Pong) => {}
                    None => {
                        return BridgeExit::FatalError("ACP connection closed".into());
                    }
                }
            }
        }
    }
}

/// Connect to the OpenClaw Gateway, preferring a paired device identity when
/// one has been saved for the same Gateway URL.
async fn connect_openclaw_backend(
    gateway_url: &str,
    acp_config: &AcpBridgeConfig,
) -> anyhow::Result<crate::openclaw_gateway::client::OpenClawGatewayClient> {
    use crate::device::{DeviceIdentity, load_paired_token};
    use crate::openclaw_gateway::client::OpenClawGatewayClient;

    let ws_url = crate::openclaw_ws_url(gateway_url);
    let local_token = match acp_config.local_token.as_deref() {
        Some(t) if !t.is_empty() => t,
        _ => match acp_config.token.as_str() {
            "" => "",
            t => t,
        },
    };

    if let Ok(Some(token)) = load_paired_token() {
        if token.gateway_url == gateway_url {
            if let Ok(Some(device)) = DeviceIdentity::load_existing() {
                tracing::info!("using paired device identity for OpenClaw Gateway");
                return OpenClawGatewayClient::connect_with_device(
                    &ws_url,
                    token.auth_token(),
                    &device,
                )
                .await;
            }
        }
    }

    OpenClawGatewayClient::connect(&ws_url, local_token).await
}

/// Relay one session over the local Kimi Desktop OpenClaw Gateway.
#[allow(clippy::too_many_arguments)]
async fn run_acp_openclaw_gateway_once(
    acp_config: &AcpBridgeConfig,
    gateway_url: &str,
    acp_client: AcpBridgeClient,
    acp_events: &mut AcpEventReceiver,
    chat_id: &mut Option<String>,
    shutdown: &mut tokio::sync::broadcast::Receiver<()>,
    state_path: Option<&std::path::Path>,
    status_tx: &Option<tokio::sync::watch::Sender<AcpBridgeStatus>>,
    reconnect_count: u32,
) -> BridgeExit {
    use crate::openclaw_gateway::chat::OpenClawChatApi;
    use crate::openclaw_gateway::client::OpenClawEvent;

    let oc_client = match connect_openclaw_backend(gateway_url, acp_config).await {
        Ok(c) => c,
        Err(e) => return BridgeExit::FatalError(format!("OpenClaw Gateway connect: {}", e)),
    };

    // Wait for the handshake to complete before declaring the relay ready.
    for _ in 0..60 {
        if oc_client.hello_ok().is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if oc_client.hello_ok().is_none() {
        return BridgeExit::FatalError("OpenClaw Gateway handshake timed out".into());
    }

    let mut oc_events = oc_client.subscribe();

    emit_status(
        status_tx,
        AcpBridgeStatus {
            connected: true,
            chat_id: chat_id.clone(),
            reconnect_count,
            last_error: None,
        },
    );

    loop {
        tokio::select! {
            biased;
            result = shutdown.recv() => {
                match result {
                    Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        tracing::info!("shutdown requested, closing ACP bridge relay");
                        return BridgeExit::CleanShutdown;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {}
                }
            }
            event = oc_events.recv() => {
                match event {
                    Ok(OpenClawEvent::ServerEvent { event, payload, .. }) => {
                        // Forward local Gateway server events back to the cloud as
                        // gateway responses so the cloud Agent sees the local
                        // agent's replies.
                        if let Some(cid) = chat_id.as_ref() {
                            let response = serde_json::json!({
                                "event": event,
                                "payload": payload,
                            });
                            let payload = serde_json::json!({
                                "method": "send_message_stream",
                                "chat_id": cid,
                                "gateway_response": response,
                            });
                            if let Err(e) = acp_client.send(AcpMessage::SendMessageStream {
                                chat_id: cid.clone(),
                                payload,
                            }) {
                                return BridgeExit::FatalError(format!("forward to ACP: {}", e));
                            }
                        }
                    }
                    Ok(OpenClawEvent::Disconnected(reason)) => {
                        return BridgeExit::FatalError(format!("OpenClaw Gateway disconnected: {}", reason));
                    }
                    Ok(OpenClawEvent::Error(err)) => {
                        tracing::warn!("OpenClaw Gateway error: {}", err);
                    }
                    Ok(OpenClawEvent::Connected(_)) => {}
                    Err(_) => {
                        return BridgeExit::FatalError("OpenClaw Gateway event channel closed".into());
                    }
                }
            }
            event = acp_events.recv() => {
                match event {
                    Some(AcpEvent::Raw(value)) => {
                        update_chat_id(&value, chat_id, state_path);
                        if let Some(text) = extract_cloud_message_text(&value) {
                            let session_key = openclaw_session_key(chat_id.as_deref());
                            tracing::debug!(
                                session_key = %session_key,
                                text_len = text.len(),
                                "forwarding cloud message to OpenClaw Gateway"
                            );
                            if let Err(e) = oc_client.chat_send_text(&session_key, &text).await {
                                return BridgeExit::FatalError(format!("chat.send to OpenClaw Gateway: {}", e));
                            }
                        }
                    }
                    Some(AcpEvent::Error(e)) => {
                        tracing::warn!("ACP bridge error: {}", e);
                    }
                    Some(AcpEvent::Pong) => {}
                    None => {
                        return BridgeExit::FatalError("ACP connection closed".into());
                    }
                }
            }
        }
    }
}

/// Extract user message text from a raw ACP cloud event.
///
/// ponytail: heuristic based on observed KimiClaw envelopes. Known fallback
/// paths are tried in order; add new paths here as the schema is confirmed.
fn extract_cloud_message_text(value: &serde_json::Value) -> Option<String> {
    // Path 1: explicit user message envelope.
    if let Some(text) = value
        .get("payload")
        .and_then(|p| p.get("message"))
        .and_then(|m| m.as_str())
    {
        return Some(text.to_string());
    }
    // Path 2: flattened text field.
    if let Some(text) = value.get("text").and_then(|t| t.as_str()) {
        return Some(text.to_string());
    }
    // Path 3: content field used by some bot protocols.
    if let Some(text) = value.get("content").and_then(|c| c.as_str()) {
        return Some(text.to_string());
    }
    None
}

/// Non-blocking event receiver for the ACP bridge.
pub struct AcpEventReceiver {
    rx: tokio::sync::mpsc::UnboundedReceiver<AcpEvent>,
}

impl AcpEventReceiver {
    /// Wait for the next event asynchronously.
    pub async fn recv(&mut self) -> Option<AcpEvent> {
        self.rx.recv().await
    }

    /// Poll for the next event without blocking.
    pub fn try_recv(&mut self) -> Option<AcpEvent> {
        self.rx.try_recv().ok()
    }

    /// Drain all pending events.
    pub fn drain(&mut self) -> Vec<AcpEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
            out.push(ev);
        }
        out
    }
}

/// Load the ACP bridge config from `~/.kimi_openclaw/openclaw.json`.
///
/// Returns `None` if the file or the `kimi-claw` plugin config is missing.
/// To also load the local Gateway URL see [`load_acp_config_and_gateway_url`].
pub fn load_acp_config<P: AsRef<Path>>(openclaw_home: P) -> Option<AcpBridgeConfig> {
    load_acp_config_and_gateway_url(openclaw_home).map(|(config, _)| config)
}

/// Load the ACP bridge config and the local Gateway URL from `openclaw.json`.
///
/// Returns `None` if the file or the `kimi-claw` plugin config is missing.
/// The Gateway URL defaults to `ws://127.0.0.1:18679` when the plugin config
/// does not specify one, matching KimiClaw's default.
pub fn load_acp_config_and_gateway_url<P: AsRef<Path>>(
    openclaw_home: P,
) -> Option<(AcpBridgeConfig, String)> {
    let path = openclaw_home.as_ref().join("openclaw.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let plugin = value
        .get("plugins")?
        .get("entries")?
        .get("kimi-claw")?
        .get("config")?;

    let bridge = plugin.get("bridge")?;
    let mut bridge_config: AcpBridgeConfig = serde_json::from_value(bridge.clone()).ok()?;

    let (gateway_url, local_token) = plugin
        .get("gateway")
        .map(|gateway| {
            let url = gateway
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "ws://127.0.0.1:18679".to_string());
            let token = gateway
                .get("token")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (url, token)
        })
        .unwrap_or_else(|| ("ws://127.0.0.1:18679".to_string(), None));
    bridge_config.local_token = local_token;

    Some((bridge_config, gateway_url))
}

fn build_ws_request(
    url: &str,
    token: &str,
) -> anyhow::Result<tokio_tungstenite::tungstenite::http::Request<()>> {
    use tokio_tungstenite::tungstenite::handshake::client::generate_key;

    let host = url
        .parse::<tokio_tungstenite::tungstenite::http::Uri>()
        .ok()
        .and_then(|u| u.host().map(|h| h.to_string()))
        .unwrap_or_else(|| "localhost".to_string());

    tokio_tungstenite::tungstenite::http::Request::builder()
        .method("GET")
        .uri(url)
        .header("Host", host)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "Desktop Kimi Claw Plugin")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", generate_key())
        .header("Sec-WebSocket-Version", "13")
        .body(())
        .map_err(|e| anyhow::anyhow!("build request: {}", e))
}

fn parse_acp_event(text: &str) -> AcpEvent {
    if text.contains("\"method\":\"pong\"") || text.contains("\"case\":\"ping\"") {
        return AcpEvent::Pong;
    }
    match serde_json::from_str(text) {
        Ok(v) => AcpEvent::Raw(v),
        Err(e) => AcpEvent::Error(format!("parse: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_acp_config_extracts_bridge_section() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("openclaw.json"),
            br#"{
                "plugins": {
                    "entries": {
                        "kimi-claw": {
                            "enabled": true,
                            "config": {
                                "bridge": {
                                    "mode": "acp",
                                    "url": "wss://www.kimi.com/api-claw/bots/agent-ws",
                                    "kimiapiHost": "https://www.kimi.com/api-claw",
                                    "kimiFileDownloadDir": "/tmp/downloads",
                                    "token": "km_b_test"
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        let config = load_acp_config(dir.path()).unwrap();
        assert_eq!(config.mode, "acp");
        assert_eq!(config.url, "wss://www.kimi.com/api-claw/bots/agent-ws");
        assert_eq!(config.token, "km_b_test");
        assert_eq!(config.kimi_api_host, "https://www.kimi.com/api-claw");
    }

    #[test]
    fn load_acp_config_returns_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_acp_config(dir.path()).is_none());
    }

    #[test]
    fn load_acp_config_and_gateway_url_extracts_both() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("openclaw.json"),
            br#"{
                "plugins": {
                    "entries": {
                        "kimi-claw": {
                            "enabled": true,
                            "config": {
                                "bridge": {
                                    "mode": "acp",
                                    "url": "wss://www.kimi.com/api-claw/bots/agent-ws",
                                    "kimiapiHost": "https://www.kimi.com/api-claw",
                                    "kimiFileDownloadDir": "/tmp/downloads",
                                    "token": "km_b_test"
                                },
                                "gateway": {
                                    "url": "ws://127.0.0.1:18679",
                                    "token": "local-token",
                                    "agentId": "main"
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        let (config, gateway_url) = load_acp_config_and_gateway_url(dir.path()).unwrap();
        assert_eq!(config.mode, "acp");
        assert_eq!(gateway_url, "ws://127.0.0.1:18679");
    }

    #[test]
    fn load_acp_config_and_gateway_url_defaults_to_kimiclaw_port() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("openclaw.json"),
            br#"{
                "plugins": {
                    "entries": {
                        "kimi-claw": {
                            "enabled": true,
                            "config": {
                                "bridge": {
                                    "mode": "acp",
                                    "url": "wss://www.kimi.com/api-claw/bots/agent-ws",
                                    "kimiapiHost": "https://www.kimi.com/api-claw",
                                    "kimiFileDownloadDir": "/tmp/downloads",
                                    "token": "km_b_test"
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        let (config, gateway_url) = load_acp_config_and_gateway_url(dir.path()).unwrap();
        assert_eq!(config.mode, "acp");
        assert_eq!(gateway_url, "ws://127.0.0.1:18679");
    }

    #[test]
    fn parse_acp_event_detects_pong() {
        assert!(matches!(
            parse_acp_event(r#"{"method":"pong"}"#),
            AcpEvent::Pong
        ));
    }

    #[test]
    fn parse_acp_event_falls_back_to_raw() {
        let event = parse_acp_event(r#"{"method":"subscribe_event","payload":{}}"#);
        assert!(
            matches!(event, AcpEvent::Raw(ref v) if v.get("method").and_then(|m| m.as_str()) == Some("subscribe_event"))
        );
    }

    #[test]
    fn build_ws_request_includes_websocket_headers() {
        let req =
            build_ws_request("wss://www.kimi.com/api-claw/bots/agent-ws", "km_b_test").unwrap();
        assert_eq!(req.method(), "GET");
        let headers = req.headers();
        assert_eq!(headers.get("Host").unwrap(), "www.kimi.com");
        assert_eq!(headers.get("Upgrade").unwrap(), "websocket");
        assert_eq!(headers.get("Connection").unwrap(), "Upgrade");
        assert!(headers.get("Sec-WebSocket-Key").is_some());
        assert_eq!(headers.get("Sec-WebSocket-Version").unwrap(), "13");
        assert!(
            headers
                .get("Authorization")
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("Bearer ")
        );
    }

    #[test]
    fn acp_message_serializes_subscribe() {
        let msg = AcpMessage::Subscribe {
            chat_id: "chat-abc".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("subscribe"));
        assert!(json.contains("chat-abc"));
    }

    #[test]
    fn extract_cloud_message_text_payload_path() {
        let value = serde_json::json!({
            "chat_id": "chat-1",
            "payload": { "message": "hello from cloud" }
        });
        assert_eq!(
            extract_cloud_message_text(&value),
            Some("hello from cloud".to_string())
        );
    }

    #[test]
    fn extract_cloud_message_text_flat_text_path() {
        let value = serde_json::json!({
            "chat_id": "chat-1",
            "text": "flat hello"
        });
        assert_eq!(
            extract_cloud_message_text(&value),
            Some("flat hello".to_string())
        );
    }

    #[test]
    fn extract_cloud_message_text_content_path() {
        let value = serde_json::json!({
            "chat_id": "chat-1",
            "content": "content hello"
        });
        assert_eq!(
            extract_cloud_message_text(&value),
            Some("content hello".to_string())
        );
    }

    #[test]
    fn extract_cloud_message_text_returns_none_when_no_text() {
        let value = serde_json::json!({ "chat_id": "chat-1", "metadata": {} });
        assert_eq!(extract_cloud_message_text(&value), None);
    }

    #[test]
    fn acp_bridge_state_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let state = AcpBridgeState {
            chat_id: Some("chat-persisted".into()),
        };
        state.save(&path);
        let loaded = AcpBridgeState::load(&path);
        assert_eq!(loaded.chat_id, Some("chat-persisted".into()));
    }

    #[test]
    fn acp_bridge_state_load_returns_default_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.json");
        let loaded = AcpBridgeState::load(&path);
        assert!(loaded.chat_id.is_none());
    }

    // ------------------------------------------------------------------
    // Async integration-style unit tests for the resilient bridge.
    // ------------------------------------------------------------------

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    async fn mock_acp_server_holding() -> (
        std::net::SocketAddr,
        tokio::sync::mpsc::UnboundedReceiver<Message>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (mut ws_write, mut ws_read) = accept_async(stream).await.unwrap().split();
            loop {
                tokio::select! {
                    msg = ws_read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                let _ = tx.send(Message::Text(text));
                            }
                            Some(Ok(Message::Binary(bin))) => {
                                let _ = tx.send(Message::Binary(bin));
                            }
                            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                            _ => {}
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                }
            }
            let _ = ws_write.close().await;
        });

        (addr, rx)
    }

    async fn mock_gateway_server_holding() -> (
        std::net::SocketAddr,
        tokio::sync::mpsc::UnboundedReceiver<Message>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (mut ws_write, mut ws_read) = accept_async(stream).await.unwrap().split();
            let _ = ws_write
                .send(Message::Text(r#"{"type":"welcome"}"#.into()))
                .await;
            loop {
                tokio::select! {
                    msg = ws_read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                let _ = tx.send(Message::Text(text));
                            }
                            Some(Ok(Message::Binary(bin))) => {
                                let _ = tx.send(Message::Binary(bin));
                            }
                            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                            _ => {}
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                }
            }
            let _ = ws_write.close().await;
        });

        (addr, rx)
    }

    #[tokio::test]
    async fn bridge_exits_cleanly_on_shutdown() {
        let (acp_addr, _acp_rx) = mock_acp_server_holding().await;
        let (gw_addr, _gw_rx) = mock_gateway_server_holding().await;

        let config = AcpBridgeConfig {
            mode: "acp".into(),
            url: format!("ws://{}", acp_addr),
            kimi_api_host: "http://localhost".into(),
            kimi_file_download_dir: std::env::temp_dir(),
            token: "test-token".into(),
            ..Default::default()
        };

        let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        let gateway_url = format!("http://{}", gw_addr);

        let handle = tokio::spawn(async move {
            run_acp_gateway_bridge_with_options(
                &config,
                &gateway_url,
                LocalBackend::ClarityGateway,
                shutdown_rx,
                RetryConfig::aggressive(),
                None,
                None,
            )
            .await
        });

        // Give the bridge time to establish both handshakes.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let _ = shutdown_tx.send(());

        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(result.is_ok(), "bridge did not shut down in time");
        assert!(result.unwrap().is_ok());
    }

    async fn mock_acp_server_periodic(
        event: serde_json::Value,
    ) -> (
        std::net::SocketAddr,
        tokio::sync::mpsc::UnboundedReceiver<Message>,
        Arc<AtomicUsize>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                count_clone.fetch_add(1, Ordering::SeqCst);
                let tx = tx.clone();
                let event = event.clone();
                tokio::spawn(async move {
                    let (mut ws_write, mut ws_read) = accept_async(stream).await.unwrap().split();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    let _ = ws_write.send(Message::Text(event.to_string())).await;
                    loop {
                        tokio::select! {
                            msg = ws_read.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        let _ = tx.send(Message::Text(text));
                                    }
                                    Some(Ok(Message::Binary(bin))) => {
                                        let _ = tx.send(Message::Binary(bin));
                                    }
                                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                                    _ => {}
                                }
                            }
                            _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                        }
                    }
                });
            }
        });

        (addr, rx, count)
    }

    async fn mock_gateway_server_flaky(
        reply: serde_json::Value,
    ) -> (std::net::SocketAddr, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                count_clone.fetch_add(1, Ordering::SeqCst);
                let reply = reply.clone();
                tokio::spawn(async move {
                    let (mut ws_write, mut ws_read) = accept_async(stream).await.unwrap().split();
                    let _ = ws_write
                        .send(Message::Text(r#"{"type":"welcome"}"#.into()))
                        .await;
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    while let Ok(Some(Ok(Message::Text(text)))) =
                        tokio::time::timeout(Duration::from_millis(50), ws_read.next()).await
                    {
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                            if value.get("type").and_then(|t| t.as_str()) == Some("chat") {
                                let _ = ws_write.send(Message::Text(reply.to_string())).await;
                            }
                        }
                    }
                });
            }
        });

        (addr, count)
    }

    #[tokio::test]
    async fn bridge_reconnects_after_gateway_disconnect() {
        let event = serde_json::json!({
            "chat_id": "chat-1",
            "payload": { "message": "hello" },
        });
        let (acp_addr, _acp_rx, _acp_count) = mock_acp_server_periodic(event).await;
        let reply = serde_json::json!({"type":"chat","message":"gateway reply"});
        let (gw_addr, gw_count) = mock_gateway_server_flaky(reply).await;

        let config = AcpBridgeConfig {
            mode: "acp".into(),
            url: format!("ws://{}", acp_addr),
            kimi_api_host: "http://localhost".into(),
            kimi_file_download_dir: std::env::temp_dir(),
            token: "test-token".into(),
            ..Default::default()
        };

        let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        let gateway_url = format!("http://{}", gw_addr);

        let _handle = tokio::spawn(async move {
            run_acp_gateway_bridge_with_options(
                &config,
                &gateway_url,
                LocalBackend::ClarityGateway,
                shutdown_rx,
                RetryConfig {
                    max_retries: 10,
                    initial_backoff_ms: 50,
                    max_backoff_ms: 200,
                    backoff_multiplier: 2.0,
                },
                None,
                None,
            )
            .await
        });

        // Wait long enough for the initial connection plus at least one reconnect.
        tokio::time::sleep(Duration::from_millis(900)).await;

        let connections = gw_count.load(Ordering::SeqCst);
        assert!(
            connections >= 2,
            "expected at least 2 Gateway connections, got {}",
            connections
        );
    }

    async fn mock_acp_server_with_event(
        event: serde_json::Value,
    ) -> (
        std::net::SocketAddr,
        tokio::sync::mpsc::UnboundedReceiver<Message>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (mut ws_write, mut ws_read) = accept_async(stream).await.unwrap().split();
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = ws_write.send(Message::Text(event.to_string())).await;
            while let Some(Ok(msg)) = ws_read.next().await {
                if matches!(msg, Message::Close(_)) {
                    break;
                }
                if matches!(msg, Message::Text(_) | Message::Binary(_)) {
                    let _ = tx.send(msg);
                }
            }
        });

        (addr, rx)
    }

    async fn mock_gateway_server_replying(
        reply: serde_json::Value,
    ) -> (
        std::net::SocketAddr,
        tokio::sync::mpsc::UnboundedReceiver<Message>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (mut ws_write, mut ws_read) = accept_async(stream).await.unwrap().split();
            let _ = ws_write
                .send(Message::Text(r#"{"type":"welcome"}"#.into()))
                .await;
            while let Some(Ok(msg)) = ws_read.next().await {
                if matches!(msg, Message::Close(_)) {
                    break;
                }
                if matches!(msg, Message::Text(_) | Message::Binary(_)) {
                    let _ = tx.send(msg.clone());
                    if let Ok(text) = msg.to_text() {
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
                            if value.get("type").and_then(|t| t.as_str()) == Some("chat") {
                                let _ = ws_write.send(Message::Text(reply.to_string())).await;
                            }
                        }
                    }
                }
            }
        });

        (addr, rx)
    }

    #[tokio::test]
    async fn bridge_uses_persisted_chat_id() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("acp-bridge-state.json");
        AcpBridgeState {
            chat_id: Some("chat-persisted".into()),
        }
        .save(&state_path);

        // Cloud event has text but no chat_id; bridge must use the persisted one.
        let acp_event = serde_json::json!({ "text": "hello without chat_id" });
        let (acp_addr, mut acp_rx) = mock_acp_server_with_event(acp_event).await;
        let reply = serde_json::json!({"type":"chat","message":"gateway reply"});
        let (gw_addr, _gw_rx) = mock_gateway_server_replying(reply).await;

        let config = AcpBridgeConfig {
            mode: "acp".into(),
            url: format!("ws://{}", acp_addr),
            kimi_api_host: "http://localhost".into(),
            kimi_file_download_dir: std::env::temp_dir(),
            token: "test-token".into(),
            ..Default::default()
        };

        let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        let gateway_url = format!("http://{}", gw_addr);
        let state_path_for_bridge = state_path.clone();

        let _handle = tokio::spawn(async move {
            run_acp_gateway_bridge_with_options(
                &config,
                &gateway_url,
                LocalBackend::ClarityGateway,
                shutdown_rx,
                RetryConfig::aggressive(),
                Some(state_path_for_bridge.as_path()),
                None,
            )
            .await
            .ok()
        });

        let acp_msg = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let msg = acp_rx.recv().await.expect("acp server closed");
                let text = msg.to_text().unwrap();
                let value: serde_json::Value = serde_json::from_str(text).unwrap();
                let method = value.get("method").and_then(|m| m.as_str());
                if method == Some("ping") || method == Some("subscribe") {
                    continue;
                }
                break msg;
            }
        })
        .await
        .expect("acp receive timed out");

        let text = acp_msg.to_text().unwrap();
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(value["method"], "send_message_stream");
        assert_eq!(value["chat_id"], "chat-persisted");

        // Verify the state file still contains the persisted chat_id.
        let loaded = AcpBridgeState::load(&state_path);
        assert_eq!(loaded.chat_id, Some("chat-persisted".into()));
    }

    #[tokio::test]
    async fn bridge_sends_subscribe_when_chat_id_known() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("acp-bridge-state.json");
        AcpBridgeState {
            chat_id: Some("chat-sub".into()),
        }
        .save(&state_path);

        let (acp_addr, mut acp_rx) = mock_acp_server_holding().await;
        let (gw_addr, _gw_rx) = mock_gateway_server_holding().await;

        let config = AcpBridgeConfig {
            mode: "acp".into(),
            url: format!("ws://{}", acp_addr),
            kimi_api_host: "http://localhost".into(),
            kimi_file_download_dir: std::env::temp_dir(),
            token: "test-token".into(),
            ..Default::default()
        };

        let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        let gateway_url = format!("http://{}", gw_addr);
        let state_path_for_bridge = state_path.clone();

        let _handle = tokio::spawn(async move {
            run_acp_gateway_bridge_with_options(
                &config,
                &gateway_url,
                LocalBackend::ClarityGateway,
                shutdown_rx,
                RetryConfig::aggressive(),
                Some(state_path_for_bridge.as_path()),
                None,
            )
            .await
            .ok()
        });

        let sub_msg = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let msg = acp_rx.recv().await.expect("acp server closed");
                let text = msg.to_text().unwrap();
                let value: serde_json::Value = serde_json::from_str(text).unwrap();
                if value.get("method").and_then(|m| m.as_str()) == Some("subscribe") {
                    break value;
                }
            }
        })
        .await
        .expect("did not receive subscribe");

        assert_eq!(sub_msg["chat_id"], "chat-sub");
    }

    #[tokio::test]
    async fn bridge_status_reports_connected_and_reconnects() {
        let event = serde_json::json!({
            "chat_id": "chat-1",
            "payload": { "message": "hello" },
        });
        let (acp_addr, _acp_rx, _acp_count) = mock_acp_server_periodic(event).await;
        let reply = serde_json::json!({"type":"chat","message":"gateway reply"});
        let (gw_addr, _gw_count) = mock_gateway_server_flaky(reply).await;

        let config = AcpBridgeConfig {
            mode: "acp".into(),
            url: format!("ws://{}", acp_addr),
            kimi_api_host: "http://localhost".into(),
            kimi_file_download_dir: std::env::temp_dir(),
            token: "test-token".into(),
            ..Default::default()
        };

        let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        let (status_tx, mut status_rx) = tokio::sync::watch::channel(AcpBridgeStatus::default());
        let gateway_url = format!("http://{}", gw_addr);

        let _handle = tokio::spawn(async move {
            run_acp_gateway_bridge_with_options(
                &config,
                &gateway_url,
                LocalBackend::ClarityGateway,
                shutdown_rx,
                RetryConfig {
                    max_retries: 10,
                    initial_backoff_ms: 50,
                    max_backoff_ms: 200,
                    backoff_multiplier: 2.0,
                },
                None,
                Some(status_tx),
            )
            .await
            .ok()
        });

        let connected = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let status = status_rx.borrow_and_update().clone();
                if status.connected {
                    break true;
                }
                if status_rx.changed().await.is_err() {
                    break false;
                }
            }
        })
        .await
        .expect("status timeout");
        assert!(connected);

        // Wait long enough for the flaky Gateway to force at least one reconnect.
        tokio::time::sleep(Duration::from_millis(900)).await;

        let status = status_rx.borrow().clone();
        assert!(
            status.reconnect_count > 0,
            "expected reconnect_count > 0, got {:?}",
            status
        );
    }

    // ------------------------------------------------------------------
    // OpenClaw Gateway backend relay tests.
    // ------------------------------------------------------------------

    use crate::openclaw_gateway::protocol::OpenClawFrame;

    async fn mock_openclaw_server_for_bridge() -> (
        std::net::SocketAddr,
        tokio::sync::mpsc::UnboundedReceiver<String>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (mut write, mut read) = accept_async(stream).await.unwrap().split();

            // 1. Send challenge.
            let challenge = serde_json::json!({
                "type": "event",
                "event": "connect.challenge",
                "payload": { "nonce": "test-nonce", "ts": 1234 }
            });
            write
                .send(Message::Text(challenge.to_string()))
                .await
                .unwrap();

            let mut connected = false;
            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                if !connected {
                                    // 2. Reply hello-ok.
                                    let req: OpenClawFrame = serde_json::from_str(&text).unwrap();
                                    let id = match req {
                                        OpenClawFrame::Req { id, .. } => id,
                                        _ => "unknown".to_string(),
                                    };
                                    let hello = serde_json::json!({
                                        "type": "res",
                                        "id": id,
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
                                    continue;
                                }

                                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                                    if value.get("method").and_then(|m| m.as_str()) == Some("ping") {
                                        continue;
                                    }
                                }

                                // Echo the request for test verification.
                                let _ = tx.send(text.clone());

                                // Reply ok to any RPC so the caller does not time out.
                                if let Ok(OpenClawFrame::Req { id, .. }) =
                                    serde_json::from_str::<OpenClawFrame>(&text)
                                {
                                    let res = serde_json::json!({
                                        "type": "res",
                                        "id": id,
                                        "ok": true,
                                        "payload": { "message": { "id": "msg-1", "blocks": [] } }
                                    });
                                    write.send(Message::Text(res.to_string())).await.unwrap();
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
    async fn bridge_openclaw_forwards_cloud_message_to_chat_send() {
        let cloud_event = serde_json::json!({
            "chat_id": "chat-openclaw-1",
            "text": "hello from cloud via OpenClaw",
        });
        let (acp_addr, _acp_rx) = mock_acp_server_with_event(cloud_event).await;
        let (oc_addr, mut oc_rx) = mock_openclaw_server_for_bridge().await;

        let config = AcpBridgeConfig {
            mode: "acp".into(),
            url: format!("ws://{}", acp_addr),
            kimi_api_host: "http://localhost".into(),
            kimi_file_download_dir: std::env::temp_dir(),
            token: "test-token".into(),
            ..Default::default()
        };

        let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        let gateway_url = format!("http://{}", oc_addr);

        let _handle = tokio::spawn(async move {
            run_acp_gateway_bridge_with_options(
                &config,
                &gateway_url,
                LocalBackend::OpenClawGateway,
                shutdown_rx,
                RetryConfig::aggressive(),
                None,
                None,
            )
            .await
            .ok()
        });

        let req = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let text = oc_rx.recv().await.expect("OpenClaw server closed");
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                    if value.get("method").and_then(|m| m.as_str()) == Some("chat.send") {
                        break value;
                    }
                }
            }
        })
        .await
        .expect("timed out waiting for chat.send");

        assert_eq!(req["method"], "chat.send");
        assert_eq!(req["params"]["sessionKey"], "agent:main:chat-openclaw-1");
        assert_eq!(req["params"]["message"][0]["type"], "text");
        assert_eq!(
            req["params"]["message"][0]["text"],
            "hello from cloud via OpenClaw"
        );
    }
}
