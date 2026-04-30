use crate::error::EguiError;
use crate::settings::GuiSettings;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Parse approval mode string from settings into core enum.
pub fn parse_approval_mode(mode: &str) -> clarity_core::approval::ApprovalMode {
    match mode {
        "yolo" => clarity_core::approval::ApprovalMode::Yolo,
        "plan" => clarity_core::approval::ApprovalMode::Plan,
        _ => clarity_core::approval::ApprovalMode::Interactive,
    }
}

#[derive(Clone, Debug)]
pub struct LlmBinding {
    pub provider: String,
    pub local_model_path: String,
}

pub struct AppState {
    pub agent: clarity_core::Agent,
    pub llm_binding: Mutex<Option<LlmBinding>>,
    pub network_available: AtomicBool,
    pub llm_load_lock: tokio::sync::Mutex<()>,
    pub cached_settings: Mutex<GuiSettings>,
    pub prewarm_error: Mutex<Option<String>>,
    #[allow(dead_code)]
    pub initialized: AtomicBool,
    pub task_store: clarity_core::background::TaskStore,
    /// Shared approval runtime for UI↔Agent coordination.
    pub approval_runtime: Arc<clarity_core::approval::InMemoryApprovalRuntime>,
}

impl Default for AppState {
    fn default() -> Self {
        let registry = clarity_core::ToolRegistry::with_builtin_tools();
        let approval_rt = Arc::new(clarity_core::approval::InMemoryApprovalRuntime::new());
        let settings = GuiSettings::load();
        let mode = parse_approval_mode(&settings.approval_mode);
        let agent = clarity_core::Agent::new(registry)
            .with_approval_runtime(approval_rt.clone())
            .with_approval_mode(mode);
        let task_dir = dirs::data_dir()
            .map(|d| d.join("clarity").join("bg_tasks"))
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            agent,
            llm_binding: Mutex::new(None),
            network_available: AtomicBool::new(true),
            llm_load_lock: tokio::sync::Mutex::new(()),
            cached_settings: Mutex::new(settings),
            prewarm_error: Mutex::new(None),
            initialized: AtomicBool::new(false),
            task_store: clarity_core::background::TaskStore::new(task_dir),
            approval_runtime: approval_rt,
        }
    }
}

fn binding_matches(binding: &Option<LlmBinding>, provider: &str, path: &str) -> bool {
    matches!(binding, Some(b) if b.provider == provider && b.local_model_path == path)
}

/// Apply the active profile's fields onto `settings` when an active profile is set.
/// This is a pure function extracted for testability.
pub fn apply_profile_overlay(settings: &mut GuiSettings) {
    if let Some(ref profile_id) = settings.active_profile {
        if let Some(profile) = settings.profiles.get(profile_id) {
            settings.provider = profile.provider.clone();
            settings.model = profile.model.clone();
            settings.approval_mode = profile.approval_mode.clone();
            if profile.api_key.is_some() {
                settings.api_key = profile.api_key.clone();
            }
            if profile.local_model_path.is_some() {
                settings.local_model_path = profile.local_model_path.clone();
            }
        }
    }
}

