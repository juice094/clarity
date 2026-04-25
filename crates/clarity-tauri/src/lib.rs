//! Clarity Tauri GUI — Desktop + Mobile entry point.
//!
//! This crate provides a Tauri 2 application that wraps the clarity-core
//! runtime. The frontend is a React/Vite app that communicates with the
//! Rust backend via Tauri Commands (IPC).

pub mod commands;

use tauri::Manager;
use tracing::info;

/// Application state shared across Tauri commands.
pub struct AppState {
    /// The Agent instance driving conversations.
    pub agent: clarity_core::Agent,
}

impl Default for AppState {
    fn default() -> Self {
        let registry = clarity_core::ToolRegistry::with_builtin_tools();
        let agent = clarity_core::Agent::new(registry);
        Self { agent }
    }
}

/// Run the Tauri application.
///
/// This function is called from `main.rs` and sets up the Tauri app
/// with all commands, plugins, and event handlers.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .manage(AppState::default())
        .setup(|app| {
            info!("Clarity Tauri app starting");
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
            commands::computer::computer_screenshot,
            commands::computer::computer_click,
            commands::computer::computer_type,
            commands::computer::computer_scroll,
            commands::computer::computer_check_bridge,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::set_approval_mode,
            commands::settings::get_available_models,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
