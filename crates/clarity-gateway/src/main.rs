use tracing::{info, warn};

mod channels;
mod handlers;
mod server;
mod session;
mod ws;

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "clarity_gateway=debug,tower_http=debug".into()),
        )
        .init();

    info!("🚀 Clarity Gateway starting...");

    // 启动服务器
    if let Err(e) = server::run().await {
        warn!("Server error: {}", e);
    }
}
