//! Provider Schema — TOML-driven, configurable LLM provider definitions.
//!
//! # Design
//!
//! Providers are defined in TOML files under `~/.config/clarity/providers/`.
//! Each file can contain one or more provider definitions:
//!
//! ```toml
//! [provider.openai]
//! display_name = "OpenAI"
//! base_url = "https://api.openai.com/v1"
//! api_format = "openai-completions"
//! api_key_ref = "${env:OPENAI_API_KEY}"
//!
//! [provider.my-custom]
//! display_name = "My Custom LLM"
//! base_url = "https://my-llm.example.com/v1"
//! api_format = "openai-completions"
//! api_key_ref = ""
//! models = ["my-model-1", "my-model-2"]
//! ```
//!
//! The system provider definitions (built-in) are merged with user-defined
//! custom providers. User-defined providers take precedence on name conflict.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Supported API format (i.e. the wire protocol expected by the provider).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum ApiFormat {
    /// OpenAI-compatible chat completions endpoint.
    #[default]
    OpenaiCompletions,
    /// Anthropic Messages API.
    AnthropicMessages,
    /// Native Kimi API.
    Kimi,
    /// DeepSeek device-login native API (prompt-guided tool calling enabled).
    DeepSeekDevice,
}

impl ApiFormat {
    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiFormat::OpenaiCompletions => "openai-completions",
            ApiFormat::AnthropicMessages => "anthropic-messages",
            ApiFormat::Kimi => "kimi",
            ApiFormat::DeepSeekDevice => "deepseek-device",
        }
    }

    /// Returns the runtime API format expected by [`clarity_llm::runtime::build_provider`].
    pub fn runtime_api_format(&self) -> &'static str {
        match self {
            ApiFormat::OpenaiCompletions | ApiFormat::Kimi => "openai_chat",
            ApiFormat::AnthropicMessages => "anthropic_messages",
            ApiFormat::DeepSeekDevice => "deepseek_device",
        }
    }

    /// Parses from a string.
    pub fn from_str(s: &str) -> Self {
        match s {
            "anthropic-messages" => Self::AnthropicMessages,
            "kimi" => Self::Kimi,
            "deepseek-device" => Self::DeepSeekDevice,
            _ => Self::OpenaiCompletions,
        }
    }
}

/// Authentication type for a provider.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AuthType {
    /// Standard API key authentication.
    #[default]
    ApiKey,
    /// OAuth 2.0 device flow or authorization code flow.
    OAuth,
    /// No authentication required (e.g. local Ollama).
    None,
}

impl AuthType {
    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthType::ApiKey => "api-key",
            AuthType::OAuth => "oauth",
            AuthType::None => "none",
        }
    }
}

/// Authentication mode for providers that support both token and password login.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AuthMode {
    /// Authenticate with an explicit device / API token.
    #[default]
    Token,
    /// Authenticate with mobile number + password.
    Password,
}

impl AuthMode {
    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthMode::Token => "token",
            AuthMode::Password => "password",
        }
    }

    /// Parses from a string, defaulting to `Token`.
    pub fn from_str(s: &str) -> Self {
        match s {
            "password" => Self::Password,
            _ => Self::Token,
        }
    }

    /// Returns true when this is password mode.
    pub fn is_password(&self) -> bool {
        matches!(self, Self::Password)
    }
}

