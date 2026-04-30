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

use crate::error::AgentError;
use crate::llm::api::LlmProvider;
use crate::llm::{LlamaServerProvider, OpenAiCompatibleLlm};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Protocol adapter type for provider communication
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolType {
    /// OpenAI /v1/chat/completions compatible
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

/// Provider-level connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub protocol: ProtocolType,
    #[serde(default)]
    pub base_url: Option<String>,
    /// Environment variable name that holds the API key
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Provider-specific extra settings (model_path for local, etc.)
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

/// A user-facing model alias mapped to a concrete provider + model_id
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

/// Top-level configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelConfigFile {
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
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

    /// Built-in fallback when no config file exists — mirrors old LlmFactory::auto() behavior
    fn built_in_fallback() -> ModelConfigFile {
        let mut providers = HashMap::new();
        let mut models = Vec::new();

        if std::env::var("ANTHROPIC_AUTH_TOKEN").is_ok() {
            providers.insert(
                "anthropic".to_string(),
                ProviderConfig {
                    protocol: ProtocolType::AnthropicMessages,
                    base_url: Some("https://api.anthropic.com".into()),
                    api_key_env: Some("ANTHROPIC_AUTH_TOKEN".into()),
                    extra: HashMap::new(),
                },
            );
            models.push(ModelEntry {
                alias: "claude-sonnet".into(),
                provider: "anthropic".into(),
                model_id: std::env::var("ANTHROPIC_MODEL")
                    .unwrap_or_else(|_| "claude-3-7-sonnet-20250219".into()),
                temperature: None,
                max_tokens: None,
            });
        }

        if std::env::var("KIMI_CODE_API_KEY").is_ok() {
            providers.insert(
                "kimi-code".to_string(),
                ProviderConfig {
                    protocol: ProtocolType::OpenAiChat,
                    base_url: Some("https://api.kimi.com/coding/v1".into()),
                    api_key_env: Some("KIMI_CODE_API_KEY".into()),
                    extra: HashMap::new(),
                },
            );
            models.push(ModelEntry {
                alias: "kimi-k2".into(),
                provider: "kimi-code".into(),
                model_id: std::env::var("KIMI_MODEL").unwrap_or_else(|_| "kimi-k2.6".into()),
                temperature: None,
                max_tokens: None,
            });
        } else if std::env::var("KIMI_API_KEY").is_ok() {
            providers.insert(
                "kimi".to_string(),
                ProviderConfig {
                    protocol: ProtocolType::OpenAiChat,
                    base_url: Some("https://api.moonshot.cn/v1".into()),
                    api_key_env: Some("KIMI_API_KEY".into()),
                    extra: HashMap::new(),
                },
            );
            models.push(ModelEntry {
                alias: "kimi-k2".into(),
                provider: "kimi".into(),
                model_id: std::env::var("KIMI_MODEL").unwrap_or_else(|_| "kimi-k2.6".into()),
                temperature: None,
                max_tokens: None,
            });
        }

        if std::env::var("DEEPSEEK_API_KEY").is_ok() {
            providers.insert(
                "deepseek".to_string(),
                ProviderConfig {
                    protocol: ProtocolType::OpenAiChat,
                    base_url: Some("https://api.deepseek.com".into()),
                    api_key_env: Some("DEEPSEEK_API_KEY".into()),
                    extra: HashMap::new(),
                },
            );
            models.push(ModelEntry {
                alias: "deepseek-chat".into(),
                provider: "deepseek".into(),
                model_id: std::env::var("DEEPSEEK_MODEL")
                    .unwrap_or_else(|_| "deepseek-chat".into()),
                temperature: None,
                max_tokens: None,
            });
            models.push(ModelEntry {
                alias: "deepseek-reasoner".into(),
                provider: "deepseek".into(),
                model_id: "deepseek-reasoner".into(),
                temperature: None,
                max_tokens: None,
            });
        }

        if std::env::var("OPENAI_API_KEY").is_ok() {
            providers.insert(
                "openai".to_string(),
                ProviderConfig {
                    protocol: ProtocolType::OpenAiChat,
                    base_url: Some("https://api.openai.com/v1".into()),
                    api_key_env: Some("OPENAI_API_KEY".into()),
                    extra: HashMap::new(),
                },
            );
            models.push(ModelEntry {
                alias: "gpt-4o".into(),
                provider: "openai".into(),
                model_id: std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".into()),
                temperature: None,
                max_tokens: None,
            });
        }

        // Local fallback
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
                    extra,
                },
            );
            models.push(ModelEntry {
                alias: "local-qwen".into(),
                provider: "local".into(),
                model_id: "Qwen2.5-7B-Instruct".into(),
                temperature: None,
                max_tokens: None,
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

/// Build a concrete provider from registry config + model_id.
/// This is used by the legacy `LlmFactory` in `mod.rs`.
pub async fn build_provider_from_registry(
    cfg: &ProviderConfig,
    model_id: &str,
) -> Result<Box<dyn LlmProvider>, AgentError> {
    build_provider_from_registry_with_key(cfg, model_id, None).await
}

/// Build a provider with an optional API-key override (e.g. from GUI Settings).
pub async fn build_provider_from_registry_with_key(
    cfg: &ProviderConfig,
    model_id: &str,
    override_key: Option<&str>,
) -> Result<Box<dyn LlmProvider>, AgentError> {
    match cfg.protocol {
        ProtocolType::OpenAiChat => {
            let api_key = override_key
                .map(|s| s.to_string())
                .or_else(|| cfg.api_key_env.as_ref().and_then(|env_var| std::env::var(env_var).ok()))
                .unwrap_or_default();
            let base_url = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".into());
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
            let mut gguf_config = super::local_gguf::LocalGgufConfig::new(model_path);
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
