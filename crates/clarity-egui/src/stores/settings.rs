//! Settings Store
//!
//! provider config, model selection, add-provider form

use clarity_core::view_models::settings::SettingsViewModel;

/// State variants for kimi code login.
#[derive(Clone, Debug)]
pub enum KimiCodeLoginState {
    Idle,
    Requesting,
    Waiting {
        user_code: String,
        #[allow(dead_code)]
        verification_uri: String,
        verification_uri_complete: String,
    },
    Polling,
    Success,
    Error(String),
}

/// Holds settings UI state.
pub struct SettingsStore {
    pub settings_edit: crate::settings::GuiSettings,
    #[allow(dead_code)]
    pub settings_vm: SettingsViewModel,
    pub settings_active_tab: u8,
    pub show_add_provider: bool,
    pub add_provider_name: String,
    pub add_provider_url: String,
    pub add_provider_key: String,
    pub add_provider_format: String,
    pub provider_registry: crate::provider::ProviderRegistry,
    pub testing_provider: Option<String>,
    pub refreshing_provider: Option<String>,
    /// Current state of the Kimi Code OAuth login flow.
    pub kimi_code_login_state: KimiCodeLoginState,
}
