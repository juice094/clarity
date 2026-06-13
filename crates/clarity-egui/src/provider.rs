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
}

impl ApiFormat {
    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiFormat::OpenaiCompletions => "openai-completions",
            ApiFormat::AnthropicMessages => "anthropic-messages",
            ApiFormat::Kimi => "kimi",
        }
    }

    /// Parses from a string.
    pub fn from_str(s: &str) -> Self {
        match s {
            "anthropic-messages" => Self::AnthropicMessages,
            "kimi" => Self::Kimi,
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

/// A single provider definition.
#[derive(Serialize, Deserialize, Clone, Debug)]
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
                return Self::resolve_key_ref(ref_str);
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
        Self::resolve_key_ref(ref_str)
    }

    /// Resolve a key reference string (env var, file field, or literal).
    fn resolve_key_ref(ref_str: &str) -> Option<String> {
        // ${env:VAR}
        if let Some(env_var) = ref_str
            .strip_prefix("${env:")
            .and_then(|s| s.strip_suffix('}'))
        {
            return std::env::var(env_var).ok();
        }

        // ${file:path:field}
        if let Some(inner) = ref_str
            .strip_prefix("${file:")
            .and_then(|s| s.strip_suffix('}'))
        {
            let (path_part, field) = inner.split_once(':')?;

            let path = if path_part.starts_with("~/") {
                dirs::home_dir()
                    .map(|h| h.join(&path_part[2..]))
                    .unwrap_or_else(|| std::path::PathBuf::from(path_part))
            } else {
                std::path::PathBuf::from(path_part)
            };
            let content = std::fs::read_to_string(&path).ok()?;
            let json: serde_json::Value = serde_json::from_str(&content).ok()?;
            return json
                .get(field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        Some(ref_str.to_string())
    }

    /// Full display name: falls back to `id` if `display_name` is empty.
    pub fn display(&self) -> &str {
        if self.display_name.is_empty() {
            &self.id
        } else {
            &self.display_name
        }
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
                    ApiFormat::OpenaiCompletions | ApiFormat::Kimi => {
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
}

impl From<&ProviderDefinition> for clarity_llm::ProviderConfig {
    fn from(def: &ProviderDefinition) -> Self {
        let protocol = match def.api_format {
            ApiFormat::OpenaiCompletions => clarity_llm::ProtocolType::OpenAiChat,
            ApiFormat::AnthropicMessages => clarity_llm::ProtocolType::AnthropicMessages,
            ApiFormat::Kimi => clarity_llm::ProtocolType::OpenAiChat,
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
            extra: HashMap::new(),
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
        };
        assert!(def.validate_api_key_prefix().is_ok());
    }
}
