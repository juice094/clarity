//! Native Clarity Gateway WebSocket client.
//!
//! Maintains a persistent WebSocket connection to a Clarity Gateway
//! in a background thread, using the native `WsRequest`/`WsResponse`
//! protocol. Communication with the UI thread is via `std::sync::mpsc`
//! channels so the client stays UI-agnostic.

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Maximum consecutive connection failures before giving up and surfacing a
/// terminal error to the UI.
const MAX_RETRIES: usize = 5;

/// A handle to a native Gateway WebSocket client.
#[derive(Clone)]
pub struct GatewayClient {
    tx: mpsc::Sender<GatewayCommand>,
    rx: Arc<Mutex<mpsc::Receiver<GatewayResponse>>>,
}

/// Commands that can be sent from the UI thread to the Gateway thread.
#[derive(Clone, Debug)]
pub(crate) enum GatewayCommand {
    /// Send a chat message.
    Chat { message: String, use_wire: bool },
    /// Send a ping.
    Ping,
    /// Request conversation history.
    GetHistory,
    /// Request missing role-context events.
    SyncRoleContext {
        /// Role context to synchronize.
        role_id: String,
        /// Last event id known locally.
        since_event_id: Option<String>,
        /// Local device id.
        device_id: String,
    },
}

/// Responses/events emitted by the Gateway WebSocket connection.
#[derive(Clone, Debug)]
pub enum GatewayResponse {
    /// Connection established and server returned a session id.
    Connected {
        /// URL of the connected Gateway.
        gateway_url: String,
        /// Session id returned by the Gateway.
        session_id: String,
    },
    /// A single final assistant message (non-wire mode).
    Chat {
        /// Assistant message content.
        message: String,
        /// Optional tool calls produced by the assistant.
        tool_calls: Option<Vec<ToolCall>>,
    },
    /// End-of-turn marker.
    Done,
    /// A streamed `clarity_wire::WireMessage` payload.
    WireMessage {
        /// Wire message payload.
        payload: serde_json::Value,
    },
    /// History reply.
    History {
        /// Conversation messages.
        messages: Vec<GatewayMessage>,
    },
    /// Role-context sync reply.
    RoleContextSynced {
        /// Role that was synchronized.
        role_id: String,
        /// Events missing on the client.
        events: Vec<clarity_contract::ClawContextEvent>,
        /// Cursor for the next sync request.
        next_cursor: Option<String>,
        /// Devices currently online for this role.
        online_devices: Vec<String>,
    },
    /// Connection or execution error.
    Error(String),
}

/// A tool call embedded in a Gateway chat response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    /// Name of the tool.
    pub name: String,
    /// Arguments passed to the tool.
    pub arguments: serde_json::Value,
}

/// A single message in a Gateway conversation history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GatewayMessage {
    /// Role of the message author.
    pub role: String,
    /// Message content.
    pub content: String,
    /// ISO-8601 timestamp of the message.
    pub timestamp: String,
}

/// Outgoing request types for the native Gateway WebSocket protocol.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsRequest {
    /// Send a chat message.
    Chat {
        /// User message text.
        message: String,
        /// Optional session context.
        #[serde(default)]
        context: Option<serde_json::Value>,
        /// Whether to request wire-message streaming.
        #[serde(default)]
        use_wire: bool,
    },
    /// Ping the Gateway.
    Ping,
    /// Fetch conversation history.
    GetHistory,
    /// Request missing role-context events.
    SyncRoleContext {
        /// Role context to synchronize.
        role_id: String,
        /// Last event id known locally.
        #[serde(skip_serializing_if = "Option::is_none")]
        since_event_id: Option<String>,
        /// Local device id.
        device_id: String,
    },
}

