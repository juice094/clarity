//! Layer 2 — Loader: async, fallible LLM instantiation.
//!
//! No fallback logic (that's Layer 1's job). No side effects on Agent state.

use crate::app_state::LlmBinding;
use crate::llm_policy::ProviderSelection;
use crate::provider::{ApiFormat, ProviderDefinition, ProviderRegistry};
use crate::settings::GuiSettings;
use clarity_llm::runtime::{RuntimeProviderConfig, build_provider};
use std::sync::Arc;

/// Async loader: given a selection, produce a live LLM backend.
///
/// # Transparency
/// - All errors are propagated verbatim.
/// - No fallback logic (that's Layer 1's job).
pub async fn load_llm(
    selection: ProviderSelection,
    settings: &GuiSettings,
) -> Result<(Arc<dyn clarity_llm::LlmProvider>, Option<LlmBinding>), crate::error::EguiError> {
    match selection {
        ProviderSelection::Preferred { provider } => {
            let llm = try_load_cloud(&provider, settings).await?;
            Ok((
                llm,
                Some(LlmBinding {
                    provider,
                    model: settings.model.clone(),
                    local_model_path: String::new(),
                }),
            ))
        }
        ProviderSelection::Fallback {
            preferred,
            fallback,
            reason,
        } => match try_load_cloud(&preferred, settings).await {
            Ok(llm) => Ok((
                llm,
                Some(LlmBinding {
                    provider: preferred,
                    model: settings.model.clone(),
                    local_model_path: String::new(),
                }),
            )),
            Err(_e) => {
                tracing::warn!(
                    "Preferred provider '{}' failed (reason: {}), falling back to '{}'",
                    preferred,
                    reason,
                    fallback
                );
                if fallback == "local" {
                    try_load_local(settings)
                        .await
                        .map(|(llm, binding)| (llm, Some(binding)))
                } else {
                    try_load_cloud(&fallback, settings).await.map(|llm| {
                        (
                            llm,
                            Some(LlmBinding {
                                provider: fallback,
                                model: settings.model.clone(),
                                local_model_path: String::new(),
                            }),
                        )
                    })
                }
            }
        },
        ProviderSelection::LocalOnly { .. } => {
            let (llm, binding) = try_load_local(settings).await?;
            Ok((llm, Some(binding)))
        }
    }
}

async fn try_load_local(
    settings: &GuiSettings,
) -> Result<(Arc<dyn clarity_llm::LlmProvider>, LlmBinding), crate::error::EguiError> {
    let desired_path = settings
        .local_model_path
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            clarity_llm::resolve_local_model_path().map(|p| p.to_string_lossy().into_owned())
        })
        .unwrap_or_default();

    if desired_path.is_empty() {
        return Err(crate::error::EguiError::LlmLoad(
            "No local model configured. Place .gguf in ~/models/ or set CLARITY_LOCAL_MODEL_PATH."
                .to_string(),
        ));
    }
    let model_path = std::path::PathBuf::from(&desired_path);
    let sibling_tokenizer = model_path.with_file_name("tokenizer.json");

    let mut config = clarity_llm::LocalGgufConfig::new(&desired_path)
        .map_err(|e| crate::error::EguiError::LlmLoad(e.to_string()))?
        .with_tokenizer_repo("Qwen/Qwen2.5-7B-Instruct");

    if sibling_tokenizer.exists() {
        if let Ok(meta) = std::fs::metadata(&sibling_tokenizer) {
            if meta.len() < 1024 {
                return Err(crate::error::EguiError::LlmLoad(format!(
                    "Tokenizer file {} seems corrupted (size {} bytes). \
                     Please re-download a valid tokenizer.json.",
                    sibling_tokenizer.display(),
                    meta.len()
                )));
            }
        }
        tracing::info!("Using local tokenizer at {}", sibling_tokenizer.display());
        config = config.with_tokenizer_path(&sibling_tokenizer);
    }

    let provider = clarity_llm::LocalGgufProvider::new(config)
        .await
        .map_err(|e| {
            crate::error::EguiError::LlmLoad(format!("Failed to load local model: {}", e))
        })?;

    let binding = LlmBinding {
        provider: "local".to_string(),
        model: String::new(),
        local_model_path: desired_path,
    };

    Ok((Arc::new(provider), binding))
}

