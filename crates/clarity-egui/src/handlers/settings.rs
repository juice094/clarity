use crate::stores::{SettingsStore, UiStore};
use crate::ui::types::ToastLevel;

/// Handles the provider test result event.
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

/// Handles the provider model list event.
pub fn on_provider_model_list(
    settings_store: &mut SettingsStore,
    ui_store: &mut UiStore,
    provider_id: String,
    result: Result<Vec<String>, String>,
) {
    match &result {
        Ok(models) => {
            let count = models.len();
            settings_store
                .provider_registry
                .update_models(&provider_id, models.clone());
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
        Err(message) => {
            let locale = crate::i18n::Locale::from_code(
                settings_store
                    .settings_edit
                    .language
                    .as_deref()
                    .unwrap_or("en"),
            );
            crate::handlers::system::push_toast(
                ui_store,
                format!(
                    "{}: {}: {}",
                    provider_id,
                    locale.t("Model refresh failed"),
                    message
                ),
                ToastLevel::Error,
            );
        }
    }
    // Feed the outcome back into the ViewModel so the settings panel renders
    // the Ready badge / inline error for this provider.
    settings_store
        .settings_vm
        .apply_refresh_result(&provider_id, result);
}
