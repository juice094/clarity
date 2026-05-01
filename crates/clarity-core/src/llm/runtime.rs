//! Runtime Provider Configuration
//!
//! In-process cache for the currently active LLM provider configuration.
//! Set by the egui settings panel, consumed by `ensure_llm`.
//!
//! ## Architecture
//!
//! ```text
//! egui (settings UI)                    core (runtime)
//! ┌─────────────────┐                  ┌──────────────────┐
//! │ Provider Panel  │── set_provider_config() ──▶│ OnceLock cache    │
//! │ Test Connection │── test_connection() ──────▶│ reqwest HTTP      │
//! │ Refresh Models  │── list_models() ──────────▶│ GET /v1/models    │
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

use crate::error::AgentError;
use crate::llm::api::LlmProvider;
use crate::llm::{AnthropicLlm, LlamaServerProvider, OllamaProvider, OpenAiCompatibleLlm};

/// Runtime provider configuration set by the frontend settings panel.
///
/// Once written, this acts as the primary source of truth for `ensure_llm`,
/// bypassing the static `ModelRegistry` until `clear_provider_config()` is called.
#[derive(Debug, Clone)]
pub struct RuntimeProviderConfig {
    /// Human-readable identifier (e.g. "my-custom-openai").
    pub provider_id: String,
    /// API base URL (e.g. "https://api.openai.com/v1").
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

use std::sync::Mutex;

// ── Mutex cache ─────────────────────────────────────────────────────────────

static ACTIVE_CONFIG: Mutex<Option<RuntimeProviderConfig>> = Mutex::new(None);

/// Write the active provider config into the runtime cache.
pub fn set_provider_config(cfg: RuntimeProviderConfig) {
    let mut guard = ACTIVE_CONFIG.lock().unwrap();
    let provider_id = cfg.provider_id.clone();
    *guard = Some(cfg);
    tracing::info!("RuntimeProviderConfig set: provider_id={}", provider_id);
}

/// Get a reference to the active runtime config, if one is set.
pub fn get_active_config() -> Option<RuntimeProviderConfig> {
    ACTIVE_CONFIG.lock().unwrap().clone()
}

/// Clear the active runtime config.
pub fn clear_provider_config() {
    *ACTIVE_CONFIG.lock().unwrap() = None;
    tracing::info!("RuntimeProviderConfig cleared");
}

// ── Provider construction ──────────────────────────────────────────────────

/// Build an `LlmProvider` from the currently active runtime config.
///
/// Returns `Err` if no config has been set, or if the `api_format` is unknown.
pub async fn build_from_active_config() -> Result<Box<dyn LlmProvider>, AgentError> {
    let cfg = ACTIVE_CONFIG
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AgentError::Llm("No active runtime provider config".into()))?;
    build_provider(&cfg).await
}

async fn build_provider(cfg: &RuntimeProviderConfig) -> Result<Box<dyn LlmProvider>, AgentError> {
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
        other => Err(format!("Unsupported api_format for connection test: {}", other)),
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
/// For OpenAI-compatible APIs, issues `GET /v1/models` and extracts
/// `data[].id` from the response.
///
/// Returns an empty list for Anthropic (no public model listing endpoint).
pub async fn list_models(cfg: &RuntimeProviderConfig) -> Result<Vec<String>, String> {
    match cfg.api_format.as_str() {
        "openai_chat" | "ollama" | "llama_server" => fetch_openai_models(cfg).await,
        "anthropic_messages" => {
            tracing::warn!(
                "list_models: Anthropic does not expose a public model listing API"
            );
            Ok(Vec::new())
        }
        other => Err(format!("Unsupported api_format for model listing: {}", other)),
    }
}

async fn fetch_openai_models(cfg: &RuntimeProviderConfig) -> Result<Vec<String>, String> {
    let base = normalise_base_url(&cfg.base_url);
    let url = format!("{}/models", base);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", cfg.api_key))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("API error ({}): {}", status, body));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let models = body["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(models)
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
        assert_eq!(normalise_base_url("https://api.openai.com/v1"), "https://api.openai.com/v1");
        assert_eq!(normalise_base_url("https://api.openai.com/v1/"), "https://api.openai.com/v1");
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
    fn test_set_and_get_provider_config() {
        clear_provider_config();
        assert!(get_active_config().is_none());

        let cfg = RuntimeProviderConfig {
            provider_id: "test".into(),
            base_url: "https://test.com/v1".into(),
            api_format: "openai_chat".into(),
            api_key: "sk-test".into(),
            model: "test-model".into(),
        };
        set_provider_config(cfg.clone());

        let cached = get_active_config().expect("config should be set");
        assert_eq!(cached.provider_id, "test");
        assert_eq!(cached.base_url, "https://test.com/v1");
        assert_eq!(cached.api_format, "openai_chat");
        assert_eq!(cached.api_key, "sk-test");
        assert_eq!(cached.model, "test-model");

        clear_provider_config();
        assert!(get_active_config().is_none());
    }

    #[test]
    fn test_set_provider_config_replaces_old() {
        clear_provider_config();

        let cfg1 = RuntimeProviderConfig {
            provider_id: "first".into(),
            base_url: "https://first.com/v1".into(),
            api_format: "openai_chat".into(),
            api_key: "sk-1".into(),
            model: "model-1".into(),
        };
        set_provider_config(cfg1);

        let cached = get_active_config().expect("config should be set");
        assert_eq!(cached.provider_id, "first");

        let cfg2 = RuntimeProviderConfig {
            provider_id: "second".into(),
            base_url: "https://second.com/v1".into(),
            api_format: "anthropic_messages".into(),
            api_key: "sk-2".into(),
            model: "model-2".into(),
        };
        set_provider_config(cfg2);

        let cached = get_active_config().expect("config should be replaced");
        assert_eq!(cached.provider_id, "second");
        assert_eq!(cached.api_format, "anthropic_messages");

        clear_provider_config();
    }
}
