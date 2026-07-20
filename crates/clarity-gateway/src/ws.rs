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
use crate::transports::{
    GatewayWebSocketTransport, ServerTransportContext, transport_event_to_ws_response,
};
use clarity_contract::{
    ClawTransport, GovernedTransport, MessageContext, SessionSource, ThreadId, TransportAuth,
    TransportEvent,
};
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
    state
        .metrics
        .messages_received
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

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
                            handle_chat_with_wire(state.clone(), &session_id, message, &mut sender)
                                .await;
                        }
                        WsRequest::Subscribe { since_event_id } => {
                            let since = since_event_id.unwrap_or(0);
                            info!(
                                "WebSocket {} subscribed to events, since_id={}",
                                session_id, since
                            );
                            // Replay buffered events.
                            let buffered = state.event_wire.events_since(since);
                            for msg in &buffered {
                                let payload = match serde_json::to_value(msg) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        error!(
                                            "Failed to serialize wire message for replay: {}",
                                            e
                                        );
                                        continue;
                                    }
                                };
                                let envelope = WsResponse::WireMessage { payload };
                                if let Ok(text) = serde_json::to_string(&envelope)
                                    && sender.send(WsMessage::Text(text)).await.is_err()
                                {
                                    warn!("WebSocket closed during event replay");
                                    return;
                                }
                            }
                            // Send the current latest id so the client can track.
                            let latest = state.event_wire.latest_event_id();
                            let ack = WsResponse::EventAck {
                                latest_event_id: latest,
                            };
                            if let Ok(text) = serde_json::to_string(&ack)
                                && sender.send(WsMessage::Text(text)).await.is_err()
                            {
                                warn!("Failed to send EventAck");
                            }
                        }
                        request @ (WsRequest::RunSubagentStream { .. }
                        | WsRequest::RunParallelSubagentsStream { .. }) => {
                            handle_subagent_stream(state.clone(), request, &mut sender).await;
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
///
/// This delegates to the transport-agnostic `GatewayWebSocketTransport` so the
/// native `/ws` endpoint and the OpenClaw endpoint share the same chat/history
/// implementation.
async fn handle_chat_with_wire(
    state: Arc<AppState>,
    session_id: &SessionId,
    message: String,
    sender: &mut futures::stream::SplitSink<WebSocket, WsMessage>,
) {
    debug!(
        "Processing wire chat request from {}: message={}",
        session_id, message
    );

    let ctx = ServerTransportContext::new(state.clone(), session_id.to_string());
    let transport = GatewayWebSocketTransport::new(ctx);
    let transport = GovernedTransport::with_metrics(
        transport,
        TransportAuth {
            token: Some("gateway-server".into()),
            ..Default::default()
        },
        state.metrics.clone(),
    );

    if let Err(e) = transport.handshake(TransportAuth::default()).await {
        send_ws_response(
            sender,
            WsResponse::Error {
                error: format!("transport handshake failed: {}", e),
            },
        )
        .await;
        return;
    }

    let msg_ctx = MessageContext {
        message,
        ..Default::default()
    };
    if let Err(e) = transport.send_message(msg_ctx).await {
        send_ws_response(
            sender,
            WsResponse::Error {
                error: format!("transport send_message failed: {}", e),
            },
        )
        .await;
        return;
    }

    let mut events = transport.events();
    while let Some(ev) = events.next().await {
        let is_done = matches!(ev, TransportEvent::Done);
        if let Some(resp) = transport_event_to_ws_response(ev) {
            send_ws_response(sender, resp).await;
        }
        if is_done {
            break;
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
    /// Subscribe to streaming events with optional replay.
    ///
    /// If `since_event_id` is provided, the gateway replays buffered events
    /// from the shared Wire ring buffer before starting live streaming.
    Subscribe {
        /// Last event id known to the client (0 for all buffered events).
        #[serde(default)]
        since_event_id: Option<u64>,
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
    /// Authenticate with user identity (Phase 7).
    Authenticate {
        /// Device identifier.
        device_id: String,
        /// Optional user id to bind this device to.
        #[serde(default)]
        user_id: Option<String>,
        /// Human-readable device name.
        #[serde(default)]
        device_name: Option<String>,
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
    /// Run a single subagent and return the final result.
    RunSubagent {
        /// Subagent run specification.
        #[serde(flatten)]
        spec: SubagentRunSpec,
    },
    /// Run multiple subagents in parallel and return the final result.
    RunParallelSubagents {
        /// Subagent specifications.
        tasks: Vec<SubagentRunSpec>,
        /// Maximum concurrency.
        #[serde(default)]
        max_concurrency: Option<usize>,
        /// Cancel remaining tasks when one fails.
        #[serde(default)]
        cancel_on_error: bool,
    },
    /// Stream a single subagent run with progress events.
    RunSubagentStream {
        /// Subagent run specification.
        #[serde(flatten)]
        spec: SubagentRunSpec,
    },
    /// Stream a parallel subagent run with progress events.
    RunParallelSubagentsStream {
        /// Subagent specifications.
        tasks: Vec<SubagentRunSpec>,
        /// Maximum concurrency.
        #[serde(default)]
        max_concurrency: Option<usize>,
        /// Cancel remaining tasks when one fails.
        #[serde(default)]
        cancel_on_error: bool,
    },
    /// List registered subagent types.
    ListSubagentTypes,
}

/// Subagent run specification used by WebSocket requests.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SubagentRunSpec {
    /// Human-readable task description.
    pub description: String,
    /// Agent type registered in the labor market.
    pub agent_type: String,
    /// Prompt / instructions for the subagent.
    pub prompt: String,
    /// Optional model alias override.
    #[serde(default)]
    pub model: Option<String>,
    /// Optional maximum iteration override.
    #[serde(default)]
    pub max_iterations: Option<usize>,
    /// Whether to force read-only mode.
    #[serde(default)]
    pub read_only: bool,
    /// Optional goal tags for routing.
    #[serde(default)]
    pub goal_tags: Vec<String>,
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
    /// The current chat turn finished.
    Done,
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
    /// Acknowledgement after event replay with the latest event id.
    EventAck {
        /// Latest event id from the shared wire buffer.
        latest_event_id: u64,
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
    /// Authentication acknowledgement (Phase 7).
    Authenticated {
        /// Bound device id.
        device_id: String,
        /// Bound user id, if provided.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user_id: Option<String>,
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
    /// Subagent run progress event.
    SubagentProgress {
        /// Agent identifier.
        agent_id: String,
        /// Agent type.
        agent_type: String,
        /// Progress stage name.
        stage: String,
        /// Optional output text.
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        /// Optional status.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
        /// Optional step progress.
        #[serde(skip_serializing_if = "Option::is_none")]
        steps: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        /// Optional maximum steps.
        max_steps: Option<usize>,
    },
    /// Single subagent run result.
    SubagentResult {
        /// Agent identifier.
        agent_id: String,
        /// Agent type.
        agent_type: String,
        /// Execution status.
        status: String,
        /// Result summary.
        summary: String,
        /// Full output.
        full_output: String,
        /// Steps taken.
        steps_taken: usize,
        /// Elapsed milliseconds.
        elapsed_ms: u64,
    },
    /// Parallel subagent run result.
    ParallelSubagentsResult {
        /// Overall success rate.
        success_rate: f64,
        /// Total elapsed milliseconds.
        total_elapsed_ms: u64,
        /// Successful results.
        results: Vec<SubagentRunResult>,
        /// Failures as (description, error) pairs.
        failures: Vec<ParallelSubagentFailure>,
        /// Optional aggregated summary.
        #[serde(skip_serializing_if = "Option::is_none")]
        aggregated_summary: Option<String>,
    },
    /// Subagent type list response.
    SubagentTypes {
        /// Registered agent types.
        types: Vec<SubagentTypeSummary>,
    },
    /// Subagent run started acknowledgement (streaming only).
    SubagentRunStarted {
        /// Run identifier.
        run_id: String,
    },
}

/// Failure entry for a parallel subagent run.
#[derive(Debug, Serialize, Deserialize)]
pub struct ParallelSubagentFailure {
    /// Task description.
    pub description: String,
    /// Error message.
    pub error: String,
}

/// Single subagent run result payload (shared by single and parallel responses).
#[derive(Debug, Serialize, Deserialize)]
pub struct SubagentRunResult {
    /// Agent identifier.
    pub agent_id: String,
    /// Agent type.
    pub agent_type: String,
    /// Execution status.
    pub status: String,
    /// Result summary.
    pub summary: String,
    /// Full output.
    pub full_output: String,
    /// Steps taken.
    pub steps_taken: usize,
    /// Elapsed milliseconds.
    pub elapsed_ms: u64,
}

/// Registered subagent type summary.
#[derive(Debug, Serialize, Deserialize)]
pub struct SubagentTypeSummary {
    /// Agent type name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Default maximum iterations.
    pub max_iterations: usize,
    /// Whether the agent is read-only by default.
    pub read_only: bool,
    /// Capability tags.
    pub capabilities: Vec<String>,
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

/// Convert a WebSocket subagent spec into the contract `RunSpec`.
fn build_run_spec(spec: SubagentRunSpec) -> clarity_contract::subagent::RunSpec {
    let mut run_spec = clarity_contract::subagent::RunSpec::new(spec.description, spec.prompt)
        .with_type(spec.agent_type);
    if let Some(model) = spec.model {
        run_spec = run_spec.with_model(model);
    }
    if let Some(max_iterations) = spec.max_iterations {
        run_spec = run_spec.with_max_iterations(max_iterations);
    }
    if spec.read_only {
        run_spec = run_spec.with_read_only(true);
    }
    if !spec.goal_tags.is_empty() {
        run_spec = run_spec.with_goal_tags(spec.goal_tags);
    }
    run_spec
}

/// Build a `WsResponse::ParallelSubagentsResult` from a `ParallelResult`.
fn build_parallel_subagents_response(
    result: clarity_contract::subagent::ParallelResult,
) -> WsResponse {
    let success_rate = result.success_rate();
    let total_elapsed_ms = result.total_elapsed_ms;
    let aggregated_summary = result.aggregated_summary.clone();

    let results: Vec<SubagentRunResult> = result
        .results
        .into_iter()
        .map(|r| SubagentRunResult {
            agent_id: r.agent_id,
            agent_type: r.agent_type,
            status: r.status.to_string(),
            summary: r.summary,
            full_output: r.full_output,
            steps_taken: r.steps_taken,
            elapsed_ms: r.elapsed_ms,
        })
        .collect();

    let failures: Vec<ParallelSubagentFailure> = result
        .failures
        .into_iter()
        .map(|(description, error)| ParallelSubagentFailure { description, error })
        .collect();

    WsResponse::ParallelSubagentsResult {
        success_rate,
        total_elapsed_ms,
        results,
        failures,
        aggregated_summary,
    }
}

/// Stream a subagent run over WebSocket.
///
/// Blocks the socket until the run completes, emitting progress events and a
/// final result envelope. This mirrors the wire-chat streaming pattern.
async fn handle_subagent_stream(
    state: Arc<AppState>,
    request: WsRequest,
    sender: &mut futures::stream::SplitSink<WebSocket, WsMessage>,
) {
    let run_id = uuid::Uuid::new_v4().to_string();
    let started = WsResponse::SubagentRunStarted {
        run_id: run_id.clone(),
    };
    if let Ok(json) = serde_json::to_string(&started)
        && sender.send(WsMessage::Text(json)).await.is_err()
    {
        return;
    }

    match request {
        WsRequest::RunSubagentStream { spec } => {
            let run_spec = build_run_spec(spec);
            let (progress_tx, mut progress_rx) =
                tokio::sync::mpsc::channel::<clarity_contract::subagent::SubagentProgressEvent>(64);

            let mut manager = state.subagent_manager.lock().await;
            let run_future = manager.run(run_spec, Some(progress_tx));
            tokio::pin!(run_future);

            loop {
                tokio::select! {
                    Some(event) = progress_rx.recv() => {
                        let response = subagent_progress_to_ws(event);
                        if let Ok(json) = serde_json::to_string(&response)
                            && sender.send(WsMessage::Text(json)).await.is_err()
                        {
                            break;
                        }
                    }
                    result = &mut run_future => {
                        let response = match result {
                            Ok(result) => WsResponse::SubagentResult {
                                agent_id: result.agent_id,
                                agent_type: result.agent_type,
                                status: result.status.to_string(),
                                summary: result.summary,
                                full_output: result.full_output,
                                steps_taken: result.steps_taken,
                                elapsed_ms: result.elapsed_ms,
                            },
                            Err(e) => WsResponse::Error {
                                error: format!("subagent run failed: {}", e),
                            },
                        };
                        let _ = send_ws_response(sender, response).await;
                        break;
                    }
                }
            }
        }
        WsRequest::RunParallelSubagentsStream {
            tasks,
            max_concurrency,
            cancel_on_error,
        } => {
            let specs: Vec<clarity_contract::subagent::RunSpec> =
                tasks.into_iter().map(build_run_spec).collect();
            let mut config = clarity_contract::subagent::ParallelConfig::new()
                .with_max_concurrency(max_concurrency.unwrap_or(4).max(1));
            if cancel_on_error {
                config = config.cancel_on_error();
            }

            let progress = std::sync::Arc::new(parking_lot::Mutex::new(
                clarity_contract::subagent::BatchProgress::new(run_id.clone(), &specs),
            ));
            let progress_clone = progress.clone();

            let manager = state.subagent_manager.lock().await;
            let run_future = manager.run_parallel(specs, config, Some(progress_clone), None);
            tokio::pin!(run_future);

            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
            let mut last_completed = 0usize;
            let mut last_failed = 0usize;

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let (newly_completed, newly_failed, total) = {
                            let p = progress.lock();
                            (p.results.len(), p.failures.len(), p.total)
                        };
                        if newly_completed != last_completed || newly_failed != last_failed {
                            last_completed = newly_completed;
                            last_failed = newly_failed;
                            let response = WsResponse::SubagentProgress {
                                agent_id: run_id.clone(),
                                agent_type: "parallel-batch".to_string(),
                                stage: "progress".to_string(),
                                output: None,
                                status: Some(format!(
                                    "{}/{} completed, {} failed",
                                    newly_completed, total, newly_failed
                                )),
                                steps: Some(newly_completed + newly_failed),
                                max_steps: Some(total),
                            };
                            if let Ok(json) = serde_json::to_string(&response)
                                && sender.send(WsMessage::Text(json)).await.is_err()
                            {
                                break;
                            }
                        }
                    }
                    result = &mut run_future => {
                        let response = match result {
                            Ok(result) => build_parallel_subagents_response(result),
                            Err(e) => WsResponse::Error {
                                error: format!("parallel subagent run failed: {}", e),
                            },
                        };
                        let _ = send_ws_response(sender, response).await;
                        break;
                    }
                }
            }
        }
        _ => {}
    }
}

/// Convert a contract progress event into a WebSocket response.
fn subagent_progress_to_ws(event: clarity_contract::subagent::SubagentProgressEvent) -> WsResponse {
    match event {
        clarity_contract::subagent::SubagentProgressEvent::Stage { agent_id, name } => {
            WsResponse::SubagentProgress {
                agent_id,
                agent_type: String::new(),
                stage: name,
                output: None,
                status: Some("stage".to_string()),
                steps: None,
                max_steps: None,
            }
        }
        clarity_contract::subagent::SubagentProgressEvent::Output { agent_id, text } => {
            WsResponse::SubagentProgress {
                agent_id,
                agent_type: String::new(),
                stage: "output".to_string(),
                output: Some(text),
                status: None,
                steps: None,
                max_steps: None,
            }
        }
        clarity_contract::subagent::SubagentProgressEvent::StatusChange {
            agent_id,
            agent_type,
            status,
        } => WsResponse::SubagentProgress {
            agent_id,
            agent_type,
            stage: "status".to_string(),
            output: None,
            status: Some(format!("{:?}", status)),
            steps: None,
            max_steps: None,
        },
        clarity_contract::subagent::SubagentProgressEvent::Progress {
            agent_id,
            steps,
            max_steps,
        } => WsResponse::SubagentProgress {
            agent_id,
            agent_type: String::new(),
            stage: "progress".to_string(),
            output: None,
            status: None,
            steps: Some(steps),
            max_steps: Some(max_steps),
        },
    }
}

/// Serialize and send a WebSocket response, logging failures.
async fn send_ws_response(
    sender: &mut futures::stream::SplitSink<WebSocket, WsMessage>,
    response: WsResponse,
) {
    match serde_json::to_string(&response) {
        Ok(json) => {
            if let Err(e) = sender.send(WsMessage::Text(json)).await {
                warn!("Failed to send WebSocket response: {}", e);
            }
        }
        Err(e) => {
            error!("Failed to serialize WebSocket response: {}", e);
        }
    }
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

            // Serialize Agent turns across all WebSocket connections.
            let _permit = match state.agent_turn_sem.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    return WsResponse::Error {
                        error: "Agent turn queue closed".into(),
                    };
                }
            };

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
        WsRequest::Subscribe { since_event_id: _ } => {
            // Subscribe is handled in handle_socket for streaming; this arm is
            // reached when a non-streaming client sends Subscribe.
            WsResponse::EventAck {
                latest_event_id: state.event_wire.latest_event_id(),
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
        WsRequest::Authenticate {
            device_id,
            user_id,
            device_name: _,
        } => {
            tracing::debug!(device_id=%device_id, ?user_id, "WebSocket Authenticate");
            // ponytail: device→user binding is logged; persistence deferred to Phase 7+.
            WsResponse::Authenticated { device_id, user_id }
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
        WsRequest::RunSubagent { spec } => {
            let run_spec = build_run_spec(spec);
            let mut manager = state.subagent_manager.lock().await;
            match manager.run(run_spec, None).await {
                Ok(result) => WsResponse::SubagentResult {
                    agent_id: result.agent_id,
                    agent_type: result.agent_type,
                    status: result.status.to_string(),
                    summary: result.summary,
                    full_output: result.full_output,
                    steps_taken: result.steps_taken,
                    elapsed_ms: result.elapsed_ms,
                },
                Err(e) => WsResponse::Error {
                    error: format!("subagent run failed: {}", e),
                },
            }
        }
        WsRequest::RunParallelSubagents {
            tasks,
            max_concurrency,
            cancel_on_error,
        } => {
            let specs: Vec<clarity_contract::subagent::RunSpec> =
                tasks.into_iter().map(build_run_spec).collect();
            let mut config = clarity_contract::subagent::ParallelConfig::new()
                .with_max_concurrency(max_concurrency.unwrap_or(4).max(1));
            if cancel_on_error {
                config = config.cancel_on_error();
            }
            let manager = state.subagent_manager.lock().await;
            match manager.run_parallel(specs, config, None, None).await {
                Ok(result) => build_parallel_subagents_response(result),
                Err(e) => WsResponse::Error {
                    error: format!("parallel subagent run failed: {}", e),
                },
            }
        }
        WsRequest::RunSubagentStream { .. } => WsResponse::Error {
            error: "RunSubagentStream must be handled by the streaming router".into(),
        },
        WsRequest::RunParallelSubagentsStream { .. } => WsResponse::Error {
            error: "RunParallelSubagentsStream must be handled by the streaming router".into(),
        },
        WsRequest::ListSubagentTypes => {
            let manager = state.subagent_manager.lock().await;
            let types = manager
                .labor_market()
                .list()
                .into_iter()
                .map(|def| SubagentTypeSummary {
                    name: def.name.clone(),
                    description: def.description.clone(),
                    max_iterations: def.max_iterations,
                    read_only: def.read_only,
                    capabilities: def.capabilities.clone(),
                })
                .collect();
            WsResponse::SubagentTypes { types }
        }
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

    #[test]
    fn test_ws_request_deserialization_run_subagent() {
        let json = r#"{"type":"run_subagent","description":"test","agent_type":"coder","prompt":"hello","max_iterations":1,"read_only":true}"#;
        let req: WsRequest = serde_json::from_str(json).unwrap();
        match req {
            WsRequest::RunSubagent { spec } => {
                assert_eq!(spec.description, "test");
                assert_eq!(spec.agent_type, "coder");
                assert_eq!(spec.prompt, "hello");
                assert_eq!(spec.max_iterations, Some(1));
                assert!(spec.read_only);
            }
            _ => panic!("expected RunSubagent variant"),
        }
    }

    #[test]
    fn test_ws_request_deserialization_run_parallel_subagents() {
        let json = r#"{"type":"run_parallel_subagents","tasks":[{"description":"a","agent_type":"coder","prompt":"p1"},{"description":"b","agent_type":"review","prompt":"p2"}],"max_concurrency":2,"cancel_on_error":true}"#;
        let req: WsRequest = serde_json::from_str(json).unwrap();
        match req {
            WsRequest::RunParallelSubagents {
                tasks,
                max_concurrency,
                cancel_on_error,
            } => {
                assert_eq!(tasks.len(), 2);
                assert_eq!(tasks[0].agent_type, "coder");
                assert_eq!(tasks[1].agent_type, "review");
                assert_eq!(max_concurrency, Some(2));
                assert!(cancel_on_error);
            }
            _ => panic!("expected RunParallelSubagents variant"),
        }
    }

    #[test]
    fn test_ws_request_deserialization_list_subagent_types() {
        let json = r#"{"type":"list_subagent_types"}"#;
        let req: WsRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req, WsRequest::ListSubagentTypes));
    }

    #[test]
    fn test_ws_response_serialization_subagent_result() {
        let resp = WsResponse::SubagentResult {
            agent_id: "a1".into(),
            agent_type: "coder".into(),
            status: "completed".into(),
            summary: "done".into(),
            full_output: "full".into(),
            steps_taken: 3,
            elapsed_ms: 100,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "subagent_result");
        assert_eq!(json["agent_id"], "a1");
        assert_eq!(json["status"], "completed");
    }

    #[test]
    fn test_ws_response_serialization_subagent_types() {
        let resp = WsResponse::SubagentTypes {
            types: vec![SubagentTypeSummary {
                name: "coder".into(),
                description: "Code tasks".into(),
                max_iterations: 20,
                read_only: false,
                capabilities: vec!["code".into()],
            }],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "subagent_types");
        assert!(json["types"].is_array());
        assert_eq!(json["types"][0]["name"], "coder");
    }

    #[tokio::test]
    async fn test_handle_request_list_subagent_types() {
        let state = test_state().await;
        let session_id = SessionId::new();
        let response = handle_request(&state, &session_id, WsRequest::ListSubagentTypes).await;
        match response {
            WsResponse::SubagentTypes { types } => {
                assert!(!types.is_empty());
                assert!(types.iter().any(|t| t.name == "coder"));
            }
            _ => panic!("expected SubagentTypes variant"),
        }
    }

    #[tokio::test]
    async fn test_handle_request_run_subagent() {
        let state = test_state().await;
        let session_id = SessionId::new();
        let response = handle_request(
            &state,
            &session_id,
            WsRequest::RunSubagent {
                spec: SubagentRunSpec {
                    description: "ws-test".into(),
                    agent_type: "coder".into(),
                    prompt: "Say hello".into(),
                    model: None,
                    max_iterations: Some(1),
                    read_only: false,
                    goal_tags: Vec::new(),
                },
            },
        )
        .await;
        match response {
            WsResponse::SubagentResult { agent_type, .. } => {
                assert_eq!(agent_type, "coder");
            }
            _ => panic!("expected SubagentResult variant, got {:?}", response),
        }
    }
}
