use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use std::sync::Arc;

use crate::handlers::AgentHandle;
use crate::server::AppState;
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetConfigRequest {
    pub provider: String,
    pub api_key: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConfigResponse {
    pub provider: String,
    pub api_key_masked: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConfigStatusResponse {
    pub configured: bool,
    pub config: Option<ConfigResponse>,
}

fn config_file_path() -> PathBuf {
    PathBuf::from(".clarity").join("user_config.json")
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}****{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Load persisted user config from JSON file
pub async fn load_persisted_config() -> Option<SetConfigRequest> {
    let path = config_file_path();
    if !path.exists() {
        return None;
    }
    let contents = tokio::fs::read_to_string(&path).await.ok()?;
    serde_json::from_str(&contents).ok()
}

/// Save user config to JSON file
async fn save_persisted_config(cfg: &SetConfigRequest) -> Result<(), String> {
    let path = config_file_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| e.to_string())
}

/// Build an LLM provider from a user config request
pub async fn build_provider_from_config(
    cfg: &SetConfigRequest,
) -> Result<Box<dyn clarity_core::agent::LlmProvider>, String> {
    use clarity_llm::LlmFactory;
    use clarity_llm::{
        AnthropicLlm, DeepSeekProvider, KimiLlm, OAuthLlm, OpenAiCompatibleLlm,
    };

    let provider_lower = cfg.provider.to_lowercase();
    match provider_lower.as_str() {
        "openai" => {
            let base = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".into());
            let model = cfg.model.clone().unwrap_or_else(|| "gpt-4o".into());
            Ok(Box::new(OpenAiCompatibleLlm::new(
                &cfg.api_key,
                base,
                model,
            )))
        }
        "kimi" | "moonshot" => {
            let base = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.moonshot.cn/v1".into());
            let model = cfg.model.clone().unwrap_or_else(|| "kimi-k2-07132k".into());
            Ok(Box::new(KimiLlm::new(&cfg.api_key, base, model)))
        }
        "kimi-code" | "kimi_code" => {
            let base = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.kimi.com/coding/v1".into());
            let model = cfg.model.clone().unwrap_or_else(|| "kimi-k2-07132k".into());
            Ok(Box::new(OAuthLlm::new(
                &cfg.api_key,
                base,
                model,
                clarity_llm::auth::OAuthTokenManager::new(),
            )))
        }
        "anthropic" | "claude" => {
            let base = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".into());
            let model = cfg
                .model
                .clone()
                .unwrap_or_else(|| "claude-3-5-sonnet-20241022".into());
            Ok(Box::new(AnthropicLlm::new(&cfg.api_key, base, model)))
        }
        "deepseek" => {
            let base = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.deepseek.com/v1".into());
            let model = cfg.model.clone().unwrap_or_else(|| "deepseek-chat".into());
            Ok(Box::new(DeepSeekProvider::new(&cfg.api_key, base, model)))
        }
        alias => {
            // Try ModelRegistry alias
            match LlmFactory::create(alias).await {
                Ok(p) => Ok(p),
                Err(e) => Err(format!("Unknown provider '{}': {}", cfg.provider, e)),
            }
        }
    }
}

pub(crate) async fn admin_get_config() -> impl IntoResponse {
    match load_persisted_config().await {
        Some(cfg) => {
            let resp = ConfigResponse {
                provider: cfg.provider.clone(),
                api_key_masked: mask_key(&cfg.api_key),
                base_url: cfg.base_url.clone(),
                model: cfg.model.clone(),
            };
            (
                StatusCode::OK,
                Json(ConfigStatusResponse {
                    configured: true,
                    config: Some(resp),
                }),
            )
        }
        None => (
            StatusCode::OK,
            Json(ConfigStatusResponse {
                configured: false,
                config: None,
            }),
        ),
    }
}

pub(crate) async fn admin_set_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetConfigRequest>,
) -> impl IntoResponse {
    if req.api_key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "api_key is required"})),
        );
    }
    if req.provider.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "provider is required"})),
        );
    }

    // Validate by trying to build the provider
    match build_provider_from_config(&req).await {
        Ok(provider) => {
            // Save to file
            if let Err(e) = save_persisted_config(&req).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to save config: {}", e)})),
                );
            }
            // Apply to agent
            state.set_llm(Arc::from(provider));

            let resp = ConfigResponse {
                provider: req.provider.clone(),
                api_key_masked: mask_key(&req.api_key),
                base_url: req.base_url.clone(),
                model: req.model.clone(),
            };
            (
                StatusCode::OK,
                Json(json!({"status": "ok", "config": resp})),
            )
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
    }
}

