//! Tauri commands for LSP (Language Server Protocol) proxy layer.

use crate::lsp_manager::LspServerInfo;

/// Start an LSP server process.
/// Returns a process handle ID that can be used for subsequent operations.
#[tauri::command]
pub async fn lsp_start(
    state: tauri::State<'_, crate::AppState>,
    server_path: String,
    args: Vec<String>,
    root_path: String,
) -> Result<String, String> {
    state
        .lsp_manager
        .start(server_path, args, root_path)
        .await
}

/// Send an LSP JSON-RPC message to the server.
#[tauri::command]
pub async fn lsp_send(
    state: tauri::State<'_, crate::AppState>,
    process_id: String,
    message: String,
) -> Result<(), String> {
    state.lsp_manager.send(process_id, message).await
}

/// Read the next LSP JSON-RPC message from the server.
#[tauri::command]
pub async fn lsp_recv(
    state: tauri::State<'_, crate::AppState>,
    process_id: String,
) -> Result<Option<String>, String> {
    state.lsp_manager.recv(process_id).await
}

/// Stop an LSP server process.
#[tauri::command]
pub async fn lsp_stop(
    state: tauri::State<'_, crate::AppState>,
    process_id: String,
) -> Result<(), String> {
    state.lsp_manager.stop(process_id).await
}

/// List running LSP servers.
#[tauri::command]
pub async fn lsp_list(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<LspServerInfo>, String> {
    Ok(state.lsp_manager.list().await)
}
