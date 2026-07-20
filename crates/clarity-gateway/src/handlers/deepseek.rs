//! DeepSeek Chat Android app-compatible API handlers.
//!
//! These endpoints emulate the private `chat.deepseek.com/api/v0/*` protocol so
//! that a patched DeepSeek APK can talk directly to Clarity Gateway. The
//! implementation is intentionally minimal: authentication is mocked, PoW is
//! trivially bypassed, and chat turns are executed by the shared Clarity Agent.
//!
//! ponytail: This is a pragmatic shim for a patched client. Do not expose these
//! endpoints to untrusted clients — there is no real account verification.

use crate::handlers::AgentHandle;
use crate::server::AppState;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response, Sse, sse::Event as SseEvent},
};
use clarity_contract::SessionSource;
use clarity_core::agent::{
    AgentController, ControllerEvent, Message as AgentMessage, MessageRole, Op,
    driver::ConversationChatDriver,
};
use futures::{Stream, stream};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{info, warn};

use crate::handlers::chat::truncate_messages_by_bytes;

// ==================== Request / Response types ====================

/// DeepSeek app login request.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Mobile number in E.164-ish form.
    #[serde(default)]
    pub mobile: Option<String>,
    /// Email address (used by some flows).
    #[serde(default)]
    pub email: Option<String>,
    /// Account password.
    #[serde(default)]
    pub password: Option<String>,
    /// Device identifier persisted by the app.
    #[serde(default)]
    pub device_id: Option<String>,
    /// Operating system identifier.
    #[serde(default)]
    pub os: Option<String>,
}

/// DeepSeek app login response payload.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    /// Outer status code.
    pub code: i32,
    /// Outer status message.
    pub msg: String,
    /// Wrapped business data.
    pub data: LoginData,
}

/// Wrapped login response.
#[derive(Debug, Serialize)]
pub struct LoginData {
    /// Business status code.
    pub biz_code: i32,
    /// Business status message.
    pub biz_msg: String,
    /// User object and token.
    pub biz_data: LoginBizData,
}

/// User + token section.
#[derive(Debug, Serialize)]
pub struct LoginBizData {
    /// Internal status code.
    pub code: i32,
    /// Internal status message.
    pub msg: String,
    /// User record.
    pub user: DeepSeekUser,
}

/// Mock DeepSeek user profile.
#[derive(Debug, Serialize)]
pub struct DeepSeekUser {
    /// Stable user UUID.
    pub id: String,
    /// Bearer token used for subsequent requests.
    pub token: String,
    /// Masked email.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Masked mobile number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mobile_number: Option<String>,
    /// Country code.
    pub area_code: String,
    /// Account status.
    pub status: i32,
    /// Identity profile summary.
    pub id_profile: serde_json::Value,
    /// All identity profiles.
    pub id_profiles: Vec<serde_json::Value>,
    /// Chat-specific flags.
    pub chat: serde_json::Value,
    /// Whether legacy chat history exists.
    pub has_legacy_chat_history: bool,
    /// Whether birthday input is required.
    pub need_birthday: bool,
}

/// DeepSeek chat completion request.
#[derive(Debug, Deserialize)]
pub struct DeepSeekCompletionRequest {
    /// DeepSeek session id (mapped to a Clarity thread).
    pub chat_session_id: String,
    /// Parent message id (unused in shim).
    #[serde(default)]
    pub parent_message_id: Option<serde_json::Value>,
    /// User prompt.
    pub prompt: String,
    /// Referenced file ids (unused).
    #[serde(default)]
    pub ref_file_ids: Vec<String>,
    /// Whether to enable thinking mode.
    #[serde(default)]
    pub thinking_enabled: bool,
    /// Whether to enable web search.
    #[serde(default)]
    pub search_enabled: bool,
    /// Voice input id (unused).
    #[serde(default)]
    pub audio_id: Option<serde_json::Value>,
    /// Preempt flag (unused).
    #[serde(default)]
    pub preempt: bool,
    /// Model type alias (`default`, `reasoner`, ...).
    #[serde(default)]
    pub model_type: String,
    /// Optional action (unused).
    #[serde(default)]
    pub action: Option<serde_json::Value>,
}

/// DeepSeek session create response.
#[derive(Debug, Serialize)]
pub struct SessionCreateResponse {
    /// Outer status code.
    pub code: i32,
    /// Outer status message.
    pub msg: String,
    /// Wrapped business data.
    pub data: SessionCreateData,
}

