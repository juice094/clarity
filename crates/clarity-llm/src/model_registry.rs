//! Model Registry — Configuration-driven LLM provider routing
//!
//! Inspired by LiteLLM's `model_list` and OpenRouter's alias system.
//!
//! ## Configuration file (`models.toml`)
//!
//! ```toml
//! [providers.kimi-code]
//! protocol = "openai_chat"
//! base_url = "https://api.kimi.com/coding/v1"
//! api_key_env = "KIMI_CODE_API_KEY"
//!
//! [providers.anthropic]
//! protocol = "anthropic_messages"
//! base_url = "https://api.anthropic.com"
//! api_key_env = "ANTHROPIC_AUTH_TOKEN"
//!
//! [providers.local]
//! protocol = "kalosm_local"
//! model_path = "C:\\Users\\22414\\Desktop\\model\\Qwen2.5-7B-Instruct.Q4_K_M.gguf"
//!
//! [[models]]
//! alias = "kimi-k2"
//! provider = "kimi-code"
//! model_id = "kimi-k2-07132k"
//!
//! [[models]]
//! alias = "claude-sonnet"
//! provider = "anthropic"
//! model_id = "claude-3-5-sonnet-20241022"
//!
//! [[models]]
//! alias = "local-qwen"
//! provider = "local"
//! model_id = "Qwen2.5-7B-Instruct"
//! ```
//!
//! ## Search paths
//! 1. `CLARITY_MODELS_CONFIG` env var
//! 2. `./.clarity/models.toml`
//! 3. `~/.config/clarity/models.toml`
//! 4. Built-in fallback (auto-detect from env vars)

use crate::api::LlmProvider;
use crate::{LlamaServerProvider, OpenAiCompatibleLlm};
use async_trait::async_trait;
use clarity_contract::AgentError;
use clarity_contract::llm::{LlmProviderFactory, Pricing};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Protocol adapter type for provider communication
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolType {
    /// OpenAI /v1/chat/completions compatible (default).
    #[default]
    OpenAiChat,
    /// Anthropic /v1/messages API (content blocks, tool_use, etc.)
    AnthropicMessages,
    /// Local inference via Kalosm (non-HTTP)
    #[cfg(feature = "local-llm")]
    KalosmLocal,
    /// Ollama /api/generate or /api/chat
    Ollama,
    /// llama.cpp server (OpenAI-compatible HTTP endpoint)
    LlamaServer,
}

/// Authentication type for a provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    /// Standard API key authentication (default).
    #[default]
    ApiKey,
    /// OAuth 2.0 device flow or authorization code flow.
    OAuth,
    /// No authentication required (e.g. local Ollama).
    None,
}

/// OAuth-specific configuration for a provider.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OAuthProviderConfig {
    /// OAuth client ID.
    pub client_id: String,
    /// OAuth authorization host (e.g. `https://auth.kimi.com`).
    #[serde(default = "default_oauth_host")]
    pub host: String,
    /// Device authorization endpoint path (default `/api/oauth/device_authorization`).
    #[serde(default = "default_device_auth_path")]
    pub device_auth_path: String,
    /// Token endpoint path (default `/api/oauth/token`).
    #[serde(default = "default_token_path")]
    pub token_path: String,
}

fn default_oauth_host() -> String {
    "https://auth.kimi.com".into()
}

fn default_device_auth_path() -> String {
    "/api/oauth/device_authorization".into()
}

fn default_token_path() -> String {
    "/api/oauth/token".into()
}

