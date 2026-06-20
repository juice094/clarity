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

/// Request to create or update an alias and make it active.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetAliasConfigRequest {
    /// Alias name (e.g. "deepseek-v4-pro"). If it already exists it is updated.
    pub alias: String,
    /// Provider family (e.g. "deepseek", "openai", "anthropic").
    pub provider: String,
    /// Plaintext API key. It is encrypted with the project secret store before persistence.
    pub api_key: String,
    /// Optional model ID override. Defaults to the alias name.
    #[serde(default)]
    pub model_id: Option<String>,
    /// Optional base URL override.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Optional protocol override, e.g. "openai_chat", "deepseek_device".
    /// Defaults to the provider family's existing protocol or "openai_chat".
    #[serde(default)]
    pub protocol: Option<String>,
    /// Fallback aliases to try if this provider fails.
    #[serde(default)]
    pub fallback_aliases: Vec<String>,
}

/// Response returned by the admin config endpoints.
#[derive(Debug, Serialize)]
pub struct AliasConfigResponse {
    /// Active alias name.
    pub alias: String,
    /// Provider family, e.g. `"deepseek"`.
    pub provider: String,
    /// Masked API key for display.
    pub api_key_masked: String,
    /// Resolved model identifier.
    pub model_id: String,
    /// Optional custom base URL.
    pub base_url: Option<String>,
    /// Fallback aliases for failover.
    pub fallback_aliases: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConfigStatusResponse {
    pub configured: bool,
    pub config: Option<AliasConfigResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveAliasFile {
    alias: String,
}

fn project_clarity_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".clarity")
}

fn models_toml_path() -> PathBuf {
    project_clarity_dir().join("models.toml")
}

fn active_alias_path() -> PathBuf {
    project_clarity_dir().join("active_alias.json")
}

fn secret_key_path() -> PathBuf {
    project_clarity_dir().join("secrets.key")
}

/// Parse a protocol string into [`clarity_llm::ProtocolType`].
fn parse_protocol(s: &str) -> Option<clarity_llm::ProtocolType> {
    match s {
        "openai_chat" => Some(clarity_llm::ProtocolType::OpenAiChat),
        "anthropic_messages" => Some(clarity_llm::ProtocolType::AnthropicMessages),
        "ollama" => Some(clarity_llm::ProtocolType::Ollama),
        "llama_server" => Some(clarity_llm::ProtocolType::LlamaServer),
        "deepseek_device" => Some(clarity_llm::ProtocolType::DeepSeekDevice),
        _ => None,
    }
}

fn mask_key(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }
    if key.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}****{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Load the project-level secret store.
pub fn load_secret_store() -> Result<clarity_secrets::SecretStore, String> {
    let dir = project_clarity_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create .clarity dir: {e}"))?;
    clarity_secrets::SecretStore::load_or_create(secret_key_path())
        .map_err(|e| format!("Failed to load secret store: {e}"))
}

/// Load the project-level model registry.
///
/// Falls back to the user-level default registry when no project-level file exists.
pub async fn load_model_registry() -> Result<clarity_llm::ModelRegistry, String> {
    let path = models_toml_path();
    if path.exists() {
        let contents = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| format!("Failed to read models.toml: {e}"))?;
        let file: clarity_llm::ModelConfigFile =
            toml::from_str(&contents).map_err(|e| format!("Failed to parse models.toml: {e}"))?;
        clarity_llm::ModelRegistry::from_config(file)
            .map_err(|e| format!("Invalid model registry: {e}"))
    } else {
        clarity_llm::ModelRegistry::load_async()
            .await
            .map_err(|e| format!("Failed to load default model registry: {e}"))
    }
}

async fn save_model_registry(registry: &clarity_llm::ModelRegistry) -> Result<(), String> {
    let path = models_toml_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create .clarity dir: {e}"))?;
    }
    let file = registry.config().clone();
    let text = toml::to_string_pretty(&file)
        .map_err(|e| format!("Failed to serialize models.toml: {e}"))?;
    tokio::fs::write(&path, text)
        .await
        .map_err(|e| format!("Failed to write models.toml: {e}"))
}

/// Load the currently active alias, if any.
/// Load the currently active alias, if any.
pub async fn load_active_alias() -> Option<String> {
    let path = active_alias_path();
    if !path.exists() {
        return None;
    }
    let contents = tokio::fs::read_to_string(&path).await.ok()?;
    let file: ActiveAliasFile = serde_json::from_str(&contents).ok()?;
    Some(file.alias)
}

/// Persist the given alias as the active alias.
pub async fn save_active_alias(alias: &str) -> Result<(), String> {
    let path = active_alias_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    let file = ActiveAliasFile {
        alias: alias.to_string(),
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| e.to_string())
}

