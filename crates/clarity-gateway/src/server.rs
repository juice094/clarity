use axum::{
    Router,
    body::Body,
    http::{HeaderValue, Method, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::signal;
use tokio::sync::{RwLock, Semaphore};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::handlers;
use crate::role_context_store::RoleContextStore;
use crate::session_store::{PersistentSessionStore, SessionStoreError};
use chrono::{DateTime, Utc};
use clarity_contract::subagent::BatchProgress;
use clarity_core::activity::ActivityLogger;
use clarity_core::agent::Agent;
use clarity_core::background::BackgroundTaskManager;
use clarity_core::thread::ThreadManager;
use clarity_rollout::RolloutConfig;
use clarity_thread_store::{InMemoryThreadStore, LocalThreadStore, ThreadStore};
use parking_lot::Mutex;
use std::collections::HashMap;

/// Allowed CORS origins for the public API.
const ALLOWED_ORIGINS: [HeaderValue; 5] = [
    HeaderValue::from_static("http://localhost:3000"),
    HeaderValue::from_static("http://localhost:5173"),
    HeaderValue::from_static("http://127.0.0.1:3000"),
    HeaderValue::from_static("http://127.0.0.1:5173"),
    HeaderValue::from_static("http://127.0.0.1:18800"),
];

/// 应用状态
pub struct AppState {
    /// Shared Clarity agent.
    pub agent: Arc<Agent>,
    /// Persistent SQLite session store.
    pub session_store: Arc<PersistentSessionStore>,
    /// Thread store (v2 session persistence).
    pub thread_store: Arc<dyn ThreadStore>,
    /// Thread manager helper.
    pub thread_manager: ThreadManager,
    /// Background task manager.
    pub task_manager: Arc<BackgroundTaskManager>,
    /// Activity/event logger.
    pub activity_logger: ActivityLogger,
    /// Server start timestamp.
    pub started_at: DateTime<Utc>,
    /// Registry of in-flight parallel batch progress for UI polling.
    pub parallel_batches: Arc<RwLock<HashMap<String, Arc<Mutex<BatchProgress>>>>>,
    /// Concurrency limit for /v1/chat/completions to prevent unbounded spawn.
    pub chat_sem: Arc<Semaphore>,
    /// Concurrency limit for /ws to prevent unbounded agent sessions.
    pub ws_sem: Arc<Semaphore>,
    /// Unified OAuth service for providers that use OAuth device flow.
    pub oauth_service: Arc<clarity_llm::auth::OAuthService>,
    /// Registry of connected Claw daemon instances (heartbeat-based liveness).
    pub device_registry: crate::handlers::claw::DeviceRegistry,
    /// Persistent SQLite store for Claw Mesh role-context events.
    pub role_context_store: Arc<RoleContextStore>,
}

impl AppState {
    /// Create a new shared application state using the current working directory
    /// as the Clarity home.
    ///
    /// Falls back to an in-memory session store if the persistent SQLite store
    /// cannot be opened. Returns an error only if both attempts fail.
    pub async fn new(
        agent: Arc<Agent>,
        task_manager: Arc<BackgroundTaskManager>,
    ) -> Result<Self, SessionStoreError> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new_with_home(agent, task_manager, cwd.join(".clarity")).await
    }

    /// Create a new shared application state with an explicit Clarity home.
    ///
    /// This is used by tests to avoid shared-disk state between parallel runs.
    pub async fn new_with_home(
        agent: Arc<Agent>,
        task_manager: Arc<BackgroundTaskManager>,
        clarity_home: impl AsRef<Path>,
    ) -> Result<Self, SessionStoreError> {
        let clarity_home = clarity_home.as_ref().to_path_buf();
        let db_path = clarity_home.join("sessions.db");

        let session_store = match PersistentSessionStore::new(&db_path).await {
            Ok(store) => {
                info!("Persistent session store initialized at {:?}", db_path);
                Arc::new(store)
            }
            Err(e) => {
                warn!(
                    "Failed to create persistent session store at {:?}: {}. Falling back to in-memory store.",
                    db_path, e
                );
                Arc::new(PersistentSessionStore::new_in_memory().map_err(|inner| {
                    warn!("Failed to create in-memory session store: {}", inner);
                    inner
                })?)
            }
        };

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let rollout_config = RolloutConfig {
            clarity_home: clarity_home.clone(),
            sqlite_home: clarity_home.clone(),
            cwd: cwd.clone(),
            model_provider_id: String::new(),
            generate_memories: false,
        };

        let thread_store: Arc<dyn ThreadStore> = match tokio::task::spawn_blocking({
            let config = rollout_config.clone();
            move || {
                LocalThreadStore::new(
                    config.clone(),
                    LocalThreadStore::default_state_db_path(&config),
                )
            }
        })
        .await
        {
            Ok(Ok(store)) => {
                info!("Local thread store initialized at {:?}", clarity_home);
                Arc::new(store)
            }
            Ok(Err(e)) => {
                warn!(
                    "Failed to create local thread store at {:?}: {}. Falling back to in-memory store.",
                    clarity_home, e
                );
                Arc::new(InMemoryThreadStore::new())
            }
            Err(e) => {
                warn!(
                    "Thread store initialization panicked: {}. Falling back to in-memory store.",
                    e
                );
                Arc::new(InMemoryThreadStore::new())
            }
        };
        let thread_manager = ThreadManager::new(thread_store.clone());

        let oauth_service = Arc::new(clarity_llm::auth::OAuthService::new());
        oauth_service.register_kimi_code();

        let role_context_db = clarity_home.join("role_context.db");
        let role_context_store = match RoleContextStore::new(&role_context_db).await {
            Ok(store) => {
                info!("Role context store initialized at {:?}", role_context_db);
                Arc::new(store)
            }
            Err(e) => {
                warn!(
                    "Failed to create persistent role context store at {:?}: {}. Falling back to in-memory store.",
                    role_context_db, e
                );
                Arc::new(RoleContextStore::new_in_memory().map_err(|inner| {
                    warn!("Failed to create in-memory role context store: {}", inner);
                    SessionStoreError::Io(std::io::Error::other(inner.to_string()))
                })?)
            }
        };

        Ok(Self {
            agent: agent.clone(),
            session_store,
            thread_store,
            thread_manager,
            task_manager,
            activity_logger: ActivityLogger::new(),
            started_at: Utc::now(),
            parallel_batches: Arc::new(RwLock::new(HashMap::new())),
            chat_sem: Arc::new(Semaphore::new(32)),
            ws_sem: Arc::new(Semaphore::new(64)),
            oauth_service,
            device_registry: crate::handlers::claw::DeviceRegistry::new(),
            role_context_store,
        })
    }
}

impl crate::handlers::AgentHandle for AppState {
    fn clone_agent(&self) -> clarity_core::agent::Agent {
        (*self.agent).clone()
    }

    fn registry(&self) -> &clarity_core::registry::ToolRegistry {
        self.agent.registry()
    }

    fn set_approval_mode(&self, mode: clarity_core::approval::ApprovalMode) {
        self.agent.set_approval_mode(mode);
    }

    fn approval_mode(&self) -> clarity_core::approval::ApprovalMode {
        self.agent.approval_mode()
    }

    fn set_llm(&self, backend: std::sync::Arc<dyn clarity_core::agent::LlmProvider>) {
        self.agent.set_llm(backend);
    }

    fn set_provider_label<S: Into<String>>(&self, label: S) {
        self.agent.set_provider_label(label);
    }
}

/// 运行双端口服务器
pub async fn run(
    agent: Arc<Agent>,
    task_manager: Arc<BackgroundTaskManager>,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState::new(agent, task_manager).await?);

    // 启动会话清理后台任务
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = cleanup_state.session_store.cleanup_expired(10).await {
                warn!("Session cleanup error: {}", e);
            }
        }
    });

    // 并行批次进度清理（每5分钟清除所有非运行中的批次记录）
    // egui 面板已缓存副本，服务端清理不影响 UI
    let batch_cleanup_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            let mut batches = batch_cleanup_state.parallel_batches.write().await;
            let before = batches.len();
            batches.retain(|_batch_id, progress| {
                let p = progress.lock();
                p.status == clarity_contract::subagent::BatchStatus::Running
            });
            let removed = before - batches.len();
            if removed > 0 {
                info!(
                    "Cleaned up {} stale parallel batch progress records",
                    removed
                );
            }
        }
    });

    // 创建 API 服务器 (端口 18790) — 允许外部访问
    let api_app = {
        let app = create_api_router(state.clone());
        #[cfg(feature = "telemetry-api")]
        {
            use axum::routing::get;
            app.route("/v1/metrics", get(crate::handlers::telemetry::get_metrics))
                .route("/v1/traces", get(crate::handlers::telemetry::get_traces))
                .route(
                    "/v1/events/recent",
                    get(crate::handlers::telemetry::get_recent_events),
                )
        }
        #[cfg(not(feature = "telemetry-api"))]
        {
            app
        }
    };
    let api_addr: SocketAddr = "0.0.0.0:18790".parse()?;

    // 创建 Admin 服务器 (端口 18800) — 仅限本地回环，降低暴露面
    let admin_app = create_admin_router(state.clone());
    let admin_addr: SocketAddr = "127.0.0.1:18800".parse()?;

    info!("📡 API Server listening on http://{}", api_addr);
    info!(
        "🎛️  Admin UI listening on http://{} (localhost only)",
        admin_addr
    );
    if std::env::var("CLARITY_ADMIN_TOKEN").is_ok() {
        info!("🔒 Admin authentication enabled via CLARITY_ADMIN_TOKEN");
    } else {
        warn!("⚠️  CLARITY_ADMIN_TOKEN not set — Admin UI is open to any local process");
    }

    // 创建两个服务器的监听
    let api_listener = tokio::net::TcpListener::bind(api_addr).await?;
    let admin_listener = tokio::net::TcpListener::bind(admin_addr).await?;

    // 同时运行两个服务器，等待关闭信号
    tokio::select! {
        result = axum::serve(api_listener, api_app) => {
            if let Err(e) = result {
                warn!("API server error: {}", e);
            }
        }
        result = axum::serve(admin_listener, admin_app) => {
            if let Err(e) = result {
                warn!("Admin server error: {}", e);
            }
        }
        _ = shutdown_signal() => {
            info!("🛑 Shutdown signal received");
        }
    }

    info!("👋 Clarity Gateway stopped");
    Ok(())
}

