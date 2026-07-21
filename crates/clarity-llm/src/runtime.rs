//! Runtime Provider Configuration
//!
//! Provides pure functions for constructing LLM providers from an explicit
//! [`RuntimeProviderConfig`]. The config is a value type: callers derive it
//! from frontend settings on every `ensure_llm` call and pass it directly to
//! [`build_provider`]. There is no global mutable cache.
//!
//! ```text
//! egui (settings UI)                    core (runtime)
//! ┌─────────────────┐                  ┌──────────────────┐
//! │ Provider Panel  │── derive config ──▶│ RuntimeProviderConfig
//! │ Test Connection │── test_connection() │  (value, no cache) │
//! │ Refresh Models  │── list_models() ────│                  │
//! └─────────────────┘                  └──────────────────┘
//!                                             │
//!                                             ▼
//!                                      ┌──────────────────┐
//!                                      │ build_provider()  │──▶ LlmProvider
//!                                      └──────────────────┘
//! ```
//!
//! **Why not Wire?** egui and core are the same process. Wire is for
//! Soul→UI event streaming (conversation turns, tool calls). Provider
//! config management is a local setup concern — wrapping it in broadcast
//! channel messages adds serde tag complexity and handler overhead for
//! zero benefit.

use crate::api::LlmProvider;
use crate::catalog::service::ModelCatalogService;
use crate::{AnthropicLlm, LlamaServerProvider, OllamaProvider, OpenAiCompatibleLlm};
use clarity_contract::AgentError;

/// Runtime provider configuration.
///
/// A value type representing the resolved parameters for a single LLM provider.
/// Callers derive it from frontend settings on every `ensure_llm` call rather
/// than caching it globally.
#[derive(Debug, Clone)]
pub struct RuntimeProviderConfig {
    /// Human-readable identifier (e.g. "my-custom-openai").
    pub provider_id: String,
    /// API base URL (e.g. <https://api.openai.com/v1>).
    pub base_url: String,
    /// Protocol format identifier.
    ///
    /// Known values: `"openai_chat"`, `"anthropic_messages"`, `"ollama"`, `"llama_server"`.
    /// The `"openai_chat"` variant covers all OpenAI-compatible APIs including
    /// Kimi (Moonshot), DeepSeek, and any other provider using `/v1/chat/completions`.
    pub api_format: String,
    /// Resolved API key (after `${env:VAR}` substitution).
    pub api_key: String,
    /// Model identifier (e.g. "gpt-4o", "kimi-k2.6", "claude-sonnet-4-20250514").
    pub model: String,
}

impl RuntimeProviderConfig {
    /// Whether this provider's model catalog can be pulled from a remote API.
    ///
    /// Delegates to [`crate::catalog::capability`], the single source of truth
    /// for catalog-pull capability. Frontends should gate any "refresh models"
    /// affordance on this instead of hard-coding provider lists.
    pub fn supports_model_catalog(&self) -> bool {
        crate::catalog::capability::api_format_supports_catalog(&self.api_format)
    }
}

// ── Provider construction ──────────────────────────────────────────────────

/// Build an `LlmProvider` from an explicit [`RuntimeProviderConfig`].
///
/// This is the preferred entry point for S3.3+. The caller derives the config
/// from frontend settings and passes it directly — no global cache involved.
pub async fn build_provider(
    cfg: &RuntimeProviderConfig,
) -> Result<Box<dyn LlmProvider>, AgentError> {
    match cfg.api_format.as_str() {
        "openai_chat" => {
            let llm = OpenAiCompatibleLlm::new(&cfg.api_key, &cfg.base_url, &cfg.model);
            Ok(Box::new(llm))
        }
        "anthropic_messages" => {
            let llm = AnthropicLlm::new(&cfg.api_key, &cfg.base_url, &cfg.model);
            Ok(Box::new(llm))
        }
        "ollama" => {
            let provider = OllamaProvider::new(&cfg.base_url, &cfg.model);
            Ok(Box::new(provider))
        }
        "llama_server" => {
            let provider = LlamaServerProvider::new(&cfg.base_url, &cfg.model);
            Ok(Box::new(provider))
        }
        other => Err(AgentError::Llm(format!(
            "Unknown api_format '{}'. Supported: openai_chat, anthropic_messages, ollama, llama_server",
            other
        ))),
    }
}

// ── Connection testing ─────────────────────────────────────────────────────

