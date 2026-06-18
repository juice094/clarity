#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)
)]
//! clarity-claw —— 联邦运行时协调器 + 系统托盘常驻应用
//!
//! Entry point: initializes the Claw Coordinator, registers federal nodes,
//! then starts the system tray event loop.

use clarity_claw::coordinator::Coordinator;
use clarity_claw::nodes::core::CoreNode;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    clarity_core::logging::init_with_default("clarity_claw=info");

    tracing::info!("🐾 Clarity Claw starting...");

    // ------------------------------------------------------------------
    // 0. Single-instance guard
    // ------------------------------------------------------------------
    if !clarity_claw::tray::ensure_single_instance() {
        tracing::warn!("Another Clarity Claw instance is already running. Exiting.");
        return Ok(());
    }

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
    // 3. Register this instance as a Claw device with the Gateway
    // ------------------------------------------------------------------
    let gateway_url = clarity_claw::resolve_gateway_url();
    let device_id = match clarity_claw::register_device(&gateway_url).await {
        Ok(id) => {
            tracing::info!("Registered as device '{}'", id);
            Some(id)
        }
        Err(e) => {
            tracing::warn!(
                "Failed to register device with Gateway: {}. Will retry on heartbeat.",
                e
            );
            None
        }
    };

    // Spawn periodic heartbeat task (every 30 s). The heartbeat also acts
    // as a lazy registration: if the initial register_device failed, the
    // first heartbeat retries the registration (same POST endpoint).
    if let Some(ref did) = device_id {
        let gw = gateway_url.clone();
        let did = did.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                if let Err(e) = clarity_claw::send_heartbeat(&gw, &did).await {
                    tracing::warn!(
                        device_id = %did,
                        error = %e,
                        "Heartbeat failed — device may appear offline"
                    );
                }
            }
        });
    }

    // ------------------------------------------------------------------
    // 4. Start the system tray (blocks until user selects Quit)
    // ------------------------------------------------------------------
    clarity_claw::tray::run()?;

    tracing::info!("🐾 Clarity Claw shutting down.");
    Ok(())
}
