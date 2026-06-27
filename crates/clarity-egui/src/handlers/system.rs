use std::time::Instant;

use crate::stores::UiStore;
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
