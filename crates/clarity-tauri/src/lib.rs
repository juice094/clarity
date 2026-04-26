//! Clarity Tauri GUI — Desktop + Mobile entry point.
//!
//! This crate provides a Tauri 2 application that wraps the clarity-core
//! runtime. The frontend is a React/Vite app that communicates with the
//! Rust backend via Tauri Commands (IPC).

pub mod commands;
pub mod lsp_manager;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tracing::info;

/// Tracks which provider and model path the agent is currently bound to.
#[derive(Clone, Debug)]
pub struct LlmBinding {
    pub provider: String,
    pub local_model_path: String,
}

/// Application state shared across Tauri commands.
pub struct AppState {
    /// The Agent instance driving conversations.
    pub agent: clarity_core::Agent,
    /// LSP server process manager.
    pub lsp_manager: lsp_manager::LspManager,
    /// Current LLM binding so we can detect provider/path drift and reload.
    pub llm_binding: Mutex<Option<LlmBinding>>,
    /// Cached network reachability, updated by the background monitor.
    pub network_available: AtomicBool,
    /// Prevents concurrent LLM loading tasks from wasting memory / CPU.
    pub llm_load_lock: tokio::sync::Mutex<()>,
    /// In-memory cache of GUI settings to avoid disk I/O on every request.
    pub cached_settings: Mutex<crate::commands::settings::GuiSettings>,
    /// Caches the last prewarm error so the frontend can query it after mount.
    pub prewarm_error: Mutex<Option<String>>,
    /// Whether `init_async` has completed successfully.
    pub initialized: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        let registry = clarity_core::ToolRegistry::with_builtin_tools();
        let agent = clarity_core::Agent::new(registry);
        Self {
            agent,
            lsp_manager: lsp_manager::LspManager::new(),
            llm_binding: Mutex::new(None),
            network_available: AtomicBool::new(true),
            llm_load_lock: tokio::sync::Mutex::new(()),
            cached_settings: Mutex::new(crate::commands::settings::GuiSettings::load()),
            prewarm_error: Mutex::new(None),
            initialized: AtomicBool::new(false),
        }
    }
}

/// Simple TCP probe to detect basic internet connectivity.
/// `probe` is a "host:port" string (e.g. "1.1.1.1:443").
async fn check_network(probe: &str) -> bool {
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            tokio::net::TcpStream::connect(probe),
        )
        .await,
        Ok(Ok(_))
    )
}

/// Background task that probes network every 30s and rebinds the LLM
/// when going offline (fallback → local) or coming back online.
///
/// Debouncing (threshold = 2 consecutive probes) prevents spurious
/// toggles during brief network hiccups.
async fn network_monitor(handle: tauri::AppHandle) {
    const THRESHOLD: u32 = 2;
    let mut consecutive_failures: u32 = 0;
    let mut consecutive_successes: u32 = 0;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let state = handle.state::<AppState>();
        let probe = {
            let guard = state.cached_settings.lock().unwrap();
            guard
                .network_probe_url
                .clone()
                .unwrap_or_else(|| "1.1.1.1:443".to_string())
        };
        let available = check_network(&probe).await;
        let current = state.network_available.load(Ordering::Relaxed);

        if available {
            consecutive_failures = 0;
            consecutive_successes = consecutive_successes.saturating_add(1);
        } else {
            consecutive_successes = 0;
            consecutive_failures = consecutive_failures.saturating_add(1);
        }

        // Determine whether we should flip the cached state
        let should_flip = (!available && current && consecutive_failures >= THRESHOLD)
            || (available && !current && consecutive_successes >= THRESHOLD);

        if should_flip {
            let prev = state.network_available.swap(available, Ordering::Relaxed);
            tracing::info!("Network status changed: available={}", available);
            let _ = handle.emit(
                "network:status",
                serde_json::json!({ "available": available }),
            );

            if !available && prev {
                tracing::info!("Network lost; triggering offline fallback");
                if let Err(e) = commands::agent::ensure_llm(&state).await {
                    tracing::warn!("Offline fallback failed: {}", e);
                    let _ = handle.emit(
                        "llm:fallback_error",
                        serde_json::json!({ "message": e, "context": "offline_fallback" }),
                    );
                } else {
                    let _ = handle.emit(
                        "llm:fallback",
                        serde_json::json!({ "fallback": true, "reason": "offline" }),
                    );
                }
            } else if available && !prev {
                tracing::info!("Network restored; reverting to preferred provider");
                if let Err(e) = commands::agent::ensure_llm(&state).await {
                    tracing::warn!("Online restore failed: {}", e);
                    let _ = handle.emit(
                        "llm:fallback_error",
                        serde_json::json!({ "message": e, "context": "online_restore" }),
                    );
                } else {
                    let _ = handle.emit(
                        "llm:fallback",
                        serde_json::json!({ "fallback": false, "reason": "online" }),
                    );
                }
            }
        }
    }
}

/// Prewarm the LLM on startup so the first user request is not blocked by
/// model loading. Runs after an initial network probe.
async fn prewarm_llm(handle: &tauri::AppHandle) -> Result<(), String> {
    let state = handle.state::<AppState>();
    commands::agent::ensure_llm(&state).await
}

/// Run the Tauri application.
///
/// This function is called from `main.rs` and sets up the Tauri app
/// with all commands, plugins, and event handlers.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .setup(|app| {
            info!("Clarity Tauri app starting");

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let probe = {
                    let guard = state.cached_settings.lock().unwrap();
                    guard
                        .network_probe_url
                        .clone()
                        .unwrap_or_else(|| "1.1.1.1:443".to_string())
                };

                // Probe network before prewarm so ensure_llm sees the real state
                let available = check_network(&probe).await;
                state.network_available.store(available, Ordering::Relaxed);

                if let Err(e) = prewarm_llm(&handle).await {
                    tracing::warn!("LLM prewarm failed: {}", e);
                    let state = handle.state::<AppState>();
                    let mut guard = state.prewarm_error.lock().unwrap();
                    *guard = Some(e.clone());
                    let _ = handle.emit("llm:config_error", serde_json::json!({ "message": e }));
                }
                network_monitor(handle).await;
            });

            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agent::greet,
            commands::agent::get_app_version,
            commands::agent::agent_run,
            commands::agent::agent_run_streaming,
            commands::agent::agent_interrupt,
            commands::agent::get_agent_status,
            commands::agent::get_prewarm_status,
            commands::computer::computer_screenshot,
            commands::computer::computer_click,
            commands::computer::computer_type,
            commands::computer::computer_scroll,
            commands::computer::computer_check_bridge,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::reload_llm,
            commands::ftue::get_launch_status,
            commands::settings::set_approval_mode,
            commands::settings::get_available_models,
            commands::settings::get_local_models,
            commands::settings::get_approval_modes,
            commands::session::list_sessions,
            commands::session::load_session,
            commands::session::save_session,
            commands::session::delete_session,
            commands::task::list_tasks,
            commands::task::cancel_task,
            commands::task::create_task,
            commands::task::complete_task,
            commands::file::get_file_tree,
            commands::file::read_file,
            commands::diff::compute_diff,
            commands::lsp::lsp_start,
            commands::lsp::lsp_send,
            commands::lsp::lsp_recv,
            commands::lsp::lsp_stop,
            commands::lsp::lsp_list,
            commands::update::check_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
