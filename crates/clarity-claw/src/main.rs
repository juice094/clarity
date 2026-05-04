//! clarity-claw —— 联邦运行时协调器 + 系统托盘常驻应用
//!
//! Entry point: initializes the Claw Coordinator, registers federal nodes,
//! then starts the system tray event loop.

use clarity_claw::coordinator::Coordinator;
use clarity_claw::nodes::core::CoreNode;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("clarity_claw=info")
        .init();

    tracing::info!("🐾 Clarity Claw starting...");

    // ------------------------------------------------------------------
    // 1. Initialize the Coordinator
    // ------------------------------------------------------------------
    let mut coordinator = Coordinator::new();

    // ------------------------------------------------------------------
    // 2. Create and register the Core node
    // ------------------------------------------------------------------
    let core_node = Arc::new(CoreNode::new());
    coordinator.register_node(core_node);
    tracing::info!(
        "CoreNode registered — {} node(s) active",
        coordinator.node_count()
    );

    // ------------------------------------------------------------------
    // 3. Start the system tray (blocks until user selects Quit)
    // ------------------------------------------------------------------
    clarity_claw::tray::run()?;

    tracing::info!("🐾 Clarity Claw shutting down.");
    Ok(())
}
