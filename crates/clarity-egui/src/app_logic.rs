use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::app_state::{ensure_llm, AppState};
use crate::session::{load_sessions, new_session, now_millis, save_session_internal, session_path};
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
        let wire = Arc::new(clarity_wire::Wire::new());

        Self {
            state,
            runtime,
            ui_tx,
            ui_rx,
            sessions,
            active_session_id: active_id,
            sidebar_collapsed: false,
            input: String::new(),
            drafts: std::collections::HashMap::new(),
            is_loading: false,
            agent_status: AgentStatus::Unconfigured,
            network_banner: None,
            tool_calls: vec![],
            compacting: false,
            settings_open: false,
            settings_edit,
            settings_vm,
            wire,
            frame_count: 0,
            last_fps_time: cc.egui_ctx.input(|i| i.time),
            fps: 0.0,
            start: now,
            theme,
            locale: crate::i18n::Locale::default(),
            provider_registry: crate::provider::ProviderRegistry::load(),
            settings_active_tab: 0,
            show_add_provider: false,
            add_provider_name: String::new(),
            add_provider_url: String::new(),
            add_provider_key: String::new(),
            add_provider_format: "openai-completions".into(),
            attachments: vec![],
            task_panel_open: false,
            tasks: vec![],
            last_task_refresh: now,
            parallel_batches: vec![],
            last_parallel_poll: now,
            toasts: vec![],
            mcp_panel_open: false,
            mcp_config: crate::ui::mcp_panel::load_mcp_config(),
            mcp_changed: false,
            last_scroll_offset: 0.0,
            preview_file: None,
            pending_send: None,
            last_input_modified: now,
            pending_approvals: Vec::new(),
            last_usage: None,
            pending_plan: None,
            plan_tracker: None,
            skill_panel_open: false,
            toolbar_open: true,
            active_category: "engineering".to_string(),
            task_create_modal_open: false,
            task_create_name: String::new(),
            task_create_desc: String::new(),
            task_create_prompt: String::new(),
            task_create_priority: 2,
            onboarding_state: if crate::onboarding::should_show_onboarding() {
                crate::onboarding::OnboardingState::ChooseProvider
            } else {
                crate::onboarding::OnboardingState::Hidden
            },
            onboarding_progress_rx: None,
        }
    }

    pub(crate) fn push_toast(&mut self, message: impl Into<String>, level: ToastLevel) {
        self.toasts.push(Toast {
            message: message.into(),
            level,
            created_at: Instant::now(),
        });
        // Keep max 5 toasts
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
    }

    /// Poll all tracked parallel batch statuses from the Gateway.
    pub(crate) fn poll_parallel_batches(&mut self) {
        let gateway = std::env::var("CLARITY_GATEWAY_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:18790".to_string());
        let batch_ids: Vec<String> =
            self.parallel_batches.iter().map(|b| b.batch_id.clone()).collect();
        let tx = self.ui_tx.clone();

        self.last_parallel_poll = std::time::Instant::now();

        self.runtime.spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());

            for batch_id in &batch_ids {
                let url = format!("{}/v1/parallel/{}/status", gateway, batch_id);
                match client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(status) = resp.json::<serde_json::Value>().await {
                            let _ = tx.send(UiEvent::SubAgentBatch(batch_id.clone(), status));
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    pub(crate) fn refresh_tasks(&self) {
        let store = self.state.task_store.clone();
        let tx = self.ui_tx.clone();
        self.runtime.spawn(async move {
            match store.list_all().await {
                Ok(tasks) => {
                    if let Err(e) = tx.send(UiEvent::TaskList(tasks)) {
                        tracing::warn!("Failed to send TaskList: {}", e);
                    }
                }
                Err(e) => tracing::warn!("Failed to list tasks: {}", e),
            }
        });
    }

    pub(crate) fn active_session(&self) -> Option<&Session> {
        self.sessions
            .iter()
            .find(|s| s.id == self.active_session_id)
    }
    pub(crate) fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions
            .iter_mut()
            .find(|s| s.id == self.active_session_id)
    }

    pub(crate) fn send(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() && self.attachments.is_empty() {
            return;
        }
        // Clear any stale plan tracker from a previous turn.
        self.plan_tracker = None;

        // If currently streaming, steer: cancel the current turn and queue the
        // message for immediate send when the cancellation completes.
        //
        // FIXME-WEEK1-RISK: cancel() is cooperative; LLM API may block 1-3s before
        //   noticing the cancellation token. If agent is mid-tool-call, side-effects
        //   may already have occurred. Optimize: add visual "stopping..." state.
        // FIXME-WEEK1-RISK: Rapid consecutive Enter presses overwrite pending_send.
        //   Optimize: debounce (200ms) or append to pending_send instead of replace.
        if self.is_loading {
            self.stop();
            self.pending_send = Some((text, std::mem::take(&mut self.attachments)));
            self.input.clear();
            return;
        }

        let mut full_message = text.clone();
        for att in &self.attachments {
            if let Ok(content) = std::fs::read_to_string(&att.path) {
                full_message.push_str(&format!("\n\n[File: {}]\n```\n{}\n```", att.name, content));
            } else {
                full_message.push_str(&format!("\n\n[File: {} (binary or unreadable)]", att.name));
            }
        }
        self.attachments.clear();

        if let Some(session) = self.active_session_mut() {
            let mut msg = Message {
                role: Role::User,
                content: full_message.clone(),
                timestamp: Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: false,
            };
            msg.prepare();
            session.messages.push(msg);
            session.updated_at = now_millis();
            // Auto-name session from first user message
            if session.title == "New Chat" {
                let trimmed = text.trim();
                session.title = if trimmed.chars().count() > 20 {
                    format!("{}…", trimmed.chars().take(20).collect::<String>())
                } else {
                    trimmed.to_string()
                };
            }
        }
        self.input.clear();
        self.drafts.remove(&self.active_session_id);
        self.is_loading = true;
        self.agent_status = AgentStatus::Busy;
        self.tool_calls.clear();

        let state = self.state.clone();
        let tx = self.ui_tx.clone();
        let query = full_message;

        // Plan mode command: /plan <query>
        if let Some(plan_query) = text.strip_prefix("/plan ") {
            let plan_query = plan_query.to_string();
            self.runtime.spawn(async move {
                if let Err(e) = ensure_llm(&state).await {
                    if let Err(err) = tx.send(UiEvent::Error(e.to_string())) {
                        tracing::warn!("Failed to send Error: {}", err);
                    }
                    return;
                }
                match state.agent.plan(plan_query).await {
                    Ok(plan) => {
                        if let Err(e) = tx.send(UiEvent::PlanReady(plan)) {
                            tracing::warn!("Failed to send PlanReady: {}", e);
                        }
                    }
                    Err(e) => {
                        if let Err(err) =
                            tx.send(UiEvent::Error(format!("Plan generation failed: {}", e)))
                        {
                            tracing::warn!("Failed to send Error: {}", err);
                        }
                    }
                }
                if let Err(e) = tx.send(UiEvent::Done) {
                    tracing::warn!("Failed to send Done: {}", e);
                }
            });
            return;
        }

        self.runtime.spawn(async move {
            if let Err(e) = ensure_llm(&state).await {
                if let Err(err) = tx.send(UiEvent::Error(e.to_string())) {
                    tracing::warn!("Failed to send Error: {}", err);
                }
                return;
            }

            let wire = Arc::new(clarity_wire::Wire::new());
            let agent = state.agent.clone().with_wire(wire.clone());

            let tx_wire = tx.clone();
            tokio::spawn(async move {
                let mut wire_ui = wire.ui_side(false);
                while let Some(msg) = wire_ui.recv().await {
                    let event = match msg {
                        clarity_wire::WireMessage::ToolCall {
                            id,
                            name,
                            arguments,
                        } => Some(UiEvent::ToolStart {
                            id,
                            name,
                            arguments,
                        }),
                        clarity_wire::WireMessage::ToolResult { id, result } => {
                            Some(UiEvent::ToolResult { id, result })
                        }
                        clarity_wire::WireMessage::StepBegin { tool_name } => {
                            Some(UiEvent::StepBegin { tool_name })
                        }
                        clarity_wire::WireMessage::CompactionBegin => {
                            Some(UiEvent::CompactionBegin)
                        }
                        clarity_wire::WireMessage::CompactionEnd => Some(UiEvent::CompactionEnd),
                        clarity_wire::WireMessage::PlanStepBegin { step_id, tool_name } => {
                            Some(UiEvent::PlanStepBegin { step_id, tool_name })
                        }
                        clarity_wire::WireMessage::PlanStepEnd { step_id, success } => {
                            Some(UiEvent::PlanStepEnd { step_id, success })
                        }
                        clarity_wire::WireMessage::Usage {
                            prompt_tokens,
                            completion_tokens,
                            total_tokens,
                        } => Some(UiEvent::Usage {
                            prompt_tokens,
                            completion_tokens,
                            total_tokens,
                        }),
                        _ => None,
                    };
                    if let Some(ev) = event {
                        if let Err(e) = tx_wire.send(ev) {
                            tracing::warn!("Failed to send wire event: {}", e);
                        }
                    }
                }
            });

            let tx_chunk = tx.clone();
            let result = agent
                .run_streaming(&query, move |chunk: &str| {
                    if let Err(e) = tx_chunk.send(UiEvent::Chunk(chunk.to_string())) {
                        tracing::warn!("Failed to send Chunk: {}", e);
                    }
                })
                .await;

            match result {
                Ok(_) => {
                    if let Err(e) = tx.send(UiEvent::Done) {
                        tracing::warn!("Failed to send Done: {}", e);
                    }
                }
                Err(clarity_core::AgentError::Cancelled) => {
                    if let Err(e) = tx.send(UiEvent::Done) {
                        tracing::warn!("Failed to send Done: {}", e);
                    }
                }
                Err(e) => {
                    if let Err(err) = tx.send(UiEvent::Error(format!("Agent error: {}", e))) {
                        tracing::warn!("Failed to send Error: {}", err);
                    }
                }
            }
        });
    }

    pub(crate) fn process_events(&mut self) {
        while let Ok(event) = self.ui_rx.try_recv() {
            match event {
                UiEvent::Chunk(text) => {
                    if let Some(session) = self.active_session_mut() {
                        if let Some(last) = session.messages.last_mut() {
                            if last.role == Role::Agent {
                                last.content.push_str(&text);
                                last.prepare();
                                continue;
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
                UiEvent::ToolStart {
                    id,
                    name,
                    arguments,
                } => {
                    self.tool_calls.push(ToolCallInfo {
                        id,
                        name,
                        status: ToolCallStatus::Running,
                        result: Some(arguments.to_string()),
                    });
                }
                UiEvent::ToolResult { id, result } => {
                    if let Some(tc) = self.tool_calls.iter_mut().find(|t| t.id == id) {
                        tc.status = ToolCallStatus::Done;
                        tc.result = Some(result);
                    }
                }
                UiEvent::StepBegin { tool_name } => {
                    tracing::info!("Step begin: {}", tool_name);
                }
                UiEvent::CompactionBegin => self.compacting = true,
                UiEvent::CompactionEnd => self.compacting = false,
                UiEvent::Done => {
                    self.is_loading = false;
                    self.agent_status = AgentStatus::Online;
                    self.state.agent.reset();
                    self.save_current_session();
                    // Auto-send any queued message.
                    if let Some((text, attachments)) = self.pending_send.take() {
                        self.input = text;
                        self.attachments = attachments;
                        self.send();
                    }
                }
                UiEvent::Error(msg) => {
                    self.is_loading = false;
                    self.agent_status = AgentStatus::Online;
                    self.push_toast(&msg, ToastLevel::Error);
                    // Release queued message back to input so user can retry.
                    if let Some((text, mut attachments)) = self.pending_send.take() {
                        if self.input.is_empty() {
                            self.input = text;
                        } else {
                            self.input.push('\n');
                            self.input.push_str(&text);
                        }
                        self.attachments.append(&mut attachments);
                    }
                    if let Some(session) = self.active_session_mut() {
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
                UiEvent::Fallback { fallback, reason } => {
                    let msg = if fallback {
                        format!(
                            "Network probe failed ({}). External provider will still be tried.",
                            reason
                        )
                    } else {
                        format!("Network probe restored ({})", reason)
                    };
                    self.push_toast(&msg, ToastLevel::Warn);
                    self.network_banner = if fallback { Some(msg) } else { None };
                }
                UiEvent::TaskList(tasks) => {
                    self.tasks = tasks;
                    self.last_task_refresh = Instant::now();
                }
                UiEvent::SubAgentBatch(batch_id, status) => {
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

                    // Update or insert
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
                        .parallel_batches
                        .iter_mut()
                        .find(|b| b.batch_id == batch_id)
                    {
                        *existing = entry;
                    } else {
                        self.parallel_batches.push(entry);
                    }
                    self.last_parallel_poll = Instant::now();
                }
                UiEvent::Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                } => {
                    self.last_usage = Some((prompt_tokens, completion_tokens, total_tokens));
                }
                UiEvent::PlanReady(plan) => {
                    self.is_loading = false;
                    self.agent_status = AgentStatus::Online;
                    self.pending_plan = Some(plan);
                }
                UiEvent::PlanStepBegin { step_id, tool_name: _ } => {
                    if let Some(ref mut tracker) = self.plan_tracker {
                        for step in &mut tracker.steps {
                            if step.id == step_id {
                                step.status = crate::ui::types::PlanStepStatus::Running;
                                break;
                            }
                        }
                    }
                }
                UiEvent::PlanStepEnd { step_id, success } => {
                    if let Some(ref mut tracker) = self.plan_tracker {
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
            }
        }
    }

    pub(crate) fn save_current_session(&self) {
        if let Some(session) = self.active_session() {
            if let Err(e) = save_session_internal(session) {
                tracing::warn!("Failed to save session: {}", e);
            }
        }
    }
    pub(crate) fn switch_category(&mut self, category: &str) {
        if self.active_category == category {
            return;
        }
        self.save_current_session();
        let old_id = self.active_session_id.clone();
        if !self.input.trim().is_empty() {
            self.drafts.insert(old_id, self.input.clone());
        } else {
            self.drafts.remove(&old_id);
        }
        self.active_category = category.to_string();
        // Find an existing session of this category, or create one.
        if let Some(s) = self.sessions.iter().find(|s| s.category == category) {
            self.active_session_id = s.id.clone();
            self.input = self.drafts.remove(&s.id).unwrap_or_default();
        } else {
            let s = new_session(category);
            let id = s.id.clone();
            self.sessions.push(s);
            self.active_session_id = id.clone();
            self.input = String::new();
        }
        self.last_usage = None;
    }

    pub(crate) fn new_session(&mut self) {
        self.save_current_session();
        // Save draft for current session before switching.
        let old_id = self.active_session_id.clone();
        if !self.input.trim().is_empty() {
            self.drafts.insert(old_id, self.input.clone());
        } else {
            self.drafts.remove(&old_id);
        }
        let category = self.active_category.clone();
        // Emotion is singleton: refuse to create multiple emotion sessions.
        if category == "emotion" {
            if let Some(existing) = self.sessions.iter().find(|s| s.category == "emotion") {
                self.active_session_id = existing.id.clone();
                self.input = self.drafts.remove(&existing.id).unwrap_or_default();
                return;
            }
        }
        // Lazy creation: if an empty session already exists, focus it instead of creating another.
        if let Some(existing) = self
            .sessions
            .iter()
            .find(|s| s.messages.is_empty() && s.category == category)
        {
            self.active_session_id = existing.id.clone();
            self.input = self.drafts.remove(&existing.id).unwrap_or_default();
            return;
        }
        let s = new_session(&category);
        let id = s.id.clone();
        self.sessions.push(s);
        self.active_session_id = id.clone();
        self.input = self.drafts.remove(&id).unwrap_or_default();
        self.last_usage = None;
    }
    pub(crate) fn stop(&mut self) {
        self.state.agent.cancel();
        // run_streaming will detect cancellation, return AgentError::Cancelled,
        // and send UiEvent::Done → process_events calls reset() and is_loading=false.
    }
    /// Convenience: translate `key` for the current locale.
    pub(crate) fn t(&self, key: &'static str) -> &'static str {
        self.locale.t(key)
    }

    /// Save current settings to disk and reload the LLM.
    #[allow(dead_code)]
    pub(crate) fn save_settings_and_reload(&mut self) {
        if let Err(e) = self.settings_edit.save() {
            tracing::error!("Failed to save settings: {}", e);
        } else {
            {
                let mut guard = self.state.cached_settings.lock();
                *guard = self.settings_edit.clone();
            }
            let mode = crate::app_state::parse_approval_mode(
                &self.settings_edit.approval_mode,
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
        if let Err(e) = self.settings_edit.save() {
            tracing::error!("Failed to save settings: {}", e);
        } else {
            {
                let mut guard = self.state.cached_settings.lock();
                *guard = self.settings_edit.clone();
            }
            let mode = crate::app_state::parse_approval_mode(
                &self.settings_edit.approval_mode,
            );
            self.state.agent.set_approval_mode(mode);
            self.state.mode_aware_approval_runtime.set_mode(mode);
        }
    }

    /// Save settings to disk without reloading LLM.
    #[allow(dead_code)]
    pub(crate) fn save_settings_internal(&self) {
        if let Err(e) = self.settings_edit.save() {
            tracing::error!("Failed to save settings: {}", e);
        } else {
            let mut guard = self.state.cached_settings.lock();
            *guard = self.settings_edit.clone();
        }
    }

    #[allow(dead_code)]
    pub(crate) fn delete_session(&mut self, id: String) {
        self.sessions.retain(|s| s.id != id);
        self.drafts.remove(&id);
        // FIXME-WEEK1-RISK: Switching to sessions[0] restores its draft, which may
        //   overwrite user input if delete happens during typing. Acceptable for now
        //   because delete is an explicit user action unlikely to coincide with typing.
        if let Err(e) = std::fs::remove_file(session_path(&id)) {
            tracing::warn!("Failed to remove session file: {}", e);
        }
        if self.sessions.is_empty() {
            self.new_session();
        } else if self.active_session_id == id {
            let new_id = self.sessions[0].id.clone();
            self.active_session_id = new_id.clone();
            self.input = self.drafts.remove(&new_id).unwrap_or_default();
        }
    }
}
