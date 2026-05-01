use crate::stores::{SettingsStore, UiStore};
use crate::ui::types::ToastLevel;

pub fn on_provider_test_result(
    settings_store: &mut SettingsStore,
    ui_store: &mut UiStore,
    provider_id: String,
    success: bool,
    error: Option<String>,
) {
    settings_store.testing_provider = None;
    if success {
        crate::handlers::system::push_toast(
            ui_store,
            format!("{}: Connection OK", provider_id),
            ToastLevel::Info,
        );
    } else {
        crate::handlers::system::push_toast(
            ui_store,
            format!(
                "{}: {}",
                provider_id,
                error.unwrap_or_else(|| "Connection failed".into())
            ),
            ToastLevel::Error,
        );
    }
}

pub fn on_provider_model_list(
    settings_store: &mut SettingsStore,
    ui_store: &mut UiStore,
    provider_id: String,
    models: Vec<String>,
) {
    settings_store.refreshing_provider = None;
    let count = models.len();
    settings_store
        .provider_registry
        .update_models(&provider_id, models);
    if count > 0 {
        crate::handlers::system::push_toast(
            ui_store,
            format!("{}: {} models found", provider_id, count),
            ToastLevel::Info,
        );
    } else {
        crate::handlers::system::push_toast(
            ui_store,
            format!("{}: No models returned", provider_id),
            ToastLevel::Warn,
        );
    }
}
