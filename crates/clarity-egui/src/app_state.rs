use crate::error::EguiError;
use crate::llm_binder::{bind_llm, unbind_llm};
use crate::llm_loader::load_llm;
use crate::llm_policy::resolve_provider;
use crate::settings::GuiSettings;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;

/// Parse approval mode string from settings into core enum.
pub fn parse_approval_mode(mode: &str) -> clarity_core::approval::ApprovalMode {
    match mode {
        "yolo" => clarity_core::approval::ApprovalMode::Yolo,
        "plan" => clarity_core::approval::ApprovalMode::Plan,
        "smart" => clarity_core::approval::ApprovalMode::Smart,
        _ => clarity_core::approval::ApprovalMode::Interactive,
    }
}

/// Holds llm binding state.
#[derive(Clone, Debug)]
pub struct LlmBinding {
    pub provider: String,
    pub local_model_path: String,
}

/// Holds app state.
pub struct AppState {
    pub agent: clarity_core::Agent,
    pub llm_binding: Mutex<Option<LlmBinding>>,
    pub network_available: AtomicBool,
    pub llm_load_lock: tokio::sync::Mutex<()>,
    pub cached_settings: Mutex<GuiSettings>,
    pub prewarm_error: Mutex<Option<String>>,
    pub task_store: clarity_core::background::TaskStore,
    /// Background task manager (holds cron scheduler, worker pool, and task queue).
    pub bg_manager: Arc<clarity_core::background::BackgroundTaskManager>,
    /// Mode-aware wrapper used by the Agent (holds batch grants & session approvals).
    pub mode_aware_approval_runtime: Arc<
        clarity_core::approval::ModeAwareApprovalRuntime<
            clarity_core::approval::InMemoryApprovalRuntime,
        >,
    >,
    /// Long-term memory store for cross-session fact retrieval.
    /// Initialized lazily in the background so it does not block window creation.
    pub memory_store: OnceLock<clarity_memory::MemoryStore>,
}