/// Provider-level connection configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    /// Communication protocol used by this provider.
    pub protocol: ProtocolType,
    /// Base URL for the provider API.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Environment variable name that holds the API key
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Authentication type (defaults to ApiKey).
    #[serde(default)]
    pub auth_type: AuthType,
    /// OAuth token storage key. When `auth_type` is `OAuth`, this key is used
    /// to look up the persisted access token. Defaults to the provider name.
    #[serde(default)]
    pub auth_token_key: Option<String>,
    /// OAuth-specific configuration (only used when `auth_type` is `OAuth`).
    #[serde(default)]
    pub oauth: Option<OAuthProviderConfig>,
    /// Provider-specific extra settings (model_path for local, etc.)
    #[serde(default)]
    pub extra: HashMap<String, String>,
    /// Optional pricing info for cost-aware routing.
    #[serde(default)]
    pub pricing: Option<Pricing>,
    /// Capability tags for hint-based routing (e.g. "cheap", "coding", "vision").
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A user-facing model alias mapped to a concrete provider + model_id.
///
/// Alias-level overrides allow the same provider family to be configured
/// multiple times with different keys, endpoints, or model IDs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelEntry {
    /// Human-friendly name shown in UI (e.g. "kimi-k2", "claude-sonnet")
    pub alias: String,
    /// Reference to a provider definition
    pub provider: String,
    /// Provider-side model identifier (e.g. "claude-3-5-sonnet-20241022")
    pub model_id: String,
    /// Optional per-model temperature override
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Optional per-model max_tokens override
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Optional encrypted or literal API key.
    ///
    /// If present, it overrides the provider's `api_key_env`.
    /// Encrypted values use the `enc2:` prefix and are decrypted by
    /// `SecretStore` at provider construction time.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Optional override for the environment variable that holds the API key.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Optional override for the provider base URL.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Per-alias extra settings merged over `ProviderConfig.extra`.
    #[serde(default)]
    pub extra: HashMap<String, String>,
    /// Per-alias extra HTTP headers.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Fallback aliases to try if this alias fails.
    #[serde(default)]
    pub fallback_aliases: Vec<String>,
    /// Optional per-alias pricing override.
    #[serde(default)]
    pub pricing: Option<Pricing>,
    /// Capability tags for hint-based routing.
    #[serde(default)]
    pub tags: Vec<String>,
}

impl ModelEntry {
    /// Merge alias-level overrides into a base provider config.
    pub fn merge_into(&self, base: &ProviderConfig) -> ProviderConfig {
        let mut cfg = base.clone();
        if let Some(ref env) = self.api_key_env {
            cfg.api_key_env = Some(env.clone());
        }
        if let Some(ref url) = self.base_url {
            cfg.base_url = Some(url.clone());
        }
        if self.pricing.is_some() {
            cfg.pricing = self.pricing;
        }
        for tag in &self.tags {
            if !cfg.tags.contains(tag) {
                cfg.tags.push(tag.clone());
            }
        }
        for (k, v) in &self.extra {
            cfg.extra.insert(k.clone(), v.clone());
        }
        cfg
    }
}

/// Top-level configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelConfigFile {
    /// Provider family definitions keyed by name.
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    /// Model aliases exposed to callers.
    #[serde(default)]
    pub models: Vec<ModelEntry>,
}

/// Runtime registry that resolves aliases to concrete providers
#[derive(Debug, Clone)]
pub struct ModelRegistry {
    config: ModelConfigFile,
    /// Pre-built index: alias → model entry
    index: HashMap<String, ModelEntry>,
}

impl ModelRegistry {
    /// Load from default search paths, or build from environment variables
    pub fn load() -> Result<Self, AgentError> {
        if let Some(path) = Self::find_config_file() {
            tracing::info!("Loading model registry from {}", path.display());
            let contents = std::fs::read_to_string(&path)
                .map_err(|e| AgentError::Llm(format!("Failed to read model config: {}", e)))?;
            let config: ModelConfigFile = toml::from_str(&contents)
                .map_err(|e| AgentError::Llm(format!("Failed to parse model config: {}", e)))?;
            return Self::from_config(config);
        }

        tracing::info!("No model config found; using built-in env-var fallback");
        Self::from_config(Self::built_in_fallback())
    }

    /// Async wrapper around `load` that offloads blocking file I/O to
    /// Tokio's blocking thread pool.
    pub async fn load_async() -> Result<Self, AgentError> {
        tokio::task::spawn_blocking(Self::load)
            .await
            .map_err(|e| AgentError::Llm(format!("Model registry load panicked: {}", e)))?
    }

