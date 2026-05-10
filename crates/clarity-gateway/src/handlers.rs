use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, Sse},
        IntoResponse, Response,
    },
};
use chrono::Utc;
use clarity_core::activity::WindowActivity;
use clarity_core::agent::{
    driver::ConversationChatDriver, AgentController, ControllerEvent, Message as AgentMessage,
    MessageRole, Op,
};
use clarity_llm::LlmFactory;
use futures::stream;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use std::convert::Infallible;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

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

// ==================== Admin API ====================

#[derive(Serialize)]
pub(crate) struct StatsResponse {
    pub active_sessions: usize,
    pub total_requests: u64,
    pub uptime_seconds: u64,
}

pub(crate) async fn admin_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let active_sessions = state.session_store.session_count().await.unwrap_or(0);
    let total_requests = state.session_store.total_requests().await.unwrap_or(0);
    let uptime_seconds = (Utc::now() - state.started_at).num_seconds() as u64;
    let stats = StatsResponse {
        active_sessions,
        total_requests,
        uptime_seconds,
    };
    (StatusCode::OK, Json(stats))
}

#[derive(Serialize)]
pub(crate) struct ToolsResponse {
    pub tools: Vec<ToolInfo>,
}

#[derive(Serialize)]
pub(crate) struct ToolInfo {
    pub name: String,
    pub description: String,
    pub enabled: bool,
}

