use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, Sse},
        IntoResponse, Response,
    },
};
use clarity_core::agent::{
    driver::ConversationChatDriver, AgentController, ControllerEvent, Message as AgentMessage,
    MessageRole, Op,
};
use futures::stream;
use serde::{Deserialize, Serialize};

use std::convert::Infallible;
use std::sync::Arc;
use tracing::{debug, info, warn};

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
    /// Optional session ID for persisting the conversation.
    /// If provided, the assistant's reply will be appended to the session.
    #[serde(default)]
    pub session_id: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
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

    // Acquire concurrency permit — prevents unbounded spawn under load.
    let _permit = match state.chat_sem.acquire().await {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "Gateway is at maximum concurrency limit"
                })),
            )
                .into_response();
        }
    };

    let _ = state.session_store.record_request().await;

    // Resolve or create session id
    let session_id = req
        .session_id
        .clone()
        .or_else(|| Some(format!("http-{}", uuid::Uuid::new_v4().simple())));

    // Validate that there is at least one user message
    let has_user = req.messages.iter().any(|m| m.role == "user");
    if !has_user {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "No user message found in request"
            })),
        )
            .into_response();
    }

    // Create a per-request AgentController so that streaming events are isolated.
    let agent = (*state.agent).clone();

    // Security: log a warning when the agent is in Yolo mode via Gateway,
    // since HTTP clients cannot interactively approve dangerous tool calls.
    if agent.approval_mode() == clarity_core::approval::ApprovalMode::Yolo {
        warn!(
            "Gateway chat_completions running with Yolo approval mode — \
             tool calls will execute without user confirmation"
        );
    }

    // Build the internal message list:
    // 1. Agent system prompt
    // 2. Session history (if session_id provided and session exists)
    // 3. All non-system messages from the request
    let mut messages: Vec<AgentMessage> = vec![AgentMessage::system(agent.build_system_prompt())];

    // Load session history if a session_id was explicitly provided
    if let Some(ref sid) = req.session_id {
        match state.session_store.load_session(sid).await {
            Ok(history) => {
                for msg in history {
                    let role = match msg.role.as_str() {
                        "user" => MessageRole::User,
                        "assistant" => MessageRole::Assistant,
                        "tool" => MessageRole::Tool,
                        _ => MessageRole::User,
                    };
                    messages.push(AgentMessage {
                        role,
                        content: msg.content,
                        tool_calls: msg
                            .tool_calls
                            .map(|s| serde_json::from_str(&s).unwrap_or_default()),
                        tool_call_id: msg.tool_call_id,
                    });
                }
            }
            Err(e) => {
                debug!("Failed to load session {}: {}. Starting fresh.", sid, e);
            }
        }
    }

    for msg in &req.messages {
        if msg.role == "system" {
            continue;
        }
        let role = match msg.role.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "tool" => MessageRole::Tool,
            _ => MessageRole::User,
        };
        messages.push(AgentMessage {
            role,
            content: msg.content.clone(),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    // Pre-calculate token estimate before messages are moved into the controller.
    let prompt_tokens = messages.iter().map(|m| m.content.len()).sum::<usize>() as u32 / 4;

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<ControllerEvent>();
    let driver = Arc::new(ConversationChatDriver {
        history: messages.clone(),
    });
    let (controller, op_tx) = AgentController::new_with_events(agent, event_tx, Some(driver));
    tokio::spawn(controller.run());

    let last_user_query = req
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();
    if let Err(e) = op_tx.send(Op::UserTurn(last_user_query)) {
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

        // Clone session store for background persistence in streaming mode
        let store = state.session_store.clone();
        let sid = session_id.clone();
        let last_user_content = req
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.clone());

        let sse_stream = stream::unfold((event_rx, 0u8), move |(mut rx, step)| {
            let model = model.clone();
            let id = id.clone();
            let store = store.clone();
            let sid = sid.clone();
            let last_user_content = last_user_content.clone();
            async move {
                if step == 2 {
                    return None;
                }
                if step == 1 {
                    let event = SseEvent::default().data("[DONE]");
                    return Some((Ok::<_, Infallible>(event), (rx, 2)));
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
                        Some((Ok(event), (rx, 0)))
                    }
                    Some(ControllerEvent::ToolCallStart {
                        id: tc_id,
                        name,
                        arguments,
                    }) => {
                        let args_str = arguments.to_string();
                        let data = serde_json::json!({
                            "id": &id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": &model,
                            "choices": [{
                                "index": 0,
                                "delta": {
                                    "tool_calls": [{
                                        "index": 0,
                                        "id": tc_id,
                                        "type": "function",
                                        "function": {
                                            "name": name,
                                            "arguments": args_str
                                        }
                                    }]
                                },
                                "finish_reason": null
                            }]
                        });
                        let event = SseEvent::default().data(data.to_string());
                        Some((Ok(event), (rx, 0)))
                    }
                    Some(ControllerEvent::ToolResult { id: tc_id, result }) => {
                        let data = serde_json::json!({
                            "object": "clarity.event",
                            "type": "tool_result",
                            "id": tc_id,
                            "result": result
                        });
                        let event = SseEvent::default().data(data.to_string());
                        Some((Ok(event), (rx, 0)))
                    }
                    Some(ControllerEvent::StepBegin { tool_name }) => {
                        let data = serde_json::json!({
                            "object": "clarity.event",
                            "type": "step_begin",
                            "tool_name": tool_name
                        });
                        let event = SseEvent::default().data(data.to_string());
                        Some((Ok(event), (rx, 0)))
                    }
                    Some(ControllerEvent::Complete(final_text)) => {
                        // Persist the turn in the background
                        if let (Some(s), Some(uc)) = (sid, last_user_content) {
                            tokio::spawn(async move {
                                let _ = store
                                    .append_message(
                                        &s,
                                        &crate::session_store::SessionMessage::new("user", &uc),
                                    )
                                    .await;
                                let _ = store
                                    .append_message(
                                        &s,
                                        &crate::session_store::SessionMessage::new(
                                            "assistant",
                                            &final_text,
                                        ),
                                    )
                                    .await;
                            });
                        }
                        let data = serde_json::json!({
                            "id": &id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": &model,
                            "choices": [{"index":0,"delta":{},"finish_reason":"stop"}]
                        });
                        let event = SseEvent::default().data(data.to_string());
                        Some((Ok(event), (rx, 1)))
                    }
                    Some(ControllerEvent::Error(_)) | None => {
                        let data = serde_json::json!({
                            "id": &id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": &model,
                            "choices": [{"index":0,"delta":{},"finish_reason":"stop"}]
                        });
                        let event = SseEvent::default().data(data.to_string());
                        Some((Ok(event), (rx, 1)))
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
                // Tool-calling events are not included in non-streaming text response.
                ControllerEvent::ToolCallStart { .. }
                | ControllerEvent::ToolResult { .. }
                | ControllerEvent::StepBegin { .. } => {}
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

        let completion_tokens = content.len() as u32 / 4;

        // Persist the turn to the session store (non-streaming)
        if let Some(ref sid) = session_id {
            if let Some(last_user) = req.messages.iter().rev().find(|m| m.role == "user") {
                let _ = state
                    .session_store
                    .append_message(
                        sid,
                        &crate::session_store::SessionMessage::new("user", &last_user.content),
                    )
                    .await;
            }
            let _ = state
                .session_store
                .append_message(
                    sid,
                    &crate::session_store::SessionMessage::new("assistant", &content),
                )
                .await;
        }

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
            session_id,
        };

        (StatusCode::OK, Json(response)).into_response()
    }
}

