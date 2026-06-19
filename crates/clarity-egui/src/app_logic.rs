use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use crate::App;
use crate::app_state::AppState;
use crate::session::{load_sessions, new_session, save_session_internal, session_path};
use crate::settings::GuiSettings;
use crate::theme::Theme;
use crate::ui::types::*;
impl App {
    /// Returns true when the active bot is an OpenClaw remote device with a
    /// live WebSocket connection. In this mode the main chat composer should
    /// route messages through `claw_ws` instead of the local Agent.
    pub(crate) fn is_claw_active(&self) -> bool {
        if self.claw_ws.is_none() {
            return false;
        }
        if self.ui_store.active_bot_id.is_empty() {
            return false;
        }
        if self.ui_store.active_bot_id != self.claw_ws_device_id {
            return false;
        }
        self.device_state
            .connection(&self.ui_store.active_bot_id)
            .is_some()
    }

    /// Creates a new instance.
    pub(crate) fn new(
        cc: &eframe::CreationContext<'_>,
        gateway_manager: Option<crate::services::gateway_manager::GatewayManager>,
        tray_manager: Option<crate::services::tray::TrayManager>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let startup_t0 = Instant::now();
        let mark = |label: &str| {
            tracing::info!("Startup: {} took {:?}", label, startup_t0.elapsed());
        };

        crate::theme::setup_fonts(&cc.egui_ctx);
        mark("setup_fonts");

        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        mark("tokio_runtime");

        let state = AppState::default();

        // Load persisted cron tasks into the scheduler
        let bg_manager = Arc::clone(&state.bg_manager);
        runtime.block_on(async {
            if let Err(e) = bg_manager.load_cron_tasks().await {
                tracing::warn!("Failed to load cron tasks: {}", e);
            }
        });
        mark("load_cron_tasks");

        // Initialize long-term memory store lazily so it does not block window
        // creation. The first memory-dependent operation will use `.get()` and
        // simply skip enrichment until the store is ready.
        let memory_db = dirs::data_dir()
            .map(|d| d.join("clarity").join("memory.db"))
            .unwrap_or_else(|| std::path::PathBuf::from("memory.db"));

        let state = Arc::new(state);
        mark("arc_state");

        // Memory store initialization runs in the background.
        let state_for_memory = Arc::clone(&state);
        runtime.spawn(async move {
            match clarity_memory::MemoryStore::new_auto(&memory_db).await {
                Ok(store) => {
                    if state_for_memory.memory_store.set(store).is_err() {
                        tracing::debug!("MemoryStore was already initialized");
                    } else {
                        tracing::info!("MemoryStore initialized at {:?}", memory_db);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize MemoryStore: {}", e);
                }
            }
        });
        mark("memory_store_spawn");
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
        mark("load_sessions");

        let theme = Theme::default();
        let mut style = (*cc.egui_ctx.style()).clone();
        theme.apply(&mut style);
        cc.egui_ctx.set_style(style);

        let settings_edit = GuiSettings::load();
        let font_scale = settings_edit
            .font_scale
            .unwrap_or(crate::theme::Theme::DEFAULT_FONT_SCALE);
        let content_width = settings_edit.content_width.unwrap_or(600.0);
        let right_rail_visible = settings_edit.right_rail_visible;
        let right_rail_context = settings_edit.right_rail_context;
        let right_rail_card_order = settings_edit.right_rail_card_order.clone();
        let plugin_order = settings_edit.plugin_order.clone();
        let debug_layout_overlay = settings_edit.debug_layout_overlay;
        let nav_context = settings_edit.nav_context;
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
        mark("settings_load");

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
        mark("skill_watcher");

        let settings_store = crate::stores::SettingsStore {
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
            kimi_code_login_state: crate::stores::KimiCodeLoginState::Idle,
        };
        // Discover Claw devices after settings are loaded so user-configured
        // OpenClaw connections participate in the bot list.
        let device_state =
            crate::claw::discover(&settings_store.settings_edit.openclaw_connections);

        let mut app = Self {
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
                agent_status: AgentStatus::Unconfigured,
                gateway_status: crate::ui::types::GatewayStatus::Checking,
                tool_calls: first_session_tool_calls,
                pending_send: None,
                last_usage: None,
                pending_plan: None,
                plan_tracker: None,
                stick_to_bottom: true,
                editing_message_idx: None,
                edit_buffer: String::new(),
                last_snapshot: None,
                input_history: Vec::new(),
                input_history_idx: None,
                draft_status: crate::ui::types::DraftStatus::None,
                status_message: None,
                chunks_since_save: 0,
            },
            settings_store,
            device_state,
            task_store: crate::stores::TaskStore {
                tasks: vec![],
                last_task_refresh: now,
                task_create_name: String::new(),
                task_create_desc: String::new(),
                task_create_prompt: String::new(),
                task_create_priority: 2,
                viewing_task_id: None,
                viewing_task_result: None,
            },
            cron_store: crate::stores::CronStore {
                tasks: vec![],
                last_refresh: now,
                create_name: String::new(),
                create_desc: String::new(),
                create_prompt: String::new(),
                create_expr: String::new(),
                create_priority: 2,
            },
            ui_store: crate::stores::UiStore {
                network_banner: None,
                frame_count: 0,
                last_fps_time: cc.egui_ctx.input(|i| i.time),
                fps: 0.0,
                start: now,
                theme,
                content_max_width: content_width,
                right_rail_width: None,
                locale: crate::i18n::Locale::default(),
                last_scroll_offset: 0.0,
                preview_item: None,
                last_input_modified: now,
                pending_approvals: Vec::new(),
                toasts: vec![],
                focus_input_requested: false,
                kimi_conversation_style: true,
                line_cursor_selected: None,
                line_cursor_total_lines: 0,
                shell_prompt: String::new(),
                active_project: None,
                pretext_probe_open: false,
                pretext_probe_wrap_width: 400.0,
                pretext_estimate_enabled: true,
                // Populated at runtime by device_state.snapshot() — see
                // the frame-loop sync in update().
                bot_instances: Vec::new(),
                active_bot_id: String::new(),
                claw_history: Vec::new(),
            },
            subagent_store: crate::stores::SubAgentStore {
                parallel_batches: vec![],
                last_parallel_poll: now,
                running_agents: std::collections::HashMap::new(),
                last_gateway_health_poll: now,
                viewing_subagent_id: None,
            },
            mcp_store: crate::stores::McpStore {
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
            team_store: {
                let persisted = clarity_tools::team::load_teams_sync();
                let teams = persisted
                    .into_iter()
                    .map(|tc| crate::stores::Team {
                        name: tc.name,
                        goal: tc.goal,
                        members: tc
                            .members
                            .into_iter()
                            .map(|m| crate::stores::TeamMember {
                                name: m.name,
                                description: m.description,
                                agent_type: m.agent_type,
                            })
                            .collect(),
                        max_concurrency: tc.max_concurrency,
                        timeout_secs: tc.timeout_secs,
                    })
                    .collect();
                crate::stores::TeamStore {
                    teams,
                    create_name: String::new(),
                    create_goal: String::new(),
                    create_members: vec![],
                    create_max_concurrency: 4,
                    create_timeout_secs: 300,
                }
            },
            project_store: crate::stores::ProjectStore::new(),
            snapshot_store: crate::stores::SnapshotStore::default(),
            gateway_manager,
            skill_watcher,
            tray_manager,
            tray_quit_requested: false,
            last_tray_status: None,
            last_frame_width: None,
            command_palette: crate::widgets::command_palette::CommandPalette::new(),
            view_state: {
                let mut vs = clarity_core::ui::ViewState::new();
                vs.left_rail = clarity_core::ui::LeftRailSection::Sessions;
                vs.left_rail_expanded = true;
                vs.right_rail_visible = right_rail_visible;
                vs.right_rail_context = right_rail_context;
                vs.debug_layout_overlay = debug_layout_overlay;
                if right_rail_card_order.is_empty() {
                    vs.right_rail_card_order = vec![
                        clarity_core::ui::RightRailCard::Progress,
                        clarity_core::ui::RightRailCard::Context,
                    ];
                } else {
                    vs.right_rail_card_order = right_rail_card_order;
                }
                vs
            },
            pretext_metrics: crate::pretext::EguiFontMetrics::new(cc.egui_ctx.clone()),
            nav_context,
            claw_ws: None,
            claw_ws_device_id: String::new(),
            claw_device_identity: clarity_openclaw::DeviceIdentity::load_existing().unwrap_or_else(
                |e| {
                    tracing::warn!("Failed to load Clarity device identity: {}", e);
                    None
                },
            ),
            claw_device_token: clarity_openclaw::load_paired_token().unwrap_or_else(|e| {
                tracing::warn!("Failed to load Clarity device token: {}", e);
                None
            }),
            claw_ws_uses_sessions_send: false,
        };
        mark("app_struct_init");

        // Seed default work templates on first launch.
        // Templates are empty shells — users are expected to rename and
        // populate them. Names use the locale-aware "Work Templates" label
        // as a prefix so they read naturally in any language.
        if !app.settings_store.settings_edit.work_templates_initialized {
            let base = app.t("Work Templates");
            app.settings_store.settings_edit.work_templates = vec![
                crate::settings::WorkTemplate {
                    name: format!("{} 1", base),
                    prompt: String::new(),
                },
                crate::settings::WorkTemplate {
                    name: format!("{} 2", base),
                    prompt: String::new(),
                },
                crate::settings::WorkTemplate {
                    name: format!("{} 3", base),
                    prompt: String::new(),
                },
            ];
            app.settings_store.settings_edit.work_templates_initialized = true;
        }

        // Sync the initial plugin order back into settings so that new defaults
        // are persisted on first run.
        if app.settings_store.settings_edit.plugin_order.is_empty() {
            app.settings_store.settings_edit.plugin_order = plugin_order;
        }
        app.refresh_tasks();
        mark("App::new complete");
        Ok(app)
    }

    /// Adds a transient toast notification.
    pub(crate) fn push_toast(&mut self, message: impl Into<String>, level: ToastLevel) {
        crate::handlers::system::push_toast(&mut self.ui_store, message, level);
    }

    /// Refresh the cached shell prompt (cwd + git branch).
    /// Called periodically from App::update to avoid IO on every frame.
    pub(crate) fn refresh_shell_prompt(&mut self) {
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let branch = Self::detect_git_branch().unwrap_or_default();
        let prompt = if branch.is_empty() {
            cwd
        } else {
            format!("{} {}", cwd, branch)
        };
        self.ui_store.shell_prompt = prompt;
    }

    /// Detect current git branch by reading .git/HEAD (no subprocess spawn).
    fn detect_git_branch() -> Option<String> {
        let head = std::fs::read_to_string(".git/HEAD").ok()?;
        let line = head.trim();
        if let Some(prefix) = line.strip_prefix("ref: refs/heads/") {
            return Some(prefix.to_string());
        }
        // Detached HEAD: show abbreviated hash
        if line.len() >= 7 {
            return Some(line[..7].to_string());
        }
        None
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

    /// Dispatches queued UI events to handlers.
    pub(crate) fn process_events(&mut self) {
        crate::handlers::process_events(self);
    }

    /// Persists current session to disk.
    pub(crate) fn save_current_session(&self) {
        if let Some(session) = self.session_store.active_session() {
            if let Err(e) = save_session_internal(session) {
                tracing::warn!("Failed to save session: {}", e);
            }
        }
    }
    /// Switch category.
    #[allow(dead_code)]
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
            let _ = self.ui_tx.send(crate::ui::types::UiEvent::ThreadDeleted {
                thread_id: old_id.clone(),
            });
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

        // Emotion is singleton: refuse to create multiple emotion sessions.
        let target = if category == "emotion" {
            self.session_store
                .sessions
                .iter()
                .find(|s| s.category == "emotion")
                .cloned()
        } else {
            self.session_store
                .sessions
                .iter()
                .find(|s| s.category == category)
                .cloned()
        };

        if let Some(s) = target {
            let _ = self.ui_tx.send(crate::ui::types::UiEvent::ThreadActive {
                thread_id: s.id.clone(),
                title: Some(s.title.clone()),
            });
            self.process_events();
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
            let _ = self
                .ui_tx
                .send(crate::ui::types::UiEvent::ThreadCreated { session: s });
            self.process_events();
            self.chat_store.input = String::new();
        }
        self.chat_store.last_usage = None;
    }

    /// Creates a new session.
    pub(crate) fn new_session(&mut self) {
        let category = self.session_store.active_category.clone();

        // If the current session is empty, reuse it rather than accumulating blank tabs.
        let is_empty = self
            .session_store
            .active_session()
            .map(|s| s.messages.is_empty())
            .unwrap_or(false);
        if is_empty {
            self.chat_store.input = String::new();
            self.chat_store.last_usage = None;
            return;
        }

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

        // Emit event-driven session creation so navigation and future backend
        // Thread* wire messages converge on the same path.
        let _ = self
            .ui_tx
            .send(crate::ui::types::UiEvent::ThreadCreated { session: s });
        self.process_events();

        self.chat_store.input = self.session_store.drafts.remove(&id).unwrap_or_default();
        self.chat_store.last_usage = None;
    }
    /// Stop.
    pub(crate) fn stop(&mut self) {
        self.state.agent.cancel();
        // run_streaming will detect cancellation, return AgentError::Cancelled,
        // and send UiEvent::Done → process_events calls reset() and is_loading=false.
    }
    /// Convenience: translate `key` for the current locale.
    pub(crate) fn t(&self, key: &'static str) -> &'static str {
        self.ui_store.locale.t(key)
    }

    /// S3.2 (ADR-006 follow-up): centralized "commit settings" primitive.
    ///
    /// Persists `settings_edit` to disk **and** mirrors into `cached_settings`
    /// in a single atomic step. Callers that need approval-mode propagation
    /// or LLM reload should call [`Self::apply_approval_mode_to_runtime`] and
    /// [`Self::trigger_llm_reload`] explicitly afterwards.
    ///
    /// Returns `Err` only on disk save failure; caller decides whether to
    /// skip downstream side effects.
    pub(crate) fn commit_settings(&self) -> Result<(), String> {
        self.settings_store.settings_edit.save()?;
        let mut guard = self.state.cached_settings.lock();
        *guard = self.settings_store.settings_edit.clone();
        Ok(())
    }

    /// S6 Phase C: mirror layout state into `settings_edit` and persist.
    ///
    /// Call this whenever the right rail visibility, active context, card order,
    /// plugin order, or nav_context changes. Errors are logged but not surfaced
    /// as toasts to avoid spamming the user every frame.
    pub(crate) fn persist_layout_settings(&mut self) {
        self.settings_store.settings_edit.right_rail_visible = self.view_state.right_rail_visible;
        self.settings_store.settings_edit.right_rail_context = self.view_state.right_rail_context;
        self.settings_store.settings_edit.right_rail_card_order =
            self.view_state.right_rail_card_order.clone();
        self.settings_store.settings_edit.debug_layout_overlay =
            self.view_state.debug_layout_overlay;
        self.settings_store.settings_edit.nav_context = self.nav_context;
        if let Err(e) = self.commit_settings() {
            tracing::warn!("Failed to persist layout settings: {}", e);
        }
    }

    /// Open a URL in the system browser.
    ///
    /// The URL is validated to prevent command injection through shell
    /// metacharacters before being passed to platform-specific openers.
    pub(crate) fn open_web_link(&self, url: &str) {
        if !is_safe_url(url) {
            tracing::warn!("Refusing to open unsafe URL: {}", url);
            return;
        }

        #[cfg(target_os = "windows")]
        {
            // Use rundll32 to avoid cmd.exe shell metacharacter interpretation.
            let _ = std::process::Command::new("rundll32.exe")
                .args(["url.dll,FileProtocolHandler", url])
                .spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(url).spawn();
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
        }
    }

    /// Create a new session and pre-fill the chat input with the given prompt.
    pub(crate) fn new_session_with_prompt(&mut self, prompt: &str) {
        if self.view_state.turn == clarity_core::ui::TurnState::Loading {
            return;
        }
        self.new_session();
        self.chat_store.input = prompt.to_string();
    }

    /// Switch the active session, preserving the current session's draft and
    /// restoring the target session's input and tool-call state.
    pub(crate) fn switch_to_session(&mut self, session_id: String) {
        self.save_current_session();
        let old_id = self.session_store.active_session_id.clone();
        if !self.chat_store.input.trim().is_empty() {
            self.session_store
                .drafts
                .insert(old_id, self.chat_store.input.clone());
        } else {
            self.session_store.drafts.remove(&old_id);
        }

        // Event-driven activation: the centralized handler updates SessionStore
        // so future backend ThreadActive wire messages use the same path.
        let _ = self.ui_tx.send(crate::ui::types::UiEvent::ThreadActive {
            thread_id: session_id.clone(),
            title: None,
        });
        self.process_events();

        // SAFE: unwrap_or_default is acceptable — a missing draft means the
        // session input was empty, which is the expected default.
        let active_id = self.session_store.active_session_id.clone();
        self.chat_store.input = self
            .session_store
            .drafts
            .remove(&active_id)
            .unwrap_or_default();
        if let Some(s) = self
            .session_store
            .sessions
            .iter()
            .find(|s| s.id == active_id)
            .cloned()
        {
            self.chat_store.tool_calls = crate::stores::rebuild_tool_calls(&s.messages);
        }
    }

    /// S6 Phase C: persist the current plugin order.
    #[allow(dead_code)]
    pub(crate) fn persist_plugin_order(&mut self, order: Vec<String>) {
        self.settings_store.settings_edit.plugin_order = order;
        if let Err(e) = self.commit_settings() {
            tracing::warn!("Failed to persist plugin order: {}", e);
        }
    }

    /// Propagate the current `settings_edit.approval_mode` to the agent and
    /// the mode-aware approval runtime. Idempotent.
    pub(crate) fn apply_approval_mode_to_runtime(&self) {
        let mode =
            crate::app_state::parse_approval_mode(&self.settings_store.settings_edit.approval_mode);
        self.state.agent.set_approval_mode(mode);
        self.state.mode_aware_approval_runtime.set_mode(mode);
    }

    /// Spawn an async task that triggers `reload_llm`. Fire-and-forget; errors
    /// are logged via `tracing::warn!`.
    pub(crate) fn trigger_llm_reload(&self) {
        let state = self.state.clone();
        self.runtime.spawn(async move {
            if let Err(e) = crate::app_state::reload_llm(&state).await {
                tracing::warn!("reload_llm failed: {}", e);
            }
        });
    }

    /// Save current settings to disk and reload the LLM.
    #[allow(dead_code)]
    pub(crate) fn save_settings_and_reload(&mut self) {
        if let Err(e) = self.commit_settings() {
            tracing::error!("Failed to save settings: {}", e);
            return;
        }
        self.apply_approval_mode_to_runtime();
        self.trigger_llm_reload();
    }

    /// Auto-save after any change (no user confirmation needed).
    pub(crate) fn auto_save_settings(&mut self) {
        if let Err(e) = self.commit_settings() {
            tracing::error!("Failed to save settings: {}", e);
            return;
        }
        self.apply_approval_mode_to_runtime();
    }

    /// Save settings to disk without reloading LLM.
    #[allow(dead_code)]
    pub(crate) fn save_settings_internal(&self) {
        if let Err(e) = self.commit_settings() {
            tracing::error!("Failed to save settings: {}", e);
        }
    }

    /// Increase the global font scale by one step, persist, and re-apply the theme.
    pub(crate) fn increase_font_scale(&mut self) {
        let current = self
            .settings_store
            .settings_edit
            .font_scale
            .unwrap_or(crate::theme::Theme::DEFAULT_FONT_SCALE);
        let next = (current + crate::theme::Theme::FONT_SCALE_STEP)
            .min(crate::theme::Theme::MAX_FONT_SCALE);
        self.set_font_scale(next);
    }

    /// Decrease the global font scale by one step, persist, and re-apply the theme.
    pub(crate) fn decrease_font_scale(&mut self) {
        let current = self
            .settings_store
            .settings_edit
            .font_scale
            .unwrap_or(crate::theme::Theme::DEFAULT_FONT_SCALE);
        let next = (current - crate::theme::Theme::FONT_SCALE_STEP)
            .max(crate::theme::Theme::MIN_FONT_SCALE);
        self.set_font_scale(next);
    }

    /// Set the font scale to an explicit value and persist it.
    pub(crate) fn set_font_scale(&mut self, scale: f32) {
        self.settings_store.settings_edit.font_scale = Some(scale);
        let theme_name = self.settings_store.settings_edit.theme.clone();
        self.ui_store.theme = if theme_name == "light" {
            crate::theme::Theme::light().with_font_scale(scale)
        } else {
            crate::theme::Theme::dark().with_font_scale(scale)
        };
        self.auto_save_settings();
    }

    /// Delete session.
    #[allow(dead_code)]
    pub(crate) fn delete_session(&mut self, id: String) {
        // Event-driven deletion: the centralized handler updates SessionStore so
        // future backend ThreadDeleted wire messages use the same path.
        let _ = self.ui_tx.send(crate::ui::types::UiEvent::ThreadDeleted {
            thread_id: id.clone(),
        });
        self.process_events();

        self.session_store.drafts.remove(&id);
        if let Err(e) = std::fs::remove_file(session_path(&id)) {
            tracing::warn!("Failed to remove session file: {}", e);
        }
        if self.session_store.sessions.is_empty() {
            self.new_session();
            return;
        }

        // If the handler moved activation to a different session, restore its
        // draft only when the user is not actively typing (prevents race).
        let active_id = self.session_store.active_session_id.clone();
        if active_id != id && self.chat_store.input.is_empty() {
            self.chat_store.input = self
                .session_store
                .drafts
                .remove(&active_id)
                .unwrap_or_default();
        }
    }

    /// Set a session's archived flag.
    #[allow(dead_code)]
    pub(crate) fn set_session_archived(&mut self, id: String, archived: bool) {
        // Event-driven archive update so backend ThreadUpdated wire messages use
        // the same centralized SessionStore mutation path.
        let _ = self.ui_tx.send(crate::ui::types::UiEvent::ThreadUpdated {
            thread_id: id,
            title: None,
            archived: Some(archived),
        });
        self.process_events();
    }
}

/// Returns true if `url` has a safe scheme and contains no shell metacharacters.
fn is_safe_url(url: &str) -> bool {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }
    // Reject shell metacharacters to prevent command injection when the URL
    // is passed to platform-specific openers.
    !url.contains(['&', '|', '<', '>', '^', '%', '!', '$', '`', '\n', '\r'])
}
