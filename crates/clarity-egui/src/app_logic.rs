use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::app_state::AppState;
use crate::session::{load_sessions, new_session, save_session_internal, session_path};
use crate::settings::GuiSettings;
use crate::theme::Theme;
use crate::ui::types::*;
use crate::App;
use clarity_core::approval::ApprovalRuntime;

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
            let s = new_session("engineering");
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
                locale: crate::i18n::Locale::default(),
                last_scroll_offset: 0.0,
                preview_file: None,
                last_input_modified: now,
                pending_approvals: Vec::new(),
                toasts: vec![],
                skill_panel_open: false,
                toolbar_open: true,
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
        self.ui_store.toasts.push(Toast {
            message: message.into(),
            level,
            created_at: Instant::now(),
        });
        // Keep max 5 toasts
        if self.ui_store.toasts.len() > 5 {
            self.ui_store.toasts.remove(0);
        }
    }

    pub(crate) fn process_events(&mut self) {
        while let Ok(event) = self.ui_rx.try_recv() {
            match event {
                UiEvent::Chunk(text) => self.on_chunk(text),
                UiEvent::ToolStart {
                    id,
                    name,
                    arguments,
                } => self.on_tool_start(id, name, arguments),
                UiEvent::ToolResult { id, result } => self.on_tool_result(id, result),
                UiEvent::StepBegin { tool_name } => self.on_step_begin(tool_name),
                UiEvent::CompactionBegin => self.on_compaction_begin(),
                UiEvent::CompactionEnd => self.on_compaction_end(),
                UiEvent::Done => self.on_done(),
                UiEvent::Error(msg) => self.on_error(msg),
                UiEvent::Fallback { fallback, reason } => self.on_fallback(fallback, reason),
                UiEvent::TaskList(tasks) => self.on_task_list(tasks),
                UiEvent::SubAgentBatch(batch_id, status) => {
                    self.on_subagent_batch(batch_id, status);
                }
                UiEvent::Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                } => self.on_usage(prompt_tokens, completion_tokens, total_tokens),
                UiEvent::PlanReady(plan) => self.on_plan_ready(plan),
                UiEvent::PlanStepBegin { step_id, tool_name } => {
                    self.on_plan_step_begin(step_id, tool_name);
                }
                UiEvent::PlanStepEnd { step_id, success } => {
                    self.on_plan_step_end(step_id, success);
                }
                UiEvent::ProviderTestResult {
                    provider_id,
                    success,
                    error,
                } => self.on_provider_test_result(provider_id, success, error),
                UiEvent::ProviderModelList {
                    provider_id,
                    models,
                } => self.on_provider_model_list(provider_id, models),
                UiEvent::ResolveApproval { req_id, response } => {
                    self.on_resolve_approval(req_id, response);
                }
            }
        }
    }

    fn on_chunk(&mut self, text: String) {
        if let Some(session) = self.session_store.active_session_mut() {
            if let Some(last) = session.messages.last_mut() {
                if last.role == Role::Agent {
                    last.content.push_str(&text);
                    last.prepare();
                    return;
                }
            }
            let mut msg = Message {
                role: Role::Agent,
                content: text,
                timestamp: Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: false,
            };
            msg.prepare();
            session.messages.push(msg);
        }
    }

    fn on_tool_start(&mut self, id: String, name: String, arguments: serde_json::Value) {
        self.chat_store.tool_calls.push(ToolCallInfo {
            id,
            name,
            status: ToolCallStatus::Running,
            result: Some(arguments.to_string()),
        });
    }

    fn on_tool_result(&mut self, id: String, result: String) {
        if let Some(tc) = self.chat_store.tool_calls.iter_mut().find(|t| t.id == id) {
            tc.status = ToolCallStatus::Done;
            tc.result = Some(result);
        }
    }

    fn on_step_begin(&mut self, tool_name: String) {
        tracing::info!("Step begin: {}", tool_name);
    }

    fn on_compaction_begin(&mut self) {
        self.chat_store.compacting = true;
    }

    fn on_compaction_end(&mut self) {
        self.chat_store.compacting = false;
    }

    fn on_done(&mut self) {
        self.chat_store.is_loading = false;
        self.chat_store.agent_status = AgentStatus::Online;
        self.state.agent.reset();
        self.save_current_session();
        // Auto-send any queued message.
        if let Some((text, attachments)) = self.chat_store.pending_send.take() {
            self.chat_store.input = text;
            self.chat_store.attachments = attachments;
            self.send();
        }
    }

    fn on_error(&mut self, msg: String) {
        self.chat_store.is_loading = false;
        self.chat_store.agent_status = AgentStatus::Online;
        self.push_toast(&msg, ToastLevel::Error);
        // Release queued message back to input so user can retry.
        if let Some((text, mut attachments)) = self.chat_store.pending_send.take() {
            if self.chat_store.input.is_empty() {
                self.chat_store.input = text;
            } else {
                self.chat_store.input.push('\n');
                self.chat_store.input.push_str(&text);
            }
            self.chat_store.attachments.append(&mut attachments);
        }
        if let Some(session) = self.session_store.active_session_mut() {
            let mut m = Message {
                role: Role::Agent,
                content: msg.clone(),
                timestamp: Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: true,
            };
            m.prepare();
            session.messages.push(m);
        }
    }

    fn on_fallback(&mut self, fallback: bool, reason: String) {
        let msg = if fallback {
            format!(
                "Network probe failed ({}). External provider will still be tried.",
                reason
            )
        } else {
            format!("Network probe restored ({})", reason)
        };
        self.push_toast(&msg, ToastLevel::Warn);
        self.ui_store.network_banner = if fallback { Some(msg) } else { None };
    }

    fn on_task_list(&mut self, tasks: Vec<clarity_core::background::TaskInfo>) {
        self.task_store.tasks = tasks;
        self.task_store.last_task_refresh = Instant::now();
    }

    fn on_subagent_batch(&mut self, batch_id: String, status: serde_json::Value) {
        use crate::ui::types::{AgentStatusEntry, SubAgentProgress};
        let total = status["total"].as_u64().unwrap_or(0) as usize;
        let completed = status["completed"].as_u64().unwrap_or(0) as usize;
        let failed = status["failed"].as_u64().unwrap_or(0) as usize;
        let status_str = status["status"].as_str().unwrap_or("Running").to_string();
        let elapsed = status["elapsed_ms"].as_u64().unwrap_or(0);

        let agent_statuses: Vec<AgentStatusEntry> = status["agent_statuses"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|v| AgentStatusEntry {
                        agent_id: v["agent_id"].as_str().unwrap_or("").to_string(),
                        status: v["status"].as_str().unwrap_or("").to_string(),
                        summary: v["summary"].as_str().map(|s| s.to_string()),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let entry = SubAgentProgress {
            batch_id: batch_id.clone(),
            total,
            completed,
            failed,
            status: status_str,
            elapsed_ms: elapsed,
            agent_statuses,
            last_poll: Instant::now(),
        };
        if let Some(existing) = self
            .subagent_store.parallel_batches
            .iter_mut()
            .find(|b| b.batch_id == batch_id)
        {
            *existing = entry;
        } else {
            self.subagent_store.parallel_batches.push(entry);
        }
        self.subagent_store.last_parallel_poll = Instant::now();
    }

    fn on_usage(&mut self, prompt_tokens: u32, completion_tokens: u32, total_tokens: u32) {
        self.chat_store.last_usage = Some((prompt_tokens, completion_tokens, total_tokens));
    }

    fn on_plan_ready(&mut self, plan: clarity_core::agent::Plan) {
        self.chat_store.is_loading = false;
        self.chat_store.agent_status = AgentStatus::Online;
        self.chat_store.pending_plan = Some(plan);
    }

    fn on_plan_step_begin(&mut self, step_id: String, _tool_name: String) {
        if let Some(ref mut tracker) = self.chat_store.plan_tracker {
            for step in &mut tracker.steps {
                if step.id == step_id {
                    step.status = crate::ui::types::PlanStepStatus::Running;
                    break;
                }
            }
        }
    }

    fn on_plan_step_end(&mut self, step_id: String, success: bool) {
        if let Some(ref mut tracker) = self.chat_store.plan_tracker {
            for step in &mut tracker.steps {
                if step.id == step_id {
                    step.status = if success {
                        crate::ui::types::PlanStepStatus::Success
                    } else {
                        crate::ui::types::PlanStepStatus::Failed
                    };
                    break;
                }
            }
        }
    }

    fn on_provider_test_result(
        &mut self,
        provider_id: String,
        success: bool,
        error: Option<String>,
    ) {
        self.settings_store.testing_provider = None;
        if success {
            self.push_toast(
                format!("{}: Connection OK", provider_id),
                ToastLevel::Info,
            );
        } else {
            self.push_toast(
                format!(
                    "{}: {}",
                    provider_id,
                    error.unwrap_or_else(|| "Connection failed".into())
                ),
                ToastLevel::Error,
            );
        }
    }

    fn on_resolve_approval(&self, req_id: String, response: clarity_core::approval::ApprovalResponse) {
        let rt = self.state.approval_runtime.clone();
        self.runtime.spawn(async move {
            if let Err(e) = rt.resolve(&req_id, response).await {
                tracing::warn!("Approval resolve failed for {}: {}", req_id, e);
            }
        });
    }

    fn on_provider_model_list(&mut self, provider_id: String, models: Vec<String>) {
        self.settings_store.refreshing_provider = None;
        let count = models.len();
        self.settings_store.provider_registry.update_models(&provider_id, models);
        if count > 0 {
            self.push_toast(
                format!("{}: {} models found", provider_id, count),
                ToastLevel::Info,
            );
        } else {
            self.push_toast(
                format!("{}: No models returned", provider_id),
                ToastLevel::Warn,
            );
        }
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
            let s = new_session(category);
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
        let s = new_session(&category);
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