/// Wrapped session create response.
#[derive(Debug, Serialize)]
pub struct SessionCreateData {
    /// Business status code.
    pub biz_code: i32,
    /// Business status message.
    pub biz_msg: String,
    /// Session object.
    pub biz_data: SessionCreateBizData,
}

/// Session + TTL section.
#[derive(Debug, Serialize)]
pub struct SessionCreateBizData {
    /// The created session.
    pub chat_session: DeepSeekChatSession,
    /// TTL in seconds.
    pub ttl_seconds: u64,
}

/// DeepSeek chat session object.
#[derive(Debug, Serialize)]
pub struct DeepSeekChatSession {
    /// Session UUID.
    pub id: String,
    /// Sequence id.
    pub seq_id: i64,
    /// Agent kind.
    pub agent: String,
    /// Model type.
    pub model_type: String,
    /// Session title (null until generated).
    pub title: Option<String>,
    /// Title type.
    pub title_type: String,
    /// Version counter.
    pub version: i32,
    /// Current message id.
    pub current_message_id: Option<serde_json::Value>,
    /// Pinned flag.
    pub pinned: bool,
    /// Creation timestamp.
    pub inserted_at: f64,
    /// Last update timestamp.
    pub updated_at: f64,
}

/// PoW challenge request.
#[derive(Debug, Deserialize)]
pub struct PowChallengeRequest {
    /// Target API path.
    pub target_path: String,
}

/// PoW challenge response.
#[derive(Debug, Serialize)]
pub struct PowChallengeResponse {
    /// Outer status code.
    pub code: i32,
    /// Outer status message.
    pub msg: String,
    /// Wrapped business data.
    pub data: PowChallengeData,
}

/// Wrapped PoW challenge response.
#[derive(Debug, Serialize)]
pub struct PowChallengeData {
    /// Business status code.
    pub biz_code: i32,
    /// Business status message.
    pub biz_msg: String,
    /// Challenge object.
    pub biz_data: PowChallengeBizData,
}

/// PoW challenge payload.
#[derive(Debug, Serialize)]
pub struct PowChallengeBizData {
    /// The challenge.
    pub challenge: DeepSeekChallenge,
}

/// PoW challenge fields.
#[derive(Debug, Serialize)]
pub struct DeepSeekChallenge {
    /// Algorithm name.
    pub algorithm: String,
    /// Challenge hex string.
    pub challenge: String,
    /// Salt hex string.
    pub salt: String,
    /// Server signature.
    pub signature: String,
    /// Difficulty target.
    pub difficulty: u64,
    /// Expiration timestamp.
    pub expire_at: u64,
    /// Validity window in milliseconds.
    pub expire_after: u64,
    /// Target API path.
    pub target_path: String,
}

// ==================== Handlers ====================

fn ok_json<T: Serialize>(body: T) -> Response {
    (StatusCode::OK, Json(body)).into_response()
}

fn deepseek_error(msg: impl Into<String>) -> Response {
    let payload = serde_json::json!({
        "code": -1,
        "msg": msg.into(),
        "data": { "biz_code": -1, "biz_msg": "", "biz_data": null }
    });
    (StatusCode::OK, Json(payload)).into_response()
}

/// Return the country code matching the request IP.
pub async fn ip_to_country_code() -> Response {
    ok_json(serde_json::json!({
        "code": 0,
        "msg": "",
        "data": {
            "biz_code": 0,
            "biz_msg": "",
            "biz_data": { "ip": "127.0.0.1", "code": "CN" }
        }
    }))
}

/// Return app update information.
pub async fn check_client_update() -> Response {
    ok_json(serde_json::json!({
        "code": 0,
        "msg": "",
        "data": { "biz_code": 0, "biz_msg": "", "biz_data": null }
    }))
}