/// Build a concrete provider from an alias using the project-level encrypted store.
pub async fn build_provider_from_alias(
    alias: &str,
) -> Result<Box<dyn clarity_core::agent::LlmProvider>, String> {
    let registry = load_model_registry().await?;
    let entry = registry
        .get(alias)
        .ok_or_else(|| format!("Model alias '{}' not found in registry", alias))?
        .clone();
    let provider_cfg = registry
        .get_provider(&entry.provider)
        .ok_or_else(|| {
            format!(
                "Provider '{}' for alias '{}' not found",
                entry.provider, alias
            )
        })?
        .clone();
    let secrets = load_secret_store().map_err(|e| format!("Secret store error: {e}"))?;

    clarity_llm::build_provider_from_registry_entry(&provider_cfg, &entry, None, Some(&secrets))
        .await
        .map_err(|e| format!("Failed to build provider for alias '{}': {}", alias, e))
}

/// Build a provider chain for an alias, including configured fallbacks, wrapped
/// in [`clarity_llm::ReliableProvider`].
pub async fn build_reliable_provider_for_alias(
    alias: &str,
) -> Result<Arc<dyn clarity_core::agent::LlmProvider>, String> {
    // Router aliases are resolved at request time.
    if clarity_llm::runtime_router::RouterHint::is_router_alias(alias) {
        let registry = load_model_registry().await?;
        return clarity_llm::runtime_router::RouterLlmProvider::from_alias(alias, registry)
            .map(|router| {
                tracing::info!("Built runtime router for alias: {}", alias);
                Arc::new(router) as Arc<dyn clarity_core::agent::LlmProvider>
            })
            .ok_or_else(|| format!("Invalid router alias '{}'", alias));
    }

    let registry = load_model_registry().await?;
    let entry = registry
        .get(alias)
        .ok_or_else(|| format!("Model alias '{}' not found in registry", alias))?
        .clone();

    let mut providers: Vec<Arc<dyn clarity_core::agent::LlmProvider>> = Vec::new();

    // Primary alias
    match build_provider_from_alias(alias).await {
        Ok(p) => providers.push(Arc::from(p)),
        Err(e) => tracing::warn!("Failed to build primary provider '{}': {}", alias, e),
    }

    // Configured fallbacks
    for fallback in &entry.fallback_aliases {
        if fallback == alias {
            continue;
        }
        match build_provider_from_alias(fallback).await {
            Ok(p) => providers.push(Arc::from(p)),
            Err(e) => tracing::warn!(
                "Failed to build fallback provider '{}' for alias '{}': {}",
                fallback,
                alias,
                e
            ),
        }
    }

    if providers.is_empty() {
        return Err(format!(
            "Could not build any provider for alias '{}'",
            alias
        ));
    }

    Ok(Arc::new(clarity_llm::ReliableProvider::new(providers)))
}

/// Load the currently active alias and build its provider.
pub async fn load_active_provider() -> Option<Arc<dyn clarity_core::agent::LlmProvider>> {
    let alias = load_active_alias().await?;
    match build_reliable_provider_for_alias(&alias).await {
        Ok(provider) => {
            tracing::info!("Active provider built from alias: {}", alias);
            Some(provider)
        }
        Err(e) => {
            tracing::warn!(
                "Failed to build active provider for alias '{}': {}",
                alias,
                e
            );
            None
        }
    }
}

/// Apply a user-submitted alias configuration: encrypt the key, update
/// `models.toml`, and persist the alias as active.
pub async fn apply_alias_config(
    req: &SetAliasConfigRequest,
) -> Result<AliasConfigResponse, String> {
    if req.alias.is_empty() {
        return Err("alias is required".to_string());
    }
    if req.provider.is_empty() {
        return Err("provider is required".to_string());
    }
    if req.api_key.is_empty() {
        return Err("api_key is required".to_string());
    }

    let mut registry = load_model_registry().await?;
    let secrets = load_secret_store()?;
    let encrypted_key = secrets
        .encrypt(&req.api_key)
        .map_err(|e| format!("Failed to encrypt API key: {e}"))?;

    // Ensure provider family exists.
    if registry.get_provider(&req.provider).is_none() {
        // Create a minimal provider family. Protocol defaults to OpenAI-compatible
        // unless explicitly overridden (e.g. "deepseek_device").
        let protocol = req
            .protocol
            .as_deref()
            .and_then(parse_protocol)
            .unwrap_or(clarity_llm::ProtocolType::OpenAiChat);
        let mut provider_cfg = clarity_llm::ProviderConfig {
            protocol,
            auth_type: clarity_llm::AuthType::ApiKey,
            ..Default::default()
        };
        if let Some(ref url) = req.base_url {
            provider_cfg.base_url = Some(url.clone());
        }
        registry.add_provider(req.provider.clone(), provider_cfg);
    }

    // Update or create the alias entry.
    let model_id = req.model_id.clone().unwrap_or_else(|| req.alias.clone());
    let mut entry = clarity_llm::ModelEntry {
        alias: req.alias.clone(),
        provider: req.provider.clone(),
        model_id,
        api_key: Some(encrypted_key),
        fallback_aliases: req.fallback_aliases.clone(),
        ..Default::default()
    };
    if let Some(ref url) = req.base_url {
        entry.base_url = Some(url.clone());
    }
    registry.add_or_update_model(entry);

    save_model_registry(&registry).await?;
    save_active_alias(&req.alias).await?;

    Ok(AliasConfigResponse {
        alias: req.alias.clone(),
        provider: req.provider.clone(),
        api_key_masked: mask_key(&req.api_key),
        model_id: req.model_id.clone().unwrap_or_else(|| req.alias.clone()),
        base_url: req.base_url.clone(),
        fallback_aliases: req.fallback_aliases.clone(),
    })
}