/// A single provider definition.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProviderDefinition {
    /// Internal identifier (e.g. "openai", "my-custom").
    /// Used as the TOML section key and the `provider` field value.
    #[serde(skip)]
    pub id: String,

    /// Human-readable name (e.g. "OpenAI", "My Custom LLM").
    #[serde(default)]
    pub display_name: String,

    /// Base URL for API requests.
    pub base_url: String,

    /// API format / wire protocol.
    #[serde(default)]
    pub api_format: ApiFormat,

    /// Authentication type.
    #[serde(default)]
    pub auth_type: AuthType,

    /// Reference to API key — either literal or `${env:VAR_NAME}` syntax.
    /// For OAuth providers this is usually empty; the token is read from
    /// the token store via `auth_token_key`.
    #[serde(default)]
    pub api_key_ref: String,

    /// OAuth token storage key. When `auth_type` is `OAuth`, this key is used
    /// to look up the persisted access token in the global token store.
    /// Defaults to the provider `id` when empty.
    #[serde(default)]
    pub auth_token_key: String,

    /// Optional list of known models from this provider.
    /// When empty, the UI should either fetch dynamically or accept free-text input.
    #[serde(default)]
    pub models: Vec<String>,

    /// Whether this provider is built-in (cannot be deleted).
    #[serde(default)]
    pub builtin: bool,

    /// Capability / routing tags (e.g. "chat-only").
    #[serde(default)]
    pub tags: Vec<String>,

    /// True when this provider supports tool calling via prompt-guided
    /// generation (no native function-calling API). When enabled, the provider
    /// is allowed in Work sessions even if it is tagged as chat-only.
    #[serde(default)]
    pub prompt_guided_tool_calling: bool,

    /// Mobile number for providers that support phone/password login (e.g. deepseek-device).
    #[serde(default)]
    pub mobile: String,

    /// Encrypted password for phone/password login. Uses the project-wide
    /// `clarity_secrets` store (`enc2:` prefix) so it is never persisted in plaintext.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_enc: Option<String>,

    /// Authentication mode for providers that support both token and password login
    /// (currently only `deepseek-device`). Persisted so the UI does not flip back to
    /// token mode while the user is typing a mobile number before the password is set.
    #[serde(default)]
    pub auth_mode: AuthMode,

    /// Provider-specific key/value options.
    ///
    /// Used by `deepseek-device` to store `model_type` (fast/expert/vision) and
    /// `search_enabled` without polluting the generic schema.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

/// Top-level TOML structure for a provider config file.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct ProviderConfigFile {
    #[serde(default)]
    provider: HashMap<String, ProviderDefinition>,
}

impl ProviderDefinition {
    /// Resolve the actual API key or OAuth access token.
    ///
    /// For `AuthType::ApiKey` / `AuthType::None`:
    ///   - `${env:VAR_NAME}` — read environment variable.
    ///   - `${file:path:field}` — read `field` from JSON file at `path` (`~` expanded).
    ///   - plain string — returned as-is.
    ///
    /// For `AuthType::OAuth`:
    ///   - If `api_key_ref` is non-empty, resolves it as above (static override).
    ///   - Otherwise reads the access token from the global OAuth token store
    ///     using `auth_token_key` (falls back to provider `id`).
    pub fn resolve_api_key(&self) -> Option<String> {
        let ref_str = self.api_key_ref.trim();

        // OAuth path: static key takes precedence, then token store
        if self.auth_type == AuthType::OAuth {
            if !ref_str.is_empty() {
                return clarity_contract::resolve_key_ref(ref_str);
            }
            let token_key = if self.auth_token_key.is_empty() {
                &self.id
            } else {
                &self.auth_token_key
            };
            let store = clarity_llm::auth::TokenStore::for_provider(token_key);
            return store.load().ok().flatten().map(|t| t.access_token);
        }

        // ApiKey / None path
        if ref_str.is_empty() {
            return None;
        }
        clarity_contract::resolve_key_ref(ref_str)
    }

    /// Full display name: falls back to `id` if `display_name` is empty.
    pub fn display(&self) -> &str {
        if self.display_name.is_empty() {
            &self.id
        } else {
            &self.display_name
        }
    }

    /// Returns true when this provider is tagged as chat-only and should not be
    /// used in Work or Claw sessions.
    pub fn is_chat_only(&self) -> bool {
        self.tags.contains(&"chat-only".to_string())
    }

    /// Returns true when this provider can drive workspace tools.
    ///
    /// A provider supports tools when it is either not chat-only or explicitly
    /// enables prompt-guided tool calling.
    pub fn supports_tools(&self) -> bool {
        !self.is_chat_only() || self.prompt_guided_tool_calling
    }

    /// Encrypt and store a login password using the project-wide SecretStore.
    ///
    /// Returns `Ok(())` on success, or an error message if the SecretStore cannot
    /// be loaded (e.g. the OS keyring is unavailable).
    pub fn set_password(&mut self, password: &str) -> Result<(), String> {
        let store = clarity_llm::default_secret_store()
            .map_err(|e| format!("Failed to load secret store: {e}"))?;
        self.password_enc = Some(store.encrypt(password).map_err(|e| e.to_string())?);
        Ok(())
    }