    /// Load from an explicit file path
    pub fn load_from(path: &PathBuf) -> Result<Self, AgentError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| AgentError::Llm(format!("Failed to read model config: {}", e)))?;
        let config: ModelConfigFile = toml::from_str(&contents)
            .map_err(|e| AgentError::Llm(format!("Failed to parse model config: {}", e)))?;
        Self::from_config(config)
    }

    /// Async wrapper around `load_from`.
    pub async fn load_from_async(path: &Path) -> Result<Self, AgentError> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || Self::load_from(&path))
            .await
            .map_err(|e| AgentError::Llm(format!("Model registry load panicked: {}", e)))?
    }

    /// Build from an in-memory config (useful for tests)
    pub fn from_config(config: ModelConfigFile) -> Result<Self, AgentError> {
        let mut index = HashMap::new();
        for entry in &config.models {
            if !config.providers.contains_key(&entry.provider) {
                return Err(AgentError::Llm(format!(
                    "Model '{}' references unknown provider '{}'",
                    entry.alias, entry.provider
                )));
            }
            index.insert(entry.alias.clone(), entry.clone());
        }
        Ok(Self { config, index })
    }

    /// Search for config file in known locations
    fn find_config_file() -> Option<PathBuf> {
        // 1. Explicit env var
        if let Ok(path) = std::env::var("CLARITY_MODELS_CONFIG") {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }

        // 2. Project-local .clarity/models.toml
        let local = PathBuf::from(".clarity").join("models.toml");
        if local.exists() {
            return Some(local);
        }

        // 3. User config dir
        if let Some(config_dir) = dirs::config_dir() {
            let user = config_dir.join("clarity").join("models.toml");
            if user.exists() {
                return Some(user);
            }
        }

        None
    }

    /// Built-in fallback when no config file exists — mirrors old LlmFactory::auto() behavior.
    ///
    /// Family defaults are sourced from [`crate::registry_table`] so that the
    /// env-var fallback and the canonical registry never drift.
    fn built_in_fallback() -> ModelConfigFile {
        let mut providers = HashMap::new();
        let mut models = Vec::new();

        for family in super::registry_table::all_family_names() {
            let Some(defaults) = super::registry_table::family_defaults(family) else {
                tracing::warn!("registered family '{}' has no defaults; skipping", family);
                continue;
            };

            // Only auto-include families that have a configured API key.
            let has_key = defaults
                .api_key_env
                .as_ref()
                .map(|env| std::env::var(env).is_ok())
                .unwrap_or(false);
            if !has_key {
                continue;
            }

            providers.insert(
                family.to_string(),
                ProviderConfig {
                    protocol: defaults.protocol.clone(),
                    base_url: defaults.base_url.clone(),
                    api_key_env: defaults.api_key_env.clone(),
                    auth_type: defaults.auth_type.clone(),
                    auth_token_key: defaults.auth_token_key.clone(),
                    oauth: defaults.oauth.clone(),
                    ..Default::default()
                },
            );

            let model_id = defaults
                .default_model
                .clone()
                .unwrap_or_else(|| family.to_string());
            models.push(ModelEntry {
                alias: model_id.clone(),
                provider: family.to_string(),
                model_id,
                ..Default::default()
            });
        }

        // Local fallback requires a model path and is handled separately.
        #[cfg(feature = "local-llm")]
        if let Some(model_path) = super::resolve_local_model_path() {
            let mut extra = HashMap::new();
            extra.insert(
                "model_path".into(),
                model_path.to_string_lossy().into_owned(),
            );
            if let Ok(repo) = std::env::var("CLARITY_LOCAL_TOKENIZER_REPO") {
                extra.insert("tokenizer_repo".into(), repo);
            }
            providers.insert(
                "local".to_string(),
                ProviderConfig {
                    protocol: ProtocolType::KalosmLocal,
                    base_url: None,
                    api_key_env: None,
                    auth_type: AuthType::None,
                    extra,
                    ..Default::default()
                },
            );
            models.push(ModelEntry {
                alias: "local-qwen".into(),
                provider: "local".into(),
                model_id: "Qwen2.5-7B-Instruct".into(),
                ..Default::default()
            });
        }

        ModelConfigFile { providers, models }
    }

    /// Get a model entry by alias
    pub fn get(&self, alias: &str) -> Option<&ModelEntry> {
        self.index.get(alias)
    }

    /// List all available model aliases
    pub fn list_models(&self) -> Vec<&ModelEntry> {
        self.config.models.iter().collect()
    }

    /// Get a provider config by name
    pub fn get_provider(&self, name: &str) -> Option<&ProviderConfig> {
        self.config.providers.get(name)
    }

    /// List all provider names
    pub fn list_providers(&self) -> Vec<&String> {
        self.config.providers.keys().collect()
    }

    /// Add or replace a provider family configuration.
    pub fn add_provider(&mut self, name: String, cfg: ProviderConfig) {
        self.config.providers.insert(name, cfg);
    }

    /// Add a model alias, replacing any existing alias with the same name.
    pub fn add_or_update_model(&mut self, entry: ModelEntry) {
        self.index.insert(entry.alias.clone(), entry.clone());
        self.config.models.retain(|m| m.alias != entry.alias);
        self.config.models.push(entry);
    }

    /// Resolve env-var placeholders in a string (e.g. "${OPENAI_API_KEY}")
    pub fn resolve_env(value: &str) -> String {
        let mut result = value.to_string();
        // Simple ${VAR} substitution
        loop {
            if let Some(start) = result.find("${") {
                if let Some(end) = result[start..].find('}') {
                    let var_name = &result[start + 2..start + end];
                    let replacement = std::env::var(var_name).unwrap_or_default();
                    result.replace_range(start..start + end + 1, &replacement);
                    continue;
                }
            }
            break;
        }
        result
    }

    /// Get the raw configuration (for serialization / admin API)
    pub fn config(&self) -> &ModelConfigFile {
        &self.config
    }
}

