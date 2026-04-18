use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, Sse},
        IntoResponse, Response,
    },
};
use std::path::{Path, PathBuf};
use clarity_core::activity::WindowActivity;
use clarity_core::agent::{AgentController, ControllerEvent, Message as AgentMessage, MessageRole, Op};
use clarity_core::llm::LlmFactory;
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
    let agent = state.agent.read().await.clone();

    // Build the internal message list:
    // 1. Agent system prompt
    // 2. All non-system messages from the request
    let mut messages: Vec<AgentMessage> = vec![AgentMessage::system(agent.build_system_prompt())];
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
    let (controller, op_tx) = AgentController::new_with_events(agent, event_tx);
    tokio::spawn(controller.run());

    let op = if req.stream {
        Op::ConversationTurn(messages)
    } else {
        Op::ConversationTurnSync(messages)
    };
    if let Err(e) = op_tx.send(op) {
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

        let sse_stream = stream::unfold(
            (event_rx, 0u8),
            move |(mut rx, step)| {
                let model = model.clone();
                let id = id.clone();
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
                        Some(ControllerEvent::ToolCallStart { id: tc_id, name, arguments }) => {
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
                        Some(ControllerEvent::Complete(_))
                        | Some(ControllerEvent::Error(_))
                        | None => {
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
            },
        );

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
pub struct ModelsResponse {
    pub models: Vec<ModelInfo>,
}

#[derive(Serialize)]
pub struct ModelInfo {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub protocol: String,
}

pub async fn admin_models() -> impl IntoResponse {
    let registry = match clarity_core::llm::ModelRegistry::load() {
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

use clarity_core::background::{TaskResult, TaskSpec, TaskStatus};
use clarity_core::background::TaskId;

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub name: String,
    pub prompt: String,
    #[serde(default)]
    pub max_iterations: Option<usize>,
}

#[derive(Serialize)]
pub struct TaskCreateResponse {
    pub task_id: TaskId,
    pub status: TaskStatus,
}

#[derive(Serialize)]
pub struct TaskDetailResponse {
    pub task_id: TaskId,
    pub name: String,
    pub status: TaskStatus,
    pub prompt: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub result: Option<TaskResult>,
}

pub async fn create_task(
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

pub async fn get_task(
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

pub async fn cancel_task(
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

// ==================== Admin: Switch Provider ====================

#[derive(Deserialize)]
pub struct SwitchProviderRequest {
    pub provider: String,
}

#[derive(Serialize)]
pub struct SwitchProviderResponse {
    pub provider: String,
    pub message: String,
}

pub async fn admin_switch_provider(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SwitchProviderRequest>,
) -> impl IntoResponse {
    info!("Admin: switching provider to '{}'", req.provider);

    match LlmFactory::create(&req.provider).await {
        Ok(new_llm) => {
            state.agent.read().await.set_llm(Arc::from(new_llm));
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
pub struct ConfigResponse {
    pub provider: String,
    pub api_key_masked: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConfigStatusResponse {
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
pub fn load_persisted_config() -> Option<SetConfigRequest> {
    let path = config_file_path();
    if !path.exists() {
        return None;
    }
    let contents = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Save user config to JSON file
fn save_persisted_config(cfg: &SetConfigRequest) -> Result<(), String> {
    let path = config_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// Build an LLM provider from a user config request
pub async fn build_provider_from_config(cfg: &SetConfigRequest) -> Result<Box<dyn clarity_core::agent::LlmProvider>, String> {
    use clarity_core::llm::{OpenAiCompatibleLlm, KimiLlm, KimiCodeLlm, DeepSeekProvider, AnthropicLlm};
    use clarity_core::llm::LlmFactory;

    let provider_lower = cfg.provider.to_lowercase();
    match provider_lower.as_str() {
        "openai" => {
            let base = cfg.base_url.clone().unwrap_or_else(|| "https://api.openai.com/v1".into());
            let model = cfg.model.clone().unwrap_or_else(|| "gpt-4o".into());
            Ok(Box::new(OpenAiCompatibleLlm::new(&cfg.api_key, base, model)))
        }
        "kimi" | "moonshot" => {
            let base = cfg.base_url.clone().unwrap_or_else(|| "https://api.moonshot.cn/v1".into());
            let model = cfg.model.clone().unwrap_or_else(|| "kimi-k2-07132k".into());
            Ok(Box::new(KimiLlm::new(&cfg.api_key, base, model)))
        }
        "kimi-code" => {
            let base = cfg.base_url.clone().unwrap_or_else(|| "https://api.kimi.com/coding/v1".into());
            let model = cfg.model.clone().unwrap_or_else(|| "kimi-k2-07132k".into());
            Ok(Box::new(KimiCodeLlm::new(&cfg.api_key, base, model)))
        }
        "anthropic" | "claude" => {
            let base = cfg.base_url.clone().unwrap_or_else(|| "https://api.anthropic.com".into());
            let model = cfg.model.clone().unwrap_or_else(|| "claude-3-5-sonnet-20241022".into());
            Ok(Box::new(AnthropicLlm::new(&cfg.api_key, base, model)))
        }
        "deepseek" => {
            let base = cfg.base_url.clone().unwrap_or_else(|| "https://api.deepseek.com/v1".into());
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

pub async fn admin_get_config() -> impl IntoResponse {
    match load_persisted_config() {
        Some(cfg) => {
            let resp = ConfigResponse {
                provider: cfg.provider.clone(),
                api_key_masked: mask_key(&cfg.api_key),
                base_url: cfg.base_url.clone(),
                model: cfg.model.clone(),
            };
            (StatusCode::OK, Json(ConfigStatusResponse { configured: true, config: Some(resp) }))
        }
        None => {
            (StatusCode::OK, Json(ConfigStatusResponse { configured: false, config: None }))
        }
    }
}

pub async fn admin_set_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetConfigRequest>,
) -> impl IntoResponse {
    if req.api_key.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "api_key is required"})));
    }
    if req.provider.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "provider is required"})));
    }

    // Validate by trying to build the provider
    match build_provider_from_config(&req).await {
        Ok(provider) => {
            // Save to file
            if let Err(e) = save_persisted_config(&req) {
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to save config: {}", e)})));
            }
            // Apply to agent
            state.agent.read().await.set_llm(Arc::from(provider));

            let resp = ConfigResponse {
                provider: req.provider.clone(),
                api_key_masked: mask_key(&req.api_key),
                base_url: req.base_url.clone(),
                model: req.model.clone(),
            };
            (StatusCode::OK, Json(json!({"status": "ok", "config": resp})))
        }
        Err(e) => {
            (StatusCode::BAD_REQUEST, Json(json!({"error": e})))
        }
    }
}

// ==================== File System API ====================

#[derive(Debug, Deserialize)]
pub struct FileTreeParams {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FileReadParams {
    pub path: String,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct FileWriteBody {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct FileGlobParams {
    pub pattern: String,
}

fn sanitize_path(raw: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    let abs = if path.is_absolute() {
        path
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
    };
    let canonical = abs.canonicalize().map_err(|e| format!("Invalid path: {}", e))?;
    Ok(canonical)
}

fn is_sensitive_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    [
        ".env", "id_rsa", "id_ed25519", ".ssh", ".p12", ".pfx",
        ".htpasswd", "secrets", "credentials", "token", "api_key",
        "private_key", "password", "passwd",
    ]
    .iter()
    .any(|s| path_str.contains(s))
}

fn build_tree<'a>(
    path: &'a Path,
    root: &'a Path,
    depth: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'a>> {
    Box::pin(async move {
        if depth > 10 {
            return Ok(json!({"name": "...", "type": "directory", "path": "", "children": []}));
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy().to_string();

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
                if child_path.file_name()
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

pub async fn file_tree(
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
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Not a directory"})));
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

pub async fn file_read(
    Query(params): Query<FileReadParams>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let path = match sanitize_path(&params.path) {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
    };

    if path.is_dir() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Path is a directory"})));
    }

    if is_sensitive_path(&path) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Access to sensitive file denied"})));
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
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

pub async fn file_write(
    State(state): State<Arc<AppState>>,
    Json(body): Json<FileWriteBody>,
) -> impl IntoResponse {
    let path = match sanitize_path(&body.path) {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
    };

    if is_sensitive_path(&path) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Writing to sensitive path denied"})));
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
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

pub async fn file_glob(
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
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}


