use axum::{
    extract::{
        State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::handlers::AgentHandle;
use crate::server::AppState;
use crate::session::SessionId;
use crate::session_store::SessionMessage;

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

    // 创建持久化会话
    if let Err(e) = state
        .session_store
        .create_session(&session_id.to_string())
        .await
    {
        error!("Failed to create session in store: {}", e);
    }

    let (mut sender, mut receiver) = socket.split();

    // 发送欢迎消息
    let welcome = WsResponse::Welcome {
        session_id: session_id.to_string(),
        message: "Connected to Clarity Gateway".to_string(),
    };
    if let Ok(msg) = serde_json::to_string(&welcome)
        && let Err(e) = sender.send(WsMessage::Text(msg)).await
    {
        warn!("Failed to send welcome message: {}", e);
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
                        if let Ok(json) = serde_json::to_string(&error)
                            && let Err(e) = sender.send(WsMessage::Text(json)).await
                        {
                            warn!("Failed to send error response: {}", e);
                            break;
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

    // 记录用户消息到持久化存储
    let user_msg = SessionMessage::new("user", &message);
    if let Err(e) = state
        .session_store
        .append_message(&session_id.to_string(), &user_msg)
        .await
    {
        error!("Failed to append user message: {}", e);
    }

    // Create wire and wire-enabled agent
    let wire = clarity_wire::Wire::new();
    let agent = state.clone_agent().with_wire(Arc::new(wire.clone()));

    // Run agent in background
    let message_clone = message.clone();
    let agent_task = tokio::spawn(async move { agent.run(&message_clone).await });

    // Forward wire messages and view commands to WebSocket via a merge channel
    let (merge_tx, mut merge_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let mut ui_side = wire.ui_side(false);
    let merge_tx_wire = merge_tx.clone();
    let wire_task = tokio::spawn(async move {
        while let Some(msg) = ui_side.recv().await {
            match serde_json::to_string(&msg) {
                Ok(json) => {
                    if merge_tx_wire.send(json).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize wire message: {}", e);
                }
            }
        }
    });
    while let Some(json) = merge_rx.recv().await {
        if let Err(e) = sender.send(WsMessage::Text(json)).await {
            warn!("Failed to send merged message: {}", e);
            break;
        }
    }

    // Clean up background forwarding tasks
    let _ = wire_task.await;

    // Wait for agent to complete
    match agent_task.await {
        Ok(Ok(response_text)) => {
            // 记录助手回复到持久化存储
            let assistant_msg = SessionMessage::new("assistant", &response_text);
            if let Err(e) = state
                .session_store
                .append_message(&session_id.to_string(), &assistant_msg)
                .await
            {
                error!("Failed to append assistant message: {}", e);
            }
        }
        Ok(Err(e)) => {
            error!("Agent execution error in WebSocket: {}", e);
            let error = WsResponse::Error {
                error: format!("Agent execution error: {}", e),
            };
            if let Ok(json) = serde_json::to_string(&error)
                && let Err(e) = sender.send(WsMessage::Text(json)).await
            {
                warn!("Failed to send agent error: {}", e);
            }
        }
        Err(e) => {
            error!("Agent task panicked: {}", e);
            let error = WsResponse::Error {
                error: format!("Agent task panicked: {}", e),
            };
            if let Ok(json) = serde_json::to_string(&error)
                && let Err(e) = sender.send(WsMessage::Text(json)).await
            {
                warn!("Failed to send panic error: {}", e);
            }
        }
    }
}

/// WebSocket 请求类型.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum WsRequest {
    /// Chat message request.
    Chat {
        /// Message text from the user.
        message: String,
        /// Optional request context.
        #[serde(default)]
        context: Option<serde_json::Value>,
        /// Whether to stream wire events.
        #[serde(default)]
        use_wire: bool,
    },
    /// Client keep-alive ping.
    Ping,
    /// Request the conversation history.
    GetHistory,
}

/// WebSocket 响应类型.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum WsResponse {
    /// Initial welcome message on connection.
    Welcome {
        /// Newly assigned session ID.
        session_id: String,
        /// Welcome text.
        message: String,
    },
    /// Assistant chat response.
    Chat {
        /// Response text.
        message: String,
        /// Tool calls issued by the assistant.
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCall>>,
    },
    /// Pong response to a ping.
    Pong,
    /// Conversation history response.
    History {
        /// Messages in the session.
        messages: Vec<ChatMessage>,
    },
    /// Error response.
    Error {
        /// Error message.
        error: String,
    },
}

/// Tool call representation in a WebSocket response.
#[derive(Debug, Serialize)]
pub struct ToolCall {
    /// Name of the tool/function.
    pub name: String,
    /// Arguments passed to the tool.
    pub arguments: serde_json::Value,
}

/// A single chat message returned in the WebSocket history response.
#[derive(Debug, Serialize)]
pub struct ChatMessage {
    /// Message role.
    pub role: String,
    /// Message content.
    pub content: String,
    /// UTC timestamp in RFC 3339 format.
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

            // 记录用户消息到持久化存储
            let user_msg = SessionMessage::new("user", &message);
            if let Err(e) = state
                .session_store
                .append_message(&session_id.to_string(), &user_msg)
                .await
            {
                error!("Failed to append user message: {}", e);
            }

            // 使用 Agent 处理消息
            match state.clone_agent().run(&message).await {
                Ok(response_text) => {
                    // 记录助手回复到持久化存储
                    let assistant_msg = SessionMessage::new("assistant", &response_text);
                    if let Err(e) = state
                        .session_store
                        .append_message(&session_id.to_string(), &assistant_msg)
                        .await
                    {
                        error!("Failed to append assistant message: {}", e);
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
            let messages = state
                .session_store
                .load_session(&session_id.to_string())
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|m| ChatMessage {
                    role: m.role,
                    content: m.content,
                    timestamp: m.created_at.to_rfc3339(),
                })
                .collect();

            WsResponse::History { messages }
        }
    }
}