#[async_trait]
impl LlmProviderFactory for ModelRegistry {
    async fn build_for_alias(&self, alias: &str) -> Result<Arc<dyn LlmProvider>, AgentError> {
        let entry = self.get(alias).ok_or_else(|| {
            AgentError::Llm(format!("Model alias '{}' not found in registry", alias))
        })?;
        let provider_cfg = self.get_provider(&entry.provider).ok_or_else(|| {
            AgentError::Llm(format!(
                "Provider '{}' for model '{}' not found",
                entry.provider, alias
            ))
        })?;
        let secrets = default_secret_store().ok();
        let provider =
            build_provider_from_registry_entry(provider_cfg, entry, None, secrets.as_ref()).await?;
        Ok(Arc::from(provider))
    }
}

/// Expand a key reference string.
///
/// Supported syntax:
/// - `${file:path:field}` — read `field` from JSON file at `path` (`~` is expanded).
/// - `${env:VAR}` — read environment variable `VAR`.
/// - plain string — treated as an env-var name for backward compat, or returned as-is.
pub fn resolve_key_ref(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    // ${file:path:field}
    if let Some(inner) = raw
        .strip_prefix("${file:")
        .and_then(|s| s.strip_suffix('}'))
    {
        let (path_part, field) = inner.split_once(':')?;

        let path = if path_part.starts_with("~/") {
            dirs::home_dir()
                .map(|h| h.join(&path_part[2..]))
                .unwrap_or_else(|| PathBuf::from(path_part))
        } else {
            PathBuf::from(path_part)
        };
        let content = std::fs::read_to_string(&path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        return json
            .get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }

    // ${env:VAR}
    if let Some(var) = raw.strip_prefix("${env:").and_then(|s| s.strip_suffix('}')) {
        return std::env::var(var).ok();
    }

    // Try env var, fall back to literal
    std::env::var(raw).ok().or_else(|| Some(raw.to_string()))
}

/// Resolve an API key from a hierarchy of sources.
///
/// Priority (highest first):
/// 1. Explicit runtime override (`override_key`)
/// 2. Alias-level literal or encrypted key (`alias_api_key`)
/// 3. Alias-level environment-variable name (`alias_api_key_env`)
/// 4. Provider-level environment-variable name (`provider_api_key_env`)
fn resolve_api_key(
    provider_api_key_env: Option<&str>,
    alias_api_key: Option<&str>,
    alias_api_key_env: Option<&str>,
    override_key: Option<&str>,
    secrets: Option<&clarity_secrets::SecretStore>,
) -> Option<String> {
    if let Some(key) = override_key {
        return Some(key.to_string());
    }
    if let Some(key) = alias_api_key {
        if clarity_secrets::SecretStore::is_encrypted(key) {
            return secrets.and_then(|s| s.decrypt(key).ok());
        }
        return Some(key.to_string());
    }
    let env_name = alias_api_key_env.or(provider_api_key_env)?;
    resolve_key_ref(env_name)
}

/// Build a concrete provider from registry config + model_id.
/// This is used by the legacy `LlmFactory` in `mod.rs`.
pub async fn build_provider_from_registry(
    cfg: &ProviderConfig,
    model_id: &str,
) -> Result<Box<dyn LlmProvider>, AgentError> {
    build_provider_from_registry_with_key(cfg, model_id, None, None, None, None).await
}

/// Build a provider with an optional API-key override (e.g. from GUI Settings).
///
/// `alias_api_key` / `alias_api_key_env` represent alias-level overrides
/// and take precedence over the provider-level `cfg.api_key_env`.
/// `secrets` is required when `alias_api_key` is encrypted with `enc2:`.
pub async fn build_provider_from_registry_with_key(
    cfg: &ProviderConfig,
    model_id: &str,
    override_key: Option<&str>,
    alias_api_key: Option<&str>,
    alias_api_key_env: Option<&str>,
    secrets: Option<&clarity_secrets::SecretStore>,
) -> Result<Box<dyn LlmProvider>, AgentError> {
    let merged_api_key_env = alias_api_key_env.or(cfg.api_key_env.as_deref());
    match cfg.protocol {
        ProtocolType::OpenAiChat => {
            let api_key = resolve_api_key(
                merged_api_key_env,
                alias_api_key,
                alias_api_key_env,
                override_key,
                secrets,
            )
            .unwrap_or_default();
            let base_url = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".into());

            // OAuth path: auto-refresh token when no static key is provided.
            if cfg.auth_type == AuthType::OAuth {
                let oauth_cfg = cfg.oauth.clone().unwrap_or_default();
                let token_key = cfg.auth_token_key.clone()
                    .unwrap_or_else(|| "kimi-code".into());
                let manager = crate::auth::OAuthTokenManager::with_config(
                    crate::auth::OAuthDeviceFlowConfig {
                        client_id: oauth_cfg.client_id,
                        oauth_host: oauth_cfg.host,
                    },
                    &token_key,
                );
                let llm = super::OAuthLlm::new(api_key, base_url, model_id, manager);
                return Ok(Box::new(llm));
            }

            let llm = OpenAiCompatibleLlm::new(api_key, base_url, model_id);
            Ok(Box::new(llm))
        }
        ProtocolType::AnthropicMessages => {
            Err(AgentError::Llm(
                "Anthropic Messages adapter not yet implemented. Use OpenAI-compatible proxy or set ANTHROPIC_AUTH_TOKEN for legacy fallback.".into(),
            ))
        }
        #[cfg(feature = "local-llm")]
        ProtocolType::KalosmLocal => {
            let model_path = cfg
                .extra
                .get("model_path")
                .map(PathBuf::from)
                .or_else(super::resolve_local_model_path)
                .ok_or_else(|| AgentError::Llm(
                    "No local model path configured.\n".to_string()
                    + super::LOCAL_MODEL_HELP
                ))?;
            let tokenizer_repo = cfg.extra.get("tokenizer_repo").cloned();
            let mut gguf_config = super::local_gguf::LocalGgufConfig::new(model_path)?;
            if let Some(repo) = tokenizer_repo {
                gguf_config = gguf_config.with_tokenizer_repo(repo);
            }
            let provider = super::local_gguf::LocalGgufProvider::new(gguf_config).await?;
            Ok(Box::new(provider))
        }
        // KalosmLocal is cfg-gated; when local-llm is disabled it won't appear in matches
        ProtocolType::Ollama => {
            let base_url = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434".into());
            let provider = super::ollama::OllamaProvider::new(base_url, model_id);
            Ok(Box::new(provider))
        }
        ProtocolType::LlamaServer => {
            let base_url = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:8080".into());
            let provider = LlamaServerProvider::new(base_url, model_id);
            Ok(Box::new(provider))
        }
    }
}

/// Build a provider from a merged alias/provider configuration.
///
/// This is the preferred entry point for runtime provider construction:
/// it applies alias-level overrides and decrypts alias keys.
pub async fn build_provider_from_registry_entry(
    base_cfg: &ProviderConfig,
    entry: &ModelEntry,
    override_key: Option<&str>,
    secrets: Option<&clarity_secrets::SecretStore>,
) -> Result<Box<dyn LlmProvider>, AgentError> {
    let merged = entry.merge_into(base_cfg);
    build_provider_from_registry_with_key(
        &merged,
        &entry.model_id,
        override_key,
        entry.api_key.as_deref(),
        entry.api_key_env.as_deref(),
        secrets,
    )
    .await
}

/// Load the default `SecretStore` for the active user profile.
///
/// Search order:
/// 1. `CLARITY_SECRETS_KEY` env var (path to the master key file)
/// 2. `<config_dir>/clarity/secrets.key`
pub fn default_secret_store() -> Result<clarity_secrets::SecretStore, AgentError> {
    let key_path = if let Ok(path) = std::env::var("CLARITY_SECRETS_KEY") {
        PathBuf::from(path)
    } else {
        let dir = dirs::config_dir()
            .ok_or_else(|| {
                AgentError::Llm("Cannot determine config directory for secret store".into())
            })?
            .join("clarity");
        std::fs::create_dir_all(&dir)
            .map_err(|e| AgentError::Llm(format!("Failed to create config dir: {e}")))?;
        dir.join("secrets.key")
    };
    clarity_secrets::SecretStore::load_or_create(key_path)
        .map_err(|e| AgentError::Llm(format!("Failed to load secret store: {e}")))
}
