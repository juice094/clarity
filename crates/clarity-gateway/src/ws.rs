use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::server::AppState;
use crate::session::SessionId;

/// WebSocket 升级处理器
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// 处理 WebSocket 连接
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let session_id = SessionId::new();
    info!("WebSocket connected: session_id={}", session_id);

    // 创建会话
    {
        let mut manager = state.session_manager.write().await;
        manager.create_session(session_id.clone());
    }

    let (mut sender, mut receiver) = socket.split();

    // 发送欢迎消息
    let welcome = WsResponse::Welcome {
        session_id: session_id.to_string(),
        message: "Connected to Clarity Gateway".to_string(),
    };
    if let Ok(msg) = serde_json::to_string(&welcome) {
        let _ = sender.send(WsMessage::Text(msg)).await;
    }

    // 处理消息循环
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            WsMessage::Text(text) => {
                debug!("Received message from {}: {}", session_id, text);

                match serde_json::from_str::<WsRequest>(&text) {
                    Ok(request) => match request {
                        WsRequest::Chat {
                            message,
                            context: _,
                            use_wire: true,
                        } => {
                            handle_chat_with_wire(&state, &session_id, message, &mut sender).await;
                        }
                        request => {
                            let response = handle_request(&state, &session_id, request).await;
                            match serde_json::to_string(&response) {
                                Ok(json) => {
                                    if let Err(e) = sender.send(WsMessage::Text(json)).await {
                                        warn!("Failed to send message: {}", e);
                                        break;
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to serialize response: {}", e);
                                }
                            }
                        }
                    },
                    Err(e) => {
                        warn!("Invalid request format: {}", e);
                        let error = WsResponse::Error {
                            error: format!("Invalid request: {}", e),
                        };
                        if let Ok(json) = serde_json::to_string(&error) {
                            let _ = sender.send(WsMessage::Text(json)).await;
                        }
                    }
                }
            }
            WsMessage::Close(_) => {
                info!("WebSocket closed by client: session_id={}", session_id);
                break;
            }
            WsMessage::Ping(data) => {
                if let Err(e) = sender.send(WsMessage::Pong(data)).await {
                    warn!("Failed to send pong: {}", e);
                    break;
                }
            }
            _ => {}
        }
    }

    // 清理会话
    {
        let mut manager = state.session_manager.write().await;
        manager.destroy_session(&session_id);
    }
    info!("WebSocket disconnected: session_id={}", session_id);
}

/// Handle a Chat request with wire streaming.
async fn handle_chat_with_wire(
    state: &AppState,
    session_id: &SessionId,
    message: String,
    sender: &mut futures::stream::SplitSink<WebSocket, WsMessage>,
) {
    debug!(
        "Processing wire chat request from {}: message={}",
        session_id, message
    );

    // 记录用户消息
    {
        let mut manager = state.session_manager.write().await;
        if let Some(session) = manager.get_session_mut(session_id) {
            session.record_message("user", &message);
        }
    }

    // Create wire and wire-enabled agent
    let wire = clarity_wire::Wire::new();
    let agent = (*state.agent).clone().with_wire(Arc::new(wire.clone()));

    // Run agent in background
    let message_clone = message.clone();
    let agent_task = tokio::spawn(async move { agent.run(&message_clone).await });

    // Forward wire messages to WebSocket
    let mut ui_side = wire.ui_side(false);
    while let Some(msg) = ui_side.recv().await {
        match serde_json::to_string(&msg) {
            Ok(json) => {
                if let Err(e) = sender.send(WsMessage::Text(json)).await {
                    warn!("Failed to send wire message: {}", e);
                    break;
                }
            }
            Err(e) => {
                error!("Failed to serialize wire message: {}", e);
            }
        }
    }

    // Wait for agent to complete
    match agent_task.await {
        Ok(Ok(response_text)) => {
            // 记录助手回复
            {
                let mut manager = state.session_manager.write().await;
                if let Some(session) = manager.get_session_mut(session_id) {
                    session.record_message("assistant", &response_text);
                }
            }
        }
        Ok(Err(e)) => {
            error!("Agent execution error in WebSocket: {}", e);
            let error = WsResponse::Error {
                error: format!("Agent execution error: {}", e),
            };
            if let Ok(json) = serde_json::to_string(&error) {
                let _ = sender.send(WsMessage::Text(json)).await;
            }
        }
        Err(e) => {
            error!("Agent task panicked: {}", e);
            let error = WsResponse::Error {
                error: format!("Agent task panicked: {}", e),
            };
            if let Ok(json) = serde_json::to_string(&error) {
                let _ = sender.send(WsMessage::Text(json)).await;
            }
        }
    }
}

/// WebSocket 请求
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum WsRequest {
    Chat {
        message: String,
        #[serde(default)]
        context: Option<serde_json::Value>,
        #[serde(default)]
        use_wire: bool,
    },
    Ping,
    GetHistory,
}

/// WebSocket 响应
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum WsResponse {
    Welcome {
        session_id: String,
        message: String,
    },
    Chat {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCall>>,
    },
    Pong,
    History {
        messages: Vec<ChatMessage>,
    },
    Error {
        error: String,
    },
}

#[derive(Debug, Serialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

/// 处理 WebSocket 请求
async fn handle_request(
    state: &AppState,
    session_id: &SessionId,
    request: WsRequest,
) -> WsResponse {
    match request {
        WsRequest::Chat {
            message,
            context: _,
            use_wire: _,
        } => {
            debug!(
                "Processing chat request from {}: message={}",
                session_id, message
            );

            // 记录用户消息
            {
                let mut manager = state.session_manager.write().await;
                if let Some(session) = manager.get_session_mut(session_id) {
                    session.record_message("user", &message);
                }
            }

            // 使用 Agent 处理消息
            match state.agent.run(&message).await {
                Ok(response_text) => {
                    // 记录助手回复
                    {
                        let mut manager = state.session_manager.write().await;
                        if let Some(session) = manager.get_session_mut(session_id) {
                            session.record_message("assistant", &response_text);
                        }
                    }

                    WsResponse::Chat {
                        message: response_text,
                        tool_calls: None,
                    }
                }
                Err(e) => {
                    error!("Agent execution error in WebSocket: {}", e);
                    WsResponse::Error {
                        error: format!("Agent execution error: {}", e),
                    }
                }
            }
        }
        WsRequest::Ping => WsResponse::Pong,
        WsRequest::GetHistory => {
            let messages = {
                let manager = state.session_manager.read().await;
                manager
                    .get_session(session_id)
                    .map(|session| {
                        session
                            .get_messages()
                            .iter()
                            .map(|m| ChatMessage {
                                role: m.role.clone(),
                                content: m.content.clone(),
                                timestamp: m.timestamp.to_rfc3339(),
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            };

            WsResponse::History { messages }
        }
    }
}
