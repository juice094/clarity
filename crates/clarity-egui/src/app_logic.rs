use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::app_state::AppState;
use crate::session::{load_sessions, new_session, save_session_internal, session_path};
use crate::settings::GuiSettings;
use crate::theme::Theme;
use crate::ui::types::*;
use crate::App;
impl App {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::setup_fonts(&cc.egui_ctx);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
        let state = Arc::new(AppState::default());
        let (ui_tx, ui_rx) = channel::<UiEvent>();

        let state_for_monitor = state.clone();
        let tx_for_monitor = ui_tx.clone();
        runtime.spawn(async move {
            let probe = {
                let guard = state_for_monitor.cached_settings.lock();
                guard
                    .network_probe_url
                    .clone()
                    .unwrap_or_else(|| "1.1.1.1:443".to_string())
            };
            let available = crate::app_state::check_network(&probe).await;
            state_for_monitor
                .network_available
                .store(available, std::sync::atomic::Ordering::Relaxed);

            if let Err(e) = crate::app_state::prewarm_llm(&state_for_monitor).await {
                tracing::warn!("LLM prewarm failed: {}", e);
                let mut guard = state_for_monitor.prewarm_error.lock();
                *guard = Some(e.to_string());
            }

            let mut consecutive_failures: u32 = 0;
            let mut consecutive_successes: u32 = 0;
            const THRESHOLD: u32 = 2;
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let probe = {
                    let guard = state_for_monitor.cached_settings.lock();
                    guard
                        .network_probe_url
                        .clone()
                        .unwrap_or_else(|| "1.1.1.1:443".to_string())
                };
                let available = crate::app_state::check_network(&probe).await;
                let current = state_for_monitor
                    .network_available
                    .load(std::sync::atomic::Ordering::Relaxed);

                if available {
                    consecutive_failures = 0;
                    consecutive_successes += 1;
                } else {
                    consecutive_successes = 0;
                    consecutive_failures += 1;
                }

                let should_flip = (!available && current && consecutive_failures >= THRESHOLD)
                    || (available && !current && consecutive_successes >= THRESHOLD);

                if should_flip {
                    let prev = state_for_monitor
                        .network_available
                        .swap(available, std::sync::atomic::Ordering::Relaxed);
                    if !available && prev {
                        // Network went offline: show banner only. Provider stays as-configured.
                        if let Err(e) = tx_for_monitor.send(UiEvent::Fallback {
                            fallback: true,
                            reason: "offline".into(),
                        }) {
                            tracing::warn!("Failed to send Fallback: {}", e);
                        }
                    } else if available && !prev {
                        // Network came back online: clear banner.
                        if let Err(e) = tx_for_monitor.send(UiEvent::Fallback {
                            fallback: false,
                            reason: "online".into(),
                        }) {
                            tracing::warn!("Failed to send Fallback: {}", e);
                        }
                    }
                }
            }
        });

        let now = Instant::now();
        let loaded = load_sessions();
        let (sessions, active_id) = if loaded.is_empty() {
            let s = new_session("engineering", 0);
            let id = s.id.clone();
            (vec![s], id)
        } else {
            let id = loaded[0].id.clone();
            (loaded, id)
        };

        let theme = Theme::default();
        let mut style = (*cc.egui_ctx.style()).clone();
        theme.apply(&mut style);
        cc.egui_ctx.set_style(style);

        let settings_edit = GuiSettings::load();
        let font_scale = settings_edit.font_scale.unwrap_or(1.0);
        let content_width = settings_edit.content_width.unwrap_or(720.0);
        let theme = Theme::default().with_font_scale(font_scale);
        let settings_snapshot = clarity_core::view_models::settings::SettingsSnapshot {
            provider: settings_edit.provider.clone(),
            model: settings_edit.model.clone(),
            approval_mode: settings_edit.approval_mode.clone(),
            api_key: settings_edit.api_key.clone(),
            local_model_path: settings_edit.local_model_path.clone(),
            theme: settings_edit.theme.clone(),
            active_profile: settings_edit.active_profile.clone(),
        };
        let profile_list: Vec<(String, String)> = settings_edit
            .profiles
            .keys()
            .map(|k| (k.clone(), k.clone()))
            .collect();
        let settings_vm = clarity_core::view_models::settings::SettingsViewModel::from_snapshot(
            &settings_snapshot,
            profile_list,
        );
        Self {
            state,
            runtime,
            ui_tx,
            ui_rx,
            session_store: crate::stores::SessionStore {
                sessions,
                active_session_id: active_id,
                drafts: std::collections::HashMap::new(),
                active_category: "engineering".to_string(),
            },
            chat_store: crate::stores::ChatStore {
                input: String::new(),
                attachments: vec![],
                is_loading: false,
                agent_status: AgentStatus::Unconfigured,
                tool_calls: vec![],
                compacting: false,
                pending_send: None,
                last_usage: None,
                pending_plan: None,
                plan_tracker: None,
                stick_to_bottom: true,
            },
            settings_store: crate::stores::SettingsStore {
                settings_open: false,
                settings_edit,
                settings_vm,
                settings_active_tab: 0,
                show_add_provider: false,
                add_provider_name: String::new(),
                add_provider_url: String::new(),
                add_provider_key: String::new(),
                add_provider_format: "openai-completions".into(),
                provider_registry: crate::provider::ProviderRegistry::load(),
                testing_provider: None,
                refreshing_provider: None,
            },
            task_store: crate::stores::TaskStore {
                task_panel_open: false,
                tasks: vec![],
                last_task_refresh: now,
                task_create_modal_open: false,
                task_create_name: String::new(),
                task_create_desc: String::new(),
                task_create_prompt: String::new(),
                task_create_priority: 2,
            },
            ui_store: crate::stores::UiStore {
                sidebar_collapsed: false,
                network_banner: None,
                frame_count: 0,
                last_fps_time: cc.egui_ctx.input(|i| i.time),
                fps: 0.0,
                start: now,
                theme,
                content_max_width: content_width,
                locale: crate::i18n::Locale::default(),
                last_scroll_offset: 0.0,
                preview_file: None,
                last_input_modified: now,
                pending_approvals: Vec::new(),
                toasts: vec![],
                skill_panel_open: false,
                toolbar_open: false,
                tools_expanded: false,
                editing_session_id: None,
                editing_title: String::new(),
            },
            subagent_store: crate::stores::SubAgentStore {
                parallel_batches: vec![],
                last_parallel_poll: now,
            },
            mcp_store: crate::stores::McpStore {
                mcp_panel_open: false,
                mcp_config: crate::ui::mcp_panel::load_mcp_config(),
                mcp_changed: false,
            },
            onboarding_store: crate::stores::OnboardingStore {
                onboarding_state: if crate::onboarding::should_show_onboarding() {
                    crate::onboarding::OnboardingState::ChooseProvider
                } else {
                    crate::onboarding::OnboardingState::Hidden
                },
                onboarding_progress_rx: None,
            },
        }
    }

    pub(crate) fn push_toast(&mut self, message: impl Into<String>, level: ToastLevel) {
        crate::handlers::system::push_toast(&mut self.ui_store, message, level);
    }

    pub(crate) fn process_events(&mut self) {
        crate::handlers::process_events(self);
    }

    pub(crate) fn save_current_session(&self) {
        if let Some(session) = self.session_store.active_session() {
            if let Err(e) = save_session_internal(session) {
                tracing::warn!("Failed to save session: {}", e);
            }
        }
    }
    pub(crate) fn switch_category(&mut self, category: &str) {
        if self.session_store.active_category == category {
            return;
        }
        self.save_current_session();
        let old_id = self.session_store.active_session_id.clone();
        if !self.chat_store.input.trim().is_empty() {
            self.session_store.drafts.insert(old_id, self.chat_store.input.clone());
        } else {
            self.session_store.drafts.remove(&old_id);
        }
        self.session_store.active_category = category.to_string();
        // Find an existing session of this category, or create one.
        if let Some(s) = self.session_store.sessions.iter().find(|s| s.category == category) {
            self.session_store.active_session_id = s.id.clone();
            self.chat_store.input = self.session_store.drafts.remove(&s.id).unwrap_or_default();
        } else {
            let count = self.session_store.sessions.iter().filter(|s| s.category == category).count();
            let s = new_session(category, count);
            let id = s.id.clone();
            self.session_store.sessions.push(s);
            self.session_store.active_session_id = id.clone();
            self.chat_store.input = String::new();
        }
        self.chat_store.last_usage = None;
    }

    pub(crate) fn new_session(&mut self) {
        self.save_current_session();
        // Save draft for current session before switching.
        let old_id = self.session_store.active_session_id.clone();
        if !self.chat_store.input.trim().is_empty() {
            self.session_store.drafts.insert(old_id, self.chat_store.input.clone());
        } else {
            self.session_store.drafts.remove(&old_id);
        }
        let category = self.session_store.active_category.clone();
        // Emotion is singleton: refuse to create multiple emotion sessions.
        if category == "emotion" {
            if let Some(existing) = self.session_store.sessions.iter().find(|s| s.category == "emotion") {
                self.session_store.active_session_id = existing.id.clone();
                self.chat_store.input = self.session_store.drafts.remove(&existing.id).unwrap_or_default();
                return;
            }
        }
        // Lazy creation: if an empty session already exists, focus it instead of creating another.
        if let Some(existing) = self
            .session_store.sessions
            .iter()
            .find(|s| s.messages.is_empty() && s.category == category)
        {
            self.session_store.active_session_id = existing.id.clone();
            self.chat_store.input = self.session_store.drafts.remove(&existing.id).unwrap_or_default();
            return;
        }
        let count = self.session_store.sessions.iter().filter(|s| s.category == category).count();
        let s = new_session(&category, count);
        let id = s.id.clone();
        self.session_store.sessions.push(s);
        self.session_store.active_session_id = id.clone();
        self.chat_store.input = self.session_store.drafts.remove(&id).unwrap_or_default();
        self.chat_store.last_usage = None;
    }
    pub(crate) fn stop(&mut self) {
        self.state.agent.cancel();
        // run_streaming will detect cancellation, return AgentError::Cancelled,
        // and send UiEvent::Done → process_events calls reset() and is_loading=false.
    }
    /// Convenience: translate `key` for the current locale.
    pub(crate) fn t(&self, key: &'static str) -> &'static str {
        self.ui_store.locale.t(key)
    }

    /// Save current settings to disk and reload the LLM.
    #[allow(dead_code)]
    pub(crate) fn save_settings_and_reload(&mut self) {
        if let Err(e) = self.settings_store.settings_edit.save() {
            tracing::error!("Failed to save settings: {}", e);
        } else {
            {
                let mut guard = self.state.cached_settings.lock();
                *guard = self.settings_store.settings_edit.clone();
            }
            let mode = crate::app_state::parse_approval_mode(
                &self.settings_store.settings_edit.approval_mode,
            );
            self.state.agent.set_approval_mode(mode);
            self.state.mode_aware_approval_runtime.set_mode(mode);
            let state = self.state.clone();
            self.runtime.spawn(async move {
                if let Err(e) = crate::app_state::reload_llm(&state).await {
                    tracing::warn!("reload_llm failed: {}", e);
                }
            });
        }
    }

    /// Auto-save after any change (no user confirmation needed).
    pub(crate) fn auto_save_settings(&mut self) {
        if let Err(e) = self.settings_store.settings_edit.save() {
            tracing::error!("Failed to save settings: {}", e);
        } else {
            {
                let mut guard = self.state.cached_settings.lock();
                *guard = self.settings_store.settings_edit.clone();
            }
            let mode = crate::app_state::parse_approval_mode(
                &self.settings_store.settings_edit.approval_mode,
            );
            self.state.agent.set_approval_mode(mode);
            self.state.mode_aware_approval_runtime.set_mode(mode);
        }
    }

    /// Save settings to disk without reloading LLM.
    #[allow(dead_code)]
    pub(crate) fn save_settings_internal(&self) {
        if let Err(e) = self.settings_store.settings_edit.save() {
            tracing::error!("Failed to save settings: {}", e);
        } else {
            let mut guard = self.state.cached_settings.lock();
            *guard = self.settings_store.settings_edit.clone();
        }
    }

    #[allow(dead_code)]
    pub(crate) fn delete_session(&mut self, id: String) {
        self.session_store.sessions.retain(|s| s.id != id);
        self.session_store.drafts.remove(&id);
        // FIXME-WEEK1-RISK: Switching to sessions[0] restores its draft, which may
        //   overwrite user input if delete happens during typing. Acceptable for now
        //   because delete is an explicit user action unlikely to coincide with typing.
        if let Err(e) = std::fs::remove_file(session_path(&id)) {
            tracing::warn!("Failed to remove session file: {}", e);
        }
        if self.session_store.sessions.is_empty() {
            self.new_session();
        } else if self.session_store.active_session_id == id {
            let new_id = self.session_store.sessions[0].id.clone();
            self.session_store.active_session_id = new_id.clone();
            self.chat_store.input = self.session_store.drafts.remove(&new_id).unwrap_or_default();
        }
    }
}