/// Incoming response types for the native Gateway WebSocket protocol.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsResponse {
    /// Initial handshake with session id.
    Welcome { session_id: String, message: String },
    /// Final assistant response.
    Chat {
        message: String,
        /// Optional tool calls produced by the assistant.
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCall>>,
    },
    /// Ping reply.
    Pong,
    /// End-of-turn marker.
    Done,
    /// History payload.
    History { messages: Vec<GatewayMessage> },
    /// Error payload.
    Error { error: String },
    /// Wire message payload.
    WireMessage { payload: serde_json::Value },
    /// Role-context sync payload.
    RoleContextSynced {
        role_id: String,
        events: Vec<clarity_contract::ClawContextEvent>,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_cursor: Option<String>,
        online_devices: Vec<String>,
    },
}

impl GatewayClient {
    /// Open a native Gateway WebSocket connection in a background thread.
    ///
    /// Returns immediately; poll [`Self::try_recv`] or [`Self::drain`] for
    /// [`GatewayResponse::Connected`] and subsequent events.
    pub fn connect(url: &str) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<GatewayCommand>();
        let (resp_tx, resp_rx) = mpsc::channel::<GatewayResponse>();

        let url = url.to_string();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = resp_tx.send(GatewayResponse::Error(format!("tokio runtime: {}", e)));
                    return;
                }
            };
            rt.block_on(run_connection(&url, cmd_rx, resp_tx));
        });

        Self {
            tx: cmd_tx,
            rx: Arc::new(Mutex::new(resp_rx)),
        }
    }

    /// Send a chat message to the Gateway.
    pub fn chat(&self, message: &str, use_wire: bool) {
        let _ = self.tx.send(GatewayCommand::Chat {
            message: message.into(),
            use_wire,
        });
    }

    /// Send a ping to the Gateway.
    pub fn ping(&self) {
        let _ = self.tx.send(GatewayCommand::Ping);
    }

    /// Request the conversation history from the Gateway.
    pub fn get_history(&self) {
        let _ = self.tx.send(GatewayCommand::GetHistory);
    }

    /// Request missing role-context events from the Gateway.
    pub fn sync_role_context(&self, role_id: &str, since_event_id: Option<&str>, device_id: &str) {
        let _ = self.tx.send(GatewayCommand::SyncRoleContext {
            role_id: role_id.into(),
            since_event_id: since_event_id.map(Into::into),
            device_id: device_id.into(),
        });
    }

    /// Non-blocking poll for a response from the Gateway.
    pub fn try_recv(&self) -> Option<GatewayResponse> {
        self.rx.lock().try_recv().ok()
    }

    /// Drain all pending responses from the Gateway.
    pub fn drain(&self) -> Vec<GatewayResponse> {
        let mut out = Vec::new();
        let rx = self.rx.lock();
        while let Ok(r) = rx.try_recv() {
            out.push(r);
        }
        out
    }
}

async fn run_connection(
    url: &str,
    cmd_rx: Receiver<GatewayCommand>,
    resp_tx: Sender<GatewayResponse>,
) {
    let (async_tx, mut async_rx) = tokio::sync::mpsc::unbounded_channel::<GatewayCommand>();

    let mut bridge = tokio::task::spawn_blocking(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            if async_tx.send(cmd).is_err() {
                break;
            }
        }
    });

    let mut failures = 0usize;
    loop {
        tokio::select! {
            // When the UI drops the client the command channel closes. Prefer
            // detecting this so we stop reconnecting and let the thread exit.
            biased;
            _ = &mut bridge => return,
            result = run_single_connection(url, &mut async_rx, &resp_tx) => {
                match result {
                    Ok(()) => {
                        // Clean close: reset the failure counter so the next
                        // transient outage starts from a short backoff again.
                        failures = 0;
                    }
                    Err(e) => {
                        failures += 1;
                        if failures >= MAX_RETRIES {
                            let _ = resp_tx.send(GatewayResponse::Error(format!(
                                "OpenClaw connection error: {} ({} retries exhausted)",
                                e, failures
                            )));
                            return;
                        }
                        tokio::time::sleep(crate::util::next_backoff(failures)).await;
                    }
                }
            }
        }
    }
}

