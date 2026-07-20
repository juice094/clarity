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
        dismissed_at: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_toast_adds_to_list() {
        let mut store = UiStore::default();
        push_toast(&mut store, "test message", ToastLevel::Info);
        assert_eq!(store.toasts.len(), 1);
        assert_eq!(store.toasts[0].message, "test message");
        assert_eq!(store.toasts[0].level, ToastLevel::Info);
    }

    #[test]
    fn push_toast_caps_at_five() {
        let mut store = UiStore::default();
        for i in 0..10 {
            push_toast(&mut store, format!("msg {}", i), ToastLevel::Info);
        }
        assert_eq!(store.toasts.len(), 5);
        // Oldest toasts (0-4) should be evicted.
        assert_eq!(store.toasts[0].message, "msg 5");
        assert_eq!(store.toasts[4].message, "msg 9");
    }

    #[test]
    fn push_toast_preserves_order() {
        let mut store = UiStore::default();
        push_toast(&mut store, "first", ToastLevel::Info);
        push_toast(&mut store, "second", ToastLevel::Warn);
        push_toast(&mut store, "third", ToastLevel::Error);
        assert_eq!(store.toasts[0].message, "first");
        assert_eq!(store.toasts[1].message, "second");
        assert_eq!(store.toasts[2].message, "third");
        assert_eq!(store.toasts[0].level, ToastLevel::Info);
        assert_eq!(store.toasts[1].level, ToastLevel::Warn);
        assert_eq!(store.toasts[2].level, ToastLevel::Error);
    }

    #[test]
    fn on_fallback_sets_network_banner() {
        let mut store = UiStore::default();
        on_fallback(&mut store, true, "connection refused".into());
        assert!(store.network_banner.is_some());
        assert!(store.network_banner.unwrap().contains("connection refused"));
        assert_eq!(store.toasts.len(), 1);
    }

    #[test]
    fn on_fallback_clears_network_banner_on_restore() {
        let mut store = UiStore::default();
        // First set fallback
        on_fallback(&mut store, true, "timeout".into());
        assert!(store.network_banner.is_some());
        // Then restore
        on_fallback(&mut store, false, "reachable".into());
        assert!(store.network_banner.is_none());
        assert!(store.toasts[1].message.contains("restored"));
    }

    #[test]
    fn push_toast_accepts_str_and_string() {
        let mut store = UiStore::default();
        push_toast(&mut store, "&str message", ToastLevel::Info);
        push_toast(&mut store, String::from("String message"), ToastLevel::Warn);
        assert_eq!(store.toasts[0].message, "&str message");
        assert_eq!(store.toasts[1].message, "String message");
    }
}
