//! Factory for creating LLM providers.
//!
//! **Frozen for new providers** — use [`ModelRegistry::load()`] +
//! [`build_provider_from_registry()`](crate::model_registry::build_provider_from_registry)
//! for configuration-driven routing.
//!
//! `auto()` / `create()` / `create_with_key()` remain active for backward
//! compatibility with existing callers (gateway, tui, egui). They first
//! consult the registry, then fall back to legacy env-var detection.
//!
//! Provider-specific helpers (`anthropic`, `deepseek`, `kimi`, `openai`)
//! are deprecated; prefer registry aliases or `create_with_key()`.

use crate::api::LlmProvider;
use crate::auth::OAuthTokenManager;
use crate::deepseek::DeepSeekProvider;
use crate::deepseek_device::{DeepSeekDeviceOptions, DeepSeekDeviceProvider};
use crate::model_registry::{
    build_provider_from_registry_entry, default_secret_store, ModelRegistry,
};
use crate::providers::{AnthropicLlm, KimiCodeLlm, KimiLlm, OAuthLlm, OpenAiCompatibleLlm};
use crate::resolve_local_model_path;
use crate::LOCAL_MODEL_HELP;
use clarity_contract::AgentError;
use std::env;
use std::sync::Arc;

#[cfg(feature = "local-llm")]
use crate::local_gguf::{LocalGgufConfig, LocalGgufProvider};

/// Factory for creating LLM providers.
///
/// **Frozen for new providers** — use `ModelRegistry::load()` +
/// `build_provider_from_registry()` for configuration-driven routing.
///
/// `auto()` / `create()` / `create_with_key()` remain active for backward
/// compatibility with existing callers (gateway, tui, egui). They first
/// consult the registry, then fall back to legacy env-var detection.
///
/// Provider-specific helpers (`anthropic`, `deepseek`, `kimi`, `openai`)
/// are deprecated; prefer registry aliases or `create_with_key()`.
pub struct LlmFactory;

impl LlmFactory {
    /// Auto-detect provider — uses ModelRegistry if available, otherwise legacy env-var scan.
    pub async fn auto() -> Result<Box<dyn LlmProvider>, AgentError> {
        // Try registry first.
        match ModelRegistry::load_async().await {
            Ok(registry) => {
                if let Some(first) = registry.list_models().into_iter().next() {
                    return Self::create(&first.alias).await;
                }
            }
            Err(e) => {
                tracing::debug!(
                    "ModelRegistry not available ({}), falling back to legacy auto-detect",
                    e
                );
            }
        }

        // Legacy fallback: hard-coded env-var priority.
        if env::var("ANTHROPIC_AUTH_TOKEN").is_ok() {
            return Ok(Box::new(AnthropicLlm::from_env()?));
        }

        if env::var("KIMI_CODE_API_KEY").is_ok() {
            return Ok(Box::new(KimiCodeLlm::from_env()?));
        }

        if let Ok(kimi_key) = env::var("KIMI_API_KEY") {
            if kimi_key.starts_with("sk-kimi-") {
                let base_url = env::var("KIMI_BASE_URL")
                    .unwrap_or_else(|_| "https://api.kimi.com/coding/v1".into());
                let model = env::var("KIMI_MODEL").unwrap_or_else(|_| "kimi-k2.6".into());
                return Ok(Box::new(OAuthLlm::new(
                    kimi_key,
                    base_url,
                    model,
                    OAuthTokenManager::new(),
                )));
            }
            return Ok(Box::new(KimiLlm::from_env()?));
        }

        if env::var("DEEPSEEK_API_KEY").is_ok() {
            return Ok(Box::new(DeepSeekProvider::from_env()?));
        }

        if env::var("OPENAI_API_KEY").is_ok() {
            return Ok(Box::new(OpenAiCompatibleLlm::from_env()?));
        }

        #[cfg(feature = "local-llm")]
        if let Some(model_path) = resolve_local_model_path() {
            tracing::info!(
                "No cloud LLM configured; falling back to local GGUF model at {}",
                model_path.display()
            );
            let repo = std::env::var("CLARITY_LOCAL_TOKENIZER_REPO")
                .unwrap_or_else(|_| "Qwen/Qwen2.5-7B-Instruct".into());
            let config = LocalGgufConfig::new(model_path)?.with_tokenizer_repo(repo);
            return Ok(Box::new(LocalGgufProvider::new(config).await?));
        }

        Err(AgentError::Llm(
            "No LLM provider configured. Please set one of:\n\
             - ANTHROPIC_AUTH_TOKEN (for Claude)\n\
             - KIMI_CODE_API_KEY (for Kimi Code)\n\
             - KIMI_API_KEY (for Moonshot)\n\
             - DEEPSEEK_API_KEY\n\
             - OPENAI_API_KEY\n\
             Or create ~/.config/clarity/models.toml\n\
             Or use local inference:\n"
                .to_string()
                + LOCAL_MODEL_HELP,
        ))
    }