async fn run_single_connection(
    url: &str,
    async_rx: &mut tokio::sync::mpsc::UnboundedReceiver<GatewayCommand>,
    resp_tx: &Sender<GatewayResponse>,
) -> Result<(), String> {
    let (ws_stream, _) = connect_async(url)
        .await
        .map_err(|e| format!("WebSocket connect: {}", e))?;
    let (mut write, mut read) = ws_stream.split();

    // The Gateway sends a welcome frame immediately after the handshake.
    let welcome_text = match read.next().await {
        Some(Ok(Message::Text(text))) => text,
        Some(Ok(other)) => return Err(format!("unexpected welcome message: {:?}", other)),
        Some(Err(e)) => return Err(format!("WebSocket error: {}", e)),
        None => return Err("Connection closed before welcome".into()),
    };

    run_gateway_protocol(url, welcome_text, &mut write, &mut read, async_rx, resp_tx).await
}

/// Run the native Gateway WebSocket protocol over an already-established
/// stream. This is the protocol handler used by [`ClawConnectionManager`] so
/// a single connection can be auto-detected and then handed off without a
/// second handshake.
pub(crate) async fn run_gateway_protocol<
    W: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin + Send,
    R: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin
        + Send,
>(
    url: &str,
    welcome_text: String,
    write: &mut W,
    read: &mut R,
    async_rx: &mut tokio::sync::mpsc::UnboundedReceiver<GatewayCommand>,
    resp_tx: &Sender<GatewayResponse>,
) -> Result<(), String> {
    let welcome = serde_json::from_str::<WsResponse>(&welcome_text)
        .map_err(|e| format!("welcome parse: {}", e))?;

    let session_id = match welcome {
        WsResponse::Welcome { session_id, .. } => session_id,
        other => return Err(format!("expected welcome, got {:?}", other)),
    };

    let _ = resp_tx.send(GatewayResponse::Connected {
        gateway_url: url.into(),
        session_id,
    });

    loop {
        tokio::select! {
            cmd = async_rx.recv() => {
                let request = match cmd {
                    Some(GatewayCommand::Chat { message, use_wire }) => {
                        WsRequest::Chat { message, context: None, use_wire }
                    }
                    Some(GatewayCommand::Ping) => WsRequest::Ping,
                    Some(GatewayCommand::GetHistory) => WsRequest::GetHistory,
                    Some(GatewayCommand::SyncRoleContext { role_id, since_event_id, device_id }) => {
                        WsRequest::SyncRoleContext { role_id, since_event_id, device_id }
                    }
                    None => return Ok(()),
                };
                let text = serde_json::to_string(&request)
                    .map_err(|e| format!("serialize request: {}", e))?;
                if let Err(e) = write.send(Message::Text(text)).await {
                    return Err(format!("send request: {}", e));
                }
            }

            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match parse_incoming_frame(&text) {
                            Ok(Some(out)) => {
                                let _ = resp_tx.send(out);
                            }
                            Ok(None) => {}
                            Err(e) => {
                                let _ = resp_tx.send(GatewayResponse::Error(e));
                                continue;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => return Ok(()),
                    Some(Ok(_)) => {}
                    Some(Err(e)) => return Err(format!("WebSocket error: {}", e)),
                }
            }
        }
    }
}

/// Parse a single text frame from the Gateway.
///
/// First tries the native `WsResponse` envelope. If that fails, falls back to
/// treating the frame as an unwrapped `clarity_wire::WireMessage` payload as
/// long as it contains a `type` field. This keeps us compatible with older or
/// alternative Gateway implementations that stream raw wire messages.
fn parse_incoming_frame(text: &str) -> Result<Option<GatewayResponse>, String> {
    match serde_json::from_str::<WsResponse>(text) {
        Ok(r) => Ok(translate_response(r)),
        Err(e) => {
            // ponytail: fallback parse for unwrapped WireMessage frames such as
            // `turn_begin`. If the JSON has no `type` we cannot safely interpret
            // it, so report the original serde error to aid debugging.
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(text)
                && value.get("type").is_some()
            {
                return Ok(Some(GatewayResponse::WireMessage { payload: value }));
            }
            Err(format!("parse response: {}", e))
        }
    }
}

