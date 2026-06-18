//! Thread-scoped chat completion handler — `POST /api/v2/threads/:id/chat`.
//!
//! Streams (or returns) a chat completion within an existing V2 thread,
//! loading the thread's rollout history as the conversation context.

use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use clarity_contract::ThreadId;
use clarity_core::agent::{
    AgentController, ControllerEvent, Message as AgentMessage, MessageRole, Op,
    driver::ConversationChatDriver,
};
use clarity_core::approval::ApprovalMode;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::handlers::AgentHandle;
use crate::handlers::chat::{
    ChatCompletionResponse, Choice, Message as ResponseMessage, Usage, chat_completion_sse_stream,
    truncate_messages_by_bytes,
};
use crate::server::AppState;

/// OpenAI-compatible chat completion request scoped to a thread.
#[derive(Debug, Deserialize)]
pub struct ThreadChatRequest {
    /// Model alias or identifier requested by the client.
    pub model: String,
    /// Conversation messages for this turn.
    pub messages: Vec<crate::handlers::chat::Message>,
    /// Whether to stream the response as SSE.
    #[serde(default)]
    pub stream: bool,
}

fn bad_request(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
        .into_response()
}

fn thread_not_found(id: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "error": format!("thread not found: {id}") })),
    )
        .into_response()
}

fn store_error(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
        .into_response()
}

/// Chat completion within an existing V2 thread.
///
/// Loads the thread's persisted history, appends the new request messages,
/// runs one agent turn, and persists the resulting user/assistant exchange
/// back to the thread rollout. Supports both streaming SSE and non-streaming
/// JSON responses in the same shape as `/v1/chat/completions`.
pub async fn thread_chat(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ThreadChatRequest>,
) -> Response {
    info!(
        "Thread chat request: thread_id={id}, model={}, stream={}",
        req.model, req.stream
    );
    debug!("Request messages: {:?}", req.messages);

    let thread_id = match ThreadId::from_string(&id) {
        Ok(t) => t,
        Err(e) => return bad_request(format!("invalid thread id: {e}")),
    };

    // Verify the thread exists before spending any resources on it.
    match state.thread_manager.read_thread(thread_id, false).await {
        Ok(_) => {}
        Err(clarity_core::thread::ThreadManagerError::ThreadStore(
            clarity_thread_store::ThreadStoreError::NotFound { .. },
        )) => return thread_not_found(&id),
        Err(e) => return store_error(e),
    }

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

    // Validate that there is at least one user message and locate the last one.
    let last_user_content = match req
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
    {
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

    // Load persisted history and convert it into agent-ready messages.
    // A missing or unreadable rollout is treated as an empty history so that
    // freshly created threads can still accept their first turn.
    let history = match state.thread_manager.load_llm_history(thread_id).await {
        Ok(h) => h,
        Err(e) => {
            warn!(
                "Failed to load thread history for {}, starting fresh: {}",
                thread_id, e
            );
            Vec::new()
        }
    };

    let mut messages: Vec<AgentMessage> = Vec::with_capacity(history.len() + req.messages.len());
    for msg in history {
        messages.push(msg);
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

    // Create a per-request AgentController so that streaming events are isolated.
    let agent = state.clone_agent();

    // Security: log a warning when the agent is in Yolo mode via Gateway,
    // since HTTP clients cannot interactively approve dangerous tool calls.
    if agent.approval_mode() == ApprovalMode::Yolo {
        warn!(
            "Gateway thread_chat running with Yolo approval mode — \
             tool calls will execute without user confirmation"
        );
    }

    const MAX_BODY_BYTES: usize = 1_500_000;
    let original_count = messages.len();
    messages = truncate_messages_by_bytes(messages, MAX_BODY_BYTES);
    if messages.len() < original_count {
        warn!(
            "Truncated thread context from {} to {} messages to fit {} byte budget",
            original_count,
            messages.len(),
            MAX_BODY_BYTES
        );
    }

    let prompt_tokens = messages.iter().map(|m| m.content.len()).sum::<usize>() as u32 / 4;

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<ControllerEvent>();
    let driver = Arc::new(ConversationChatDriver {
        history: messages.clone(),
    });
    let (controller, op_tx) = AgentController::new_with_events(agent, event_tx, Some(driver));
    tokio::spawn(controller.run());

    if let Err(e) = op_tx.send(Op::UserTurn(last_user_content.clone())) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to start agent turn: {}", e)
            })),
        )
            .into_response();
    }

    if req.stream {
        let thread_manager = state.thread_manager.clone();
        let user_content = Arc::new(last_user_content.clone());
        let on_complete = move |final_text: String| {
            let tm = thread_manager.clone();
            let uc = (*user_content).clone();
            tokio::spawn(async move {
                let _ = tm.append_turn(thread_id, uc, final_text).await;
            });
        };

        chat_completion_sse_stream(req.model.clone(), event_rx, on_complete).into_response()
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

        // Persist the completed turn back to the thread rollout.
        let _ = state
            .thread_manager
            .append_turn(thread_id, last_user_content, content.clone())
            .await;

        let response = ChatCompletionResponse {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: req.model,
            choices: vec![Choice {
                index: 0,
                message: ResponseMessage {
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
            session_id: None,
        };

        (StatusCode::OK, Json(response)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use crate::handlers::tests::test_state;
    use crate::server::create_api_router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use clarity_contract::SessionSource;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_thread_chat_not_found() {
        let state = test_state().await;
        let app = create_api_router(state);
        let missing_thread_id = uuid::Uuid::new_v4().to_string();

        let body = serde_json::json!({
            "model": "mock",
            "messages": [{"role": "user", "content": "hello"}],
            "stream": false
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/threads/{missing_thread_id}/chat"))
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_thread_chat_non_streaming_shape() {
        let state = test_state().await;
        let app = create_api_router(state.clone());

        let cwd = state.agent.config().working_dir.clone();
        let thread_id = state
            .thread_manager
            .create_thread(&cwd, "test", SessionSource::Test)
            .await
            .unwrap();

        let body = serde_json::json!({
            "model": "mock",
            "messages": [{"role": "user", "content": "hello"}],
            "stream": false
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/threads/{thread_id}/chat"))
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["object"], "chat.completion");
        assert_eq!(json["choices"][0]["message"]["role"], "assistant");
        assert!(
            json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap()
                .contains("mock response")
        );
        assert!(json["usage"]["total_tokens"].as_u64().is_some());

        // Verify the turn was persisted back to the thread.
        let stored = state
            .thread_manager
            .read_thread(thread_id, true)
            .await
            .unwrap();
        let items = stored.history.expect("history").items;
        assert!(items.iter().any(|item| matches!(
            item,
            clarity_contract::RolloutItem::ResponseItem(
                clarity_contract::RolloutResponseItem::Message { role, content }
            ) if role == "assistant" && content.contains("mock response")
        )));
    }

    #[tokio::test]
    async fn test_thread_chat_streaming_shape() {
        let state = test_state().await;
        let app = create_api_router(state.clone());

        let cwd = state.agent.config().working_dir.clone();
        let thread_id = state
            .thread_manager
            .create_thread(&cwd, "test", SessionSource::Test)
            .await
            .unwrap();

        let body = serde_json::json!({
            "model": "mock",
            "messages": [{"role": "user", "content": "hello"}],
            "stream": true
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/threads/{thread_id}/chat"))
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("chat.completion.chunk"));
        assert!(text.contains("\"finish_reason\":\"stop\""));
        assert!(text.contains("[DONE]"));
    }
}
