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
pub enum ApiFormat {
    /// OpenAI-compatible chat completions endpoint.
    OpenaiCompletions,
    /// Anthropic Messages API.
    AnthropicMessages,
    /// Native Kimi API.
    Kimi,
}

impl Default for ApiFormat {
    fn default() -> Self {
        Self::OpenaiCompletions
    }
}

impl ApiFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiFormat::OpenaiCompletions => "openai-completions",
            ApiFormat::AnthropicMessages => "anthropic-messages",
            ApiFormat::Kimi => "kimi",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "anthropic-messages" => Self::AnthropicMessages,
            "kimi" => Self::Kimi,
            _ => Self::OpenaiCompletions,
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

    /// Reference to API key — either literal or `${env:VAR_NAME}` syntax.
    #[serde(default)]
    pub api_key_ref: String,

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
    /// Resolve the actual API key from `api_key_ref` (supports `${env:VAR}`).
    #[allow(dead_code)]
    pub fn resolve_api_key(&self) -> Option<String> {
        let ref_str = self.api_key_ref.trim();
        if ref_str.is_empty() {
            return None;
        }
        if let Some(env_var) = ref_str.strip_prefix("${env:").and_then(|s| s.strip_suffix('}')) {
            return std::env::var(env_var).ok();
        }
        Some(ref_str.to_string())
    }

    /// Full display name: falls back to `id` if `display_name` is empty.
    pub fn display(&self) -> &str {
        if self.display_name.is_empty() { &self.id } else { &self.display_name }
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
                api_key_ref: "${env:OPENAI_API_KEY}".into(),
                models: vec!["gpt-4o".into(), "gpt-4o-mini".into(), "gpt-4".into()],
                builtin: true,
            },
            ProviderDefinition {
                id: "anthropic".into(),
                display_name: "Anthropic".into(),
                base_url: "https://api.anthropic.com/v1".into(),
                api_format: ApiFormat::AnthropicMessages,
                api_key_ref: "${env:ANTHROPIC_AUTH_TOKEN}".into(),
                models: vec!["claude-sonnet-4-20250514".into(), "claude-haiku-3-5".into()],
                builtin: true,
            },
            ProviderDefinition {
                id: "deepseek".into(),
                display_name: "DeepSeek".into(),
                base_url: "https://api.deepseek.com/v1".into(),
                api_format: ApiFormat::OpenaiCompletions,
                api_key_ref: "${env:DEEPSEEK_API_KEY}".into(),
                models: vec!["deepseek-chat".into(), "deepseek-reasoner".into()],
                builtin: true,
            },
            ProviderDefinition {
                id: "kimi".into(),
                display_name: "Kimi".into(),
                base_url: "https://api.kimi.com/v1".into(),
                api_format: ApiFormat::Kimi,
                api_key_ref: "${env:KIMI_API_KEY}".into(),
                models: vec!["kimi-k2-07132k".into()],
                builtin: true,
            },
            ProviderDefinition {
                id: "local".into(),
                display_name: "Local (GGUF)".into(),
                base_url: String::new(),
                api_format: ApiFormat::OpenaiCompletions,
                api_key_ref: String::new(),
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
        assert!(providers.len() >= 5, "expected at least 5 built-in providers");
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
            api_key_ref: "${env:TEST_FAKE_KEY}".into(),
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
            api_key_ref: "sk-mykey".into(),
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
            api_key_ref: "".into(),
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
}
