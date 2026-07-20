//! WebSocket handler for the OpenClaw-compatible endpoint.

use crate::openclaw_server::{
    auth::{AuthResult, authenticate_connect, has_scopes, validate_pair_request},
    protocol::{
        ChatBlock, ChatHistory, ChatSendParams, ChatSendResponse, OpenClawMessage, OpenClawSession,
        PairRequestResult, SessionList,
    },
    state::OpenClawServerState,
};
use crate::transports::{
    OpenClawServerTransport, ServerTransportContext, openclaw_message_from_contract,
};
use axum::{
    extract::{
        State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use clarity_contract::{
    ClawTransport, GovernedTransport, MessageContext, TransportAuth, TransportEvent,
    openclaw_protocol::{ConnectChallenge, OpenClawErrorShape, OpenClawFrame, methods},
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

use crate::server::AppState;

/// WebSocket upgrade handler for `/openclaw/ws`.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.ws_sem.clone().try_acquire_owned() {
        Ok(permit) => ws.on_upgrade(move |socket| handle_socket(socket, state, permit)),
        Err(_) => {
            warn!("OpenClaw WebSocket connection rejected: concurrency limit reached");
            axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

/// Per-connection state.
#[allow(dead_code)]
struct ConnectionState {
    conn_id: String,
    session_key: Option<String>,
    auth: AuthResult,
}

async fn handle_socket(
    mut socket: WebSocket,
    state: Arc<AppState>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    let oc_state = state.openclaw_state.clone();
    let conn_id = format!("oc-{}", oc_state.next_conn_id());

    info!("OpenClaw WebSocket connected: conn_id={}", conn_id);

    // 1. Send connect.challenge
    let nonce = generate_nonce();
    let challenge = OpenClawFrame::Event {
        event: "connect.challenge".to_string(),
        payload: Some(
            serde_json::to_value(ConnectChallenge {
                nonce: nonce.clone(),
                ts: now_ms(),
            })
            .unwrap_or_default(),
        ),
        seq: None,
    };

    if send_frame_socket(&mut socket, &challenge).await.is_err() {
        warn!("OpenClaw {}: failed to send challenge", conn_id);
        return;
    }

    // 2. Wait for connect request and complete handshake.
    let mut conn = match expect_connect_socket(&mut socket, &oc_state, &conn_id).await {
        Some(c) => c,
        None => return,
    };

    let (mut sender, mut receiver) = socket.split();

    // 3. Main loop.
    loop {
        match receiver.next().await {
            Some(Ok(WsMessage::Text(text))) => {
                let frame: OpenClawFrame = match serde_json::from_str(&text) {
                    Ok(f) => f,
                    Err(e) => {
                        let _ = send_error(&mut sender, "", "PARSE_ERROR", &format!("{}", e)).await;
                        continue;
                    }
                };

                let response = handle_frame(&state, &mut conn, frame).await;
                if send_frame(&mut sender, &response).await.is_err() {
                    break;
                }
            }
            Some(Ok(WsMessage::Close(_))) | None => {
                info!("OpenClaw {}: connection closed by client", conn_id);
                break;
            }
            Some(Ok(WsMessage::Ping(data))) => {
                if sender.send(WsMessage::Pong(data)).await.is_err() {
                    break;
                }
            }
            Some(Err(e)) => {
                warn!("OpenClaw {}: websocket error: {}", conn_id, e);
                break;
            }
            _ => {}
        }
    }
}

async fn expect_connect_socket(
    socket: &mut WebSocket,
    oc_state: &OpenClawServerState,
    conn_id: &str,
) -> Option<ConnectionState> {
    loop {
        match socket.next().await {
            Some(Ok(WsMessage::Text(text))) => match serde_json::from_str::<OpenClawFrame>(&text) {
                Ok(OpenClawFrame::Req { id, method, params }) if method == "connect" => {
                    let params: clarity_contract::openclaw_protocol::ConnectParams = match params {
                        Some(p) => match serde_json::from_value(p) {
                            Ok(v) => v,
                            Err(e) => {
                                warn!("OpenClaw {}: invalid connect params: {}", conn_id, e);
                                let _ = send_error_socket(
                                    socket,
                                    &id,
                                    "INVALID_PARAMS",
                                    &format!("{}", e),
                                )
                                .await;
                                return None;
                            }
                        },
                        None => {
                            warn!("OpenClaw {}: missing connect params", conn_id);
                            return None;
                        }
                    };

                    let approved = oc_state.list_devices();
                    let approved_map: std::collections::HashMap<String, (String, Vec<String>)> =
                        approved
                            .into_iter()
                            .map(|d| (d.device_id, (d.public_key, d.scopes)))
                            .collect();

                    let auth = authenticate_connect(&params, oc_state.admin_token(), &approved_map);
                    let (role, scopes) = match &auth {
                        AuthResult::Admin { scopes } => ("operator", scopes.clone()),
                        AuthResult::Device {
                            device_id: _,
                            scopes,
                        } => ("operator", scopes.clone()),
                        AuthResult::Denied(err) => {
                            let res = OpenClawFrame::Res {
                                id,
                                ok: false,
                                payload: None,
                                error: Some(err.clone()),
                            };
                            let _ = send_frame_socket(socket, &res).await;
                            warn!(
                                "OpenClaw {}: connect denied: {} {}",
                                conn_id, err.code, err.message
                            );
                            return None;
                        }
                    };

                    let hello = oc_state.hello_ok(conn_id, role, &scopes);
                    let hello_frame = OpenClawFrame::Res {
                        id,
                        ok: true,
                        payload: Some(match serde_json::to_value(hello) {
                            Ok(v) => v,
                            Err(e) => {
                                warn!("OpenClaw {}: failed to serialize hello-ok: {}", conn_id, e);
                                return None;
                            }
                        }),
                        error: None,
                    };
                    if send_frame_socket(socket, &hello_frame).await.is_err() {
                        warn!("OpenClaw {}: failed to send hello-ok", conn_id);
                        return None;
                    }

                    return Some(ConnectionState {
                        conn_id: conn_id.to_string(),
                        session_key: None,
                        auth,
                    });
                }
                _ => {
                    warn!(
                        "OpenClaw {}: expected connect request, got: {}",
                        conn_id, text
                    );
                    return None;
                }
            },
            Some(Ok(WsMessage::Close(_))) | None => {
                info!("OpenClaw {}: closed before connect", conn_id);
                return None;
            }
            Some(Ok(WsMessage::Ping(data))) => {
                let _ = socket.send(WsMessage::Pong(data)).await;
            }
            Some(Err(e)) => {
                warn!(
                    "OpenClaw {}: websocket error during connect: {}",
                    conn_id, e
                );
                return None;
            }
            _ => {}
        }
    }
}

async fn send_frame_socket(socket: &mut WebSocket, frame: &OpenClawFrame) -> Result<(), ()> {
    let text = match serde_json::to_string(frame) {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to serialize OpenClaw frame: {}", e);
            return Err(());
        }
    };
    socket.send(WsMessage::Text(text)).await.map_err(|_| ())
}

async fn send_error_socket(
    socket: &mut WebSocket,
    id: &str,
    code: &str,
    message: &str,
) -> Result<(), ()> {
    let frame = OpenClawFrame::Res {
        id: id.to_string(),
        ok: false,
        payload: None,
        error: Some(OpenClawErrorShape {
            code: code.to_string(),
            message: message.to_string(),
            details: None,
            retryable: Some(false),
            retry_after_ms: None,
        }),
    };
    send_frame_socket(socket, &frame).await
}

async fn send_frame(
    sender: &mut futures::stream::SplitSink<WebSocket, WsMessage>,
    frame: &OpenClawFrame,
) -> Result<(), ()> {
    let text = match serde_json::to_string(frame) {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to serialize OpenClaw frame: {}", e);
            return Err(());
        }
    };
    sender.send(WsMessage::Text(text)).await.map_err(|_| ())
}

async fn send_error(
    sender: &mut futures::stream::SplitSink<WebSocket, WsMessage>,
    id: &str,
    code: &str,
    message: &str,
) -> Result<(), ()> {
    let frame = OpenClawFrame::Res {
        id: id.to_string(),
        ok: false,
        payload: None,
        error: Some(OpenClawErrorShape {
            code: code.to_string(),
            message: message.to_string(),
            details: None,
            retryable: Some(false),
            retry_after_ms: None,
        }),
    };
    send_frame(sender, &frame).await
}

async fn handle_frame(
    state: &Arc<AppState>,
    conn: &mut ConnectionState,
    frame: OpenClawFrame,
) -> OpenClawFrame {
    match frame {
        OpenClawFrame::Req { id, method, params } => {
            handle_request(state, conn, &id, &method, params).await
        }
        _ => OpenClawFrame::Res {
            id: String::new(),
            ok: false,
            payload: None,
            error: Some(OpenClawErrorShape {
                code: "INVALID_FRAME".to_string(),
                message: "Expected req frame".to_string(),
                details: None,
                retryable: Some(false),
                retry_after_ms: None,
            }),
        },
    }
}

async fn handle_request(
    state: &Arc<AppState>,
    conn: &mut ConnectionState,
    id: &str,
    method: &str,
    params: Option<serde_json::Value>,
) -> OpenClawFrame {
    let scopes = match &conn.auth {
        AuthResult::Admin { scopes } | AuthResult::Device { scopes, .. } => scopes.clone(),
        AuthResult::Denied(_) => {
            return error_res(id, "UNAUTHORIZED", "connection is not authenticated");
        }
    };

    match method {
        methods::CHAT_SEND => handle_chat_send(state, conn, id, params).await,
        methods::CHAT_HISTORY => handle_chat_history(state, conn, id, params).await,
        methods::CHAT_ABORT => ok_res(id, None),
        methods::SESSIONS_LIST => handle_sessions_list(state, id).await,
        methods::SESSIONS_PREVIEW => handle_sessions_preview(state, id, params).await,
        methods::SESSIONS_RESET => ok_res(id, None),
        methods::SESSIONS_DELETE => ok_res(id, None),
        methods::SESSIONS_COMPACT => ok_res(id, None),
        methods::DEVICE_PAIR_REQUEST => {
            handle_device_pair_request(state, &scopes, id, params).await
        }
        methods::DEVICE_PAIR_LIST => handle_device_pair_list(state, id).await,
        "ping" => ok_res(id, None),
        _ => error_res(
            id,
            "METHOD_NOT_FOUND",
            &format!("unknown method: {}", method),
        ),
    }
}

async fn handle_chat_send(
    state: &Arc<AppState>,
    conn: &mut ConnectionState,
    id: &str,
    params: Option<serde_json::Value>,
) -> OpenClawFrame {
    let params: ChatSendParams = match params {
        Some(p) => match serde_json::from_value(p) {
            Ok(v) => v,
            Err(e) => return error_res(id, "INVALID_PARAMS", &format!("{}", e)),
        },
        None => return error_res(id, "INVALID_PARAMS", "missing params"),
    };

    conn.session_key = Some(params.session_key.clone());
    let text = ChatBlock::blocks_to_text(&params.message);
    if text.is_empty() {
        return error_res(id, "INVALID_PARAMS", "empty message");
    }

    let ctx = ServerTransportContext::new(state.clone(), conn.conn_id.clone());
    let transport = OpenClawServerTransport::new(ctx);
    let transport = GovernedTransport::with_metrics(
        transport,
        TransportAuth {
            token: Some("openclaw-server".into()),
            ..Default::default()
        },
        state.metrics.clone(),
    );
    let msg_ctx = MessageContext {
        session_key: Some(params.session_key),
        message: text,
        ..Default::default()
    };

    if let Err(e) = transport.send_message(msg_ctx).await {
        return error_res(id, "AGENT_ERROR", &format!("{}", e));
    }

    let mut events = transport.events();
    let mut reply: Option<String> = None;
    while let Some(ev) = events.next().await {
        match ev {
            TransportEvent::ChatChunk { content } => {
                reply = Some(content);
                break;
            }
            TransportEvent::Error { message } => {
                return error_res(id, "AGENT_ERROR", &message);
            }
            TransportEvent::Done => break,
            _ => {}
        }
    }

    let reply = match reply {
        Some(r) => r,
        None => return error_res(id, "AGENT_ERROR", "agent produced no response"),
    };

    let response = ChatSendResponse {
        message: OpenClawMessage::assistant_text(reply),
    };
    match serde_json::to_value(response) {
        Ok(payload) => ok_res(id, Some(payload)),
        Err(e) => error_res(id, "INTERNAL_ERROR", &format!("{}", e)),
    }
}

async fn handle_chat_history(
    state: &Arc<AppState>,
    conn: &ConnectionState,
    id: &str,
    params: Option<serde_json::Value>,
) -> OpenClawFrame {
    let session_key = params
        .as_ref()
        .and_then(|p| p.get("sessionKey"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| conn.session_key.clone());

    let ctx = ServerTransportContext::new(state.clone(), conn.conn_id.clone());
    let transport = OpenClawServerTransport::new(ctx);
    let transport = GovernedTransport::with_metrics(
        transport,
        TransportAuth {
            token: Some("openclaw-server".into()),
            ..Default::default()
        },
        state.metrics.clone(),
    );
    let messages = match transport.get_history(session_key).await {
        Ok(msgs) => msgs
            .iter()
            .map(openclaw_message_from_contract)
            .collect::<Vec<_>>(),
        Err(e) => return error_res(id, "INTERNAL_ERROR", &format!("{}", e)),
    };

    let history = ChatHistory {
        messages,
        next_cursor: None,
    };
    match serde_json::to_value(history) {
        Ok(payload) => ok_res(id, Some(payload)),
        Err(e) => error_res(id, "INTERNAL_ERROR", &format!("{}", e)),
    }
}

async fn handle_sessions_list(_state: &Arc<AppState>, id: &str) -> OpenClawFrame {
    // ponytail: minimal implementation returns a single default session.
    // Full integration with session_store/thread_store can be added later.
    let list = SessionList {
        sessions: vec![OpenClawSession {
            key: "agent:main:main".to_string(),
            title: Some("Default session".to_string()),
            agent_id: Some("clarity".to_string()),
            created_at_ms: Some(now_ms()),
            updated_at_ms: Some(now_ms()),
            message_count: Some(0),
            model: None,
        }],
        total: Some(1),
    };
    match serde_json::to_value(list) {
        Ok(payload) => ok_res(id, Some(payload)),
        Err(e) => error_res(id, "INTERNAL_ERROR", &format!("{}", e)),
    }
}

async fn handle_sessions_preview(
    _state: &Arc<AppState>,
    id: &str,
    params: Option<serde_json::Value>,
) -> OpenClawFrame {
    let session_key = params
        .as_ref()
        .and_then(|p| p.get("sessionKey"))
        .and_then(|v| v.as_str())
        .unwrap_or("agent:main:main");

    let session = OpenClawSession {
        key: session_key.to_string(),
        title: Some("Default session".to_string()),
        agent_id: Some("clarity".to_string()),
        created_at_ms: Some(now_ms()),
        updated_at_ms: Some(now_ms()),
        message_count: Some(0),
        model: None,
    };
    match serde_json::to_value(session) {
        Ok(payload) => ok_res(id, Some(payload)),
        Err(e) => error_res(id, "INTERNAL_ERROR", &format!("{}", e)),
    }
}

async fn handle_device_pair_request(
    state: &Arc<AppState>,
    scopes: &[String],
    id: &str,
    params: Option<serde_json::Value>,
) -> OpenClawFrame {
    if !has_scopes(scopes, &["operator.pairing".to_string()]) {
        return error_res(id, "FORBIDDEN", "missing operator.pairing scope");
    }

    let params = match params {
        Some(p) => p,
        None => return error_res(id, "INVALID_PARAMS", "missing params"),
    };

    let (device_id, public_key) = match validate_pair_request(&params) {
        Ok(v) => v,
        Err(e) => {
            return OpenClawFrame::Res {
                id: id.to_string(),
                ok: false,
                payload: None,
                error: Some(e),
            };
        }
    };

    let record =
        state
            .openclaw_state
            .approve_device(device_id.clone(), public_key, full_server_scopes());

    let result = PairRequestResult {
        device_id: record.device_id,
        approved: true,
        token: Some(record.device_token),
        scopes: record.scopes,
    };

    match serde_json::to_value(result) {
        Ok(payload) => ok_res(id, Some(payload)),
        Err(e) => error_res(id, "INTERNAL_ERROR", &format!("{}", e)),
    }
}

async fn handle_device_pair_list(state: &Arc<AppState>, id: &str) -> OpenClawFrame {
    let devices: Vec<PairRequestResult> = state
        .openclaw_state
        .list_devices()
        .into_iter()
        .map(|d| PairRequestResult {
            device_id: d.device_id,
            approved: true,
            token: Some(d.device_token),
            scopes: d.scopes,
        })
        .collect();

    match serde_json::to_value(devices) {
        Ok(payload) => ok_res(id, Some(payload)),
        Err(e) => error_res(id, "INTERNAL_ERROR", &format!("{}", e)),
    }
}

fn ok_res(id: &str, payload: Option<serde_json::Value>) -> OpenClawFrame {
    OpenClawFrame::Res {
        id: id.to_string(),
        ok: true,
        payload,
        error: None,
    }
}

fn error_res(id: &str, code: &str, message: &str) -> OpenClawFrame {
    OpenClawFrame::Res {
        id: id.to_string(),
        ok: false,
        payload: None,
        error: Some(OpenClawErrorShape {
            code: code.to_string(),
            message: message.to_string(),
            details: None,
            retryable: Some(false),
            retry_after_ms: None,
        }),
    }
}

fn generate_nonce() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn full_server_scopes() -> Vec<String> {
    crate::openclaw_server::auth::full_scopes()
}
