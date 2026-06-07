//! Clarity Tauri App — Kimi-style frontend for clarity-core.
//!
//! Architecture:
//!   - Tauri commands expose clarity-core APIs to the Vue frontend.
//!   - State is managed via Tauri managed state.
//!   - Frontend: Vue 3 + Vite, consuming the Kimi ConversationView CSS/JS.

use std::sync::Arc;
use tokio::sync::Mutex;

/// Application state shared across Tauri commands.
struct AppState {
    // TODO: wire clarity-core runtime
    _placeholder: (),
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn list_sessions(
    _state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<String>, String> {
    // TODO: integrate with clarity-core session store
    Ok(vec!["session-1".into(), "session-2".into()])
}

fn main() {
    let state = Arc::new(Mutex::new(AppState {
        _placeholder: (),
    }));

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet, list_sessions])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