/// Derive a [`RuntimeProviderConfig`] from a registry definition and the
/// current GUI settings.
///
/// Returns `None` for chat-only providers that require a specialised
/// constructor (e.g. `deepseek-device`).
fn runtime_config_from_definition(
    def: &ProviderDefinition,
    settings: &GuiSettings,
) -> Option<RuntimeProviderConfig> {
    if def.api_format == ApiFormat::DeepSeekDevice {
        return None;
    }

    let api_key = def
        .resolve_api_key()
        .or_else(|| GuiSettings::resolve_api_key(&settings.api_key))
        .unwrap_or_default();
    let model = if settings.model.is_empty() {
        def.models.first().cloned().unwrap_or_default()
    } else {
        settings.model.clone()
    };

    Some(RuntimeProviderConfig {
        provider_id: def.id.clone(),
        base_url: def.base_url.clone(),
        api_format: def.api_format.runtime_api_format().to_string(),
        api_key,
        model,
    })
}

async fn try_load_cloud(
    desired_provider: &str,
    settings: &GuiSettings,
) -> Result<Arc<dyn clarity_llm::LlmProvider>, crate::error::EguiError> {
    // Registry-backed providers (e.g. deepseek-device) store credentials in the
    // frontend ProviderRegistry rather than GuiSettings.api_key. Resolve them
    // first so the generic cloud path works without a special-case in ensure_llm.
    // ponytail: ProviderRegistry::load hits disk; this is acceptable because
    // ensure_llm already took the load lock and disk I/O is cheap for a few TOML
    // files. If provider count grows, cache the registry in SettingsStore.
    let registry_result = tokio::task::spawn_blocking({
        let id = desired_provider.to_string();
        move || ProviderRegistry::load().get(&id).cloned()
    })
    .await;
    let def = match registry_result {
        Ok(Some(def)) => Some(def),
        Ok(None) => None,
        Err(join_err) => {
            tracing::warn!(
                "ProviderRegistry::load() task panicked or was cancelled: {}",
                join_err
            );
            None
        }
    };
    if let Some(def) = def {
        if def.api_format == ApiFormat::DeepSeekDevice {
            let model_id = if settings.model.is_empty() {
                "deepseek-chat".to_string()
            } else {
                settings.model.clone()
            };
            let provider = def
                .to_deepseek_device_provider(&model_id)
                .map_err(crate::error::EguiError::InvalidProvider)?;
            return Ok(Arc::new(provider));
        }

        if let Some(cfg) = runtime_config_from_definition(&def, settings) {
            match build_provider(&cfg).await {
                Ok(provider) => return Ok(Arc::from(provider)),
                Err(e) => {
                    tracing::warn!(
                        "Registry-based provider '{}' failed to build: {}",
                        desired_provider,
                        e
                    );
                }
            }
        }
    }

    let resolved_key = GuiSettings::resolve_api_key(&settings.api_key);
    let api_key = resolved_key.as_deref().unwrap_or("");

    let registry_llm = async {
        let registry = clarity_llm::ModelRegistry::load_async().await.ok()?;
        let provider_cfg = registry.get_provider(desired_provider)?;
        let model_id = if settings.model.is_empty() {
            registry
                .list_models()
                .into_iter()
                .find(|m| m.provider == desired_provider)
                .map(|m| m.model_id.clone())?
        } else {
            settings.model.clone()
        };
        clarity_llm::build_provider_from_registry_with_key(
            provider_cfg,
            &model_id,
            if api_key.is_empty() {
                None
            } else {
                Some(api_key)
            },
            None,
            None,
            None,
        )
        .await
        .map(Arc::from)
        .ok()
    };

    if let Some(llm) = registry_llm.await {
        return Ok(llm);
    }

    match clarity_llm::LlmFactory::create_with_key_arc(desired_provider, api_key, &settings.model) {
        Ok(llm) => Ok(llm),
        Err(e) => {
            if api_key.is_empty() {
                match clarity_llm::LlmFactory::create_arc(desired_provider).await {
                    Ok(llm) => Ok(llm),
                    Err(_) => Err(crate::error::EguiError::InvalidProvider(format!(
                        "Provider '{}' requires an API key. \
                         Please open Settings and enter your key.",
                        desired_provider
                    ))),
                }
            } else {
                Err(crate::error::EguiError::LlmLoad(format!(
                    "Failed to create provider '{}': {}. \
                     Please check your API key and network connection.",
                    desired_provider, e
                )))
            }
        }
    }
}