    /// Auto-detect provider, returning an `Arc` for direct use with `Agent::set_llm`.
    pub async fn auto_arc() -> Result<Arc<dyn LlmProvider>, AgentError> {
        Self::auto().await.map(Arc::from)
    }

    /// Create a provider by alias or legacy name.
    ///
    /// First checks ModelRegistry (with encrypted keys from the default
    /// secret store), then falls back to hard-coded legacy names.
    #[allow(deprecated)]
    pub async fn create(name: &str) -> Result<Box<dyn LlmProvider>, AgentError> {
        // Try registry first.
        if let Ok(registry) = ModelRegistry::load_async().await {
            if let Some(entry) = registry.get(name) {
                if let Some(provider_cfg) = registry.get_provider(&entry.provider) {
                    let secrets = default_secret_store().ok();
                    return build_provider_from_registry_entry(
                        provider_cfg,
                        entry,
                        None,
                        secrets.as_ref(),
                    )
                    .await;
                }
            }
        }

        // Legacy fallback.
        let lower = name.to_lowercase();
        match lower.as_str() {
            "anthropic" | "claude" => Ok(Box::new(Self::anthropic()?)),
            "deepseek" => Ok(Box::new(Self::deepseek()?)),
            "openai" => Ok(Box::new(Self::openai()?)),
            "kimi" | "kimi-code" | "moonshot" | "kimi_code" => {
                if lower == "kimi_code" || env::var("KIMI_CODE_API_KEY").is_ok() {
                    Ok(Box::new(KimiCodeLlm::from_env()?))
                } else {
                    Ok(Box::new(Self::kimi()?))
                }
            }
            "kalosm" | "local" => {
                #[cfg(feature = "local-llm")]
                if let Some(model_path) = resolve_local_model_path() {
                    let repo = std::env::var("CLARITY_LOCAL_TOKENIZER_REPO")
                        .unwrap_or_else(|_| "Qwen/Qwen2.5-7B-Instruct".into());
                    let config = LocalGgufConfig::new(model_path)?.with_tokenizer_repo(repo);
                    return Ok(Box::new(LocalGgufProvider::new(config).await?));
                }
                Err(AgentError::Llm(
                    "Local LLM not available. Ensure the local-llm feature is enabled.\n"
                        .to_string()
                        + LOCAL_MODEL_HELP,
                ))
            }
            _ => Err(AgentError::Llm(format!(
                "Unknown model alias '{}'. Create ~/.config/clarity/models.toml or use a legacy name: anthropic, kimi, deepseek, openai, kalosm",
                name
            ))),
        }
    }

    /// Create a provider by alias, returning an `Arc` for direct use with `Agent::set_llm`.
    pub async fn create_arc(name: &str) -> Result<Arc<dyn LlmProvider>, AgentError> {
        Self::create(name).await.map(Arc::from)
    }