/// Test connectivity to a provider endpoint.
///
/// For OpenAI-compatible APIs, issues a `GET /v1/models` request with the
/// configured API key. For Anthropic, probes the messages endpoint.
///
/// This function performs automatic `/v1` suffix normalisation — if the
/// configured `base_url` does not end with `/v1`, it is appended before
/// the request is sent.
pub async fn test_connection(cfg: &RuntimeProviderConfig) -> Result<(), String> {
    match cfg.api_format.as_str() {
        "openai_chat" | "ollama" | "llama_server" => test_openai_connection(cfg).await,
        "anthropic_messages" => test_anthropic_connection(cfg).await,
        other => Err(format!(
            "Unsupported api_format for connection test: {}",
            other
        )),
    }
}

async fn test_openai_connection(cfg: &RuntimeProviderConfig) -> Result<(), String> {
    let base = normalise_base_url(&cfg.base_url);
    let url = format!("{}/models", base);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", cfg.api_key))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(format!("API error ({}): {}", status, body))
    }
}

async fn test_anthropic_connection(cfg: &RuntimeProviderConfig) -> Result<(), String> {
    let base = cfg.base_url.trim_end_matches('/');
    // Anthropic doesn't have a simple GET endpoint; we probe the messages
    // endpoint with a minimal request (no messages) to verify reachability.
    let url = format!("{}/v1/messages", base);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let resp = client
        .get(&url)
        .header("x-api-key", &cfg.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    // A 400 (bad request) with "messages must be non-empty" means the
    // endpoint is reachable and the key is valid — that's success for us.
    let status = resp.status();
    if status.is_success() || status.as_u16() == 400 {
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!("API error ({}): {}", status, body))
    }
}

// ── Model listing ──────────────────────────────────────────────────────────

/// Fetch the list of available models from a provider.
///
/// Delegates to [`ModelCatalogService`] so the result is persisted in the shared
/// on-disk cache (`~/.clarity/catalogs/{provider_id}.json`) and can be reused
/// by [`crate::model_listing::get_available_models`].
///
/// Returns an empty list for Anthropic (no public model listing endpoint).
pub async fn list_models(cfg: &RuntimeProviderConfig) -> Result<Vec<String>, String> {
    let service = ModelCatalogService::with_defaults()
        .map_err(|e| format!("Failed to open catalog cache: {}", e))?;
    let api_key = if cfg.api_key.is_empty() {
        None
    } else {
        Some(cfg.api_key.as_str())
    };
    let entries = service
        .refresh_provider(&cfg.provider_id, &cfg.base_url, api_key, &cfg.api_format)
        .await
        .map_err(|e| format!("Failed to refresh model list: {}", e))?;
    Ok(entries.into_iter().map(|e| e.model_id).collect())
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Ensure the base URL ends with `/v1`.
///
/// If the URL already ends with `/v1` (with or without trailing slash),
/// it is returned as-is. Otherwise `/v1` is appended.
fn normalise_base_url(url: &str) -> String {
    let url = url.trim_end_matches('/');
    if url.ends_with("/v1") {
        url.to_string()
    } else {
        format!("{}/v1", url)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalise_base_url_already_v1() {
        assert_eq!(
            normalise_base_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalise_base_url("https://api.openai.com/v1/"),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn test_normalise_base_url_adds_v1() {
        assert_eq!(
            normalise_base_url("https://api.openai.com"),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalise_base_url("https://api.kimi.com/coding"),
            "https://api.kimi.com/coding/v1"
        );
    }

    #[test]
    fn test_normalise_base_url_trailing_slash() {
        assert_eq!(
            normalise_base_url("https://api.openai.com/v1/"),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn test_supports_model_catalog_matrix() {
        let cfg = |api_format: &str| RuntimeProviderConfig {
            provider_id: "test".into(),
            base_url: "https://test.com/v1".into(),
            api_format: api_format.into(),
            api_key: "sk-test".into(),
            model: "m".into(),
        };
        assert!(cfg("openai_chat").supports_model_catalog());
        assert!(cfg("ollama").supports_model_catalog());
        assert!(cfg("llama_server").supports_model_catalog());
        assert!(!cfg("anthropic_messages").supports_model_catalog());
        assert!(!cfg("deepseek_device").supports_model_catalog());
        assert!(!cfg("openclaw").supports_model_catalog());
    }

    #[test]
    fn test_build_provider_unknown_format() {
        let cfg = RuntimeProviderConfig {
            provider_id: "test".into(),
            base_url: "https://test.com/v1".into(),
            api_format: "unknown_format".into(),
            api_key: "sk-test".into(),
            model: "test-model".into(),
        };
        // build_provider is async; use a minimal runtime for the test.
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = rt.block_on(build_provider(&cfg));
        let err = match result {
            Ok(_) => panic!("expected error for unknown api_format"),
            Err(e) => e,
        };
        match err {
            AgentError::Llm(msg) => {
                assert!(
                    msg.contains("Unknown api_format"),
                    "unexpected error: {}",
                    msg
                );
            }
            other => panic!("expected Llm error, got {:?}", other),
        }
    }
}