// ============================================================================
// Unit tests — fallback chain behaviour
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::AuthType;

    #[tokio::test]
    async fn test_fallback_to_local_on_cloud_failure() {
        let settings = GuiSettings {
            provider: "nonexistent_cloud_42".into(),
            model: String::new(),
            api_key: None,
            local_model_path: None,
            ..Default::default()
        };
        let selection = ProviderSelection::Fallback {
            preferred: "nonexistent_cloud_42".into(),
            fallback: "local".into(),
            reason: "test fallback".into(),
        };
        let result = load_llm(selection, &settings).await;
        assert!(
            result.is_err(),
            "Expected both preferred and fallback to fail"
        );
        let err_msg = match result {
            Err(e) => format!("{}", e),
            Ok(_) => panic!("Expected error"),
        };
        // The fallback path to local should have been attempted.
        assert!(
            err_msg.contains("local") || err_msg.contains("No local model"),
            "Expected fallback to local to be attempted, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_fallback_to_another_cloud_on_failure() {
        let settings = GuiSettings {
            provider: "nonexistent_cloud_42".into(),
            model: String::new(),
            api_key: None,
            local_model_path: None,
            ..Default::default()
        };
        let selection = ProviderSelection::Fallback {
            preferred: "nonexistent_cloud_42".into(),
            fallback: "nonexistent_cloud_99".into(),
            reason: "test fallback".into(),
        };
        let result = load_llm(selection, &settings).await;
        assert!(
            result.is_err(),
            "Expected both preferred and fallback cloud providers to fail"
        );
    }

    #[test]
    fn test_runtime_config_from_definition_openai() {
        let def = ProviderDefinition {
            id: "openai".into(),
            display_name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_format: ApiFormat::OpenaiCompletions,
            auth_type: AuthType::ApiKey,
            api_key_ref: "sk-test".into(),
            models: vec!["gpt-4o".into()],
            ..Default::default()
        };
        let settings = GuiSettings {
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
            ..Default::default()
        };
        let cfg = runtime_config_from_definition(&def, &settings).expect("should derive config");
        assert_eq!(cfg.provider_id, "openai");
        assert_eq!(cfg.base_url, "https://api.openai.com/v1");
        assert_eq!(cfg.api_format, "openai_chat");
        assert_eq!(cfg.api_key, "sk-test");
        assert_eq!(cfg.model, "gpt-4o-mini");
    }

    #[test]
    fn test_runtime_config_from_definition_kimi_maps_to_openai_chat() {
        let def = ProviderDefinition {
            id: "kimi".into(),
            base_url: "https://api.kimi.com/v1".into(),
            api_format: ApiFormat::Kimi,
            auth_type: AuthType::ApiKey,
            api_key_ref: "sk-kimi".into(),
            models: vec!["kimi-k2".into()],
            ..Default::default()
        };
        let settings = GuiSettings {
            provider: "kimi".into(),
            model: String::new(),
            ..Default::default()
        };
        let cfg = runtime_config_from_definition(&def, &settings).expect("should derive config");
        assert_eq!(cfg.api_format, "openai_chat");
        assert_eq!(cfg.model, "kimi-k2");
    }

    #[test]
    fn test_runtime_config_from_definition_anthropic() {
        let def = ProviderDefinition {
            id: "anthropic".into(),
            base_url: "https://api.anthropic.com/v1".into(),
            api_format: ApiFormat::AnthropicMessages,
            auth_type: AuthType::ApiKey,
            api_key_ref: "sk-ant-test".into(),
            models: vec!["claude-sonnet".into()],
            ..Default::default()
        };
        let settings = GuiSettings {
            provider: "anthropic".into(),
            model: String::new(),
            ..Default::default()
        };
        let cfg = runtime_config_from_definition(&def, &settings).expect("should derive config");
        assert_eq!(cfg.api_format, "anthropic_messages");
        assert_eq!(cfg.api_key, "sk-ant-test");
        assert_eq!(cfg.model, "claude-sonnet");
    }

    #[test]
    fn test_runtime_config_from_definition_deepseek_device_is_none() {
        let def = ProviderDefinition {
            id: "deepseek-device".into(),
            base_url: "https://chat.deepseek.com".into(),
            api_format: ApiFormat::DeepSeekDevice,
            auth_type: AuthType::ApiKey,
            api_key_ref: "token".into(),
            ..Default::default()
        };
        let settings = GuiSettings::default();
        assert!(runtime_config_from_definition(&def, &settings).is_none());
    }

    #[test]
    fn test_runtime_config_falls_back_to_settings_api_key() {
        let def = ProviderDefinition {
            id: "custom".into(),
            base_url: "https://custom.example.com/v1".into(),
            api_format: ApiFormat::OpenaiCompletions,
            auth_type: AuthType::ApiKey,
            api_key_ref: String::new(),
            models: vec!["model-x".into()],
            ..Default::default()
        };
        let settings = GuiSettings {
            provider: "custom".into(),
            model: "model-x".into(),
            api_key: Some("sk-from-settings".into()),
            ..Default::default()
        };
        let cfg = runtime_config_from_definition(&def, &settings).expect("should derive config");
        assert_eq!(cfg.api_key, "sk-from-settings");
    }
}