pub(crate) async fn admin_tools(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tools = match state.agent.registry().get_tool_schemas() {
        Ok(schemas) => {
            if let Some(functions) = schemas.as_array() {
                functions
                    .iter()
                    .filter_map(|f| {
                        let name = f.get("function")?.get("name")?.as_str()?.to_string();
                        let description =
                            f.get("function")?.get("description")?.as_str()?.to_string();
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

// ==================== Admin: List Available Models ====================

#[derive(Serialize)]
pub(crate) struct ModelsResponse {
    pub models: Vec<ModelInfo>,
}

#[derive(Serialize)]
pub(crate) struct ModelInfo {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub protocol: String,
}

pub(crate) async fn admin_models() -> impl IntoResponse {
    let registry = match clarity_llm::ModelRegistry::load_async().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to load model registry: {}", e);
            return (StatusCode::OK, Json(ModelsResponse { models: vec![] }));
        }
    };

    let models: Vec<ModelInfo> = registry
        .list_models()
        .into_iter()
        .map(|m| {
            let protocol = registry
                .get_provider(&m.provider)
                .map(|p| format!("{:?}", p.protocol))
                .unwrap_or_else(|| "unknown".into());
            ModelInfo {
                alias: m.alias.clone(),
                provider: m.provider.clone(),
                model_id: m.model_id.clone(),
                protocol,
            }
        })
        .collect();

    (StatusCode::OK, Json(ModelsResponse { models }))
}

// ==================== Background Tasks API ====================

use clarity_core::background::TaskId;
use clarity_core::background::{TaskResult, TaskSpec, TaskStatus};

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTaskRequest {
    pub name: String,
    pub prompt: String,
    #[serde(default)]
    pub max_iterations: Option<usize>,
}

#[derive(Serialize)]
pub(crate) struct TaskCreateResponse {
    pub task_id: TaskId,
    pub status: TaskStatus,
}

#[derive(Serialize)]
pub(crate) struct TaskDetailResponse {
    pub task_id: TaskId,
    pub name: String,
    pub status: TaskStatus,
    pub prompt: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub result: Option<TaskResult>,
}

pub(crate) async fn create_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTaskRequest>,
) -> Response {
    let spec = TaskSpec::new(req.name.clone(), req.prompt)
        .with_agent_type("default")
        .with_max_iterations(req.max_iterations.unwrap_or(10));

    match state.task_manager.spawn_agent(spec).await {
        Ok(task_id) => {
            let response = TaskCreateResponse {
                task_id: task_id.clone(),
                status: TaskStatus::Pending,
            };
            info!("Created background task {}", task_id);
            (StatusCode::ACCEPTED, Json(response)).into_response()
        }
        Err(e) => {
            error!("Failed to create background task: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

pub(crate) async fn get_task(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(task_id): axum::extract::Path<TaskId>,
) -> Response {
    info!("get_task called with id={}", task_id);
    let store = state.task_manager.store();
    match store.get(&task_id).await {
        Ok(info) => {
            let result = if info.status.is_terminal() {
                store.get_result(&task_id).await.ok()
            } else {
                None
            };
            let response = TaskDetailResponse {
                task_id: info.id,
                name: info.spec.name,
                status: info.status,
                prompt: info.spec.prompt,
                created_at: info.created_at,
                updated_at: info.updated_at,
                result,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("Failed to get task {}: {}", task_id, e);
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

pub(crate) async fn cancel_task(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(task_id): axum::extract::Path<TaskId>,
) -> impl IntoResponse {
    info!("cancel_task called with id={}", task_id);
    match state.task_manager.cancel(&task_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"cancelled": true}))),
        Err(e) => {
            error!("Failed to cancel task {}: {}", task_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

#[derive(Serialize)]
pub(crate) struct TaskListResponse {
    pub tasks: Vec<TaskDetailResponse>,
}

pub(crate) async fn list_tasks(State(state): State<Arc<AppState>>) -> Response {
    info!("list_tasks called");
    match state.task_manager.list().await {
        Ok(tasks) => {
            let response = TaskListResponse {
                tasks: tasks
                    .into_iter()
                    .map(|info| TaskDetailResponse {
                        task_id: info.id,
                        name: info.spec.name,
                        status: info.status,
                        prompt: info.spec.prompt,
                        created_at: info.created_at,
                        updated_at: info.updated_at,
                        result: None,
                    })
                    .collect(),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("Failed to list tasks: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

// ==================== Parallel Subagent Execution API ====================

#[derive(Debug, Deserialize)]
pub(crate) struct ParallelTaskSpec {
    pub agent_type: String,
    pub prompt: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RunParallelRequest {
    pub tasks: Vec<ParallelTaskSpec>,
    #[serde(default)]
    pub max_concurrency: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ParallelTaskResult {
    pub agent_id: String,
    pub agent_type: String,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ParallelFailure {
    pub task_id: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RunParallelResponse {
    pub success_rate: f64,
    pub total_elapsed_ms: u64,
    pub results: Vec<ParallelTaskResult>,
    pub failures: Vec<ParallelFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
}

pub(crate) async fn run_parallel(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunParallelRequest>,
) -> Response {
    if req.tasks.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No tasks provided"})),
        )
            .into_response();
    }

    let batch_id = uuid::Uuid::new_v4().to_string();

    // Clone task specs before moving them into batch_progress
    let task_refs: Vec<(String, String)> = req
        .tasks
        .iter()
        .map(|t| (t.agent_type.clone(), t.prompt.clone()))
        .collect();

    let specs: Vec<clarity_contract::subagent::RunSpec> = task_refs
        .iter()
        .map(|(agent_type, prompt)| {
            clarity_contract::subagent::RunSpec::new(
                format!("parallel-{}", agent_type),
                prompt.clone(),
            )
            .with_type(agent_type)
        })
        .collect();

    // Create and register batch progress
    let progress = Arc::new(parking_lot::Mutex::new(
        clarity_contract::subagent::BatchProgress::new(batch_id.clone(), &specs),
    ));
    {
        let mut batches = state.parallel_batches.write().await;
        batches.insert(batch_id.clone(), progress.clone());
    }

    let config = clarity_contract::subagent::ParallelConfig::new()
        .with_max_concurrency(req.max_concurrency.unwrap_or(4).max(1));

    let agent = (*state.agent).clone();

    match agent
        .run_parallel(specs, config, Some(progress.clone()))
        .await
    {
        Ok(result) => {
            let success_rate = result.success_rate();
            let total_elapsed_ms = result.total_elapsed_ms;

            let results: Vec<ParallelTaskResult> = result
                .results
                .into_iter()
                .map(|r| ParallelTaskResult {
                    agent_id: r.agent_id,
                    agent_type: r.agent_type,
                    status: format!("{:?}", r.status),
                    summary: r.summary,
                })
                .collect();

            let failures: Vec<ParallelFailure> = result
                .failures
                .into_iter()
                .map(|(id, err)| ParallelFailure {
                    task_id: id,
                    error: err,
                })
                .collect();

            let response = RunParallelResponse {
                success_rate,
                total_elapsed_ms,
                results,
                failures,
                batch_id: Some(batch_id.clone()),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            // Mark progress as failed
            let mut p = progress.lock();
            p.status = clarity_contract::subagent::BatchStatus::Failed(e.to_string());
            error!("Parallel execution failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// Query the current progress of a parallel batch.
#[derive(Serialize)]
pub(crate) struct ParallelStatusResponse {
    pub batch_id: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub status: String,
    pub elapsed_ms: u64,
    pub agent_statuses: Vec<AgentStatusSummary>,
}

#[derive(Serialize)]
pub(crate) struct AgentStatusSummary {
    pub agent_id: String,
    pub status: String,
    pub summary: Option<String>,
}

pub(crate) async fn get_parallel_status(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(batch_id): axum::extract::Path<String>,
) -> Response {
    let batches = state.parallel_batches.read().await;
    match batches.get(&batch_id) {
        Some(progress_arc) => {
            let p = progress_arc.lock();
            let status_str = match &p.status {
                clarity_contract::subagent::BatchStatus::Running => "Running",
                clarity_contract::subagent::BatchStatus::Completed => "Completed",
                clarity_contract::subagent::BatchStatus::Cancelled => "Cancelled",
                clarity_contract::subagent::BatchStatus::Failed(_) => "Failed",
            };

            let mut agent_statuses: Vec<AgentStatusSummary> = p
                .results
                .iter()
                .map(|r| AgentStatusSummary {
                    agent_id: r.agent_id.clone(),
                    status: "Completed".to_string(),
                    summary: Some(r.summary.clone()),
                })
                .collect();

            // Add running agents
            for id in &p.running {
                agent_statuses.push(AgentStatusSummary {
                    agent_id: id.clone(),
                    status: "Running".to_string(),
                    summary: None,
                });
            }

            // Add failures
            for (id, err) in &p.failures {
                agent_statuses.push(AgentStatusSummary {
                    agent_id: id.clone(),
                    status: "Failed".to_string(),
                    summary: Some(err.clone()),
                });
            }

            let response = ParallelStatusResponse {
                batch_id: p.batch_id.clone(),
                total: p.total,
                completed: p.completed,
                failed: p.failed,
                status: status_str.to_string(),
                elapsed_ms: p.elapsed_ms,
                agent_statuses,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Batch not found"})),
        )
            .into_response(),
    }
}

// ==================== Admin: Approval Mode ====================

#[derive(Deserialize)]
pub(crate) struct SetApprovalModeRequest {
    pub mode: String,
}

#[derive(Serialize)]
pub(crate) struct ApprovalModeResponse {
    pub mode: String,
}

pub(crate) async fn admin_set_approval_mode(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetApprovalModeRequest>,
) -> Response {
    use clarity_core::approval::ApprovalMode;

    let mode = match req.mode.to_lowercase().as_str() {
        "interactive" => ApprovalMode::Interactive,
        "yolo" => ApprovalMode::Yolo,
        "plan" => ApprovalMode::Plan,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid mode. Use: interactive, yolo, plan"})),
            )
                .into_response();
        }
    };

    state.agent.set_approval_mode(mode);
    let resp = ApprovalModeResponse {
        mode: format!("{:?}", mode).to_lowercase(),
    };
    (StatusCode::OK, Json(resp)).into_response()
}

pub(crate) async fn admin_get_approval_mode(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let mode = state.agent.approval_mode();
    let resp = ApprovalModeResponse {
        mode: format!("{:?}", mode).to_lowercase(),
    };
    (StatusCode::OK, Json(resp))
}

// ==================== Admin: Switch Provider ====================

#[derive(Deserialize)]
pub(crate) struct SwitchProviderRequest {
    pub provider: String,
}

#[derive(Serialize)]
pub(crate) struct SwitchProviderResponse {
    pub provider: String,
    pub message: String,
}

pub(crate) async fn admin_switch_provider(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SwitchProviderRequest>,
) -> impl IntoResponse {
    info!("Admin: switching provider to '{}'", req.provider);

    match LlmFactory::create(&req.provider).await {
        Ok(new_llm) => {
            state.agent.set_llm(Arc::from(new_llm));
            let resp = SwitchProviderResponse {
                provider: req.provider,
                message: "Provider switched successfully".to_string(),
            };
            (StatusCode::OK, Json(resp))
        }
        Err(e) => {
            error!("Failed to switch provider: {}", e);
            let resp = SwitchProviderResponse {
                provider: req.provider,
                message: format!("Failed to create provider: {}", e),
            };
            (StatusCode::BAD_REQUEST, Json(resp))
        }
    }
}

// ==================== Configuration API ====================

use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetConfigRequest {
    pub provider: String,
    pub api_key: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConfigResponse {
    pub provider: String,
    pub api_key_masked: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConfigStatusResponse {
    pub configured: bool,
    pub config: Option<ConfigResponse>,
}

fn config_file_path() -> PathBuf {
    PathBuf::from(".clarity").join("user_config.json")
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}****{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Load persisted user config from JSON file
pub async fn load_persisted_config() -> Option<SetConfigRequest> {
    let path = config_file_path();
    if !path.exists() {
        return None;
    }
    let contents = tokio::fs::read_to_string(&path).await.ok()?;
    serde_json::from_str(&contents).ok()
}

/// Save user config to JSON file
async fn save_persisted_config(cfg: &SetConfigRequest) -> Result<(), String> {
    let path = config_file_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| e.to_string())
}

/// Build an LLM provider from a user config request
pub async fn build_provider_from_config(
    cfg: &SetConfigRequest,
) -> Result<Box<dyn clarity_core::agent::LlmProvider>, String> {
    use clarity_llm::LlmFactory;
    use clarity_llm::{
        AnthropicLlm, DeepSeekProvider, KimiLlm, OAuthLlm, OpenAiCompatibleLlm,
    };

    let provider_lower = cfg.provider.to_lowercase();
    match provider_lower.as_str() {
        "openai" => {
            let base = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".into());
            let model = cfg.model.clone().unwrap_or_else(|| "gpt-4o".into());
            Ok(Box::new(OpenAiCompatibleLlm::new(
                &cfg.api_key,
                base,
                model,
            )))
        }
        "kimi" | "moonshot" => {
            let base = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.moonshot.cn/v1".into());
            let model = cfg.model.clone().unwrap_or_else(|| "kimi-k2-07132k".into());
            Ok(Box::new(KimiLlm::new(&cfg.api_key, base, model)))
        }
        "kimi-code" | "kimi_code" => {
            let base = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.kimi.com/coding/v1".into());
            let model = cfg.model.clone().unwrap_or_else(|| "kimi-k2-07132k".into());
            Ok(Box::new(OAuthLlm::new(
                &cfg.api_key,
                base,
                model,
                clarity_llm::auth::OAuthTokenManager::new(),
            )))
        }
        "anthropic" | "claude" => {
            let base = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".into());
            let model = cfg
                .model
                .clone()
                .unwrap_or_else(|| "claude-3-5-sonnet-20241022".into());
            Ok(Box::new(AnthropicLlm::new(&cfg.api_key, base, model)))
        }
        "deepseek" => {
            let base = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.deepseek.com/v1".into());
            let model = cfg.model.clone().unwrap_or_else(|| "deepseek-chat".into());
            Ok(Box::new(DeepSeekProvider::new(&cfg.api_key, base, model)))
        }
        alias => {
            // Try ModelRegistry alias
            match LlmFactory::create(alias).await {
                Ok(p) => Ok(p),
                Err(e) => Err(format!("Unknown provider '{}': {}", cfg.provider, e)),
            }
        }
    }
}

pub(crate) async fn admin_get_config() -> impl IntoResponse {
    match load_persisted_config().await {
        Some(cfg) => {
            let resp = ConfigResponse {
                provider: cfg.provider.clone(),
                api_key_masked: mask_key(&cfg.api_key),
                base_url: cfg.base_url.clone(),
                model: cfg.model.clone(),
            };
            (
                StatusCode::OK,
                Json(ConfigStatusResponse {
                    configured: true,
                    config: Some(resp),
                }),
            )
        }
        None => (
            StatusCode::OK,
            Json(ConfigStatusResponse {
                configured: false,
                config: None,
            }),
        ),
    }
}

pub(crate) async fn admin_set_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetConfigRequest>,
) -> impl IntoResponse {
    if req.api_key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "api_key is required"})),
        );
    }
    if req.provider.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "provider is required"})),
        );
    }

    // Validate by trying to build the provider
    match build_provider_from_config(&req).await {
        Ok(provider) => {
            // Save to file
            if let Err(e) = save_persisted_config(&req).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to save config: {}", e)})),
                );
            }
            // Apply to agent
            state.agent.set_llm(Arc::from(provider));

            let resp = ConfigResponse {
                provider: req.provider.clone(),
                api_key_masked: mask_key(&req.api_key),
                base_url: req.base_url.clone(),
                model: req.model.clone(),
            };
            (
                StatusCode::OK,
                Json(json!({"status": "ok", "config": resp})),
            )
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
    }
}

// ==================== File System API ====================

#[derive(Debug, Deserialize)]
pub(crate) struct FileTreeParams {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FileReadParams {
    pub path: String,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FileWriteBody {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FileGlobParams {
    pub pattern: String,
}

fn sanitize_path(raw: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    let abs = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let canonical = abs
        .canonicalize()
        .map_err(|e| format!("Invalid path: {}", e))?;

    // Security: restrict to working directory
    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."));

    if !canonical.starts_with(&cwd) {
        return Err("Path is outside working directory".to_string());
    }

    Ok(canonical)
}

fn is_sensitive_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    [
        ".env",
        "id_rsa",
        "id_ed25519",
        ".ssh",
        ".p12",
        ".pfx",
        ".htpasswd",
        "secrets",
        "credentials",
        "token",
        "api_key",
        "private_key",
        "password",
        "passwd",
    ]
    .iter()
    .any(|s| path_str.contains(s))
}

fn build_tree<'a>(
    path: &'a Path,
    root: &'a Path,
    depth: usize,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'a>,
> {
    Box::pin(async move {
        if depth > 10 {
            return Ok(json!({"name": "...", "type": "directory", "path": "", "children": []}));
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        if path.is_file() {
            let meta = tokio::fs::metadata(path).await.map_err(|e| e.to_string())?;
            Ok(json!({
                "name": name,
                "type": "file",
                "path": rel,
                "size": meta.len(),
            }))
        } else if path.is_dir() {
            let mut children = Vec::new();
            let mut entries = tokio::fs::read_dir(path).await.map_err(|e| e.to_string())?;
            while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
                let child_path = entry.path();
                // Skip hidden files/directories
                if child_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with('.'))
                    .unwrap_or(false)
                {
                    continue;
                }
                if let Ok(child) = build_tree(&child_path, root, depth + 1).await {
                    children.push(child);
                }
            }
            children.sort_by(|a, b| {
                let a_type = a["type"].as_str().unwrap_or("");
                let b_type = b["type"].as_str().unwrap_or("");
                let a_name = a["name"].as_str().unwrap_or("");
                let b_name = b["name"].as_str().unwrap_or("");
                a_type.cmp(b_type).reverse().then(a_name.cmp(b_name))
            });
            Ok(json!({
                "name": name,
                "type": "directory",
                "path": rel,
                "children": children,
            }))
        } else {
            Err("Unknown file type".to_string())
        }
    })
}

pub(crate) async fn file_tree(
    Query(params): Query<FileTreeParams>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let root = match &params.path {
        Some(p) => match sanitize_path(p) {
            Ok(path) => path,
            Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
        },
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    if !root.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Not a directory"})),
        );
    }

    match build_tree(&root, &root, 0).await {
        Ok(tree) => {
            state.activity_logger.log_window(WindowActivity {
                timestamp: chrono::Utc::now().to_rfc3339(),
                activity_type: "file_tree".to_string(),
                topic: format!("浏览目录: {}", root.display()),
                tools_used: vec!["file_tree".to_string()],
                files_involved: vec![],
                conclusion: "".to_string(),
            });
            (StatusCode::OK, Json(json!({"tree": tree})))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))),
    }
}

pub(crate) async fn file_read(
    Query(params): Query<FileReadParams>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let path = match sanitize_path(&params.path) {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
    };

    if path.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Path is a directory"})),
        );
    }

    if is_sensitive_path(&path) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access to sensitive file denied"})),
        );
    }

    let mut args = json!({"path": path.display().to_string()});
    if let Some(offset) = params.offset {
        args["offset"] = json!(offset);
    }
    if let Some(limit) = params.limit {
        args["limit"] = json!(limit);
    }

    let ctx = clarity_core::tools::ToolContext::new();
    let registry = clarity_core::registry::ToolRegistry::with_builtin_tools();
    match registry.execute("file_read", args, ctx).await {
        Ok(result) => {
            state.activity_logger.log_window(WindowActivity {
                timestamp: chrono::Utc::now().to_rfc3339(),
                activity_type: "file_read".to_string(),
                topic: format!("读取文件: {}", params.path),
                tools_used: vec!["file_read".to_string()],
                files_involved: vec![params.path.clone()],
                conclusion: "".to_string(),
            });
            (StatusCode::OK, Json(result))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ),
    }
}

pub(crate) async fn file_write(
    State(state): State<Arc<AppState>>,
    Json(body): Json<FileWriteBody>,
) -> impl IntoResponse {
    let path = match sanitize_path(&body.path) {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
    };

    if is_sensitive_path(&path) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Writing to sensitive path denied"})),
        );
    }

    let args = json!({
        "path": path.display().to_string(),
        "content": body.content,
    });

    let ctx = clarity_core::tools::ToolContext::new();
    let registry = clarity_core::registry::ToolRegistry::with_builtin_tools();
    match registry.execute("file_write", args, ctx).await {
        Ok(result) => {
            state.activity_logger.log_window(WindowActivity {
                timestamp: chrono::Utc::now().to_rfc3339(),
                activity_type: "file_write".to_string(),
                topic: format!("写入文件: {}", body.path),
                tools_used: vec!["file_write".to_string()],
                files_involved: vec![body.path.clone()],
                conclusion: "".to_string(),
            });
            (StatusCode::OK, Json(result))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ),
    }
}

pub(crate) async fn file_glob(
    Query(params): Query<FileGlobParams>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let args = json!({"pattern": params.pattern});
    let ctx = clarity_core::tools::ToolContext::new();
    let registry = clarity_core::registry::ToolRegistry::with_builtin_tools();
    match registry.execute("glob", args, ctx).await {
        Ok(result) => {
            state.activity_logger.log_window(WindowActivity {
                timestamp: chrono::Utc::now().to_rfc3339(),
                activity_type: "file_glob".to_string(),
                topic: format!("搜索文件: {}", params.pattern),
                tools_used: vec!["glob".to_string()],
                files_involved: vec![],
                conclusion: "".to_string(),
            });
            (StatusCode::OK, Json(result))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ),
    }
}

// ==================== Admin: Session Management ====================

pub(crate) async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.session_store.list_sessions().await {
        Ok(sessions) => (StatusCode::OK, Json(json!({ "sessions": sessions }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub(crate) async fn get_session(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.session_store.load_session(&session_id).await {
        Ok(messages) => {
            let msgs: Vec<_> = messages
                .into_iter()
                .map(|m| {
                    json!({
                        "role": m.role,
                        "content": m.content,
                        "tool_calls": m.tool_calls,
                        "tool_call_id": m.tool_call_id,
                        "created_at": m.created_at.to_rfc3339(),
                    })
                })
                .collect();
            (
                StatusCode::OK,
                Json(json!({ "session_id": session_id, "messages": msgs })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub(crate) async fn delete_session(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.session_store.delete_session(&session_id).await {
        Ok(true) => (StatusCode::OK, Json(json!({ "deleted": true }))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Session not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{create_admin_router, create_api_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use clarity_core::agent::{Agent, AgentConfig, MockLlm};
    use clarity_core::background::BackgroundTaskManager;
    use clarity_core::registry::ToolRegistry;
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    async fn test_state() -> Arc<AppState> {
        let registry = ToolRegistry::with_builtin_tools();
        let config = AgentConfig::new()
            .with_max_iterations(5)
            .with_read_only(false);
        let agent = Arc::new(Agent::with_config(registry, config).with_llm(Arc::new(MockLlm)));

        let temp =
            std::env::temp_dir().join(format!("clarity-gateway-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp);
        let task_manager = Arc::new(BackgroundTaskManager::new(
            temp.join("store"),
            temp.join("work"),
            temp.join("context"),
        ));

        Arc::new(AppState::new(agent, task_manager).await)
    }

    // ==================== Security tests (preserved) ====================

    #[test]
    fn test_sanitize_path_rejects_parent_traversal() {
        let result = sanitize_path("../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_sanitize_path_rejects_deep_traversal() {
        let result = sanitize_path("src/../../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_sanitize_path_allows_relative() {
        let result = sanitize_path("src/main.rs");
        assert!(result.is_ok());
    }

    // ==================== Handler integration tests ====================

    #[tokio::test]
    async fn test_health_check() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "healthy");
        assert!(json.get("version").is_some());
        assert!(json.get("timestamp").is_some());
    }

    #[tokio::test]
    async fn test_file_tree() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/files/tree")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("tree").is_some());
    }

    #[tokio::test]
    async fn test_file_read() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/files/read?path=Cargo.toml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // file_read uses the built-in tool; it may succeed or error depending
        // on the working directory, but it should not panic / hang.
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json.get("content").is_some() || json.get("error").is_some(),
            "expected content or error key, got: {}",
            json
        );
    }

    #[tokio::test]
    async fn test_admin_tools() {
        let state = test_state().await;
        let app = create_admin_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("tools").is_some());
        assert!(json["tools"].is_array());
    }

    #[tokio::test]
    async fn test_admin_approval_mode_get_and_set() {
        let state = test_state().await;
        let app = create_admin_router(state);

        // 1. Get initial mode
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/approval-mode")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["mode"].is_string());

        // 2. Set to yolo
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/approval-mode")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"mode":"yolo"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["mode"], "yolo");

        // 3. Verify persisted
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/approval-mode")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["mode"], "yolo");
    }

    #[tokio::test]
    async fn test_admin_approval_mode_rejects_invalid() {
        let state = test_state().await;
        let app = create_admin_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/approval-mode")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"mode":"invalid"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

// ==================== MCP 服务器管理 API ====================

use clarity_core::memory::{MemoryStore, PersistentMemoryStore};

/// Overview of a single MCP server entry (from `mcp.json`).
#[derive(Serialize)]
pub(crate) struct McpServerOverview {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub disabled: bool,
    pub transport: Option<String>,
    pub url: Option<String>,
}

/// Response for listing MCP servers.
#[derive(Serialize)]
pub(crate) struct McpServersResponse {
    pub servers: Vec<McpServerOverview>,
    pub config_path: String,
}

pub(crate) async fn list_mcp_servers() -> Response {
    let mcp_path = clarity_core::mcp::config::default_config_path().ok();
    match clarity_core::mcp::config::McpConfig::load_default() {
        Ok(config) => {
            let servers: Vec<McpServerOverview> = config
                .servers
                .into_iter()
                .map(|(name, entry)| McpServerOverview {
                    name,
                    command: entry.command,
                    args: entry.args,
                    disabled: entry.disabled,
                    transport: entry.transport,
                    url: entry.url,
                })
                .collect();
            let config_path = mcp_path
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            (
                StatusCode::OK,
                Json(McpServersResponse {
                    servers,
                    config_path,
                }),
            )
                .into_response()
        }
        Err(e) => {
            warn!("Failed to load MCP config: {}", e);
            (
                StatusCode::OK,
                Json(McpServersResponse {
                    servers: Vec::new(),
                    config_path: String::new(),
                }),
            )
                .into_response()
        }
    }
}

/// Get a single MCP server by name.
pub(crate) async fn get_mcp_server(
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Response {
    match clarity_core::mcp::config::McpConfig::load_default() {
        Ok(config) => match config.servers.get(&name) {
            Some(entry) => (
                StatusCode::OK,
                Json(McpServerOverview {
                    name: name.clone(),
                    command: entry.command.clone(),
                    args: entry.args.clone(),
                    disabled: entry.disabled,
                    transport: entry.transport.clone(),
                    url: entry.url.clone(),
                }),
            )
                .into_response(),
            None => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "MCP server not found"})),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Failed to load MCP config: {}", e)})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct McpServerUpdate {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub disabled: Option<bool>,
    pub transport: Option<String>,
    pub url: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
}

/// Create or update an MCP server.
pub(crate) async fn update_mcp_server(
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(req): Json<McpServerUpdate>,
) -> Response {
    let default_path = clarity_core::mcp::config::default_config_path();
    let path = match default_path {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    // Load existing config or create new
    let mut config = clarity_core::mcp::config::McpConfig::load(&path).unwrap_or_default();

    let entry = config.servers.entry(name.clone()).or_insert_with(|| {
        clarity_core::mcp::config::McpServerEntry {
            command: req.command.clone().unwrap_or_default(),
            ..Default::default()
        }
    });

    if let Some(cmd) = req.command {
        entry.command = cmd;
    }
    if let Some(a) = req.args {
        entry.args = a;
    }
    if let Some(d) = req.disabled {
        entry.disabled = d;
    }
    if let Some(t) = req.transport {
        entry.transport = Some(t);
    }
    if let Some(u) = req.url {
        entry.url = Some(u);
    }
    if let Some(h) = req.headers {
        entry.headers = h;
    }
    if let Some(e) = req.env {
        entry.env = e;
    }

    match config.save(&path) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"saved": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Delete an MCP server.
pub(crate) async fn delete_mcp_server(
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Response {
    let default_path = clarity_core::mcp::config::default_config_path();
    let path = match default_path {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let mut config = clarity_core::mcp::config::McpConfig::load(&path).unwrap_or_default();
    config.servers.remove(&name);

    match config.save(&path) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"deleted": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ==================== Cron 任务管理 API ====================

#[derive(Serialize)]
pub(crate) struct CronTaskOverview {
    pub task_id: String,
    pub name: String,
    pub cron_expr: String,
    pub enabled: bool,
    pub next_run: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct CronTasksResponse {
    pub tasks: Vec<CronTaskOverview>,
}

#[derive(Deserialize)]
pub(crate) struct CreateCronRequest {
    pub name: String,
    pub prompt: String,
    pub cron_expr: String,
    pub agent_type: Option<String>,
    pub max_iterations: Option<usize>,
}

#[derive(Serialize)]
pub(crate) struct CreateCronResponse {
    pub task_id: String,
}

pub(crate) async fn list_cron_tasks(State(state): State<Arc<AppState>>) -> Json<CronTasksResponse> {
    let tasks = state
        .task_manager
        .list_cron_tasks()
        .await
        .unwrap_or_default();
    let overviews: Vec<CronTaskOverview> = tasks
        .into_iter()
        .map(|t| CronTaskOverview {
            task_id: t.task_id.clone(),
            name: t.task_spec.name.clone(),
            cron_expr: t.schedule.expr.clone(),
            enabled: t.enabled,
            next_run: None, // computed on next scheduler tick
        })
        .collect();
    Json(CronTasksResponse { tasks: overviews })
}

pub(crate) async fn create_cron_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCronRequest>,
) -> Response {
    let spec = clarity_core::background::TaskSpec::new(req.name.clone(), req.prompt)
        .with_agent_type(req.agent_type.unwrap_or_else(|| "default".into()))
        .with_max_iterations(req.max_iterations.unwrap_or(10));

    match state.task_manager.schedule_cron(spec, &req.cron_expr).await {
        Ok(task_id) => {
            info!("Created cron task: {} ({})", req.name, task_id);
            (StatusCode::CREATED, Json(CreateCronResponse { task_id })).into_response()
        }
        Err(e) => {
            error!("Failed to create cron task: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

pub(crate) async fn delete_cron_task(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(task_id): axum::extract::Path<String>,
) -> Response {
    match state.task_manager.cancel_cron(&task_id).await {
        Ok(()) => {
            info!("Deleted cron task: {}", task_id);
            (StatusCode::OK, Json(serde_json::json!({"deleted": true}))).into_response()
        }
        Err(e) => {
            error!("Failed to delete cron task {}: {}", task_id, e);
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

// ==================== 跨会话全文检索 API ====================

#[derive(Deserialize)]
pub(crate) struct SearchRequest {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_limit() -> usize {
    20
}

#[derive(Serialize)]
pub(crate) struct SearchResult {
    pub fact_id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub score: f32,
}

#[derive(Serialize)]
pub(crate) struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: usize,
}

pub(crate) async fn search_memory(Json(req): Json<SearchRequest>) -> Response {
    // Try to open the persistent memory store from the default location
    let clarity_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".clarity");
    let memory_db = clarity_dir.join("memory.db");

    if !memory_db.exists() {
        return (
            StatusCode::OK,
            Json(SearchResponse {
                results: Vec::new(),
                total: 0,
            }),
        )
            .into_response();
    }

    match PersistentMemoryStore::new(memory_db.as_path()).await {
        Ok(memory) => {
            let memories = memory.search(&req.query, req.limit).await;
            match memories {
                Ok(memories) => {
                    let results: Vec<SearchResult> = memories
                        .into_iter()
                        .map(|m| SearchResult {
                            fact_id: m.id,
                            content: m.content,
                            tags: m.tags,
                            score: m.importance,
                        })
                        .collect();
                    let total = results.len();
                    (StatusCode::OK, Json(SearchResponse { results, total })).into_response()
                }
                Err(e) => {
                    error!("Memory search failed: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": e.to_string()})),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            error!("Failed to open memory store: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}