pub async fn ensure_llm(state: &AppState) -> Result<(), EguiError> {
    let mut settings = {
        let guard = state.cached_settings.lock();
        guard.clone()
    };

    apply_profile_overlay(&mut settings);

    let network_available = state
        .network_available
        .load(std::sync::atomic::Ordering::Relaxed);
    let desired_provider = if !network_available && settings.provider != "local" {
        tracing::info!(
            "Network unavailable (preferred={}); falling back to local",
            settings.provider
        );
        "local".to_string()
    } else {
        settings.provider.clone()
    };

    let desired_path = if desired_provider == "local" {
        settings
            .local_model_path
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                clarity_core::llm::resolve_local_model_path()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

    {
        let guard = state.llm_binding.lock();
        if binding_matches(&guard, &desired_provider, &desired_path) && state.agent.llm().is_some()
        {
            return Ok(());
        }
    }

    let _load_guard = state.llm_load_lock.lock().await;

    {
        let guard = state.llm_binding.lock();
        if binding_matches(&guard, &desired_provider, &desired_path) && state.agent.llm().is_some()
        {
            return Ok(());
        }
    }

    let llm: Arc<dyn clarity_core::llm::LlmProvider> = match desired_provider.as_str() {
        "local" => {
            if desired_path.is_empty() {
                return Err(EguiError::LlmLoad(
                    "No local model configured. Place .gguf in ~/models/ or set CLARITY_LOCAL_MODEL_PATH.".to_string(),
                ));
            }
            let model_path = std::path::PathBuf::from(&desired_path);
            let sibling_tokenizer = model_path.with_file_name("tokenizer.json");

            let mut config = clarity_core::llm::LocalGgufConfig::new(&desired_path)
                .with_tokenizer_repo("Qwen/Qwen2.5-7B-Instruct");

            if sibling_tokenizer.exists() {
                if let Ok(meta) = std::fs::metadata(&sibling_tokenizer) {
                    if meta.len() < 1024 {
                        return Err(EguiError::LlmLoad(format!(
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

            let provider = clarity_core::llm::LocalGgufProvider::new(config)
                .await
                .map_err(|e| EguiError::LlmLoad(format!("Failed to load local model: {}", e)))?;
            Arc::new(provider)
        }
        _ => {
            let resolved_key = GuiSettings::resolve_api_key(&settings.api_key);
            let api_key = resolved_key.as_deref().unwrap_or("");

            // Phase 2: Try ModelRegistry first (supports custom providers from models.toml)
            let registry_llm = async {
                let registry = clarity_core::llm::ModelRegistry::load_async().await.ok()?;
                let provider_cfg = registry.get_provider(&desired_provider)?;
                let model_id = if settings.model.is_empty() {
                    // Pick first model for this provider from registry
                    registry
                        .list_models()
                        .into_iter()
                        .find(|m| m.provider == desired_provider)
                        .map(|m| m.model_id.clone())?
                } else {
                    settings.model.clone()
                };
                clarity_core::llm::build_provider_from_registry_with_key(
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
                llm
            } else {
                // Fallback to legacy LlmFactory for built-in providers
                match clarity_core::llm::LlmFactory::create_with_key_arc(
                    &desired_provider,
                    api_key,
                    &settings.model,
                ) {
                    Ok(llm) => llm,
                    Err(e) => {
                        if api_key.is_empty() {
                            match clarity_core::llm::LlmFactory::create_arc(&desired_provider).await
                            {
                                Ok(llm) => llm,
                                Err(_) => {
                                    return Err(EguiError::InvalidProvider(format!(
                                        "Provider '{}' requires an API key. \
                                         Please open Settings and enter your key.",
                                        desired_provider
                                    )));
                                }
                            }
                        } else {
                            return Err(EguiError::LlmLoad(format!(
                                "Failed to create provider '{}': {}. \
                                 Please check your API key and network connection.",
                                desired_provider, e
                            )));
                        }
                    }
                }
            }
        }
    };

    state.agent.set_llm(llm);

    let mut guard = state.llm_binding.lock();
    *guard = Some(LlmBinding {
        provider: desired_provider,
        local_model_path: desired_path,
    });

    Ok(())
}

pub async fn reload_llm(state: &AppState) -> Result<(), EguiError> {
    {
        let mut binding = state.llm_binding.lock();
        *binding = None;
    }
    ensure_llm(state).await
}

pub async fn check_network(probe: &str) -> bool {
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            tokio::net::TcpStream::connect(probe),
        )
        .await,
        Ok(Ok(_))
    )
}

pub async fn prewarm_llm(state: &AppState) -> Result<(), EguiError> {
    ensure_llm(state).await
}

// ============================================================================
// Unit tests for AppState pure logic
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use clarity_core::approval::ApprovalMode;

    #[test]
    fn test_parse_approval_mode_interactive() {
        assert_eq!(
            parse_approval_mode("interactive"),
            ApprovalMode::Interactive
        );
        assert_eq!(
            parse_approval_mode("Interactive"),
            ApprovalMode::Interactive
        );
    }

    #[test]
    fn test_parse_approval_mode_yolo() {
        assert_eq!(parse_approval_mode("yolo"), ApprovalMode::Yolo);
    }

    #[test]
    fn test_parse_approval_mode_plan() {
        assert_eq!(parse_approval_mode("plan"), ApprovalMode::Plan);
    }

    #[test]
    fn test_parse_approval_mode_default_fallback() {
        // Any unknown string falls back to Interactive.
        assert_eq!(parse_approval_mode("unknown"), ApprovalMode::Interactive);
        assert_eq!(parse_approval_mode(""), ApprovalMode::Interactive);
    }

    #[test]
    fn test_binding_matches_same() {
        let binding = Some(LlmBinding {
            provider: "openai".to_string(),
            local_model_path: "".to_string(),
        });
        assert!(binding_matches(&binding, "openai", ""));
    }

    #[test]
    fn test_binding_matches_different_provider() {
        let binding = Some(LlmBinding {
            provider: "openai".to_string(),
            local_model_path: "".to_string(),
        });
        assert!(!binding_matches(&binding, "anthropic", ""));
    }

    #[test]
    fn test_binding_matches_none() {
        assert!(!binding_matches(&None, "openai", ""));
    }

    // ============================================================================
    // Sprint 10 D4: Profile overlay tests
    // ============================================================================

    #[test]
    fn test_apply_profile_overlay_changes_fields() {
        use crate::settings::AgentProfile;
        let mut settings = GuiSettings {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            approval_mode: "interactive".into(),
            api_key: Some("sk-old".into()),
            local_model_path: None,
            active_profile: Some("research".into()),
            ..Default::default()
        };
        settings.profiles.insert(
            "research".into(),
            AgentProfile {
                provider: "kimi".into(),
                model: "kimi-k2".into(),
                approval_mode: "plan".into(),
                api_key: Some("sk-kimi".into()),
                local_model_path: Some("/models/local.gguf".into()),
            },
        );
        apply_profile_overlay(&mut settings);
        assert_eq!(settings.provider, "kimi");
        assert_eq!(settings.model, "kimi-k2");
        assert_eq!(settings.approval_mode, "plan");
        assert_eq!(settings.api_key, Some("sk-kimi".into()));
        assert_eq!(settings.local_model_path, Some("/models/local.gguf".into()));
    }

    #[test]
    fn test_apply_profile_overlay_no_profile_selected() {
        let mut settings = GuiSettings {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            active_profile: None,
            ..Default::default()
        };
        apply_profile_overlay(&mut settings);
        assert_eq!(settings.provider, "openai");
        assert_eq!(settings.model, "gpt-4o");
    }

    #[test]
    fn test_apply_profile_overlay_missing_profile() {
        let mut settings = GuiSettings {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            active_profile: Some("nonexistent".into()),
            ..Default::default()
        };
        apply_profile_overlay(&mut settings);
        assert_eq!(settings.provider, "openai");
        assert_eq!(settings.model, "gpt-4o");
    }
}
