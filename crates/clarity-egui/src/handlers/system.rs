use std::time::Instant;

use crate::stores::{ChatStore, UiStore};
use crate::ui::types::{Toast, ToastLevel};
use clarity_core::approval::ApprovalRuntime;

/// Adds a transient toast notification.
pub fn push_toast(ui_store: &mut UiStore, message: impl Into<String>, level: ToastLevel) {
    ui_store.toasts.push(Toast {
        message: message.into(),
        level,
        created_at: Instant::now(),
    });
    // Keep max 5 toasts
    if ui_store.toasts.len() > 5 {
        ui_store.toasts.remove(0);
    }
}

/// Handles the fallback event.
pub fn on_fallback(ui_store: &mut UiStore, fallback: bool, reason: String) {
    let msg = if fallback {
        format!(
            "Network probe failed ({}). External provider will still be tried.",
            reason
        )
    } else {
        format!("Network probe restored ({})", reason)
    };
    push_toast(ui_store, &msg, ToastLevel::Warn);
    ui_store.network_banner = if fallback { Some(msg) } else { None };
}

/// Handles the step begin event.
///
/// Displays a transient status message in the chat area so the user can see
/// which tool is currently being executed. The message is cleared automatically
/// when real content arrives, the turn ends, or an error occurs.
pub fn on_step_begin(chat_store: &mut ChatStore, tool_name: String) {
    tracing::info!("Step begin: {}", tool_name);
    chat_store.status_message = Some(format!("🔧 正在执行: {}…", tool_name));
}

/// Handles the resolve approval event.
pub fn on_resolve_approval(
    approval_runtime: std::sync::Arc<clarity_core::approval::InMemoryApprovalRuntime>,
    runtime: &tokio::runtime::Runtime,
    req_id: String,
    response: clarity_core::approval::ApprovalResponse,
) {
    runtime.spawn(async move {
        if let Err(e) = approval_runtime.resolve(&req_id, response).await {
            tracing::warn!("Approval resolve failed for {}: {}", req_id, e);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::ChatStore;

    #[test]
    fn step_begin_sets_status_message() {
        let mut chat_store = ChatStore::default();
        on_step_begin(&mut chat_store, "read_file".to_string());
        assert_eq!(
            chat_store.status_message,
            Some("🔧 正在执行: read_file…".to_string())
        );
    }

    #[test]
    fn step_begin_overwrites_existing_status() {
        let mut chat_store = ChatStore {
            status_message: Some("Old status".to_string()),
            ..Default::default()
        };
        on_step_begin(&mut chat_store, "grep".to_string());
        assert_eq!(
            chat_store.status_message,
            Some("🔧 正在执行: grep…".to_string())
        );
    }
}
