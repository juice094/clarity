use axum::{
    body::Body,
    http::{header, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::{RwLock, Semaphore};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::handlers;
use crate::session_store::PersistentSessionStore;
use chrono::{DateTime, Utc};
use clarity_core::activity::ActivityLogger;
use clarity_core::agent::Agent;
use clarity_core::background::BackgroundTaskManager;
use clarity_contract::subagent::BatchProgress;
use std::collections::HashMap;
use parking_lot::Mutex;

/// 应用状态
pub struct AppState {
    pub agent: Arc<Agent>,
    pub session_store: Arc<PersistentSessionStore>,
    pub task_manager: Arc<BackgroundTaskManager>,
    pub activity_logger: ActivityLogger,
    pub started_at: DateTime<Utc>,
    /// Registry of in-flight parallel batch progress for UI polling.
    pub parallel_batches: Arc<RwLock<HashMap<String, Arc<Mutex<BatchProgress>>>>>,
    /// Concurrency limit for /v1/chat/completions to prevent unbounded spawn.
    pub chat_sem: Arc<Semaphore>,
}

impl AppState {
    pub async fn new(agent: Arc<Agent>, task_manager: Arc<BackgroundTaskManager>) -> Self {
        let db_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(".clarity")
            .join("sessions.db");

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
                Arc::new(
                    PersistentSessionStore::new_in_memory()
                        .expect("Failed to create in-memory session store"),
                )
            }
        };

        Self {
            agent: agent.clone(),
            session_store,
            task_manager,
            activity_logger: ActivityLogger::new(),
            started_at: Utc::now(),
            parallel_batches: Arc::new(RwLock::new(HashMap::new())),
            chat_sem: Arc::new(Semaphore::new(32)),
        }
    }
}

/// 运行双端口服务器
pub async fn run(
    agent: Arc<Agent>,
    task_manager: Arc<BackgroundTaskManager>,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState::new(agent, task_manager).await);

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
    let api_app = create_api_router(state.clone());
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
        .allow_origin([
            "http://localhost:3000".parse::<HeaderValue>().unwrap(),
            "http://localhost:5173".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:3000".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:5173".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:18800".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT]);

    let assets_dir =
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/static/assets"));

    Router::new()
        .route("/", get(serve_chat))
        .route("/chat.html", get(serve_chat))
        .route("/chat-v1.html", get(serve_chat_v1))
        .nest_service("/assets", ServeDir::new(assets_dir))
        .route("/health", get(handlers::health_check))
        .route("/v1/chat/completions", post(handlers::chat_completions))
        .route(
            "/v1/tasks",
            post(handlers::create_task).get(handlers::list_tasks),
        )
        .route(
            "/v1/tasks/:id",
            get(handlers::get_task).delete(handlers::cancel_task),
        )
        .route("/v1/parallel", post(handlers::run_parallel))
        .route(
            "/v1/parallel/:batch_id/status",
            get(handlers::get_parallel_status),
        )
        .route("/api/files/tree", get(handlers::file_tree))
        .route("/api/files/read", get(handlers::file_read))
        .route("/api/files/write", post(handlers::file_write))
        .route("/api/files/glob", get(handlers::file_glob))
        .route("/api/provider", post(handlers::admin_switch_provider))
        .route("/api/mcp/servers", get(handlers::list_mcp_servers))
        .route(
            "/api/mcp/servers/:name",
            get(handlers::get_mcp_server)
                .post(handlers::update_mcp_server)
                .delete(handlers::delete_mcp_server),
        )
        .route(
            "/api/cron/tasks",
            get(handlers::list_cron_tasks).post(handlers::create_cron_task),
        )
        .route("/api/cron/tasks/:id", delete(handlers::delete_cron_task))
        .route("/api/search", post(handlers::search_memory))
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
    Router::new()
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/chat.html", get(serve_chat))
        .route("/api/stats", get(handlers::admin_stats))
        .route("/api/tools", get(handlers::admin_tools))
        .route("/api/models", get(handlers::admin_models))
        .route("/api/provider", post(handlers::admin_switch_provider))
        .route("/api/mesh", get(handlers::admin_mesh_status))
        .route(
            "/api/approval-mode",
            get(handlers::admin_get_approval_mode).post(handlers::admin_set_approval_mode),
        )
        .route(
            "/api/config",
            get(handlers::admin_get_config).post(handlers::admin_set_config),
        )
        .route("/api/sessions", get(handlers::list_sessions))
        .route(
            "/api/sessions/:id",
            get(handlers::get_session).delete(handlers::delete_session),
        )
        .layer(middleware::from_fn(admin_auth))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// 优雅关闭信号处理
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown...");
}
