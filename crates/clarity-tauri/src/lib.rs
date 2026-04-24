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
    /// Placeholder for the Agent instance pool.
    /// In production this will hold multiple Agent instances (one per tab).
    pub agent_count: std::sync::atomic::AtomicUsize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            agent_count: std::sync::atomic::AtomicUsize::new(0),
        }
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