fn translate_response(response: WsResponse) -> Option<GatewayResponse> {
    match response {
        WsResponse::Welcome { .. } => Some(GatewayResponse::Error("Unexpected welcome".into())),
        WsResponse::Chat {
            message,
            tool_calls,
        } => Some(GatewayResponse::Chat {
            message,
            tool_calls,
        }),
        WsResponse::Pong => None,
        WsResponse::Done => Some(GatewayResponse::Done),
        WsResponse::History { messages } => Some(GatewayResponse::History { messages }),
        WsResponse::Error { error } => Some(GatewayResponse::Error(error)),
        WsResponse::WireMessage { payload } => Some(GatewayResponse::WireMessage { payload }),
        WsResponse::RoleContextSynced {
            role_id,
            events,
            next_cursor,
            online_devices,
        } => Some(GatewayResponse::RoleContextSynced {
            role_id,
            events,
            next_cursor,
            online_devices,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_request_serialization() {
        let req = WsRequest::Chat {
            message: "hi".into(),
            context: None,
            use_wire: true,
        };
        let val = serde_json::to_value(&req).unwrap();
        let obj = val.as_object().unwrap();
        assert_eq!(obj.get("type").unwrap().as_str().unwrap(), "chat");
        assert_eq!(obj.get("message").unwrap().as_str().unwrap(), "hi");
        assert!(obj.get("use_wire").unwrap().as_bool().unwrap());
    }

    #[test]
    fn test_ws_response_welcome_deserialization() {
        let json = r#"{"type":"welcome","session_id":"abc","message":"hello"}"#;
        let resp: WsResponse = serde_json::from_str(json).unwrap();
        match resp {
            WsResponse::Welcome {
                session_id,
                message,
            } => {
                assert_eq!(session_id, "abc");
                assert_eq!(message, "hello");
            }
            _ => panic!("expected welcome"),
        }
    }

    #[test]
    fn test_ws_response_wire_message_deserialization() {
        let json = r#"{"type":"wire_message","payload":{"foo":"bar"}}"#;
        let resp: WsResponse = serde_json::from_str(json).unwrap();
        match resp {
            WsResponse::WireMessage { payload } => {
                assert_eq!(payload.get("foo").unwrap().as_str().unwrap(), "bar");
            }
            _ => panic!("expected wire_message"),
        }
    }

    #[test]
    fn test_ws_response_done_deserialization() {
        let json = r#"{"type":"done"}"#;
        let resp: WsResponse = serde_json::from_str(json).unwrap();
        assert!(matches!(resp, WsResponse::Done));
        assert!(matches!(
            translate_response(resp).unwrap(),
            GatewayResponse::Done
        ));
    }

    #[test]
    fn test_parse_incoming_frame_unwrapped_turn_begin() {
        let json = r#"{"type":"turn_begin","turn_id":"t1","user_input":"hi"}"#;
        let out = parse_incoming_frame(json).unwrap().unwrap();
        match out {
            GatewayResponse::WireMessage { payload } => {
                assert_eq!(payload.get("type").unwrap().as_str().unwrap(), "turn_begin");
                assert_eq!(payload.get("turn_id").unwrap().as_str().unwrap(), "t1");
            }
            _ => panic!("expected wire message fallback"),
        }
    }

    #[test]
    fn test_parse_incoming_frame_malformed_returns_error() {
        // A JSON object without a `type` field cannot be interpreted as either a
        // native WsResponse or an unwrapped WireMessage, so it must produce an error.
        let json = r#"{"value":42}"#;
        let err = parse_incoming_frame(json).unwrap_err();
        assert!(err.starts_with("parse response:"));
    }
}