impl Default for AppState {
    fn default() -> Self {
        let registry = clarity_core::ToolRegistry::with_egui_safe_tools();
        let approval_rt = Arc::new(clarity_core::approval::InMemoryApprovalRuntime::new());
        let settings = GuiSettings::load();
        let mode = parse_approval_mode(&settings.approval_mode);
        let mode_aware_rt = Arc::new(clarity_core::approval::ModeAwareApprovalRuntime::new(
            approval_rt.clone(),
            mode,
        ));
        let mut agent = clarity_core::Agent::new(registry)
            .with_approval_runtime(mode_aware_rt.clone())
            .with_approval_mode(mode);

        // Sprint X: Load KimiCLI-style agent.yaml if present in working directory
        if let Ok(working_dir) = std::env::current_dir() {
            match clarity_core::agent::definition::load_agent_definition(&working_dir) {
                Ok(def) => {
                    let mut config = agent.config().clone();
                    match clarity_core::agent::definition::apply_to_config(&def, &mut config) {
                        Ok(()) => {
                            // Apply approval_mode override from agent.yaml if present
                            let effective_mode = if let Some(ref yaml_mode) = config.approval_mode {
                                parse_approval_mode(yaml_mode)
                            } else {
                                mode
                            };
                            agent = if def.tools.is_empty() {
                                clarity_core::Agent::with_config(agent.registry().clone(), config)
                                    .with_approval_runtime(mode_aware_rt.clone())
                                    .with_approval_mode(effective_mode)
                            } else {
                                match clarity_core::agent::tool_map::filter_registry(
                                    agent.registry(),
                                    &def.tools,
                                ) {
                                    Ok(filtered_registry) => {
                                        clarity_core::Agent::with_config(filtered_registry, config)
                                            .with_approval_runtime(mode_aware_rt.clone())
                                            .with_approval_mode(effective_mode)
                                    }
                                    Err(e) => {
                                        tracing::debug!(
                                            "Failed to filter registry for agent definition: {}",
                                            e
                                        );
                                        clarity_core::Agent::with_config(
                                            agent.registry().clone(),
                                            config,
                                        )
                                        .with_approval_runtime(mode_aware_rt.clone())
                                        .with_approval_mode(effective_mode)
                                    }
                                }
                            };
                        }
                        Err(e) => {
                            tracing::debug!("Failed to apply agent definition to config: {}", e);
                        }
                    }
                }
                Err(_) => {
                    tracing::debug!(
                        "No agent.yaml found in working directory, using default agent configuration"
                    );
                }
            }
        }

        let task_dir = dirs::data_dir()
            .map(|d| d.join("clarity").join("bg_tasks"))
            .unwrap_or_else(|| PathBuf::from("."));
        let work_dir = dirs::data_dir()
            .map(|d| d.join("clarity").join("bg_work"))
            .unwrap_or_else(|| PathBuf::from("."));
        let _ = std::fs::create_dir_all(&task_dir);
        let _ = std::fs::create_dir_all(&work_dir);
        let cron_scheduler = Arc::new(tokio::sync::Mutex::new(
            clarity_core::background::CronScheduler::new(),
        ));
        let bg_manager = Arc::new(
            clarity_core::background::BackgroundTaskManager::new(&task_dir, &work_dir, &work_dir)
                .with_cron_scheduler(cron_scheduler),
        );
        agent.with_cron_manager(Arc::clone(&bg_manager));

        // 注入 SubagentOrchestrator（SubagentManager）
        let subagent_ctx = work_dir.join("subagent_context");
        let _ = std::fs::create_dir_all(&subagent_ctx);
        let orchestrator = Arc::new(clarity_subagents::SubagentManager::new(
            agent.registry().clone(),
            &work_dir,
            &subagent_ctx,
        ));
        agent = agent.with_orchestrator(orchestrator);

        // Load skill registry from well-known directories.
        if let Some(skill_registry) = load_skill_registry() {
            agent = agent.with_skill_registry(skill_registry);
        }

        Self {
            agent,
            llm_binding: Mutex::new(None),
            network_available: AtomicBool::new(true),
            llm_load_lock: tokio::sync::Mutex::new(()),
            cached_settings: Mutex::new(settings),
            prewarm_error: Mutex::new(None),
            task_store: clarity_core::background::TaskStore::new(task_dir),
            bg_manager,
            mode_aware_approval_runtime: mode_aware_rt,
            memory_store: OnceLock::new(),
        }
    }
}

