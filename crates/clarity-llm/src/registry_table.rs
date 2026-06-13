//! Canonical provider-family defaults.
//!
//! This module provides a single table of known provider families.
//! Adding a new OpenAI-compatible family should only require adding one arm
//! to [`family_defaults`]; the built-in env-var fallback and UI hint tables
//! derive from the same source of truth.

use crate::model_registry::{AuthType, OAuthProviderConfig, ProtocolType};

/// Default connection/auth settings for a provider family.
#[derive(Debug, Clone)]
pub struct FamilyDefaults {
    /// Communication protocol used by this family.
    pub protocol: ProtocolType,
    /// Base URL for the provider API.
    pub base_url: Option<String>,
    /// Environment variable name that holds the API key.
    pub api_key_env: Option<String>,
    /// Authentication type for this family.
    pub auth_type: AuthType,
    /// OAuth token storage key.
    pub auth_token_key: Option<String>,
    /// OAuth-specific configuration.
    pub oauth: Option<OAuthProviderConfig>,
    /// Default model identifier for this family.
    pub default_model: Option<String>,
}

impl Default for FamilyDefaults {
    fn default() -> Self {
        Self {
            protocol: ProtocolType::OpenAiChat,
            base_url: None,
            api_key_env: None,
            auth_type: AuthType::ApiKey,
            auth_token_key: None,
            oauth: None,
            default_model: None,
        }
    }
}

/// Look up canonical defaults for a provider family name.
pub fn family_defaults(name: &str) -> Option<FamilyDefaults> {
    match name {
        "openai" => Some(FamilyDefaults {
            base_url: Some("https://api.openai.com/v1".into()),
            api_key_env: Some("OPENAI_API_KEY".into()),
            default_model: Some("gpt-4o".into()),
            ..Default::default()
        }),
        "deepseek" => Some(FamilyDefaults {
            base_url: Some("https://api.deepseek.com/v1".into()),
            api_key_env: Some("DEEPSEEK_API_KEY".into()),
            default_model: Some("deepseek-chat".into()),
            ..Default::default()
        }),
        "kimi" => Some(FamilyDefaults {
            base_url: Some("https://api.moonshot.cn/v1".into()),
            api_key_env: Some("KIMI_API_KEY".into()),
            default_model: Some("kimi-k2.6".into()),
            ..Default::default()
        }),
        "moonshot" => Some(FamilyDefaults {
            base_url: Some("https://api.moonshot.cn/v1".into()),
            api_key_env: Some("KIMI_API_KEY".into()),
            default_model: Some("kimi-k2.6".into()),
            ..Default::default()
        }),
        "kimi-code" => Some(FamilyDefaults {
            base_url: Some("https://api.kimi.com/coding/v1".into()),
            api_key_env: Some("KIMI_CODE_API_KEY".into()),
            auth_type: AuthType::OAuth,
            auth_token_key: Some("kimi-code".into()),
            oauth: Some(OAuthProviderConfig {
                client_id: "17e5f671-d194-4dfb-9706-5516cb48c098".into(),
                ..Default::default()
            }),
            default_model: Some("kimi-k2.6".into()),
            ..Default::default()
        }),
        "anthropic" => Some(FamilyDefaults {
            protocol: ProtocolType::AnthropicMessages,
            base_url: Some("https://api.anthropic.com".into()),
            api_key_env: Some("ANTHROPIC_AUTH_TOKEN".into()),
            default_model: Some("claude-sonnet".into()),
            ..Default::default()
        }),
        "ollama" => Some(FamilyDefaults {
            protocol: ProtocolType::Ollama,
            base_url: Some("http://localhost:11434".into()),
            auth_type: AuthType::None,
            default_model: Some("ollama-llama3".into()),
            ..Default::default()
        }),
        "llama-server" => Some(FamilyDefaults {
            protocol: ProtocolType::LlamaServer,
            base_url: Some("http://localhost:8080".into()),
            auth_type: AuthType::None,
            default_model: Some("llama-server-default".into()),
            ..Default::default()
        }),
        #[cfg(feature = "local-llm")]
        "local" => Some(FamilyDefaults {
            protocol: ProtocolType::KalosmLocal,
            auth_type: AuthType::None,
            default_model: Some("local-qwen".into()),
            ..Default::default()
        }),
        _ => None,
    }
}

/// List all registered provider family names.
pub fn all_family_names() -> &'static [&'static str] {
    &[
        "openai",
        "deepseek",
        "kimi",
        "moonshot",
        "kimi-code",
        "anthropic",
        "ollama",
        "llama-server",
        #[cfg(feature = "local-llm")]
        "local",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_families() {
        assert!(family_defaults("deepseek").is_some());
        assert!(family_defaults("kimi-code").is_some());
        assert!(family_defaults("unknown").is_none());
    }

    #[test]
    fn test_all_family_names_non_empty() {
        assert!(!all_family_names().is_empty());
    }
}