/// Return client settings for the requested scope.
pub async fn client_settings() -> Response {
    ok_json(serde_json::json!({
        "code": 0,
        "msg": "",
        "data": {
            "biz_code": 0,
            "biz_msg": "",
            "biz_data": {
                "version": 1,
                "settings": {
                    "model_configs": {
                        "id": 1,
                        "value": [
                            {
                                "model_type": "default",
                                "name": "快速模式",
                                "description": "适合日常对话，即时响应",
                                "welcome_msg": "使用快速模式开始对话",
                                "is_default": true,
                                "enabled": true,
                                "switchable": true,
                                "show_model_name_in_session": true,
                                "edit_quota": 5,
                                "regenerate_quota": 5,
                                "input_character_limit": 2621440,
                                "think_feature": {},
                                "search_feature": {},
                                "regenerate_options": null,
                                "file_feature": {
                                    "token_limit": 890880,
                                    "token_limit_with_thinking": 890880,
                                    "max_input_file_count": 50,
                                    "max_upload_file_size": 104857600,
                                    "support_file_exts": ["txt", "md", "pdf", "png", "jpg", "jpeg"]
                                }
                            }
                        ]
                    },
                    "support_chat_file_exts": {
                        "id": 2,
                        "value": ["txt", "md", "pdf", "png", "jpg", "jpeg"]
                    }
                }
            }
        }
    }))
}

/// Mock login that accepts any credentials and returns a stable bearer token.
pub async fn login(State(state): State<Arc<AppState>>, Json(req): Json<LoginRequest>) -> Response {
    let identifier = req
        .mobile
        .clone()
        .or(req.email.clone())
        .unwrap_or_else(|| "anonymous".to_string());
    info!("DeepSeek login request for {}", identifier);

    let token = format!("ds-token-{}", uuid::Uuid::new_v4().simple());
    let user_id = uuid::Uuid::new_v4().to_string();

    // Remember the token so subsequent requests can be mapped back.
    {
        let mut tokens = state.deepseek_tokens.write().await;
        tokens.insert(token.clone(), user_id.clone());
    }

    let email = req.mobile.as_ref().map(|m| {
        format!(
            "{}*****{}@clarity.local",
            &m[..3],
            &m[m.len().saturating_sub(2)..]
        )
    });
    let mobile = req.mobile.as_ref().map(|m| {
        if m.len() >= 7 {
            format!("{}******{}", &m[..3], &m[m.len() - 2..])
        } else {
            m.clone()
        }
    });

    let user = DeepSeekUser {
        id: user_id,
        token: token.clone(),
        email,
        mobile_number: mobile,
        area_code: "+86".to_string(),
        status: 0,
        id_profile: serde_json::json!({
            "provider": "CLARITY",
            "id": uuid::Uuid::new_v4().to_string(),
            "name": "Clarity User",
            "picture": "",
            "locale": "zh_CN",
            "email": null
        }),
        id_profiles: vec![],
        chat: serde_json::json!({ "is_muted": 0, "mute_until": null }),
        has_legacy_chat_history: false,
        need_birthday: false,
    };

    ok_json(LoginResponse {
        code: 0,
        msg: "".to_string(),
        data: LoginData {
            biz_code: 0,
            biz_msg: "".to_string(),
            biz_data: LoginBizData {
                code: 0,
                msg: "".to_string(),
                user,
            },
        },
    })
}

/// Return the current user profile.
pub async fn current_user(State(_state): State<Arc<AppState>>) -> Response {
    // Accept any authorization header and return a mock profile.
    let user_id = uuid::Uuid::new_v4().to_string();
    ok_json(serde_json::json!({
        "code": 0,
        "msg": "",
        "data": {
            "biz_code": 0,
            "biz_msg": "",
            "biz_data": {
                "id": user_id,
                "token": "ds-token-mock",
                "email": "user@clarity.local",
                "mobile_number": "136******12",
                "area_code": "+86",
                "status": 0,
                "id_profile": {
                    "provider": "CLARITY",
                    "id": uuid::Uuid::new_v4().to_string(),
                    "name": "Clarity User",
                    "picture": "",
                    "locale": "zh_CN",
                    "email": null
                },
                "id_profiles": [],
                "chat": { "is_muted": 0, "mute_until": null },
                "has_legacy_chat_history": false,
                "need_birthday": false
            }
        }
    }))
}