/// Attempt to load skills from well-known directories.
fn load_skill_registry() -> Option<clarity_core::skills::SkillRegistry> {
    let mut all_skills = Vec::new();

    // 1. Project-local .clarity/skills/ directory
    if let Ok(cwd) = std::env::current_dir() {
        let local_dir = cwd.join(".clarity").join("skills");
        if local_dir.is_dir() {
            if let Ok(skills) = clarity_core::skills::SkillLoader::load_dir(&local_dir) {
                all_skills.extend(skills);
            }
        }
    }

    // 2. User config directory (~/.config/clarity/skills or %APPDATA%\clarity\skills)
    if let Some(config_dir) = dirs::config_dir() {
        let user_dir = config_dir.join("clarity").join("skills");
        if user_dir.is_dir() {
            if let Ok(skills) = clarity_core::skills::SkillLoader::load_dir(&user_dir) {
                all_skills.extend(skills);
            }
        }
    }

    if all_skills.is_empty() {
        None
    } else {
        Some(clarity_core::skills::SkillRegistry::from_skills(all_skills))
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

    let desired_provider = settings.provider.clone();
    // C2: `network_available` is a cached flag updated by the background probe
    // (every ~30 s).  The probe drives UI banners only; it never auto-switches
    // the active provider.  Provider selection is always explicit via Policy.
    let network_available = state
        .network_available
        .load(std::sync::atomic::Ordering::Relaxed);

    // Layer 0 — Runtime provider config: check BEFORE binding shortcut.
    // When the user clicks Apply in the settings panel, set_provider_config()
    // writes to ACTIVE_CONFIG. Without this ordering, binding_matches would
    // short-circuit on the same provider name and the new config would never
    // be consumed, even if the user changed model / key / base_url.
    if let Some(cfg) = clarity_llm::runtime::get_active_config() {
        let _load_guard = state.llm_load_lock.lock().await;
        let desc = format!("runtime:{}:{}", cfg.provider_id, cfg.model);
        let llm = clarity_llm::runtime::build_from_active_config()
            .await
            .map_err(|e| {
                crate::error::EguiError::LlmLoad(format!(
                    "Failed to build provider from runtime config: {}",
                    e
                ))
            })?;
        bind_llm(&state.agent, llm.into(), &desc);
        let mut guard = state.llm_binding.lock();
        *guard = Some(LlmBinding {
            provider: desired_provider,
            local_model_path: String::new(),
        });
        return Ok(());
    }

    let current_binding = {
        let guard = state.llm_binding.lock();
        if binding_matches(&guard, &desired_provider, "") && state.agent.llm().is_some() {
            return Ok(());
        }
        guard.clone()
    };

    let _load_guard = state.llm_load_lock.lock().await;

    {
        let guard = state.llm_binding.lock();
        if binding_matches(&guard, &desired_provider, "") && state.agent.llm().is_some() {
            return Ok(());
        }
    }

    // Layer 1 — Policy: pure function decides which provider to load.
    let selection = resolve_provider(&desired_provider, network_available, &current_binding);

    // Layer 2 — Loader: async instantiation, with post-failure fallback.
    let (llm, binding) = match load_llm(selection, &settings).await {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!(
                "Primary LLM load failed (desired={}): {}, attempting local fallback",
                desired_provider,
                e
            );
            let fallback_selection = crate::llm_policy::ProviderSelection::LocalOnly {
                path: String::new(),
            };
            load_llm(fallback_selection, &settings)
                .await
                .map_err(|e2| {
                    crate::error::EguiError::LlmLoad(format!(
                        "Primary provider '{}' failed: {}; local fallback also failed: {}",
                        desired_provider, e, e2
                    ))
                })?
        }
    };

    // Layer 3 — Binder: attach to Agent.
    bind_llm(
        &state.agent,
        llm,
        &format!("{}:{}", desired_provider, settings.model),
    );

    if let Some(b) = binding {
        let mut guard = state.llm_binding.lock();
        *guard = Some(b);
    }

    Ok(())
}

pub async fn reload_llm(state: &AppState) -> Result<(), EguiError> {
    unbind_llm(&state.agent);
    {
        let mut binding = state.llm_binding.lock();
        *binding = None;
    }
    ensure_llm(state).await
}

/// C2: Lightweight TCP probe used solely for UI state (banner icons).
/// Never blocks provider loading or triggers automatic fallback.
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
    fn test_parse_approval_mode_smart() {
        assert_eq!(parse_approval_mode("smart"), ApprovalMode::Smart);
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

    #[test]
    fn test_apply_profile_overlay_partial_fields() {
        use crate::settings::AgentProfile;
        let mut settings = GuiSettings {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            approval_mode: "interactive".into(),
            api_key: Some("sk-old".into()),
            local_model_path: None,
            active_profile: Some("minimal".into()),
            ..Default::default()
        };
        settings.profiles.insert(
            "minimal".into(),
            AgentProfile {
                provider: "anthropic".into(),
                model: "claude-sonnet-4".into(),
                approval_mode: "interactive".into(),
                api_key: None,
                local_model_path: None,
            },
        );
        apply_profile_overlay(&mut settings);
        assert_eq!(settings.provider, "anthropic");
        assert_eq!(settings.model, "claude-sonnet-4");
        // Partial overlay: fields not present in profile remain unchanged.
        assert_eq!(settings.api_key, Some("sk-old".into()));
    }

    #[test]
    fn test_binding_matches_with_local_path() {
        let binding = Some(LlmBinding {
            provider: "local".to_string(),
            local_model_path: "/models/qwen.gguf".to_string(),
        });
        assert!(binding_matches(&binding, "local", "/models/qwen.gguf"));
        assert!(!binding_matches(&binding, "local", "/models/deepseek.gguf"));
        assert!(!binding_matches(&binding, "ollama", "/models/qwen.gguf"));
    }
}