    /// Create a provider with an explicit API key, bypassing environment variables.
    ///
    /// Used by the Tauri GUI so users can configure provider + key through Settings.
    pub fn create_with_key(
        name: &str,
        api_key: &str,
        model: &str,
    ) -> Result<Box<dyn LlmProvider>, AgentError> {
        let lower = name.to_lowercase();
        // kimi_code supports OAuth: empty key is okay (token loaded from file).
        if api_key.is_empty() && lower != "kimi_code" {
            return Err(AgentError::Llm(format!(
                "Provider '{}' requires an API key. Please enter it in Settings.",
                name
            )));
        }
        match lower.as_str() {
            "anthropic" | "claude" => Ok(Box::new(AnthropicLlm::new(
                api_key,
                "https://api.anthropic.com",
                model,
            ))),
            "deepseek" => Ok(Box::new(DeepSeekProvider::new(
                api_key,
                "https://api.deepseek.com/v1",
                if model.is_empty() {
                    "deepseek-chat"
                } else {
                    model
                },
            ))),
            "deepseek-device" => {
                let options = DeepSeekDeviceOptions::from_model_id(model);
                Ok(Box::new(DeepSeekDeviceProvider::with_token_and_options(
                    api_key, options,
                )))
            }
            "openai" => Ok(Box::new(OpenAiCompatibleLlm::new(
                api_key,
                "https://api.openai.com/v1",
                if model.is_empty() { "gpt-4o" } else { model },
            ))),
            "kimi" | "kimi-code" | "moonshot" | "kimi_code" => {
                let is_kimi_code = lower == "kimi_code" || api_key.starts_with("sk-kimi-");
                if is_kimi_code {
                    Ok(Box::new(OAuthLlm::new(
                        api_key,
                        "https://api.kimi.com/coding/v1",
                        if model.is_empty() { "kimi-k2.6" } else { model },
                        OAuthTokenManager::new(),
                    )))
                } else {
                    Ok(Box::new(KimiLlm::new(
                        api_key,
                        "https://api.moonshot.ai/v1",
                        if model.is_empty() { "kimi-k2.6" } else { model },
                    )))
                }
            }
            "ollama" => Ok(Box::new(OpenAiCompatibleLlm::new(
                api_key, // Ollama usually doesn't need a key, but we pass it anyway.
                "http://localhost:11434/v1",
                if model.is_empty() { "llama3.2" } else { model },
            ))),
            _ => Err(AgentError::Llm(format!(
                "Unknown provider '{}'. Supported: openai, anthropic, kimi, deepseek, deepseek-device, ollama, local",
                name
            ))),
        }
    }

    /// `Arc` wrapper for `create_with_key`.
    pub fn create_with_key_arc(
        name: &str,
        api_key: &str,
        model: &str,
    ) -> Result<Arc<dyn LlmProvider>, AgentError> {
        Self::create_with_key(name, api_key, model).map(Arc::from)
    }

    /// Create an Anthropic provider from environment.
    #[deprecated(
        since = "0.3.2",
        note = "Use ModelRegistry + build_provider_from_registry() or create_with_key()"
    )]
    pub fn anthropic() -> Result<AnthropicLlm, AgentError> {
        AnthropicLlm::from_env()
    }

    /// Create a DeepSeek provider from environment.
    #[deprecated(
        since = "0.3.2",
        note = "Use ModelRegistry + build_provider_from_registry() or create_with_key()"
    )]
    pub fn deepseek() -> Result<DeepSeekProvider, AgentError> {
        DeepSeekProvider::from_env()
    }

    /// Create a Kimi provider from environment.
    #[deprecated(
        since = "0.3.2",
        note = "Use ModelRegistry + build_provider_from_registry() or create_with_key()"
    )]
    pub fn kimi() -> Result<KimiLlm, AgentError> {
        KimiLlm::from_env()
    }

    /// Create an OpenAI-compatible provider from environment.
    #[deprecated(
        since = "0.3.2",
        note = "Use ModelRegistry + build_provider_from_registry() or create_with_key()"
    )]
    pub fn openai() -> Result<OpenAiCompatibleLlm, AgentError> {
        OpenAiCompatibleLlm::from_env()
    }
}