/// Create a new DeepSeek chat session and map it to a Clarity thread.
pub async fn create_session(State(state): State<Arc<AppState>>) -> Response {
    let ds_session_id = uuid::Uuid::new_v4().to_string();

    let thread_id = match state
        .thread_manager
        .create_thread(
            &state.agent.config().working_dir,
            "DeepSeek session",
            SessionSource::AppServer,
        )
        .await
    {
        Ok(t) => t.to_string(),
        Err(e) => {
            warn!(
                "Failed to create Clarity thread for DeepSeek session: {}",
                e
            );
            return deepseek_error(format!("thread creation failed: {e}"));
        }
    };

    {
        let mut sessions = state.deepseek_sessions.write().await;
        sessions.insert(ds_session_id.clone(), thread_id);
    }

    let now = chrono::Utc::now().timestamp() as f64;
    ok_json(SessionCreateResponse {
        code: 0,
        msg: "".to_string(),
        data: SessionCreateData {
            biz_code: 0,
            biz_msg: "".to_string(),
            biz_data: SessionCreateBizData {
                chat_session: DeepSeekChatSession {
                    id: ds_session_id,
                    seq_id: 0,
                    agent: "chat".to_string(),
                    model_type: "default".to_string(),
                    title: None,
                    title_type: "WIP".to_string(),
                    version: 0,
                    current_message_id: None,
                    pinned: false,
                    inserted_at: now,
                    updated_at: now,
                },
                ttl_seconds: 259200,
            },
        },
    })
}

/// Return a trivial PoW challenge that the patched client can answer with zero.
pub async fn create_pow_challenge(Json(req): Json<PowChallengeRequest>) -> Response {
    let now = chrono::Utc::now().timestamp_millis() as u64;
    ok_json(PowChallengeResponse {
        code: 0,
        msg: "".to_string(),
        data: PowChallengeData {
            biz_code: 0,
            biz_msg: "".to_string(),
            biz_data: PowChallengeBizData {
                challenge: DeepSeekChallenge {
                    algorithm: "DeepSeekHashV1".to_string(),
                    challenge: "00".repeat(32),
                    salt: "clarity".to_string(),
                    signature: "00".repeat(32),
                    difficulty: 1,
                    expire_at: now + 300_000,
                    expire_after: 300_000,
                    target_path: req.target_path,
                },
            },
        },
    })
}

/// Fetch paginated sessions.
pub async fn fetch_sessions(State(state): State<Arc<AppState>>) -> Response {
    let sessions = state.deepseek_sessions.read().await;
    let now = chrono::Utc::now().timestamp() as f64;
    let items: Vec<DeepSeekChatSession> = sessions
        .keys()
        .map(|id| DeepSeekChatSession {
            id: id.clone(),
            seq_id: 0,
            agent: "chat".to_string(),
            model_type: "default".to_string(),
            title: None,
            title_type: "WIP".to_string(),
            version: 0,
            current_message_id: None,
            pinned: false,
            inserted_at: now,
            updated_at: now,
        })
        .collect();

    ok_json(serde_json::json!({
        "code": 0,
        "msg": "",
        "data": {
            "biz_code": 0,
            "biz_msg": "",
            "biz_data": { "chat_sessions": items, "has_more": false }
        }
    }))
}

// ==================== Chat completion ====================

/// DeepSeek SSE event produced for the patched app.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum DeepSeekSseData {
    /// Initial ready event.
    Ready {
        /// Request message id.
        request_message_id: i64,
        /// Response message id.
        response_message_id: i64,
        /// Model type.
        model_type: String,
    },
    /// Session timestamp update.
    UpdateSession {
        /// New updated_at timestamp.
        updated_at: f64,
    },
    /// JSON Patch style append.
    Append {
        /// Patch path.
        p: String,
        /// Operation.
        o: String,
        /// Value to append.
        v: serde_json::Value,
    },
    /// JSON Patch style set.
    Set {
        /// Patch path.
        p: String,
        /// Operation.
        o: String,
        /// Value to set.
        v: serde_json::Value,
    },
    /// JSON Patch style batch.
    Batch {
        /// Patch path.
        p: String,
        /// Operation.
        o: String,
        /// Batch operations.
        v: Vec<serde_json::Value>,
    },
    /// Title event.
    Title {
        /// Generated title.
        content: String,
    },
    /// Close event.
    Close {
        /// Click behavior hint.
        click_behavior: String,
        /// Auto resume flag.
        auto_resume: bool,
    },
}

fn deepseek_sse_event(name: Option<&str>, data: DeepSeekSseData) -> SseEvent {
    let mut event = SseEvent::default();
    if let Some(n) = name {
        event = event.event(n);
    }
    event = event.data(serde_json::to_string(&data).unwrap_or_default());
    event
}