/// 嵌入的静态文件（编译时打包进二进制，避免运行时依赖工作目录）
static INDEX_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/index.html"));
static CHAT_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/chat.html"));
static CHAT_V1_HTML: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/chat-v1.html"));

async fn serve_index() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        INDEX_HTML,
    )
}

async fn serve_chat() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        CHAT_HTML,
    )
}

async fn serve_chat_v1() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        CHAT_V1_HTML,
    )
}

/// 创建 API 路由器
pub fn create_api_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(ALLOWED_ORIGINS.to_vec())
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT]);

    let assets_dir =
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/static/assets"));

    Router::new()
        .route("/", get(serve_chat))
        .route("/chat.html", get(serve_chat))
        .route("/chat-v1.html", get(serve_chat_v1))
        .nest_service("/assets", ServeDir::new(assets_dir))
        .route("/health", get(handlers::chat::health_check))
        .route(
            "/v1/chat/completions",
            post(handlers::chat::chat_completions),
        )
        .route(
            "/v1/tasks",
            post(handlers::tasks::create_task).get(handlers::tasks::list_tasks),
        )
        .route(
            "/v1/tasks/:id",
            get(handlers::tasks::get_task).delete(handlers::tasks::cancel_task),
        )
        .route("/v1/parallel", post(handlers::tasks::run_parallel))
        .route(
            "/v1/parallel/:batch_id/status",
            get(handlers::tasks::get_parallel_status),
        )
        .route("/api/files/tree", get(handlers::files::file_tree))
        .route("/api/files/read", get(handlers::files::file_read))
        .route("/api/files/write", post(handlers::files::file_write))
        .route("/api/files/glob", get(handlers::files::file_glob))
        .route(
            "/api/provider",
            post(handlers::admin::admin_switch_provider),
        )
        .route("/api/mcp/servers", get(handlers::mcp::list_mcp_servers))
        .route(
            "/api/mcp/servers/:name",
            get(handlers::mcp::get_mcp_server)
                .post(handlers::mcp::update_mcp_server)
                .delete(handlers::mcp::delete_mcp_server),
        )
        .route(
            "/api/cron/tasks",
            get(handlers::cron::list_cron_tasks).post(handlers::cron::create_cron_task),
        )
        .route(
            "/api/cron/tasks/:id",
            delete(handlers::cron::delete_cron_task),
        )
        .route("/api/search", post(handlers::memory::search_memory))
        .route(
            "/api/v1/claw/devices",
            get(handlers::claw::list_devices).post(handlers::claw::register_device),
        )
        .route(
            "/api/v1/claw/sync",
            post(handlers::claw_sync::sync_role_context),
        )
        .route(
            "/api/v2/threads",
            post(handlers::threads::create_thread).get(handlers::threads::list_threads),
        )
        .route(
            "/api/v2/threads/:id",
            get(handlers::threads::get_thread)
                .patch(handlers::threads::update_thread)
                .delete(handlers::threads::delete_thread),
        )
        .route(
            "/api/v2/threads/:id/archive",
            post(handlers::threads::archive_thread),
        )
        .route(
            "/api/v2/threads/:id/unarchive",
            post(handlers::threads::unarchive_thread),
        )
        .route(
            "/api/v2/threads/:id/fork",
            post(handlers::threads::fork_thread),
        )
        .route(
            "/api/v2/threads/:id/chat",
            post(handlers::thread_chat::thread_chat),
        )
        .route("/ws", get(crate::ws::ws_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Admin 认证中间件
///
/// 如果 `CLARITY_ADMIN_TOKEN` 环境变量未设置，则允许无认证访问（降级兼容）。
/// 如果已设置，则要求请求头携带 `Authorization: Bearer <token>`。
async fn admin_auth(req: Request<Body>, next: Next) -> Response {
    let expected = std::env::var("CLARITY_ADMIN_TOKEN").unwrap_or_default();
    if expected.is_empty() {
        return next.run(req).await;
    }

    let token = req
        .headers()
        .get("authorization")
        .and_then(|v: &HeaderValue| v.to_str().ok())
        .unwrap_or("");

    if token == format!("Bearer {}", expected) {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
    }
}

/// 创建 Admin 路由器
pub fn create_admin_router(state: Arc<AppState>) -> Router {
    let assets_dir =
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/static/assets"));

    Router::new()
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/chat.html", get(serve_chat))
        .route("/ws", get(crate::ws::ws_handler))
        .nest_service("/assets", ServeDir::new(assets_dir))
        .route("/api/stats", get(handlers::admin::admin_stats))
        .route("/api/tools", get(handlers::admin::admin_tools))
        .route("/api/models", get(handlers::admin::admin_models))
        .route(
            "/api/provider",
            post(handlers::admin::admin_switch_provider),
        )
        .route("/api/mesh", get(handlers::admin::admin_mesh_status))
        .route(
            "/api/approval-mode",
            get(handlers::admin::admin_get_approval_mode)
                .post(handlers::admin::admin_set_approval_mode),
        )
        .route(
            "/api/config",
            get(handlers::config::admin_get_config).post(handlers::config::admin_set_config),
        )
        .route(
            "/api/config/health",
            get(handlers::admin::admin_config_health),
        )
        .route(
            "/api/config/validate",
            post(handlers::admin::admin_validate_config),
        )
        .route(
            "/api/auth/device",
            post(handlers::config::admin_start_oauth),
        )
        .route("/api/auth/poll", post(handlers::config::admin_poll_oauth))
        .route("/api/sessions", get(handlers::sessions::list_sessions))
        .route(
            "/api/sessions/:id",
            get(handlers::sessions::get_session).delete(handlers::sessions::delete_session),
        )
        .layer(middleware::from_fn(admin_auth))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// 优雅关闭信号处理
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            warn!("Failed to install Ctrl+C handler: {}", e);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(e) => {
                warn!("Failed to install terminate signal handler: {}", e);
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown...");
}
