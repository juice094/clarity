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
use clarity_contract::{SessionSource, ThreadId};
use clarity_core::background::{TaskSpec, TaskStatus};

/// WebSocket 升级处理器
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Enforce a hard ceiling on concurrent WebSocket sessions so a runaway
    // client cannot spawn an unbounded number of agent runs.
    match state.ws_sem.clone().try_acquire_owned() {
        Ok(permit) => ws.on_upgrade(move |socket| handle_socket(socket, state, permit)),
        Err(_) => {
            warn!("WebSocket connection rejected: /ws concurrency limit reached");
            axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

/// 处理 WebSocket 连接
async fn handle_socket(
    socket: WebSocket,
    state: Arc<AppState>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
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

    // ponytail: transient WebSocket sessions are deleted on disconnect to avoid
    // accumulation. If persistent sessions are needed later, add an explicit
    // lifecycle (e.g. keep-alive TTL) instead of keeping every session forever.
    if let Err(e) = state
        .session_store
        .delete_session(&session_id.to_string())
        .await
    {
        warn!(
            "Failed to delete WebSocket session {} on disconnect: {}",
            session_id, e
        );
    }
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
            // Wrap every streaming WireMessage in the unified WsResponse envelope
            // so the WebSocket always emits a single schema.
            let payload = match serde_json::to_value(&msg) {
                Ok(value) => value,
                Err(e) => {
                    error!("Failed to serialize wire message: {}", e);
                    continue;
                }
            };
            let envelope = WsResponse::WireMessage { payload };
            match serde_json::to_string(&envelope) {
                Ok(json) => {
                    if merge_tx_wire.send(json).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize wire envelope: {}", e);
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
#[derive(Debug, Deserialize, Serialize)]
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
    /// Request missing role-context events for a Claw role.
    SyncRoleContext {
        /// Role context id.
        role_id: String,
        /// Last event id known to the client.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        since_event_id: Option<String>,
        /// Device id of the requesting client.
        device_id: String,
    },
    /// Register a Claw device.
    RegisterDevice {
        /// Stable machine identifier.
        id: String,
        /// Human-readable display name.
        name: String,
        /// IP address or hostname.
        host: String,
        /// Claw daemon version string.
        version: String,
        /// Optional status override.
        #[serde(default)]
        status: Option<String>,
    },
    /// Send a heartbeat for a registered Claw device.
    Heartbeat {
        /// Stable machine identifier.
        id: String,
        /// Human-readable display name.
        name: String,
        /// IP address or hostname.
        host: String,
        /// Claw daemon version string.
        version: String,
        /// Optional status override.
        #[serde(default)]
        status: Option<String>,
    },
    /// Create a background task.
    CreateTask {
        /// Task name.
        name: String,
        /// Task prompt.
        prompt: String,
        /// Maximum agent iterations.
        #[serde(default)]
        max_iterations: Option<usize>,
    },
    /// Cancel a background task.
    CancelTask {
        /// Task identifier.
        task_id: String,
    },
    /// List background tasks.
    ListTasks,
    /// Get a single background task.
    GetTask {
        /// Task identifier.
        task_id: String,
    },
    /// Create a new thread.
    CreateThread {
        /// Optional human-readable title.
        #[serde(default)]
        title: Option<String>,
    },
    /// List threads.
    ListThreads {
        /// Maximum number of threads to return.
        #[serde(default)]
        limit: Option<usize>,
        /// Whether to include archived threads.
        #[serde(default)]
        include_archived: Option<bool>,
    },
    /// Get a single thread.
    GetThread {
        /// Thread identifier.
        thread_id: String,
        /// Include full rollout history.
        #[serde(default)]
        include_history: Option<bool>,
    },
}

/// WebSocket 响应类型.
#[derive(Debug, Serialize, Deserialize)]
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
    /// A wrapped `clarity_wire::WireMessage` streamed during a wire chat.
    WireMessage {
        /// The original WireMessage payload.
        payload: serde_json::Value,
    },
    /// Role-context sync response.
    RoleContextSynced {
        /// Role that was synchronized.
        role_id: String,
        /// Events missing on the client.
        events: Vec<clarity_contract::ClawContextEvent>,
        /// Cursor for the next sync request.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_cursor: Option<String>,
        /// Devices currently online for this role.
        online_devices: Vec<String>,
    },
    /// Device registration / heartbeat acknowledgement.
    DeviceAck {
        /// Registered device record.
        device: crate::handlers::claw::ClawDevice,
    },
    /// Background task creation response.
    TaskCreated {
        /// New task identifier.
        task_id: String,
        /// Initial task status.
        status: String,
    },
    /// Background task cancellation response.
    TaskCancelled {
        /// Whether the cancellation succeeded.
        cancelled: bool,
    },
    /// Background task list response.
    TaskList {
        /// Tasks returned by the Gateway.
        tasks: Vec<crate::handlers::tasks::TaskDetailResponse>,
    },
    /// Single background task response.
    TaskDetail {
        /// Task detail payload.
        task: crate::handlers::tasks::TaskDetailResponse,
    },
    /// Thread creation response.
    ThreadCreated {
        /// New thread identifier.
        thread_id: String,
    },
    /// Thread list response.
    ThreadList {
        /// Thread summaries for this page.
        data: Vec<crate::handlers::threads::ThreadListItem>,
        /// Cursor for the next page, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_cursor: Option<String>,
    },
    /// Single thread response.
    ThreadDetail {
        /// Thread detail payload.
        thread: crate::handlers::threads::ThreadResponse,
    },
}

/// Tool call representation in a WebSocket response.
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCall {
    /// Name of the tool/function.
    pub name: String,
    /// Arguments passed to the tool.
    pub arguments: serde_json::Value,
}

/// A single chat message returned in the WebSocket history response.
#[derive(Debug, Serialize, Deserialize)]
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
        WsRequest::SyncRoleContext {
            role_id,
            since_event_id,
            device_id,
        } => {
            if !device_id.is_empty()
                && let Err(e) = state
                    .role_context_store
                    .record_device_presence(&role_id, &device_id)
                    .await
            {
                tracing::warn!(error = %e, "failed to record device presence");
            }

            let events = state
                .role_context_store
                .list_events(&role_id, since_event_id.as_deref())
                .await
                .unwrap_or_default();
            let online_devices = state
                .role_context_store
                .online_devices(&role_id)
                .await
                .unwrap_or_default();

            WsResponse::RoleContextSynced {
                role_id,
                events,
                next_cursor: None,
                online_devices,
            }
        }
        WsRequest::RegisterDevice {
            id,
            name,
            host,
            version,
            status,
        }
        | WsRequest::Heartbeat {
            id,
            name,
            host,
            version,
            status,
        } => {
            if id.is_empty() || name.is_empty() {
                return WsResponse::Error {
                    error: "device id and name are required".into(),
                };
            }
            let device =
                state
                    .device_registry
                    .register(crate::handlers::claw::DeviceRegistration {
                        id,
                        name,
                        host,
                        version,
                        status,
                    });
            WsResponse::DeviceAck { device }
        }
        WsRequest::CreateTask {
            name,
            prompt,
            max_iterations,
        } => {
            let spec = TaskSpec::new(name.clone(), prompt)
                .with_agent_type("default")
                .with_max_iterations(max_iterations.unwrap_or(10));
            match state.task_manager.spawn_agent(spec).await {
                Ok(task_id) => WsResponse::TaskCreated {
                    task_id,
                    status: format!("{:?}", TaskStatus::Pending),
                },
                Err(e) => WsResponse::Error {
                    error: format!("failed to create task: {}", e),
                },
            }
        }
        WsRequest::CancelTask { task_id } => match state.task_manager.cancel(&task_id).await {
            Ok(()) => WsResponse::TaskCancelled { cancelled: true },
            Err(e) => WsResponse::Error {
                error: format!("failed to cancel task: {}", e),
            },
        },
        WsRequest::ListTasks => match state.task_manager.list().await {
            Ok(tasks) => {
                let tasks = tasks
                    .into_iter()
                    .map(|info| crate::handlers::tasks::TaskDetailResponse {
                        task_id: info.id,
                        name: info.spec.name,
                        status: info.status,
                        prompt: info.spec.prompt,
                        created_at: info.created_at,
                        updated_at: info.updated_at,
                        result: None,
                    })
                    .collect();
                WsResponse::TaskList { tasks }
            }
            Err(e) => WsResponse::Error {
                error: format!("failed to list tasks: {}", e),
            },
        },
        WsRequest::GetTask { task_id } => {
            let store = state.task_manager.store();
            match store.get(&task_id).await {
                Ok(info) => {
                    let result = if info.status.is_terminal() {
                        store.get_result(&task_id).await.ok()
                    } else {
                        None
                    };
                    WsResponse::TaskDetail {
                        task: crate::handlers::tasks::TaskDetailResponse {
                            task_id: info.id,
                            name: info.spec.name,
                            status: info.status,
                            prompt: info.spec.prompt,
                            created_at: info.created_at,
                            updated_at: info.updated_at,
                            result,
                        },
                    }
                }
                Err(e) => WsResponse::Error {
                    error: format!("failed to get task: {}", e),
                },
            }
        }
        WsRequest::CreateThread { title } => {
            let cwd = state.agent.config().working_dir.clone();
            match state
                .thread_manager
                .create_thread(&cwd, "clarity-gateway", SessionSource::AppServer)
                .await
            {
                Ok(thread_id) => {
                    if let Some(title) = title {
                        let _ = state
                            .thread_manager
                            .update_metadata(
                                thread_id,
                                clarity_thread_store::ThreadMetadataPatch {
                                    title: Some(title),
                                    archived: None,
                                    extra: std::collections::HashMap::new(),
                                },
                            )
                            .await;
                    }
                    WsResponse::ThreadCreated {
                        thread_id: thread_id.to_string(),
                    }
                }
                Err(e) => WsResponse::Error {
                    error: format!("failed to create thread: {}", e),
                },
            }
        }
        WsRequest::ListThreads {
            limit,
            include_archived,
        } => match state
            .thread_manager
            .list_threads(limit.unwrap_or(10), include_archived.unwrap_or(false), None)
            .await
        {
            Ok(summaries) => {
                let data: Vec<_> = summaries
                    .iter()
                    .map(crate::handlers::threads::summary_to_item)
                    .collect();
                WsResponse::ThreadList {
                    data,
                    next_cursor: None,
                }
            }
            Err(e) => WsResponse::Error {
                error: format!("failed to list threads: {}", e),
            },
        },
        WsRequest::GetThread {
            thread_id,
            include_history,
        } => match ThreadId::from_string(&thread_id) {
            Ok(id) => match state
                .thread_manager
                .read_thread(id, include_history.unwrap_or(false))
                .await
            {
                Ok(stored) => WsResponse::ThreadDetail {
                    thread: crate::handlers::threads::stored_to_response(&stored),
                },
                Err(clarity_core::thread::ThreadManagerError::ThreadStore(
                    clarity_thread_store::ThreadStoreError::NotFound { .. },
                )) => WsResponse::Error {
                    error: format!("thread not found: {}", thread_id),
                },
                Err(e) => WsResponse::Error {
                    error: format!("failed to get thread: {}", e),
                },
            },
            Err(e) => WsResponse::Error {
                error: format!("invalid thread id: {}", e),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use clarity_core::agent::{Agent, AgentConfig, MockLlm};
    use clarity_core::background::BackgroundTaskManager;
    use clarity_core::background::agent_executor::DefaultAgentTaskExecutor;
    use clarity_core::registry::ToolRegistry;
    use futures::stream::StreamExt;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    use tower::util::ServiceExt;

    async fn test_state() -> Arc<crate::server::AppState> {
        let registry = ToolRegistry::with_builtin_tools();
        let config = AgentConfig::new()
            .with_max_iterations(2)
            .with_read_only(false);
        let agent = Arc::new(Agent::with_config(registry, config).with_llm(Arc::new(MockLlm)));

        let temp = std::env::temp_dir().join(format!(
            "clarity-ws-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let _ = std::fs::create_dir_all(&temp);

        let llm = agent.llm().unwrap_or_else(|| Arc::new(MockLlm));
        let executor = Arc::new(DefaultAgentTaskExecutor::new(
            llm,
            agent.registry().clone(),
            temp.clone(),
        ));
        let task_manager = Arc::new(
            BackgroundTaskManager::new(temp.join("store"), temp.join("work"), temp.join("context"))
                .with_agent_executor(executor),
        );

        Arc::new(
            crate::server::AppState::new_with_home(agent, task_manager, temp.join(".clarity"))
                .await
                .unwrap(),
        )
    }

    #[test]
    fn test_ws_request_deserialization_chat() {
        let json = r#"{"type":"chat","message":"hello","context":{"key":"value"},"use_wire":true}"#;
        let req: WsRequest = serde_json::from_str(json).unwrap();
        match req {
            WsRequest::Chat {
                message,
                context,
                use_wire,
            } => {
                assert_eq!(message, "hello");
                assert_eq!(context.unwrap()["key"], "value");
                assert!(use_wire);
            }
            _ => panic!("expected Chat variant"),
        }
    }

    #[test]
    fn test_ws_request_deserialization_ping() {
        let json = r#"{"type":"ping"}"#;
        let req: WsRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req, WsRequest::Ping));
    }

    #[test]
    fn test_ws_request_deserialization_get_history() {
        let json = r#"{"type":"get_history"}"#;
        let req: WsRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req, WsRequest::GetHistory));
    }

    #[test]
    fn test_ws_response_serialization_welcome() {
        let resp = WsResponse::Welcome {
            session_id: "sid".to_string(),
            message: "Connected".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "welcome");
        assert_eq!(json["session_id"], "sid");
        assert_eq!(json["message"], "Connected");
    }

    #[test]
    fn test_ws_response_serialization_chat() {
        let resp = WsResponse::Chat {
            message: "hello".to_string(),
            tool_calls: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "chat");
        assert_eq!(json["message"], "hello");
        assert!(json.get("tool_calls").is_none());
    }

    #[test]
    fn test_ws_response_serialization_chat_with_tool_calls() {
        let resp = WsResponse::Chat {
            message: "ok".to_string(),
            tool_calls: Some(vec![ToolCall {
                name: "read".to_string(),
                arguments: serde_json::json!({"path": "/tmp"}),
            }]),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "chat");
        assert!(json["tool_calls"].is_array());
        assert_eq!(json["tool_calls"][0]["name"], "read");
        assert_eq!(json["tool_calls"][0]["arguments"]["path"], "/tmp");
    }

    #[test]
    fn test_ws_response_serialization_error() {
        let resp = WsResponse::Error {
            error: "bad request".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "error");
        assert_eq!(json["error"], "bad request");
    }

    #[test]
    fn test_ws_response_serialization_pong() {
        let resp = WsResponse::Pong;
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "pong");
    }

    #[test]
    fn test_ws_response_serialization_history() {
        let resp = WsResponse::History {
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            }],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "history");
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][0]["content"], "hi");
        assert_eq!(json["messages"][0]["timestamp"], "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_ws_response_serialization_wire_message() {
        let payload = serde_json::json!({
            "type": "content_part",
            "turn_id": "turn-1",
            "text": "hello"
        });
        let resp = WsResponse::WireMessage { payload };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "wire_message");
        assert_eq!(json["payload"]["type"], "content_part");
        assert_eq!(json["payload"]["turn_id"], "turn-1");
        assert_eq!(json["payload"]["text"], "hello");
    }

    #[test]
    fn test_ws_response_deserialization_wire_message() {
        let json = r#"{"type":"wire_message","payload":{"type":"turn_begin","turn_id":"t1","user_input":"hi"}}"#;
        let resp: WsResponse = serde_json::from_str(json).unwrap();
        match resp {
            WsResponse::WireMessage { payload } => {
                assert_eq!(payload["type"], "turn_begin");
                assert_eq!(payload["user_input"], "hi");
            }
            _ => panic!("expected WireMessage variant"),
        }
    }

    #[tokio::test]
    async fn test_ws_upgrade_and_welcome() {
        let state = test_state().await;
        let app = crate::server::create_api_router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let url = format!("ws://127.0.0.1:{}/ws", port);
        let (mut ws_stream, response) = tokio_tungstenite::connect_async(&url).await.unwrap();

        assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);

        let msg = ws_stream.next().await.unwrap().unwrap();
        let welcome: WsResponse = match msg {
            tokio_tungstenite::tungstenite::Message::Text(text) => {
                serde_json::from_str(&text).unwrap()
            }
            other => panic!("expected text welcome message, got {:?}", other),
        };
        match welcome {
            WsResponse::Welcome {
                session_id,
                message,
            } => {
                assert!(!session_id.is_empty());
                assert!(message.contains("Clarity Gateway"));
            }
            _ => panic!("expected Welcome variant"),
        }

        let ping = serde_json::to_string(&WsRequest::Ping).unwrap();
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(ping))
            .await
            .unwrap();

        let msg = ws_stream.next().await.unwrap().unwrap();
        let pong: WsResponse = match msg {
            tokio_tungstenite::tungstenite::Message::Text(text) => {
                serde_json::from_str(&text).unwrap()
            }
            other => panic!("expected text pong message, got {:?}", other),
        };
        assert!(matches!(pong, WsResponse::Pong));

        let _ = ws_stream.close(None).await;
        server.abort();
    }

    #[tokio::test]
    async fn test_handle_request_ping() {
        let state = test_state().await;
        let session_id = SessionId::new();
        let response = handle_request(&state, &session_id, WsRequest::Ping).await;
        assert!(matches!(response, WsResponse::Pong));
    }

    #[tokio::test]
    async fn test_handle_request_get_history() {
        let state = test_state().await;
        let session_id = SessionId::new();

        let msg = SessionMessage::new("user", "hello history");
        state
            .session_store
            .append_message(&session_id.to_string(), &msg)
            .await
            .unwrap();

        let response = handle_request(&state, &session_id, WsRequest::GetHistory).await;
        match response {
            WsResponse::History { messages } => {
                assert_eq!(messages.len(), 1);
                assert_eq!(messages[0].role, "user");
                assert_eq!(messages[0].content, "hello history");
            }
            _ => panic!("expected History variant"),
        }
    }

    #[tokio::test]
    async fn test_handle_request_sync_role_context() {
        use clarity_contract::{ClawContextEvent, ContextEventKind};

        let state = test_state().await;
        let session_id = SessionId::new();

        let event = ClawContextEvent {
            event_id: "ev-1".into(),
            origin_device: "dev-a".into(),
            origin_clock: 1,
            kind: ContextEventKind::AppendMessage {
                role: "user".into(),
                content: "hello".into(),
            },
        };
        state
            .role_context_store
            .append_event("operator", &event)
            .await
            .unwrap();

        let response = handle_request(
            &state,
            &session_id,
            WsRequest::SyncRoleContext {
                role_id: "operator".into(),
                since_event_id: None,
                device_id: "dev-b".into(),
            },
        )
        .await;

        match response {
            WsResponse::RoleContextSynced {
                role_id,
                events,
                online_devices,
                ..
            } => {
                assert_eq!(role_id, "operator");
                assert_eq!(events.len(), 1);
                assert_eq!(events[0].event_id, "ev-1");
                assert!(online_devices.contains(&"dev-b".into()));
            }
            _ => panic!("expected RoleContextSynced variant"),
        }
    }

    #[test]
    fn test_ws_request_deserialization_register_device() {
        let json = r#"{"type":"register_device","id":"claw-1","name":"Claw One","host":"host","version":"0.3.0"}"#;
        let req: WsRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(
            req,
            WsRequest::RegisterDevice {
                id,
                name,
                version,
                ..
            } if id == "claw-1" && name == "Claw One" && version == "0.3.0"
        ));
    }

    #[test]
    fn test_ws_request_deserialization_create_task() {
        let json = r#"{"type":"create_task","name":"demo","prompt":"hello","max_iterations":5}"#;
        let req: WsRequest = serde_json::from_str(json).unwrap();
        match req {
            WsRequest::CreateTask {
                name,
                prompt,
                max_iterations,
            } => {
                assert_eq!(name, "demo");
                assert_eq!(prompt, "hello");
                assert_eq!(max_iterations, Some(5));
            }
            _ => panic!("expected CreateTask variant"),
        }
    }

    #[test]
    fn test_ws_request_deserialization_list_threads() {
        let json = r#"{"type":"list_threads","limit":20,"include_archived":true}"#;
        let req: WsRequest = serde_json::from_str(json).unwrap();
        match req {
            WsRequest::ListThreads {
                limit,
                include_archived,
            } => {
                assert_eq!(limit, Some(20));
                assert_eq!(include_archived, Some(true));
            }
            _ => panic!("expected ListThreads variant"),
        }
    }

    #[test]
    fn test_ws_response_serialization_device_ack() {
        let resp = WsResponse::DeviceAck {
            device: crate::handlers::claw::ClawDevice {
                id: "d1".into(),
                name: "Dev".into(),
                host: "h".into(),
                version: "1".into(),
                status: crate::handlers::claw::DeviceStatus::Online,
                last_heartbeat: "2026-01-01T00:00:00Z".into(),
            },
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "device_ack");
        assert_eq!(json["device"]["id"], "d1");
        assert_eq!(json["device"]["status"], "online");
    }

    #[test]
    fn test_ws_response_serialization_task_created() {
        let resp = WsResponse::TaskCreated {
            task_id: "t1".into(),
            status: "Pending".into(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "task_created");
        assert_eq!(json["task_id"], "t1");
        assert_eq!(json["status"], "Pending");
    }

    #[test]
    fn test_ws_response_serialization_thread_created() {
        let resp = WsResponse::ThreadCreated {
            thread_id: "th-1".into(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "thread_created");
        assert_eq!(json["thread_id"], "th-1");
    }

    #[tokio::test]
    async fn test_handle_request_register_device() {
        let state = test_state().await;
        let session_id = SessionId::new();
        let response = handle_request(
            &state,
            &session_id,
            WsRequest::RegisterDevice {
                id: "claw-1".into(),
                name: "Claw One".into(),
                host: "localhost".into(),
                version: "0.3.0".into(),
                status: None,
            },
        )
        .await;
        match response {
            WsResponse::DeviceAck { device } => {
                assert_eq!(device.id, "claw-1");
                assert_eq!(device.name, "Claw One");
            }
            _ => panic!("expected DeviceAck variant"),
        }
    }

    #[tokio::test]
    async fn test_handle_request_create_and_list_threads() {
        let state = test_state().await;
        let session_id = SessionId::new();

        let create = handle_request(
            &state,
            &session_id,
            WsRequest::CreateThread {
                title: Some("My Thread".into()),
            },
        )
        .await;
        let thread_id = match create {
            WsResponse::ThreadCreated { thread_id } => thread_id,
            _ => panic!("expected ThreadCreated variant"),
        };

        let list = handle_request(
            &state,
            &session_id,
            WsRequest::ListThreads {
                limit: Some(10),
                include_archived: Some(false),
            },
        )
        .await;
        match list {
            WsResponse::ThreadList { data, .. } => {
                assert!(data.iter().any(|t| t.thread_id == thread_id));
            }
            _ => panic!("expected ThreadList variant"),
        }
    }

    #[tokio::test]
    async fn test_handle_request_create_task() {
        let state = test_state().await;
        let session_id = SessionId::new();
        let response = handle_request(
            &state,
            &session_id,
            WsRequest::CreateTask {
                name: "demo".into(),
                prompt: "say hi".into(),
                max_iterations: Some(1),
            },
        )
        .await;
        match response {
            WsResponse::TaskCreated { task_id, .. } => {
                assert!(!task_id.is_empty());
            }
            _ => panic!("expected TaskCreated variant"),
        }
    }

    #[tokio::test]
    async fn test_ws_upgrade_route_rejects_plain_get() {
        let state = test_state().await;
        let app = crate::server::create_api_router(state);

        let response = app
            .oneshot(Request::builder().uri("/ws").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(
            response.status().is_client_error(),
            "expected client error for non-websocket request, got {:?}",
            response.status()
        );
    }

    #[tokio::test]
    async fn test_ws_rejects_when_at_capacity() {
        let mut state = test_state().await;
        // Replace the semaphore with a zero-permit semaphore so the very next
        // /ws upgrade is rejected.
        let state_mut = Arc::get_mut(&mut state).expect("unique Arc in test");
        state_mut.ws_sem = Arc::new(Semaphore::new(0));

        let app = crate::server::create_api_router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let url = format!("ws://127.0.0.1:{}/ws", port);
        let result = tokio_tungstenite::connect_async(&url).await;
        assert!(
            result.is_err(),
            "expected WebSocket upgrade to be rejected when at capacity"
        );

        server.abort();
    }
}
