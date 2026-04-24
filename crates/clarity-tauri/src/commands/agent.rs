//! Tauri Commands for Agent operations.
//!
//! These functions are exposed to the frontend via Tauri's IPC mechanism.
//! They run on the Rust side and can access the full clarity-core runtime.

use tauri::State;
use crate::AppState;

/// A simple greeting command to verify the Tauri bridge is working.
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust.", name)
}

/// Return the current application version.
#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get the number of active agent sessions.
#[tauri::command]
pub fn get_agent_count(state: State<AppState>) -> usize {
    state
        .agent_count
        .load(std::sync::atomic::Ordering::Relaxed)
}
