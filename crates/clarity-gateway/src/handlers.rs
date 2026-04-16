use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response, sse::{Event as SseEvent, Sse}},
};
use clarity_core::agent::{AgentController, ControllerEvent, Op};
use futures::stream;
use serde::{Deserialize, Serialize};

use std::convert::Infallible;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::server::AppState;

// ==================== 健康检查 ====================

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub timestamp: String,
}

pub async fn health_check() -> impl IntoResponse {
    debug!("Health check requested");
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    (StatusCode::OK, Json(response))
}

// ==================== Chat Completions ====================

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: String,
}

#[derive(Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    info!(
        "Chat completion request: model={}, stream={}",
        req.model, req.stream
    );
    debug!("Request messages: {:?}", req.messages);

    state.session_manager.read().await.record_request();

    // Extract the last user message
    let user_message = req
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone());

    let user_message = match user_message {
        Some(content) => content,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "No user message found in request"
                })),
            )
                .into_response();
        }
    };

    // Create a per-request AgentController so that streaming events are isolated.
    let agent = (*state.agent).clone();
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<ControllerEvent>();
    let (controller, op_tx) = AgentController::new_with_events(agent, event_tx);
    tokio::spawn(controller.run());

    if let Err(e) = op_tx.send(Op::UserTurn(user_message.clone())) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to start agent turn: {}", e)
            })),
        )
            .into_response();
    }

    if req.stream {
        let model = req.model.clone();
        let created = chrono::Utc::now().timestamp();
        let id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());

        let sse_stream = stream::unfold((event_rx, Option::<String>::None), move |(mut rx, pending)| {
            let model = model.clone();
            let id = id.clone();
            async move {
                if let Some(data) = pending {
                    let event = SseEvent::default().data(data);
                    return Some((Ok::<_, Infallible>(event), (rx, None)));
                }
                match rx.recv().await {
                    Some(ControllerEvent::Chunk(text)) => {
                        let data = serde_json::json!({
                            "id": &id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": &model,
                            "choices": [{"index":0,"delta":{"content":text},"finish_reason":null}]
                        });
                        let event = SseEvent::default().data(data.to_string());
                        Some((Ok(event), (rx, None)))
                    }
                    Some(ControllerEvent::Complete(_)) | Some(ControllerEvent::Error(_)) | None => {
                        let data = serde_json::json!({
                            "id": &id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": &model,
                            "choices": [{"index":0,"delta":{},"finish_reason":"stop"}]
                        });
                        let event = SseEvent::default().data(data.to_string());
                        Some((Ok(event), (rx, Some("[DONE]".to_string()))))
                    }
                }
            }
        });

        Sse::new(sse_stream).into_response()
    } else {
        // Non-streaming: accumulate chunks until Complete/Error.
        let mut content = String::new();
        let mut error_msg: Option<String> = None;
        while let Some(ev) = event_rx.recv().await {
            match ev {
                ControllerEvent::Chunk(chunk) => content.push_str(&chunk),
                ControllerEvent::Complete(final_text) => {
                    content = final_text;
                    break;
                }
                ControllerEvent::Error(e) => {
                    error_msg = Some(e);
                    break;
                }
            }
        }

        if let Some(e) = error_msg {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Agent execution error: {}", e)
                })),
            )
                .into_response();
        }

        let prompt_tokens = user_message.len() as u32 / 4;
        let completion_tokens = content.len() as u32 / 4;

        let response = ChatCompletionResponse {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: req.model,
            choices: vec![Choice {
                index: 0,
                message: Message {
                    role: "assistant".to_string(),
                    content,
                },
                finish_reason: "stop".to_string(),
            }],
            usage: Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
        };

        (StatusCode::OK, Json(response)).into_response()
    }
}

// ==================== Admin API ====================

#[derive(Serialize)]
pub struct StatsResponse {
    pub active_sessions: usize,
    pub total_requests: u64,
    pub uptime_seconds: u64,
}

pub async fn admin_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let session_manager = state.session_manager.read().await;
    let stats = StatsResponse {
        active_sessions: session_manager.active_session_count(),
        total_requests: session_manager.total_requests(),
        uptime_seconds: session_manager.uptime_seconds(),
    };
    (StatusCode::OK, Json(stats))
}

#[derive(Serialize)]
pub struct ToolsResponse {
    pub tools: Vec<ToolInfo>,
}

#[derive(Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub enabled: bool,
}

pub async fn admin_tools(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tools = match state.tool_registry.get_tool_schemas() {
        Ok(schemas) => {
            if let Some(functions) = schemas.as_array() {
                functions
                    .iter()
                    .filter_map(|f| {
                        let name = f.get("function")?.get("name")?.as_str()?.to_string();
                        let description = f.get("function")?.get("description")?.as_str()?.to_string();
                        Some(ToolInfo {
                            name,
                            description,
                            enabled: true,
                        })
                    })
                    .collect()
            } else {
                vec![]
            }
        }
        Err(e) => {
            error!("Failed to get tool schemas: {}", e);
            vec![]
        }
    };

    (StatusCode::OK, Json(ToolsResponse { tools }))
}