    /// Decrypt the stored password, if any.
    pub fn resolve_password(&self) -> Option<String> {
        let ciphertext = self.password_enc.as_ref()?;
        // ponytail: failures to load the secret store or decrypt are silently
        // treated as "no password"; callers validate presence before use.
        let store = clarity_llm::default_secret_store().ok()?;
        store.decrypt(ciphertext).ok()
    }

    /// Clear any stored password.
    pub fn clear_password(&mut self) {
        self.password_enc = None;
    }

    /// Validate that the resolved API key matches the expected prefix for this provider.
    ///
    /// Returns `Ok(())` if the key is valid, missing, or the provider does not require one.
    /// Returns `Err` with a descriptive message if the key prefix is clearly wrong.
    pub fn validate_api_key_prefix(&self) -> Result<(), String> {
        let key = match self.resolve_api_key() {
            Some(k) if !k.is_empty() => k,
            _ => return Ok(()),
        };

        // Skip validation for providers that don't need keys.
        if self.auth_type == AuthType::None {
            return Ok(());
        }

        match self.id.as_str() {
            "openai" => {
                if !key.starts_with("sk-") {
                    return Err(format!(
                        "OpenAI API key must start with 'sk-'. Did you configure the wrong key for provider '{}'?",
                        self.display()
                    ));
                }
                if key.starts_with("sk-ant-") {
                    return Err(format!(
                        "OpenAI API key looks like an Anthropic key (starts with 'sk-ant-'). Did you configure the wrong key for provider '{}'?",
                        self.display()
                    ));
                }
            }
            "anthropic" => {
                if !key.starts_with("sk-ant-") {
                    return Err(format!(
                        "Anthropic API key must start with 'sk-ant-'. Did you configure the wrong key for provider '{}'?",
                        self.display()
                    ));
                }
            }
            "gemini" => {
                if !key.starts_with("AIza") {
                    return Err(format!(
                        "Gemini API key must start with 'AIza'. Did you configure the wrong key for provider '{}'?",
                        self.display()
                    ));
                }
            }
            "local" => {
                // No key expected.
            }
            _ => {
                // For custom providers, detect cross-provider mismatches based on api_format.
                match self.api_format {
                    ApiFormat::AnthropicMessages => {
                        if key.starts_with("sk-") && !key.starts_with("sk-ant-") {
                            return Err(format!(
                                "Provider '{}' uses Anthropic format, but the key looks like an OpenAI key (starts with 'sk-'). Did you configure the wrong key?",
                                self.display()
                            ));
                        }
                    }
                    ApiFormat::OpenaiCompletions | ApiFormat::Kimi | ApiFormat::DeepSeekDevice => {
                        if key.starts_with("sk-ant-") {
                            return Err(format!(
                                "Provider '{}' uses OpenAI-compatible format, but the key looks like an Anthropic key (starts with 'sk-ant-'). Did you configure the wrong key?",
                                self.display()
                            ));
                        }
                        if key.starts_with("AIza") {
                            return Err(format!(
                                "Provider '{}' uses OpenAI-compatible format, but the key looks like a Gemini key (starts with 'AIza'). Did you configure the wrong key?",
                                self.display()
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Build a `DeepSeekDeviceProvider` from this definition.
    ///
    /// # Errors
    /// - Provider is not configured for deepseek-device.
    /// - Token mode without a resolvable token.
    /// - Password mode without a saved password or mobile number.
    pub fn to_deepseek_device_provider(
        &self,
        model_id: &str,
    ) -> Result<clarity_llm::DeepSeekDeviceProvider, String> {
        if self.id != "deepseek-device" {
            return Err(format!(
                "Provider '{}' is not a deepseek-device definition",
                self.id
            ));
        }

        let mut options = self
            .extra
            .get("model_type")
            .map(|v| clarity_llm::DeepSeekDeviceOptions::from_model_id(v))
            .unwrap_or_else(|| clarity_llm::DeepSeekDeviceOptions::from_model_id(model_id));
        if let Some(v) = self.extra.get("search_enabled") {
            options.search_enabled = v == "true";
        }

        if self.auth_mode.is_password() {
            let password = self.resolve_password().ok_or_else(|| {
                "DeepSeek (Device) password mode requires a saved password.".to_string()
            })?;
            if self.mobile.is_empty() {
                return Err("DeepSeek (Device) password login requires a mobile number.".into());
            }
            Ok(clarity_llm::DeepSeekDeviceProvider::new(
                clarity_llm::DeepSeekDeviceConfig {
                    base_url: self.base_url.clone(),
                    client_version: "2.1.8".into(),
                    device_id: "clarity-device".into(),
                    credentials: clarity_llm::DeepSeekDeviceCredentials::Password {
                        mobile: self.mobile.clone(),
                        password,
                    },
                    options,
                },
            ))
        } else {
            let token = self.resolve_api_key().ok_or_else(|| {
                "DeepSeek (Device) token mode requires a device token.".to_string()
            })?;
            Ok(clarity_llm::DeepSeekDeviceProvider::new(
                clarity_llm::DeepSeekDeviceConfig {
                    credentials: clarity_llm::DeepSeekDeviceCredentials::Token(token),
                    options,
                    ..Default::default()
                },
            ))
        }
    }
}

impl From<&ProviderDefinition> for clarity_llm::ProviderConfig {
    fn from(def: &ProviderDefinition) -> Self {
        let protocol = match def.api_format {
            ApiFormat::OpenaiCompletions => clarity_llm::ProtocolType::OpenAiChat,
            ApiFormat::AnthropicMessages => clarity_llm::ProtocolType::AnthropicMessages,
            ApiFormat::Kimi => clarity_llm::ProtocolType::OpenAiChat,
            ApiFormat::DeepSeekDevice => clarity_llm::ProtocolType::DeepSeekDevice,
        };

        let auth_type = match def.auth_type {
            AuthType::ApiKey => clarity_llm::AuthType::ApiKey,
            AuthType::OAuth => clarity_llm::AuthType::OAuth,
            AuthType::None => clarity_llm::AuthType::None,
        };

        let oauth = if def.auth_type == AuthType::OAuth {
            Some(clarity_llm::OAuthProviderConfig {
                client_id: "17e5f671-d194-4dfb-9706-5516cb48c098".into(),
                host: "https://auth.kimi.com".into(),
                device_auth_path: "/api/oauth/device_authorization".into(),
                token_path: "/api/oauth/token".into(),
            })
        } else {
            None
        };

        Self {
            protocol,
            base_url: Some(def.base_url.clone()).filter(|s| !s.is_empty()),
            api_key_env: Some(def.api_key_ref.clone()).filter(|s| !s.is_empty()),
            auth_type,
            auth_token_key: Some(def.auth_token_key.clone()).filter(|s| !s.is_empty()),
            oauth,
            extra: def.extra.clone(),
            pricing: None,
            tags: Vec::new(),
        }
    }
}

/// Provider registry — manages built-in + custom providers.
#[derive(Clone, Debug)]
pub struct ProviderRegistry {
    /// All known providers, keyed by id.
    providers: HashMap<String, ProviderDefinition>,
    /// Config directory path for custom providers.
    config_dir: PathBuf,
}

impl ProviderRegistry {
    /// Create a new registry with built-in + custom (loaded from disk).
    pub fn load() -> Self {
        let config_dir = Self::config_dir();
        let mut registry = Self {
            providers: HashMap::new(),
            config_dir,
        };
        registry.load_builtin();
        registry.load_custom();
        registry
    }

    /// Built-in provider definitions (compiled-in defaults).
    fn load_builtin(&mut self) {
        let builtins = vec![
            ProviderDefinition {
                id: "openai".into(),
                display_name: "OpenAI".into(),
                base_url: "https://api.openai.com/v1".into(),
                api_format: ApiFormat::OpenaiCompletions,
                auth_type: AuthType::ApiKey,
                api_key_ref: "${env:OPENAI_API_KEY}".into(),
                auth_token_key: String::new(),
                models: vec!["gpt-4o".into(), "gpt-4o-mini".into(), "gpt-4".into()],
                builtin: true,
                tags: vec![],
                ..Default::default()
            },
            ProviderDefinition {
                id: "anthropic".into(),
                display_name: "Anthropic".into(),
                base_url: "https://api.anthropic.com/v1".into(),
                api_format: ApiFormat::AnthropicMessages,
                auth_type: AuthType::ApiKey,
                api_key_ref: "${env:ANTHROPIC_AUTH_TOKEN}".into(),
                auth_token_key: String::new(),
                models: vec!["claude-sonnet-4-20250514".into(), "claude-haiku-3-5".into()],
                builtin: true,
                tags: vec![],
                ..Default::default()
            },
            ProviderDefinition {
                id: "deepseek".into(),
                display_name: "DeepSeek".into(),
                base_url: "https://api.deepseek.com/v1".into(),
                api_format: ApiFormat::OpenaiCompletions,
                auth_type: AuthType::ApiKey,
                api_key_ref: "${env:DEEPSEEK_API_KEY}".into(),
                auth_token_key: String::new(),
                models: vec!["deepseek-chat".into(), "deepseek-reasoner".into()],
                builtin: true,
                tags: vec![],
                ..Default::default()
            },
            ProviderDefinition {
                id: "deepseek-device".into(),
                display_name: "DeepSeek (Device)".into(),
                base_url: "https://chat.deepseek.com".into(),
                api_format: ApiFormat::DeepSeekDevice,
                auth_type: AuthType::ApiKey,
                api_key_ref: "${env:DEEPSEEK_DEVICE_TOKEN}".into(),
                auth_token_key: String::new(),
                models: vec![
                    "deepseek-chat".into(),
                    "deepseek-reasoner".into(),
                    "deepseek-vision".into(),
                ],
                builtin: true,
                tags: vec![],
                prompt_guided_tool_calling: true,
                ..Default::default()
            },
            ProviderDefinition {
                id: "kimi".into(),
                display_name: "Kimi".into(),
                base_url: "https://api.kimi.com/v1".into(),
                api_format: ApiFormat::Kimi,
                auth_type: AuthType::ApiKey,
                api_key_ref: "${env:KIMI_API_KEY}".into(),
                auth_token_key: String::new(),
                models: vec!["kimi-k2-07132k".into()],
                builtin: true,
                tags: vec![],
                ..Default::default()
            },
            ProviderDefinition {
                id: "kimi_code".into(),
                display_name: "Kimi Code (OAuth)".into(),
                base_url: "https://api.kimi.com/coding/v1".into(),
                api_format: ApiFormat::Kimi,
                auth_type: AuthType::OAuth,
                api_key_ref: String::new(),
                auth_token_key: "kimi-code".into(),
                models: vec!["kimi-k2.6".into()],
                builtin: true,
                tags: vec![],
                ..Default::default()
            },
            ProviderDefinition {
                id: "local".into(),
                display_name: "Local (GGUF)".into(),
                base_url: String::new(),
                api_format: ApiFormat::OpenaiCompletions,
                auth_type: AuthType::None,
                api_key_ref: String::new(),
                auth_token_key: String::new(),
                models: vec![],
                builtin: true,
                tags: vec![],
                ..Default::default()
            },
        ];
        for p in builtins {
            self.providers.insert(p.id.clone(), p);
        }
    }

    /// Load custom providers from `~/.config/clarity/providers/*.toml`.
    fn load_custom(&mut self) {
        if !self.config_dir.exists() {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(&self.config_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(file) = toml::from_str::<ProviderConfigFile>(&content) {
                            for (id, mut def) in file.provider {
                                def.id = id.clone();
                                def.builtin = false;
                                self.providers.insert(id, def);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Save or update a custom provider definition.
    pub fn save_custom(&self, def: &ProviderDefinition) -> Result<(), String> {
        std::fs::create_dir_all(&self.config_dir).map_err(|e| e.to_string())?;

        let mut file = ProviderConfigFile::default();
        // Re-read existing file for this id, or create fresh
        let path = self.config_dir.join(format!("{}.toml", def.id));
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(existing) = toml::from_str::<ProviderConfigFile>(&content) {
                    file = existing;
                }
            }
        }
        file.provider.insert(def.id.clone(), def.clone());
        let content = toml::to_string_pretty(&file).map_err(|e| e.to_string())?;
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Delete a custom provider definition (built-in ones cannot be deleted).
    #[allow(dead_code)]
    pub fn delete_custom(&self, id: &str) -> Result<(), String> {
        if let Some(def) = self.providers.get(id) {
            if def.builtin {
                return Err("Cannot delete built-in provider".into());
            }
        }
        let path = self.config_dir.join(format!("{}.toml", id));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Get a provider by id.
    pub fn get(&self, id: &str) -> Option<&ProviderDefinition> {
        self.providers.get(id)
    }

    /// List all providers.
    pub fn list(&self) -> Vec<&ProviderDefinition> {
        let mut list: Vec<_> = self.providers.values().collect();
        list.sort_by(|a, b| a.display().cmp(b.display()));
        list
    }

    /// Update the model list for a provider in memory (does not save to disk).
    /// Used by the Provider panel after fetching models from the API.
    pub fn update_models(&mut self, id: &str, models: Vec<String>) {
        if let Some(def) = self.providers.get_mut(id) {
            def.models = models;
        }
    }

    /// Update a provider definition in memory and persist to disk.
    pub fn update_provider(&mut self, def: &ProviderDefinition) -> Result<(), String> {
        self.providers.insert(def.id.clone(), def.clone());
        self.save_custom(def)
    }

    /// List only custom (non-builtin) providers.
    #[allow(dead_code)]
    pub fn list_custom(&self) -> Vec<&ProviderDefinition> {
        self.providers.values().filter(|p| !p.builtin).collect()
    }

    fn config_dir() -> PathBuf {
        let mut path = if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata)
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".config")
        } else {
            PathBuf::from(".")
        };
        path.push("clarity");
        path.push("providers");
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::LlmProvider;

    #[test]
    fn test_builtin_providers_loaded() {
        let registry = ProviderRegistry::load();
        let providers = registry.list();
        assert!(
            providers.len() >= 5,
            "expected at least 5 built-in providers"
        );
        assert!(registry.get("openai").is_some());
        assert!(registry.get("local").is_some());
    }

    #[test]
    fn test_deepseek_device_supports_tools() {
        let ds = deepseek_device_def();
        assert!(ds.prompt_guided_tool_calling);
        assert!(ds.supports_tools());
        assert!(!ds.is_chat_only());
    }

    #[test]
    fn test_chat_only_provider_does_not_support_tools() {
        let mut def = ProviderDefinition {
            id: "chat-only".into(),
            display_name: String::new(),
            base_url: "https://test.com".into(),
            api_format: ApiFormat::OpenaiCompletions,
            auth_type: AuthType::None,
            api_key_ref: String::new(),
            auth_token_key: String::new(),
            models: vec![],
            builtin: false,
            tags: vec!["chat-only".into()],
            ..Default::default()
        };
        assert!(!def.supports_tools());
        // Prompt-guided tool calling overrides the chat-only tag.
        def.prompt_guided_tool_calling = true;
        assert!(def.supports_tools());
    }

    #[test]
    fn test_api_key_env_var_syntax() {
        let def = ProviderDefinition {
            id: "test".into(),
            display_name: String::new(),
            base_url: "https://test.com".into(),
            api_format: ApiFormat::OpenaiCompletions,
            auth_type: AuthType::ApiKey,
            api_key_ref: "${env:TEST_FAKE_KEY}".into(),
            auth_token_key: String::new(),
            models: vec![],
            builtin: false,
            tags: vec![],
            ..Default::default()
        };
        // No env var set → should return None
        assert!(def.resolve_api_key().is_none());
    }

    #[test]
    fn test_api_key_literal() {
        let def = ProviderDefinition {
            id: "test".into(),
            display_name: String::new(),
            base_url: "https://test.com".into(),
            api_format: ApiFormat::OpenaiCompletions,
            auth_type: AuthType::ApiKey,
            api_key_ref: "sk-mykey".into(),
            auth_token_key: String::new(),
            models: vec![],
            builtin: false,
            tags: vec![],
            ..Default::default()
        };
        assert_eq!(def.resolve_api_key(), Some("sk-mykey".into()));
    }

    #[test]
    fn test_display_name_fallback() {
        let def = ProviderDefinition {
            id: "my-provider".into(),
            display_name: "".into(),
            base_url: "https://test.com".into(),
            api_format: ApiFormat::OpenaiCompletions,
            auth_type: AuthType::ApiKey,
            api_key_ref: "".into(),
            auth_token_key: String::new(),
            models: vec![],
            builtin: false,
            tags: vec![],
            ..Default::default()
        };
        assert_eq!(def.display(), "my-provider");
    }

    #[test]
    fn test_provider_toml_roundtrip() {
        let toml_str = r#"
[provider.test]
display_name = "Test Provider"
base_url = "https://test-api.com/v1"
api_format = "openai-completions"
api_key_ref = "${env:TEST_KEY}"
models = ["model-a", "model-b"]
"#;
        let file: ProviderConfigFile = toml::from_str(toml_str).unwrap();
        let def = file.provider.get("test").unwrap();
        assert_eq!(def.display_name, "Test Provider");
        assert_eq!(def.models.len(), 2);
    }

    #[test]
    fn test_provider_definition_to_core_config() {
        let def = ProviderDefinition {
            id: "kimi_code".into(),
            display_name: "Kimi Code".into(),
            base_url: "https://api.kimi.com/coding/v1".into(),
            api_format: ApiFormat::Kimi,
            auth_type: AuthType::OAuth,
            api_key_ref: String::new(),
            auth_token_key: "kimi-code".into(),
            models: vec!["kimi-k2.6".into()],
            builtin: true,
            tags: vec![],
            ..Default::default()
        };
        let cfg: clarity_llm::ProviderConfig = (&def).into();
        assert_eq!(cfg.auth_type, clarity_llm::AuthType::OAuth);
        assert!(cfg.oauth.is_some());
    }

    #[test]
    fn test_validate_api_key_prefix_openai_ok() {
        let def = ProviderDefinition {
            id: "openai".into(),
            display_name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_format: ApiFormat::OpenaiCompletions,
            auth_type: AuthType::ApiKey,
            api_key_ref: "sk-test12345678901234567890".into(),
            auth_token_key: String::new(),
            models: vec![],
            builtin: true,
            tags: vec![],
            ..Default::default()
        };
        assert!(def.validate_api_key_prefix().is_ok());
    }

    #[test]
    fn test_validate_api_key_prefix_openai_anthropic_mismatch() {
        let def = ProviderDefinition {
            id: "openai".into(),
            display_name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_format: ApiFormat::OpenaiCompletions,
            auth_type: AuthType::ApiKey,
            api_key_ref: "sk-ant-api03-xxxx".into(),
            auth_token_key: String::new(),
            models: vec![],
            builtin: true,
            tags: vec![],
            ..Default::default()
        };
        let err = def.validate_api_key_prefix().unwrap_err();
        assert!(err.contains("Anthropic"));
    }

    #[test]
    fn test_validate_api_key_prefix_anthropic_ok() {
        let def = ProviderDefinition {
            id: "anthropic".into(),
            display_name: "Anthropic".into(),
            base_url: "https://api.anthropic.com".into(),
            api_format: ApiFormat::AnthropicMessages,
            auth_type: AuthType::ApiKey,
            api_key_ref: "sk-ant-api03-xxxx".into(),
            auth_token_key: String::new(),
            models: vec![],
            builtin: true,
            tags: vec![],
            ..Default::default()
        };
        assert!(def.validate_api_key_prefix().is_ok());
    }

    #[test]
    fn test_validate_api_key_prefix_anthropic_openai_mismatch() {
        let def = ProviderDefinition {
            id: "anthropic".into(),
            display_name: "Anthropic".into(),
            base_url: "https://api.anthropic.com".into(),
            api_format: ApiFormat::AnthropicMessages,
            auth_type: AuthType::ApiKey,
            api_key_ref: "sk-test12345678901234567890".into(),
            auth_token_key: String::new(),
            models: vec![],
            builtin: true,
            tags: vec![],
            ..Default::default()
        };
        let err = def.validate_api_key_prefix().unwrap_err();
        assert!(err.contains("sk-ant-"));
    }

    #[test]
    fn test_validate_api_key_prefix_missing_key_skips() {
        let def = ProviderDefinition {
            id: "openai".into(),
            display_name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_format: ApiFormat::OpenaiCompletions,
            auth_type: AuthType::ApiKey,
            api_key_ref: "".into(),
            auth_token_key: String::new(),
            models: vec![],
            builtin: true,
            tags: vec![],
            ..Default::default()
        };
        assert!(def.validate_api_key_prefix().is_ok());
    }

    // ============================================================================
    // DeepSeek (Device) provider builder tests
    // ============================================================================

    fn deepseek_device_def() -> ProviderDefinition {
        ProviderDefinition {
            id: "deepseek-device".into(),
            display_name: "DeepSeek (Device)".into(),
            base_url: "https://chat.deepseek.com".into(),
            api_format: ApiFormat::DeepSeekDevice,
            auth_type: AuthType::ApiKey,
            api_key_ref: String::new(),
            auth_token_key: String::new(),
            models: vec!["deepseek-chat".into(), "deepseek-reasoner".into()],
            builtin: true,
            tags: vec![],
            prompt_guided_tool_calling: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_auth_mode_default_is_token() {
        let def = deepseek_device_def();
        assert_eq!(def.auth_mode, AuthMode::Token);
    }

    #[test]
    fn test_auth_mode_toml_roundtrip() {
        // ApiFormat serializes as kebab-case: DeepSeekDevice -> "deep-seek-device".
        let toml_str = r#"
[provider.test]
display_name = "Test"
base_url = "https://chat.deepseek.com"
api_format = "deep-seek-device"
auth_mode = "password"
mobile = "13800138000"
"#;
        let file: ProviderConfigFile = toml::from_str(toml_str).unwrap();
        let def = file.provider.get("test").unwrap();
        assert_eq!(def.auth_mode, AuthMode::Password);
        assert_eq!(def.mobile, "13800138000");

        // Missing auth_mode defaults to Token.
        let toml_default = r#"
[provider.test]
display_name = "Test"
base_url = "https://chat.deepseek.com"
api_format = "deep-seek-device"
"#;
        let file: ProviderConfigFile = toml::from_str(toml_default).unwrap();
        let def = file.provider.get("test").unwrap();
        assert_eq!(def.auth_mode, AuthMode::Token);
    }

    #[test]
    fn test_password_roundtrip() {
        let mut def = deepseek_device_def();
        def.set_password("my-secret-password")
            .expect("secret store should be available in tests");
        assert!(def.password_enc.is_some());
        assert!(!def.password_enc.as_ref().unwrap().is_empty());
        assert_eq!(
            def.resolve_password().as_deref(),
            Some("my-secret-password")
        );

        def.clear_password();
        assert!(def.resolve_password().is_none());
    }

    #[test]
    fn test_to_deepseek_device_provider_token() {
        let mut def = deepseek_device_def();
        def.api_key_ref = "ds-test-token".into();
        let provider = def.to_deepseek_device_provider("deepseek-chat").unwrap();
        assert!(!provider.capabilities().native_tool_calling);
    }

    #[test]
    fn test_to_deepseek_device_provider_password() {
        let mut def = deepseek_device_def();
        def.auth_mode = AuthMode::Password;
        def.mobile = "13800138000".into();
        def.set_password("my-secret-password").unwrap();
        let provider = def
            .to_deepseek_device_provider("deepseek-reasoner")
            .unwrap();
        assert!(!provider.capabilities().native_tool_calling);
    }

    #[test]
    fn test_to_deepseek_device_provider_missing_token() {
        let def = deepseek_device_def();
        let err = def
            .to_deepseek_device_provider("deepseek-chat")
            .unwrap_err();
        assert!(err.contains("device token"));
    }

    #[test]
    fn test_to_deepseek_device_provider_missing_password() {
        let mut def = deepseek_device_def();
        def.auth_mode = AuthMode::Password;
        def.mobile = "13800138000".into();
        let err = def
            .to_deepseek_device_provider("deepseek-chat")
            .unwrap_err();
        assert!(err.contains("saved password"));
    }

    #[test]
    fn test_to_deepseek_device_provider_missing_mobile() {
        let mut def = deepseek_device_def();
        def.auth_mode = AuthMode::Password;
        def.set_password("my-secret-password").unwrap();
        let err = def
            .to_deepseek_device_provider("deepseek-chat")
            .unwrap_err();
        assert!(err.contains("mobile number"));
    }

    #[test]
    fn test_runtime_api_format_mappings() {
        assert_eq!(
            ApiFormat::OpenaiCompletions.runtime_api_format(),
            "openai_chat"
        );
        assert_eq!(ApiFormat::Kimi.runtime_api_format(), "openai_chat");
        assert_eq!(
            ApiFormat::AnthropicMessages.runtime_api_format(),
            "anthropic_messages"
        );
        assert_eq!(
            ApiFormat::DeepSeekDevice.runtime_api_format(),
            "deepseek_device"
        );
    }
}