/// Internal state for the DeepSeek SSE stream.
struct DeepSeekSseState {
    /// Receiver of agent controller events.
    rx: UnboundedReceiver<ControllerEvent>,
    /// Buffered events waiting to be yielded.
    buffer: VecDeque<SseEvent>,
    /// Original user prompt for title generation and persistence.
    user_content: String,
    /// Thread manager for persisting the turn.
    thread_manager: clarity_core::thread::ThreadManager,
    /// Target Clarity thread id.
    thread_id: clarity_contract::ThreadId,
    /// Whether the stream has finished.
    finished: bool,
    /// Whether any content chunk has already been emitted.
    has_emitted_chunk: bool,
}

/// Convert an agent controller event stream into a DeepSeek-compatible SSE stream.
fn deepseek_sse_stream(
    event_rx: UnboundedReceiver<ControllerEvent>,
    thread_manager: clarity_core::thread::ThreadManager,
    thread_id: clarity_contract::ThreadId,
    user_content: String,
    initial_events: Vec<SseEvent>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let mut buffer = VecDeque::with_capacity(initial_events.len() + 8);
    for ev in initial_events {
        buffer.push_back(ev);
    }

    let state = DeepSeekSseState {
        rx: event_rx,
        buffer,
        user_content,
        thread_manager,
        thread_id,
        finished: false,
        has_emitted_chunk: false,
    };

    let sse_stream = stream::unfold(state, |mut st| async move {
        loop {
            // Yield any buffered events before checking the finished flag so
            // that terminal events (e.g. close) are always emitted.
            if let Some(event) = st.buffer.pop_front() {
                return Some((Ok(event), st));
            }
            if st.finished {
                return None;
            }

            match st.rx.recv().await {
                Some(ControllerEvent::Chunk(text)) => {
                    st.has_emitted_chunk = true;
                    st.buffer.push_back(deepseek_sse_event(
                        None,
                        DeepSeekSseData::Append {
                            p: "response/fragments/-1/content".to_string(),
                            o: "APPEND".to_string(),
                            v: serde_json::Value::String(text),
                        },
                    ));
                }
                Some(ControllerEvent::Complete(final_text)) => {
                    let updated_at = chrono::Utc::now().timestamp() as f64;
                    let title = st
                        .user_content
                        .split_whitespace()
                        .next()
                        .unwrap_or("Chat")
                        .to_string();
                    let uc = st.user_content.clone();
                    let tm = st.thread_manager.clone();
                    let tid = st.thread_id;
                    // Some LLM providers emit Complete with the full text and no
                    // preceding chunks. In that case, emit the text as a single
                    // Append so the client can render it.
                    if !st.has_emitted_chunk && !final_text.is_empty() {
                        st.buffer.push_back(deepseek_sse_event(
                            None,
                            DeepSeekSseData::Append {
                                p: "response/fragments/-1/content".to_string(),
                                o: "APPEND".to_string(),
                                v: serde_json::Value::String(final_text.clone()),
                            },
                        ));
                    }
                    // ponytail: fire-and-forget persistence; failure is logged by the manager.
                    tokio::spawn(async move {
                        let _ = tm.append_turn(tid, uc, final_text).await;
                    });
                    st.buffer.push_back(deepseek_sse_event(
                        None,
                        DeepSeekSseData::Batch {
                            p: "response".to_string(),
                            o: "BATCH".to_string(),
                            v: vec![
                                serde_json::json!({"p": "accumulated_token_usage", "v": 0}),
                                serde_json::json!({"p": "quasi_status", "v": "FINISHED"}),
                            ],
                        },
                    ));
                    st.buffer.push_back(deepseek_sse_event(
                        None,
                        DeepSeekSseData::Set {
                            p: "response/status".to_string(),
                            o: "SET".to_string(),
                            v: serde_json::Value::String("FINISHED".to_string()),
                        },
                    ));
                    st.buffer.push_back(deepseek_sse_event(
                        Some("update_session"),
                        DeepSeekSseData::UpdateSession { updated_at },
                    ));
                    st.buffer.push_back(deepseek_sse_event(
                        Some("title"),
                        DeepSeekSseData::Title { content: title },
                    ));
                    st.buffer.push_back(deepseek_sse_event(
                        Some("close"),
                        DeepSeekSseData::Close {
                            click_behavior: "none".to_string(),
                            auto_resume: false,
                        },
                    ));
                    st.finished = true;
                }
                Some(ControllerEvent::Error(e)) => {
                    warn!("DeepSeek SSE stream error: {}", e);
                    st.buffer.push_back(deepseek_sse_event(
                        Some("close"),
                        DeepSeekSseData::Close {
                            click_behavior: "none".to_string(),
                            auto_resume: false,
                        },
                    ));
                    st.finished = true;
                }
                Some(ControllerEvent::ToolCallStart { .. })
                | Some(ControllerEvent::ToolResult { .. })
                | Some(ControllerEvent::StepBegin { .. }) => {
                    // Tool-calling events are not surfaced in the DeepSeek app protocol.
                }
                None => {
                    st.buffer.push_back(deepseek_sse_event(
                        Some("close"),
                        DeepSeekSseData::Close {
                            click_behavior: "none".to_string(),
                            auto_resume: false,
                        },
                    ));
                    st.finished = true;
                }
            }
        }
    });

    Sse::new(sse_stream)
}

