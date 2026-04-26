use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize, Clone, Debug)]
pub struct LaunchStatus {
    pub has_local_model: bool,
    pub network_available: bool,
    pub configured: bool,
    pub needs_onboarding: bool,
    pub first_launch: bool,
}

/// Check whether a settings file already exists on disk.
fn settings_file_exists() -> bool {
    PathBuf::from(&crate::commands::settings::GuiSettings::config_path()).is_file()
}

#[tauri::command]
pub async fn get_launch_status(state: tauri::State<'_, crate::AppState>) -> Result<LaunchStatus, String> {
    let settings = {
        let guard = state.cached_settings.lock().unwrap();
        guard.clone()
    };

    let local_models = crate::commands::settings::scan_local_models();
    let has_local_model = local_models
        .iter()
        .any(|(_, name)| !name.starts_with("No models"));

    let network_available = state
        .network_available
        .load(std::sync::atomic::Ordering::Relaxed);

    // "Configured" means the user has chosen a provider that is actually usable
    // right now (local model present, or remote provider + network + api_key).
    let has_api_key = settings
        .api_key
        .as_ref()
        .map(|k| !k.is_empty())
        .unwrap_or(false)
        || std::env::var("KIMI_API_KEY").is_ok()
        || std::env::var("KIMI_CODE_API_KEY").is_ok()
        || std::env::var("OPENAI_API_KEY").is_ok()
        || std::env::var("ANTHROPIC_AUTH_TOKEN").is_ok()
        || std::env::var("DEEPSEEK_API_KEY").is_ok();

    let configured = match settings.provider.as_str() {
        "local" => has_local_model,
        _ => network_available && !settings.model.is_empty() && has_api_key,
    };

    let first_launch = !settings_file_exists();

    // Auto-configure local provider on first launch if a local model is present.
    // This gives the user an immediate "ready to chat" experience without
    // forcing them through the Settings panel.
    let (configured, needs_onboarding) = if first_launch && has_local_model {
        let first_model = local_models
            .first()
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        let mut new_settings = settings.clone();
        new_settings.provider = "local".to_string();
        new_settings.model = first_model.clone();
        new_settings.local_model_path = local_models
            .first()
            .map(|(path, _)| path.clone());
        if let Err(e) = new_settings.save() {
            tracing::warn!("Failed to auto-save local settings on first launch: {}", e);
        } else {
            let mut guard = state.cached_settings.lock().unwrap();
            *guard = new_settings.clone();
            tracing::info!(
                "Auto-configured local provider on first launch: model={}",
                first_model
            );
        }
        (true, false)
    } else {
        // Onboarding is needed when it's the first launch OR when no usable
        // model/provider is configured.
        (configured, first_launch || !configured)
    };

    Ok(LaunchStatus {
        has_local_model,
        network_available,
        configured,
        needs_onboarding,
        first_launch,
    })
}
