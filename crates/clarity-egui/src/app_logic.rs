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
    pub(crate) fn new(
        cc: &eframe::CreationContext<'_>,
        gateway_manager: Option<crate::services::gateway_manager::GatewayManager>,
        tray_manager: Option<crate::services::tray::TrayManager>,
    ) -> Self {
        crate::theme::setup_fonts(&cc.egui_ctx);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
        let mut state = AppState::default();

        // Load persisted cron tasks into the scheduler
        let bg_manager = Arc::clone(&state.bg_manager);
        runtime.block_on(async {
            if let Err(e) = bg_manager.load_cron_tasks().await {
                tracing::warn!("Failed to load cron tasks: {}", e);
            }
        });

        // Initialize long-term memory store
        let memory_db = dirs::data_dir()
            .map(|d| d.join("clarity").join("memory.db"))
            .unwrap_or_else(|| std::path::PathBuf::from("memory.db"));
        state.memory_store = runtime.block_on(async {
            match clarity_memory::MemoryStore::new(&memory_db).await {
                Ok(store) => {
                    tracing::info!("MemoryStore initialized at {:?}", memory_db);
                    Some(store)
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize MemoryStore: {}", e);
                    None
                }
            }
        });

        let state = Arc::new(state);
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

            // MCP auto-connect: load mcp.json and register discovered tools
            match clarity_core::mcp::config::McpConfig::load_default() {
                Ok(config) => {
                    let manager = clarity_core::mcp::McpManager::from_config(&config).await;
                    let server_count = manager.list_servers().len();
                    let tool_count = manager.tools().len();
                    let tool_names: Vec<String> = manager
                        .tools()
                        .iter()
                        .map(|t| t.name().to_string())
                        .collect();
                    manager.register_all(state_for_monitor.agent.registry());
                    tracing::info!(
                        "MCP auto-connect: {} server(s), {} tool(s) registered",
                        server_count,
                        tool_count
                    );
                    let _ = tx_for_monitor.send(crate::ui::types::UiEvent::McpReloaded {
                        success: true,
                        tools: tool_names,
                        message: String::new(),
                    });
                }
                Err(e) => {
                    tracing::debug!("MCP config not loaded ({}), skipping auto-connect", e);
                }
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

        // OAuth token pre-refresh: refresh before expiry so the user never
        // has to wait during a chat turn. Applies to any provider with
        // auth_type == OAuth (currently only Kimi Code).
        let state_for_refresh = state.clone();
        runtime.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let provider = {
                    let guard = state_for_refresh.cached_settings.lock();
                    guard.provider.clone()
                };
                let is_oauth = crate::provider::ProviderRegistry::load()
                    .get(&provider)
                    .map(|p| p.auth_type == crate::provider::AuthType::OAuth)
                    .unwrap_or(false);
                if !is_oauth {
                    continue;
                }
                let manager = clarity_llm::auth::KimiCodeTokenManager::new();
                match manager.try_fresh().await {
                    Ok(Some(_)) => tracing::debug!("OAuth token pre-refreshed successfully"),
                    Ok(None) => tracing::debug!("OAuth token pre-refresh: no token on disk"),
                    Err(e) => tracing::warn!("OAuth token pre-refresh failed: {}", e),
                }
            }
        });

        let now = Instant::now();
        let loaded = load_sessions();
        let first_session_tool_calls = loaded
            .first()
            .map(|s| crate::stores::rebuild_tool_calls(&s.messages))
            .unwrap_or_default();
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
        let web_tabs = settings_edit.web_tabs.clone();
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
        let initial_mcp_mtime = clarity_core::mcp::config::default_config_path()
            .ok()
            .and_then(|p| std::fs::metadata(&p).ok())
            .and_then(|m| m.modified().ok());

        // Start skill file watcher for hot-reload.
        let skill_watcher = if let Some(ref registry) = state.agent.skill_registry() {
            let mut watch_paths = vec![];
            if let Ok(cwd) = std::env::current_dir() {
                watch_paths.push(cwd.join(".clarity").join("skills"));
            }
            if let Some(config_dir) = dirs::config_dir() {
                watch_paths.push(config_dir.join("clarity").join("skills"));
            }
            clarity_core::skills::SkillWatcher::start(registry.clone(), watch_paths)
        } else {
            None
        };

        let app = Self {
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
                gateway_status: crate::ui::types::GatewayStatus::Checking,
                tool_calls: first_session_tool_calls,
                compacting: false,
                pending_send: None,
                stopping: false,
                last_usage: None,
                pending_plan: None,
                plan_tracker: None,
                stick_to_bottom: true,
                editing_message_idx: None,
                edit_buffer: String::new(),
                last_snapshot: None,
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
                kimi_code_login_open: false,
                kimi_code_login_state: crate::stores::KimiCodeLoginState::Idle,
            },
            task_store: crate::stores::TaskStore {
                task_panel_open: true,
                tasks: vec![],
                last_task_refresh: now,
                task_create_modal_open: false,
                task_create_name: String::new(),
                task_create_desc: String::new(),
                task_create_prompt: String::new(),
                task_create_priority: 2,
                task_view_modal_open: false,
                viewing_task_id: None,
                viewing_task_result: None,
            },
            cron_store: crate::stores::CronStore {
                cron_expanded: false,
                tasks: vec![],
                last_refresh: now,
                create_modal_open: false,
                create_name: String::new(),
                create_desc: String::new(),
                create_prompt: String::new(),
                create_expr: String::new(),
                create_priority: 2,
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
                preview_item: None,
                preview_drawer_open: true,
                last_input_modified: now,
                web_tabs,
                web_tabs_expanded: true,
                web_tabs_add_visible: false,
                thinking_log_expanded: false,
                thinking_log_show_all: false,
                pending_approvals: Vec::new(),
                toasts: vec![],
                skill_panel_open: false,
                tools_expanded: false,
                subagents_expanded: false,
                editing_session_id: None,
                editing_title: String::new(),
                focus_input_requested: false,
                agent_turn_style: true,
                agent_turn_glass: false,
                workspace_plan_expanded: false,
                workspace_plan_manually_collapsed: false,
                dashboard_panel_open: false,
                gantt_panel_open: false,
            },
            subagent_store: crate::stores::SubAgentStore {
                parallel_batches: vec![],
                last_parallel_poll: now,
                running_agents: std::collections::HashMap::new(),
                last_gateway_health_poll: now,
                subagent_view_modal_open: false,
                viewing_subagent_id: None,
            },
            mcp_store: crate::stores::McpStore {
                mcp_panel_open: false,
                mcp_config: crate::ui::mcp_panel::load_mcp_config(),
                mcp_changed: false,
                connected_tools: vec![],
                last_mcp_poll: now,
                last_mcp_mtime: initial_mcp_mtime,
            },
            onboarding_store: crate::stores::OnboardingStore {
                onboarding_state: if crate::onboarding::should_show_onboarding() {
                    crate::onboarding::OnboardingState::ChooseProvider
                } else {
                    crate::onboarding::OnboardingState::Hidden
                },
                onboarding_progress_rx: None,
                downloading_auto: false,
                cancel_token: None,
            },
            team_store: crate::stores::TeamStore {
                team_panel_open: false,
                teams: vec![],
                create_modal_open: false,
                create_name: String::new(),
                create_goal: String::new(),
                create_members: vec![],
                create_max_concurrency: 4,
                create_timeout_secs: 300,
            },
            snapshot_store: crate::stores::SnapshotStore::default(),
            gateway_manager,
            skill_watcher,
            tray_manager,
            tray_quit_requested: false,
            last_tray_status: None,
            last_frame_width: None,
        };
        app.refresh_tasks();
        app
    }

    pub(crate) fn push_toast(&mut self, message: impl Into<String>, level: ToastLevel) {
        crate::handlers::system::push_toast(&mut self.ui_store, message, level);
    }

    /// Poll mcp.json for external changes and hot-reload if modified.
    pub(crate) fn check_mcp_config_reload(&mut self) {
        if self.mcp_store.last_mcp_poll.elapsed() < Duration::from_secs(5) {
            return;
        }
        self.mcp_store.last_mcp_poll = Instant::now();

        let path = match clarity_core::mcp::config::default_config_path() {
            Ok(p) => p,
            Err(e) => {
                tracing::debug!("MCP default config path unavailable: {}", e);
                return;
            }
        };

        let mtime = match std::fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(t) => Some(t),
            Err(e) => {
                tracing::debug!("Failed to read mcp.json metadata: {}", e);
                None
            }
        };

        if mtime == self.mcp_store.last_mcp_mtime {
            return;
        }
        self.mcp_store.last_mcp_mtime = mtime;

        let config = match clarity_core::mcp::config::McpConfig::load_default() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("MCP config reload failed: {}", e);
                self.push_toast(format!("MCP 配置加载失败: {}", e), ToastLevel::Error);
                return;
            }
        };

        self.hot_reload_mcp(config);
    }

    /// Disconnect old MCP tools and register new ones from the given config.
    pub(crate) fn hot_reload_mcp(&mut self, config: clarity_core::mcp::config::McpConfig) {
        let old_tools = self.mcp_store.connected_tools.clone();
        let agent = self.state.agent.clone();
        let tx = self.ui_tx.clone();
        self.runtime.spawn(async move {
            for name in &old_tools {
                let _ = agent.registry().unregister(name);
            }
            let manager = clarity_core::mcp::McpManager::from_config(&config).await;
            let tool_names: Vec<String> = manager
                .tools()
                .iter()
                .map(|t| t.name().to_string())
                .collect();
            manager.register_all(agent.registry());
            let _ = tx.send(crate::ui::types::UiEvent::McpReloaded {
                success: true,
                tools: tool_names,
                message: format!(
                    "MCP 配置已重新加载: {} 个服务器, {} 个工具",
                    manager.list_servers().len(),
                    manager.tools().len()
                ),
            });
        });
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
        let old_id = self.session_store.active_session_id.clone();

        // Discard empty sessions when switching categories so they don't
        // clutter the tab bar with blank tabs.
        let was_empty = self
            .session_store
            .active_session()
            .map(|s| s.messages.is_empty())
            .unwrap_or(false);
        if was_empty {
            self.session_store.sessions.retain(|s| s.id != old_id);
            self.session_store.drafts.remove(&old_id);
        } else {
            self.save_current_session();
            if !self.chat_store.input.trim().is_empty() {
                self.session_store
                    .drafts
                    .insert(old_id, self.chat_store.input.clone());
            } else {
                self.session_store.drafts.remove(&old_id);
            }
        }

        self.session_store.active_category = category.to_string();
        // Find an existing session of this category, or create one.
        if let Some(s) = self
            .session_store
            .sessions
            .iter()
            .find(|s| s.category == category)
        {
            self.session_store.active_session_id = s.id.clone();
            self.chat_store.input = self.session_store.drafts.remove(&s.id).unwrap_or_default();
            self.chat_store.tool_calls = crate::stores::rebuild_tool_calls(&s.messages);
        } else {
            let count = self
                .session_store
                .sessions
                .iter()
                .filter(|s| s.category == category)
                .count();
            let s = new_session(category, count);
            let id = s.id.clone();
            self.session_store.sessions.push(s);
            self.session_store.active_session_id = id.clone();
            self.chat_store.input = String::new();
        }
        self.chat_store.last_usage = None;
    }

    pub(crate) fn new_session(&mut self) {
        let category = self.session_store.active_category.clone();

        // Current session has real messages — save it and create a fresh one.
        let old_id = self.session_store.active_session_id.clone();
        self.save_current_session();
        if !self.chat_store.input.trim().is_empty() {
            self.session_store
                .drafts
                .insert(old_id, self.chat_store.input.clone());
        } else {
            self.session_store.drafts.remove(&old_id);
        }

        // Emotion is singleton: refuse to create multiple emotion sessions.
        if category == "emotion" {
            if let Some(existing) = self
                .session_store
                .sessions
                .iter()
                .find(|s| s.category == "emotion")
            {
                self.session_store.active_session_id = existing.id.clone();
                self.chat_store.input = self
                    .session_store
                    .drafts
                    .remove(&existing.id)
                    .unwrap_or_default();
                return;
            }
        }
        let count = self
            .session_store
            .sessions
            .iter()
            .filter(|s| s.category == category)
            .count();
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
        if let Err(e) = std::fs::remove_file(session_path(&id)) {
            tracing::warn!("Failed to remove session file: {}", e);
        }
        if self.session_store.sessions.is_empty() {
            self.new_session();
        } else if self.session_store.active_session_id == id {
            let new_id = self.session_store.sessions[0].id.clone();
            self.session_store.active_session_id = new_id.clone();
            // Only restore draft if user is not actively typing (prevents race).
            if self.chat_store.input.is_empty() {
                self.chat_store.input = self
                    .session_store
                    .drafts
                    .remove(&new_id)
                    .unwrap_or_default();
            }
        }
    }
}
