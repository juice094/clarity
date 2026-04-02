use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::handlers;
use crate::session::SessionManager;

/// 应用状态
pub struct AppState {
    pub session_manager: Arc<RwLock<SessionManager>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            session_manager: Arc::new(RwLock::new(SessionManager::new())),
        }
    }
}

/// 运行双端口服务器
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState::new());

    // 创建 API 服务器 (端口 18790)
    let api_app = create_api_router(state.clone());
    let api_addr: SocketAddr = "0.0.0.0:18790".parse()?;

    // 创建 Admin 服务器 (端口 18800)
    let admin_app = create_admin_router(state.clone());
    let admin_addr: SocketAddr = "0.0.0.0:18800".parse()?;

    info!("📡 API Server listening on http://{}", api_addr);
    info!("🎛️  Admin UI listening on http://{}", admin_addr);

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

/// 创建 API 路由器
fn create_api_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(handlers::health_check))
        .route("/v1/chat/completions", post(handlers::chat_completions))
        .route("/ws", get(crate::ws::ws_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// 创建 Admin 路由器
fn create_admin_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/stats", get(handlers::admin_stats))
        .route("/api/tools", get(handlers::admin_tools))
        .nest_service("/", ServeDir::new("static"))
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
