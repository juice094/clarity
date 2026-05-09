//! Layer 2 — Loader: async, fallible LLM instantiation.
//!
//! No fallback logic (that's Layer 1's job). No side effects on Agent state.

use crate::app_state::LlmBinding;
use crate::llm_policy::ProviderSelection;
use crate::settings::GuiSettings;
use std::sync::Arc;

/// Async loader: given a selection, produce a live LLM backend.
///
/// # Transparency
/// - All errors are propagated verbatim.
/// - No fallback logic (that's Layer 1's job).
pub async fn load_llm(
    selection: ProviderSelection,
    settings: &GuiSettings,
) -> Result<(Arc<dyn clarity_llm::LlmProvider>, Option<LlmBinding>), crate::error::EguiError>
{
    match selection {
        ProviderSelection::Preferred { provider }
        | ProviderSelection::Fallback {
            preferred: provider,
            ..
        } => {
            let llm = try_load_cloud(&provider, settings).await?;
            Ok((
                llm,
                Some(LlmBinding {
                    provider,
                    local_model_path: String::new(),
                }),
            ))
        }
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
        local_model_path: desired_path,
    };

    Ok((Arc::new(provider), binding))
}

async fn try_load_cloud(
    desired_provider: &str,
    settings: &GuiSettings,
) -> Result<Arc<dyn clarity_llm::LlmProvider>, crate::error::EguiError> {
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
        )
        .await
        .map(Arc::from)
        .ok()
    };

    if let Some(llm) = registry_llm.await {
        return Ok(llm);
    }

    match clarity_llm::LlmFactory::create_with_key_arc(
        desired_provider,
        api_key,
        &settings.model,
    ) {
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