/// DeepSeek-compatible streaming chat completion.
pub async fn chat_completion(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DeepSeekCompletionRequest>,
) -> Response {
    info!(
        "DeepSeek chat completion: session={}, model_type={}",
        req.chat_session_id, req.model_type
    );

    let _permit = match state.chat_sem.acquire().await {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "code": -1,
                    "msg": "Gateway is at maximum concurrency limit",
                    "data": { "biz_code": -1, "biz_msg": "", "biz_data": null }
                })),
            )
                .into_response();
        }
    };

    // Resolve DeepSeek session -> Clarity thread.
    let thread_id = {
        let sessions = state.deepseek_sessions.read().await;
        match sessions.get(&req.chat_session_id) {
            Some(tid) => match clarity_contract::ThreadId::from_string(tid) {
                Ok(t) => t,
                Err(e) => {
                    return deepseek_error(format!("invalid mapped thread id: {e}"));
                }
            },
            None => {
                return deepseek_error(format!(
                    "chat_session_id not found: {}",
                    req.chat_session_id
                ));
            }
        }
    };

    // Load thread history.
    let history = match state.thread_manager.load_llm_history(thread_id).await {
        Ok(h) => h,
        Err(e) => {
            warn!(
                "Failed to load thread history for DeepSeek session {}, starting fresh: {}",
                thread_id, e
            );
            Vec::new()
        }
    };

    let mut messages: Vec<AgentMessage> = Vec::with_capacity(history.len() + 1);
    for msg in history {
        messages.push(msg);
    }
    messages.push(AgentMessage {
        role: MessageRole::User,
        content: req.prompt.clone(),
        tool_calls: None,
        tool_call_id: None,
    });

    let agent = state.clone_agent();
    if agent.approval_mode() == clarity_core::approval::ApprovalMode::Yolo {
        warn!("DeepSeek chat completion running with Yolo approval mode");
    }

    messages = truncate_messages_by_bytes(messages, 1_500_000);

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<ControllerEvent>();
    let driver = Arc::new(ConversationChatDriver {
        history: messages.clone(),
    });
    let (controller, op_tx) = AgentController::new_with_events(agent, event_tx, Some(driver));
    tokio::spawn(controller.run());

    let turn_permit = match state.agent_turn_sem.clone().acquire_owned().await {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "code": -1,
                    "msg": "Agent turn queue is closed",
                    "data": { "biz_code": -1, "biz_msg": "", "biz_data": null }
                })),
            )
                .into_response();
        }
    };

    if let Err(e) = op_tx.send(Op::user_turn(req.prompt.clone())) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "code": -1,
                "msg": format!("Failed to start agent turn: {e}"),
                "data": { "biz_code": -1, "biz_msg": "", "biz_data": null }
            })),
        )
            .into_response();
    }

    // Build initial ready + update_session events.
    let now_ts = chrono::Utc::now().timestamp() as f64;
    let initial_events = vec![
        deepseek_sse_event(
            Some("ready"),
            DeepSeekSseData::Ready {
                request_message_id: 1,
                response_message_id: 2,
                model_type: req.model_type.clone(),
            },
        ),
        deepseek_sse_event(
            Some("update_session"),
            DeepSeekSseData::UpdateSession { updated_at: now_ts },
        ),
    ];

    let sse = deepseek_sse_stream(
        event_rx,
        state.thread_manager.clone(),
        thread_id,
        req.prompt,
        initial_events,
    );

    // Hold the turn permit until the response body is dropped.
    let _hold = turn_permit;

    sse.into_response()
}