pub(crate) async fn admin_get_config() -> impl IntoResponse {
    match load_active_alias().await {
        Some(alias) => match load_model_registry().await {
            Ok(registry) => match registry.get(&alias) {
                Some(entry) => {
                    let masked = entry.api_key.as_deref().map(mask_key).unwrap_or_default();
                    let resp = AliasConfigResponse {
                        alias: entry.alias.clone(),
                        provider: entry.provider.clone(),
                        api_key_masked: masked,
                        model_id: entry.model_id.clone(),
                        base_url: entry.base_url.clone(),
                        fallback_aliases: entry.fallback_aliases.clone(),
                    };
                    (
                        StatusCode::OK,
                        Json(json!(ConfigStatusResponse {
                            configured: true,
                            config: Some(resp),
                        })),
                    )
                }
                None => (
                    StatusCode::OK,
                    Json(json!(ConfigStatusResponse {
                        configured: false,
                        config: None,
                    })),
                ),
            },
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to load model registry: {}", e)})),
            ),
        },
        None => (
            StatusCode::OK,
            Json(json!(ConfigStatusResponse {
                configured: false,
                config: None,
            })),
        ),
    }
}

pub(crate) async fn admin_set_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetAliasConfigRequest>,
) -> impl IntoResponse {
    match apply_alias_config(&req).await {
        Ok(resp) => {
            // Try to build the provider chain and apply it to the running agent.
            match build_reliable_provider_for_alias(&resp.alias).await {
                Ok(provider) => {
                    state.set_llm(provider);
                    (
                        StatusCode::OK,
                        Json(json!({"status": "ok", "config": resp})),
                    )
                }
                Err(e) => (
                    StatusCode::BAD_REQUEST,
                    Json(
                        json!({"error": format!("Config saved but provider build failed: {}", e)}),
                    ),
                ),
            }
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
    }
}

// ==================== OAuth device flow admin endpoints ====================

/// Request to start an OAuth device flow.
#[derive(Debug, Deserialize)]
pub struct StartOAuthRequest {
    /// Provider name to start OAuth for.
    pub provider: String,
}

/// Response returned when starting an OAuth device flow.
#[derive(Debug, Serialize)]
pub struct StartOAuthResponse {
    /// Provider name.
    pub provider: String,
    /// User-facing verification code.
    pub user_code: String,
    /// URL the user must visit to authorize.
    pub verification_uri: String,
    /// URL with the code pre-filled, if available.
    pub verification_uri_complete: String,
    /// Lifetime of the device code in seconds.
    pub expires_in: Option<u64>,
    /// Polling interval in seconds.
    pub interval: u64,
}

/// Request to poll an OAuth device flow for completion.
#[derive(Debug, Deserialize)]
pub struct PollOAuthRequest {
    /// Provider name.
    pub provider: String,
    /// Device code returned by the start endpoint.
    pub device_code: String,
    /// Polling interval in seconds.
    pub interval: u64,
}

/// Response returned by the OAuth polling endpoint.
#[derive(Debug, Serialize)]
pub struct PollOAuthResponse {
    /// Provider name.
    pub provider: String,
    /// Current authorization status.
    pub status: String,
    /// Masked access token, if authorized.
    pub access_token_masked: Option<String>,
}

pub(crate) async fn admin_start_oauth(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartOAuthRequest>,
) -> impl IntoResponse {
    match state.oauth_service.start_device_flow(&req.provider).await {
        Ok(auth) => {
            let resp = StartOAuthResponse {
                provider: req.provider,
                user_code: auth.user_code,
                verification_uri: auth.verification_uri,
                verification_uri_complete: auth.verification_uri_complete,
                expires_in: auth.expires_in,
                interval: auth.interval,
            };
            (StatusCode::OK, Json(json!({"status": "ok", "auth": resp})))
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Failed to start OAuth flow: {}", e)})),
        ),
    }
}

pub(crate) async fn admin_poll_oauth(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PollOAuthRequest>,
) -> impl IntoResponse {
    let auth = clarity_llm::auth::DeviceAuthorization {
        user_code: String::new(),
        device_code: req.device_code,
        verification_uri: String::new(),
        verification_uri_complete: String::new(),
        expires_in: None,
        interval: req.interval,
    };
    match state
        .oauth_service
        .poll_device_flow(&req.provider, &auth)
        .await
    {
        Ok(token) => {
            let masked = if token.access_token.len() > 8 {
                Some(format!(
                    "{}****{}",
                    &token.access_token[..4],
                    &token.access_token[token.access_token.len() - 4..]
                ))
            } else {
                None
            };
            let resp = PollOAuthResponse {
                provider: req.provider,
                status: "authorized".to_string(),
                access_token_masked: masked,
            };
            (StatusCode::OK, Json(json!({"status": "ok", "token": resp})))
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("OAuth polling failed: {}", e)})),
        ),
    }
}
