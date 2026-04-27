//! Clarity egui Desktop — Application entry point.
//!
//! ARCHITECTURE CONSTRAINT (Pretext-aligned):
//!   - `update()` is the HOT PATH: only iteration, arithmetic, and egui calls.
//!   - String parsing / markdown / I/O / JSON is FORBIDDEN in `update()`.
//!   - Virtual list: `last_scroll_offset` + `estimate_height()` → visible range.
//!   - `Message::prepare()` must be called ONCE after every content mutation.
//!
//! See `crates/clarity-egui/ARCHITECTURE.md` §1–§6.

use eframe::egui;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod app_state;
mod settings;
mod theme;
mod ui;

use app_state::{ensure_llm, reload_llm, AppState};
use settings::GuiSettings;
use theme::Theme;
use ui::types::*;

// ============================================================================
// Clarity egui Desktop — Phase A: Design System Foundation
// ============================================================================

const SIDEBAR_WIDTH: f32 = 220.0;

struct App {
    state: Arc<AppState>,
    runtime: tokio::runtime::Runtime,
    ui_tx: Sender<UiEvent>,
    ui_rx: Receiver<UiEvent>,
    sessions: Vec<Session>,
    active_session_id: String,
    sidebar_collapsed: bool,
    input: String,
    is_loading: bool,
    agent_status: AgentStatus,
    network_banner: Option<String>,
    tool_calls: Vec<ToolCallInfo>,
    compacting: bool,
    settings_open: bool,
    settings_edit: GuiSettings,
    frame_count: u64,
    last_fps_time: f64,
    fps: f64,
    #[allow(dead_code)]
    start: Instant,
    theme: Theme,
    attachments: Vec<Attachment>,
    task_panel_open: bool,
    tasks: Vec<clarity_core::background::TaskInfo>,
    last_task_refresh: Instant,
    toasts: Vec<Toast>,
    mcp_panel_open: bool,
    mcp_config: Option<clarity_core::mcp::config::McpConfig>,
    mcp_changed: bool,
    /// Last frame's scroll offset for virtual list culling.
    last_scroll_offset: f32,
    /// File preview: (file_name, content_text).
    preview_file: Option<(String, String)>,
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates = [
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
        "C:\\Windows\\Fonts\\msyhbd.ttc",
    ];
    for path in &candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let name = std::path::Path::new(path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            fonts.font_data.insert(
                name.clone(),
                egui::FontData::from_owned(bytes).into(),
            );
            fonts.families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, name.clone());
            fonts.families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push(name);
            tracing::info!("Loaded CJK font from {}", path);
            break;
        }
    }
    ctx.set_fonts(fonts);
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
        let state = Arc::new(AppState::default());
        let (ui_tx, ui_rx) = channel::<UiEvent>();

        let state_for_monitor = state.clone();
        let tx_for_monitor = ui_tx.clone();
        runtime.spawn(async move {
            let probe = {
                let guard = state_for_monitor.cached_settings.lock().unwrap();
                guard.network_probe_url.clone().unwrap_or_else(|| "1.1.1.1:443".to_string())
            };
            let available = app_state::check_network(&probe).await;
            state_for_monitor.network_available.store(available, std::sync::atomic::Ordering::Relaxed);

            if let Err(e) = app_state::prewarm_llm(&state_for_monitor).await {
                tracing::warn!("LLM prewarm failed: {}", e);
                let mut guard = state_for_monitor.prewarm_error.lock().unwrap();
                *guard = Some(e.clone());
            }

            let mut consecutive_failures: u32 = 0;
            let mut consecutive_successes: u32 = 0;
            const THRESHOLD: u32 = 2;
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let probe = {
                    let guard = state_for_monitor.cached_settings.lock().unwrap();
                    guard.network_probe_url.clone().unwrap_or_else(|| "1.1.1.1:443".to_string())
                };
                let available = app_state::check_network(&probe).await;
                let current = state_for_monitor.network_available.load(std::sync::atomic::Ordering::Relaxed);

                if available { consecutive_failures = 0; consecutive_successes += 1; }
                else { consecutive_successes = 0; consecutive_failures += 1; }

                let should_flip = (!available && current && consecutive_failures >= THRESHOLD)
                    || (available && !current && consecutive_successes >= THRESHOLD);

                if should_flip {
                    let prev = state_for_monitor.network_available.swap(available, std::sync::atomic::Ordering::Relaxed);
                    if !available && prev {
                        // Network went offline: show banner only. Provider stays as-configured.
                        if let Err(e) = tx_for_monitor.send(UiEvent::Fallback { fallback: true, reason: "offline".into() }) {
                            tracing::warn!("Failed to send Fallback: {}", e);
                        }
                    } else if available && !prev {
                        // Network came back online: clear banner.
                        if let Err(e) = tx_for_monitor.send(UiEvent::Fallback { fallback: false, reason: "online".into() }) {
                            tracing::warn!("Failed to send Fallback: {}", e);
                        }
                    }
                }
            }
        });

        let now = Instant::now();
        let loaded = load_sessions();
        let (sessions, active_id) = if loaded.is_empty() {
            let s = new_session(); let id = s.id.clone(); (vec![s], id)
        } else {
            let id = loaded[0].id.clone(); (loaded, id)
        };

        let theme = Theme::default();
        let mut style = (*cc.egui_ctx.style()).clone();
        theme.apply(&mut style);
        cc.egui_ctx.set_style(style);

        Self {
            state, runtime, ui_tx, ui_rx, sessions, active_session_id: active_id,
            sidebar_collapsed: false, input: String::new(), is_loading: false,
            agent_status: AgentStatus::Unconfigured, network_banner: None,
            tool_calls: vec![], compacting: false,
            settings_open: false,
            settings_edit: GuiSettings::load(),
            frame_count: 0, last_fps_time: cc.egui_ctx.input(|i| i.time),
            fps: 0.0, start: now,
            theme,
            attachments: vec![],
            task_panel_open: false,
            tasks: vec![],
            last_task_refresh: now,
            toasts: vec![],
            mcp_panel_open: false,
            mcp_config: ui::mcp_panel::load_mcp_config(),
            mcp_changed: false,
            last_scroll_offset: 0.0,
            preview_file: None,
        }
    }

    fn push_toast(&mut self, message: impl Into<String>, level: ToastLevel) {
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

    fn refresh_tasks(&self) {
        let store = self.state.task_store.clone();
        let tx = self.ui_tx.clone();
        self.runtime.spawn(async move {
            match store.list_all().await {
                Ok(tasks) => { if let Err(e) = tx.send(UiEvent::TaskList(tasks)) { tracing::warn!("Failed to send TaskList: {}", e); } }
                Err(e) => tracing::warn!("Failed to list tasks: {}", e),
            }
        });
    }

    fn active_session(&self) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == self.active_session_id)
    }
    fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.id == self.active_session_id)
    }

    fn send(&mut self) {
        let text = self.input.trim().to_string();
        if (text.is_empty() && self.attachments.is_empty()) || self.is_loading { return; }

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
            let mut msg = Message { role: Role::User, content: full_message.clone(), timestamp: Instant::now(), parsed: vec![], cached_height: None, is_error: false };
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
        self.is_loading = true;
        self.agent_status = AgentStatus::Busy;
        self.tool_calls.clear();

        let state = self.state.clone();
        let tx = self.ui_tx.clone();
        let query = full_message;

        self.runtime.spawn(async move {
            if let Err(e) = ensure_llm(&state).await {
                if let Err(err) = tx.send(UiEvent::Error(e)) { tracing::warn!("Failed to send Error: {}", err); }
                return;
            }

            let wire = Arc::new(clarity_wire::Wire::new());
            let agent = state.agent.clone().with_wire(wire.clone());

            let tx_wire = tx.clone();
            tokio::spawn(async move {
                let mut wire_ui = wire.ui_side(false);
                while let Some(msg) = wire_ui.recv().await {
                    let event = match msg {
                        clarity_wire::WireMessage::ToolCall { id, name, arguments } => {
                            Some(UiEvent::ToolStart { id, name, arguments })
                        }
                        clarity_wire::WireMessage::ToolResult { id, result } => {
                            Some(UiEvent::ToolResult { id, result })
                        }
                        clarity_wire::WireMessage::StepBegin { tool_name } => {
                            Some(UiEvent::StepBegin { tool_name })
                        }
                        clarity_wire::WireMessage::CompactionBegin => Some(UiEvent::CompactionBegin),
                        clarity_wire::WireMessage::CompactionEnd => Some(UiEvent::CompactionEnd),
                        _ => None,
                    };
                    if let Some(ev) = event { if let Err(e) = tx_wire.send(ev) { tracing::warn!("Failed to send wire event: {}", e); } }
                }
            });

            let tx_chunk = tx.clone();
            let result = agent.run_streaming(&query, move |chunk: &str| {
                if let Err(e) = tx_chunk.send(UiEvent::Chunk(chunk.to_string())) { tracing::warn!("Failed to send Chunk: {}", e); }
            }).await;

            match result {
                Ok(_) => { if let Err(e) = tx.send(UiEvent::Done) { tracing::warn!("Failed to send Done: {}", e); } }
                Err(clarity_core::AgentError::Cancelled) => { if let Err(e) = tx.send(UiEvent::Done) { tracing::warn!("Failed to send Done: {}", e); } }
                Err(e) => { if let Err(err) = tx.send(UiEvent::Error(format!("Agent error: {}", e))) { tracing::warn!("Failed to send Error: {}", err); } }
            }
        });
    }

    fn process_events(&mut self) {
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
                        let mut msg = Message { role: Role::Agent, content: text, timestamp: Instant::now(), parsed: vec![], cached_height: None, is_error: false };
                        msg.prepare();
                        session.messages.push(msg);
                    }
                }
                UiEvent::ToolStart { id, name, arguments } => {
                    self.tool_calls.push(ToolCallInfo { id, name, status: ToolCallStatus::Running, result: Some(arguments.to_string()) });
                }
                UiEvent::ToolResult { id, result } => {
                    if let Some(tc) = self.tool_calls.iter_mut().find(|t| t.id == id) {
                        tc.status = ToolCallStatus::Done; tc.result = Some(result);
                    }
                }
                UiEvent::StepBegin { tool_name } => { tracing::info!("Step begin: {}", tool_name); }
                UiEvent::CompactionBegin => self.compacting = true,
                UiEvent::CompactionEnd => self.compacting = false,
                UiEvent::Done => {
                    self.is_loading = false; self.agent_status = AgentStatus::Online;
                    self.state.agent.reset();
                    self.save_current_session();
                }
                UiEvent::Error(msg) => {
                    self.is_loading = false; self.agent_status = AgentStatus::Online;
                    self.push_toast(&msg, ToastLevel::Error);
                    if let Some(session) = self.active_session_mut() {
                        let mut m = Message { role: Role::Agent, content: msg.clone(), timestamp: Instant::now(), parsed: vec![], cached_height: None, is_error: true };
                        m.prepare();
                        session.messages.push(m);
                    }
                }
                UiEvent::Fallback { fallback, reason } => {
                    let msg = if fallback {
                        format!("Network probe failed ({}). External provider will still be tried.", reason)
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
            }
        }
    }

    fn save_current_session(&self) {
        if let Some(session) = self.active_session() {
            if let Err(e) = save_session_internal(session) { tracing::warn!("Failed to save session: {}", e); }
        }
    }
    fn new_session(&mut self) {
        self.save_current_session();
        let s = new_session(); let id = s.id.clone();
        self.sessions.push(s); self.active_session_id = id;
    }
    fn stop(&mut self) {
        self.state.agent.cancel();
        // run_streaming will detect cancellation, return AgentError::Cancelled,
        // and send UiEvent::Done → process_events calls reset() and is_loading=false.
    }
    fn delete_session(&mut self, id: String) {
        self.sessions.retain(|s| s.id != id);
        if let Err(e) = std::fs::remove_file(session_path(&id)) { tracing::warn!("Failed to remove session file: {}", e); }
        if self.sessions.is_empty() { self.new_session(); }
        else if self.active_session_id == id { self.active_session_id = self.sessions[0].id.clone(); }
    }

    fn render_settings_panel(&mut self, ctx: &egui::Context) {
        if !self.settings_open { return; }

        // Modal dimmer: semi-transparent overlay that also catches outside clicks
        let screen_rect = ctx.screen_rect();
        let modal_response = egui::Area::new(egui::Id::new("settings_modal_overlay"))
            .fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                ui.allocate_response(screen_rect.size(), egui::Sense::click());
                ui.painter().rect_filled(screen_rect, egui::CornerRadius::ZERO, egui::Color32::from_black_alpha(120));
            })
            .response;
        if modal_response.clicked() {
            self.settings_open = false;
            return;
        }

        egui::Window::new("Settings")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_pos(ctx.screen_rect().center())
            .frame(egui::Frame::window(&ctx.style()).fill(self.theme.surface).corner_radius(egui::CornerRadius::same(self.theme.radius_lg as u8)))
            .show(ctx, |ui| {
                ui.set_min_width(400.0);
                ui.add_space(8.0);

                // Provider
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Provider").size(13.0).color(self.theme.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("provider_combo")
                            .selected_text(&self.settings_edit.provider)
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for (key, label, _) in settings::get_available_models() {
                                    ui.selectable_value(&mut self.settings_edit.provider, key.clone(), label);
                                }
                            });
                    });
                });
                ui.add_space(8.0);

                // Model
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Model").size(13.0).color(self.theme.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_sized(egui::vec2(200.0, 28.0), egui::TextEdit::singleline(&mut self.settings_edit.model).text_color(self.theme.text));
                    });
                });
                ui.add_space(8.0);

                // API Key
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("API Key").size(13.0).color(self.theme.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut key = self.settings_edit.api_key.clone().unwrap_or_default();
                        let response = ui.add_sized(egui::vec2(200.0, 28.0), egui::TextEdit::singleline(&mut key).password(true).text_color(self.theme.text));
                        if response.changed() {
                            self.settings_edit.api_key = if key.is_empty() { None } else { Some(key) };
                        }
                    });
                });
                ui.add_space(8.0);

                // Local model path
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Local Model Path").size(13.0).color(self.theme.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut path = self.settings_edit.local_model_path.clone().unwrap_or_default();
                        let response = ui.add_sized(egui::vec2(200.0, 28.0), egui::TextEdit::singleline(&mut path).text_color(self.theme.text));
                        if response.changed() {
                            self.settings_edit.local_model_path = if path.is_empty() { None } else { Some(path) };
                        }
                    });
                });
                ui.add_space(8.0);

                // Approval mode
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Approval Mode").size(13.0).color(self.theme.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("approval_combo")
                            .selected_text(&self.settings_edit.approval_mode)
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.settings_edit.approval_mode, "interactive".into(), "Interactive — Approve each tool call");
                                ui.selectable_value(&mut self.settings_edit.approval_mode, "yolo".into(), "Yolo — Auto-approve all");
                                ui.selectable_value(&mut self.settings_edit.approval_mode, "plan".into(), "Plan — Review plan before execution");
                            });
                    });
                });
                ui.add_space(16.0);

                // Buttons
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("Save").size(13.0).color(self.theme.text)).fill(self.theme.accent).min_size(egui::vec2(80.0, 32.0))).clicked() {
                            if let Err(e) = self.settings_edit.save() {
                                tracing::error!("Failed to save settings: {}", e);
                            } else {
                                {
                                    let mut guard = self.state.cached_settings.lock().unwrap();
                                    *guard = self.settings_edit.clone();
                                }
                                let state = self.state.clone();
                                self.runtime.spawn(async move {
                                    if let Err(e) = reload_llm(&state).await {
                                        tracing::warn!("reload_llm failed: {}", e);
                                    }
                                });
                            }
                            self.settings_open = false;
                        }
                        if ui.add(egui::Button::new(egui::RichText::new("Cancel").size(13.0).color(self.theme.text)).fill(self.theme.border).min_size(egui::vec2(80.0, 32.0))).clicked() {
                            self.settings_open = false;
                        }
                    });
                });
            });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        self.frame_count += 1;
        if now - self.last_fps_time >= 1.0 {
            self.fps = self.frame_count as f64 / (now - self.last_fps_time);
            self.frame_count = 0;
            self.last_fps_time = now;
        }

        self.process_events();
        if self.is_loading {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        // File drag-and-drop
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            for file in dropped_files {
                if let Some(path) = file.path {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    self.attachments.push(Attachment { path, name });
                }
            }
        }

        // Keyboard shortcuts (only when settings modal is closed)
        if !self.settings_open {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl) {
                if (!self.input.trim().is_empty() || !self.attachments.is_empty()) && !self.is_loading {
                    self.send();
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.ctrl) {
                self.new_session();
            }
        }

        // Refresh task list periodically when panel is open
        if self.task_panel_open && self.last_task_refresh.elapsed() > Duration::from_secs(3) {
            self.refresh_tasks();
        }

        use clarity_core::agent::AgentState;
        self.agent_status = match self.state.agent.state() {
            AgentState::Unconfigured => AgentStatus::Unconfigured,
            AgentState::Idle => if self.is_loading { AgentStatus::Busy } else { AgentStatus::Online },
            AgentState::Running { .. } => AgentStatus::Busy,
            AgentState::Stalled => AgentStatus::Offline,
        };

        ctx.style_mut(|style| {
            self.theme.apply(style);
        });

        if !self.sidebar_collapsed {
            egui::SidePanel::left("sidebar")
                .default_width(SIDEBAR_WIDTH)
                .min_width(180.0)
                .max_width(360.0)
                .resizable(true)
                .frame(egui::Frame::side_top_panel(&ctx.style()).fill(self.theme.bg_accent))
                .show(ctx, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Clarity").size(18.0).strong().color(self.theme.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(egui::RichText::new("⬅").size(14.0)).fill(egui::Color32::TRANSPARENT).corner_radius(egui::CornerRadius::same(self.theme.radius_sm as u8))).clicked() { self.sidebar_collapsed = true; }
                        });
                    });
                    ui.add_space(16.0);

                    if ui.add(egui::Button::new(egui::RichText::new("+ New Chat").size(13.0).color(self.theme.text))
                        .fill(self.theme.surface).corner_radius(egui::CornerRadius::same(self.theme.radius_sm as u8))
                        .min_size(egui::vec2(ui.available_width(), 36.0))).clicked() { self.new_session(); }
                    ui.add_space(12.0);

                    ui.label(egui::RichText::new("Sessions").size(11.0).color(self.theme.text_dim).weak());
                    ui.add_space(4.0);

                    let mut to_delete: Option<String> = None;
                    let sessions_clone: Vec<(String, String, bool)> = self.sessions.iter().map(|s| (s.id.clone(), s.title.clone(), s.id == self.active_session_id)).collect();
                    for (id, title, is_active) in sessions_clone {
                        let bg = if is_active { self.theme.surface } else { self.theme.bg_accent };
                        let text_color = if is_active { self.theme.text } else { self.theme.text_dim };
                        let stroke = if is_active { egui::Stroke::new(2.0, self.theme.accent) } else { egui::Stroke::NONE };
                        ui.horizontal(|ui| {
                            let response = ui.add(
                                egui::Button::new(egui::RichText::new(&title).size(13.0).color(text_color))
                                    .fill(bg)
                                    .corner_radius(egui::CornerRadius::same(self.theme.radius_md as u8))
                                    .stroke(stroke)
                                    .min_size(egui::vec2(ui.available_width() - 28.0, 36.0))
                            );
                            if response.clicked() { self.save_current_session(); self.active_session_id = id.clone(); }
                            if ui.add(egui::Button::new("🗑").fill(egui::Color32::TRANSPARENT).corner_radius(egui::CornerRadius::same(self.theme.radius_sm as u8))).clicked() { to_delete = Some(id); }
                        });
                    }
                    if let Some(id) = to_delete { self.delete_session(id); }

                    ui.add_space(16.0);
                    ui.label(egui::RichText::new("Files").size(11.0).color(self.theme.text_dim).weak());
                    ui.add_space(4.0);
                    let mut clicked_file: Option<std::path::PathBuf> = None;
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            if let Ok(cwd) = std::env::current_dir() {
                                ui::file_browser::render_file_tree(ui, &cwd, &self.theme, 0, &mut |path| {
                                    clicked_file = Some(path.to_path_buf());
                                });
                            }
                        });
                    if let Some(path) = clicked_file {
                        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        let content = std::fs::read_to_string(&path).ok();
                        self.preview_file = content.map(|c| (name, c));
                    }

                    // File preview panel
                    if let Some((ref name, ref content)) = self.preview_file {
                        let preview_name = name.clone();
                        let preview_content = content.clone();
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Preview").size(11.0).color(self.theme.text_dim).weak());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("✕").clicked() { self.preview_file = None; }
                            });
                        });
                        ui.label(egui::RichText::new(&preview_name).size(12.0).color(self.theme.text).monospace());
                        ui.add_space(4.0);
                        let mut preview_text = if preview_content.len() > 2000 {
                            format!("{}…\n\n[Preview truncated: {} total characters]", &preview_content[..2000], preview_content.len())
                        } else {
                            preview_content
                        };
                        egui::ScrollArea::vertical()
                            .max_height(180.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut preview_text)
                                        .desired_rows(10)
                                        .font(egui::TextStyle::Monospace)
                                        .text_color(self.theme.text_dim)
                                );
                            });
                    }

                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.add_space(8.0);
                        #[cfg(debug_assertions)]
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("FPS: {:.0}", self.fps)).size(10.0).color(self.theme.text_dim));
                        });
                    });
                });
        }

        if self.task_panel_open {
            egui::SidePanel::right("task_panel")
                .exact_width(280.0)
                .resizable(false)
                .frame(egui::Frame::side_top_panel(&ctx.style()).fill(self.theme.bg_accent))
                .show(ctx, |ui| {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Tasks").size(16.0).strong().color(self.theme.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✕").clicked() { self.task_panel_open = false; }
                        });
                    });
                    ui.add_space(8.0);
                    ui::task_panel::render_task_panel(ui, &self.tasks, &self.theme);
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).fill(self.theme.bg))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if self.sidebar_collapsed {
                        if ui.add(egui::Button::new(egui::RichText::new("➡").size(14.0)).fill(egui::Color32::TRANSPARENT).corner_radius(egui::CornerRadius::same(self.theme.radius_sm as u8))).clicked() {
                            self.sidebar_collapsed = false;
                        }
                    }
                    ui.label(egui::RichText::new("Chat").size(16.0).strong().color(self.theme.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        // Settings
                        if ui.add(egui::Button::new(egui::RichText::new("⚙").size(14.0)).fill(egui::Color32::TRANSPARENT).corner_radius(egui::CornerRadius::same(self.theme.radius_sm as u8))).clicked() {
                            self.settings_open = true;
                            self.settings_edit = {
                                let guard = self.state.cached_settings.lock().unwrap();
                                guard.clone()
                            };
                        }
                        // Tasks
                        let active_tasks = self.tasks.iter().filter(|t| !t.status.is_terminal()).count();
                        let task_btn = if active_tasks > 0 { format!("📝 {}", active_tasks) } else { "📝".to_string() };
                        if ui.add(egui::Button::new(egui::RichText::new(&task_btn).size(12.0)).fill(egui::Color32::TRANSPARENT).corner_radius(egui::CornerRadius::same(self.theme.radius_sm as u8))).clicked() {
                            self.task_panel_open = !self.task_panel_open;
                            if self.task_panel_open { self.refresh_tasks(); }
                        }
                        // MCP
                        let mcp_count = self.mcp_config.as_ref().map_or(0, |c| c.servers.len());
                        let mcp_btn = if mcp_count > 0 { format!("🔌 {}", mcp_count) } else { "🔌".to_string() };
                        if ui.add(egui::Button::new(egui::RichText::new(&mcp_btn).size(12.0)).fill(egui::Color32::TRANSPARENT).corner_radius(egui::CornerRadius::same(self.theme.radius_sm as u8))).clicked() {
                            self.mcp_panel_open = !self.mcp_panel_open;
                        }
                        // Status
                        let (status_color, status_label) = match self.agent_status {
                            AgentStatus::Online => (self.theme.status_online, "Online"),
                            AgentStatus::Busy => (self.theme.status_busy, "Busy"),
                            AgentStatus::Unconfigured => (self.theme.status_offline, "Unconfigured"),
                            AgentStatus::Offline => (self.theme.status_offline, "Offline"),
                        };
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, status_color);
                        ui.label(egui::RichText::new(status_label).size(12.0).color(self.theme.text_dim));
                    });
                });
                ui.add_space(4.0);
                ui.separator();

                let banner_text = self.network_banner.clone();
                if let Some(banner) = banner_text {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&banner).size(12.0).color(self.theme.status_busy));
                        if ui.button("✕").clicked() { self.network_banner = None; }
                    });
                    ui.separator();
                }

                if self.compacting {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Compacting conversation history…").size(12.0).color(self.theme.text_dim));
                    });
                    ui.separator();
                }

                let available_height = ui.available_height() - 70.0;
                let is_loading = self.is_loading;
                let theme = self.theme.clone();
                let active_id = self.active_session_id.clone();
                let tool_calls = self.tool_calls.clone();
                let scroll_y = self.last_scroll_offset;
                let mut configure_clicked = false;

                let output = egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(available_height)
                    .show(ui, |ui| {
                        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == active_id) {
                            if session.messages.is_empty() && !is_loading {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(120.0);
                                    ui.label(egui::RichText::new("Clarity").size(32.0).strong().color(theme.text_dim));
                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new("Local-first AI agent runtime").size(14.0).color(theme.text_dim));
                                    ui.add_space(24.0);
                                    if ui.add(egui::Button::new(egui::RichText::new("Configure Settings").size(13.0).color(theme.text))
                                        .fill(theme.surface).corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                                        .min_size(egui::vec2(180.0, 40.0))).clicked() { configure_clicked = true; }
                                });
                            } else {
                                // --- Virtualized message list ---
                                let estimates: Vec<f32> = session.messages.iter()
                                    .map(|m| m.cached_height.unwrap_or_else(|| estimate_height(m)))
                                    .collect();

                                let mut cumulative = 0.0;
                                let mut start_idx = 0;
                                let mut end_idx = session.messages.len();

                                for (i, h) in estimates.iter().enumerate() {
                                    if cumulative + h >= scroll_y && start_idx == 0 {
                                        start_idx = i.saturating_sub(3);
                                    }
                                    cumulative += h;
                                    if cumulative >= scroll_y + available_height && end_idx == session.messages.len() {
                                        end_idx = (i + 3).min(session.messages.len());
                                        break;
                                    }
                                }

                                if start_idx > 0 {
                                    let top = estimates[..start_idx].iter().sum::<f32>();
                                    ui.allocate_space(egui::vec2(ui.available_width(), top));
                                }

                                for i in start_idx..end_idx {
                                    let actual = ui::render::message_bubble(ui, &session.messages[i], &theme);
                                    session.messages[i].cached_height = Some(actual);
                                }

                                if end_idx < session.messages.len() {
                                    let bottom = estimates[end_idx..].iter().sum::<f32>();
                                    ui.allocate_space(egui::vec2(ui.available_width(), bottom));
                                }

                                // Tool calls & typing indicator (few items, always rendered)
                                for tc in &tool_calls { ui::render::tool_call_bubble(ui, tc, &theme); }
                                if is_loading && session.messages.last().map_or(true, |m| m.role == Role::User) && tool_calls.is_empty() {
                                    ui::render::typing_indicator(ui, &theme);
                                }
                            }
                        }
                    });

                self.last_scroll_offset = output.state.offset.y;
                if configure_clicked {
                    self.settings_open = true;
                    self.settings_edit = {
                        let guard = self.state.cached_settings.lock().unwrap();
                        guard.clone()
                    };
                }

                ui.separator();

                // Attachment chips above input bar
                if !self.attachments.is_empty() {
                    let mut to_remove: Option<usize> = None;
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new("Attachments:").size(11.0).color(self.theme.text_dim));
                        for (i, att) in self.attachments.iter().enumerate() {
                            egui::Frame::group(ui.style())
                                .fill(self.theme.surface)
                                .corner_radius(egui::CornerRadius::same(self.theme.radius_full as u8))
                                .stroke(egui::Stroke::new(1.0, self.theme.border))
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("📎").size(11.0));
                                        ui.label(egui::RichText::new(&att.name).size(11.0).color(self.theme.text).monospace());
                                        if ui.small_button("✕").clicked() { to_remove = Some(i); }
                                    });
                                });
                        }
                    });
                    if let Some(i) = to_remove {
                        self.attachments.remove(i);
                    }
                    ui.separator();
                }

                // Input bar card
                egui::Frame::group(ui.style())
                    .fill(self.theme.input_bg)
                    .corner_radius(egui::CornerRadius::same(self.theme.radius_lg as u8))
                    .stroke(egui::Stroke::new(1.0, self.theme.border))
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let available_width = ui.available_width();
                            let input_width = available_width - 52.0;
                            ui.allocate_ui_with_layout(
                                egui::vec2(input_width, 44.0),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    let hint = if self.attachments.is_empty() {
                                        "Type a message..."
                                    } else {
                                        "Type a message (files attached)..."
                                    };
                                    let text_edit = egui::TextEdit::multiline(&mut self.input)
                                        .desired_rows(1).hint_text(hint).margin(egui::vec2(8.0, 8.0));
                                    ui.add_sized(egui::vec2(input_width, 44.0), text_edit);
                                },
                            );
                            ui.vertical_centered(|ui| {
                                if self.is_loading {
                                    let btn = ui.add_sized(
                                        egui::vec2(40.0, 40.0),
                                        egui::Button::new(egui::RichText::new("■").size(16.0).color(self.theme.text))
                                            .fill(self.theme.danger).corner_radius(egui::CornerRadius::same(self.theme.radius_full as u8)),
                                    );
                                    if btn.clicked() { self.stop(); }
                                } else {
                                    let btn = ui.add_sized(
                                        egui::vec2(40.0, 40.0),
                                        egui::Button::new(egui::RichText::new("▶").size(16.0).color(self.theme.text))
                                            .fill(self.theme.accent).corner_radius(egui::CornerRadius::same(self.theme.radius_full as u8)),
                                    );
                                    if btn.clicked() { self.send(); }
                                }
                            });
                        });
                    });
            });

        self.render_settings_panel(ctx);

        // MCP configuration panel
        if self.mcp_panel_open {
            let mut config_opt = self.mcp_config.take();
            let mut save_clicked = false;
            let mut cancel_clicked = false;
            let mut create_clicked = false;
            let mut open = self.mcp_panel_open;

            egui::Window::new("MCP Servers")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size([400.0, 500.0])
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .frame(egui::Frame::window(&ctx.style()).fill(self.theme.surface).corner_radius(egui::CornerRadius::same(self.theme.radius_lg as u8)))
                .show(ctx, |ui| {
                    ui.set_min_width(360.0);
                    if let Some(ref mut config) = config_opt {
                        let mut changed = false;
                        ui::mcp_panel::render_mcp_panel(ui, config, &self.theme, &mut changed);
                        if changed {
                            self.mcp_changed = true;
                        }
                        if self.mcp_changed {
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(egui::RichText::new("Save").size(13.0).color(self.theme.text)).fill(self.theme.accent).min_size(egui::vec2(80.0, 32.0))).clicked() {
                                        save_clicked = true;
                                    }
                                    if ui.add(egui::Button::new(egui::RichText::new("Cancel").size(13.0).color(self.theme.text)).fill(self.theme.border).min_size(egui::vec2(80.0, 32.0))).clicked() {
                                        cancel_clicked = true;
                                    }
                                });
                            });
                        }
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.label(egui::RichText::new("No MCP config found").size(13.0).color(self.theme.text_dim));
                            ui.add_space(8.0);
                            if ui.add(egui::Button::new(egui::RichText::new("Create Config").size(13.0).color(self.theme.text)).fill(self.theme.accent).min_size(egui::vec2(140.0, 36.0))).clicked() {
                                create_clicked = true;
                            }
                        });
                    }
                });

            self.mcp_panel_open = open;

            if save_clicked {
                if let Some(ref mut config) = config_opt {
                    match ui::mcp_panel::save_mcp_config(config) {
                        Ok(()) => {
                            self.push_toast("MCP config saved", ToastLevel::Info);
                            self.mcp_changed = false;
                        }
                        Err(e) => self.push_toast(&format!("Save failed: {}", e), ToastLevel::Error),
                    }
                }
            }
            if cancel_clicked {
                config_opt = ui::mcp_panel::load_mcp_config();
                self.mcp_changed = false;
                self.mcp_panel_open = false;
            }
            if create_clicked {
                config_opt = Some(clarity_core::mcp::config::McpConfig::default());
                self.mcp_changed = true;
            }

            self.mcp_config = config_opt;
        }

        // Toast notifications — top-right, auto-dismiss after 5s
        let now = Instant::now();
        self.toasts.retain(|t| now.duration_since(t.created_at) < Duration::from_secs(5));
        for (i, toast) in self.toasts.iter().enumerate() {
            let (bg, text_color) = match toast.level {
                ToastLevel::Info => (self.theme.accent, self.theme.text),
                ToastLevel::Warn => (self.theme.status_busy, self.theme.text),
                ToastLevel::Error => (self.theme.danger, self.theme.text),
            };
            let screen = ctx.screen_rect();
            let x = screen.max.x - 320.0;
            let y = 20.0 + i as f32 * 56.0;
            egui::Area::new(egui::Id::new(("toast", i)))
                .fixed_pos(egui::pos2(x, y))
                .show(ctx, |ui| {
                    egui::Frame::group(&ctx.style())
                        .fill(bg)
                        .corner_radius(egui::CornerRadius::same(self.theme.radius_sm as u8))
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_max_width(280.0);
                            ui.label(egui::RichText::new(&toast.message).color(text_color).size(13.0));
                        });
                });
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_current_session();
    }
}

fn sessions_dir() -> std::path::PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push("clarity"); path.push("sessions"); path
}

fn session_path(id: &str) -> std::path::PathBuf {
    let mut path = sessions_dir(); path.push(format!("{}.json", id)); path
}

fn load_sessions() -> Vec<Session> {
    let dir = sessions_dir();
    if !dir.exists() { return Vec::new(); }
    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(data) = serde_json::from_str::<SessionData>(&content) {
                        sessions.push(Session {
                            id: data.id, title: data.title,
                            messages: data.messages.into_iter().map(|m| {
                                let mut msg = Message {
                                    role: if m.role == "user" { Role::User } else { Role::Agent },
                                    content: m.content, timestamp: Instant::now(), parsed: vec![], cached_height: None, is_error: false,
                                };
                                msg.prepare();
                                msg
                            }).collect(),
                            updated_at: data.updated_at,
                        });
                    }
                }
            }
        }
    }
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions
}

fn save_session_internal(session: &Session) -> Result<(), String> {
    let dir = sessions_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = session_path(&session.id);
    let data = SessionData {
        id: session.id.clone(), title: session.title.clone(),
        created_at: session.updated_at, updated_at: now_millis(),
        messages: session.messages.iter().map(|m| MessageData {
            role: match m.role { Role::User => "user".into(), Role::Agent => "agent".into() },
            content: m.content.clone(),
        }).collect(),
    };
    let content = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

/// Pretext-style height estimation for virtual list culling.
/// Called on the cold path (once per message when height cache is missing).
fn estimate_height(msg: &Message) -> f32 {
    use crate::ui::types::RenderBlock;
    let mut height = 28.0; // bubble padding + trailing space_8
    for block in &msg.parsed {
        match block {
            RenderBlock::Paragraph(spans) => {
                let chars: usize = spans.iter().map(|s| match s {
                    crate::ui::types::InlineSpan::Text(t)
                    | crate::ui::types::InlineSpan::Bold(t)
                    | crate::ui::types::InlineSpan::Code(t) => t.len(),
                    crate::ui::types::InlineSpan::Link { text, .. } => text.len(),
                }).sum();
                let lines = (chars / 55).max(1);
                height += lines as f32 * 18.0;
            }
            RenderBlock::Heading(_, _) => height += 24.0,
            RenderBlock::CodeBlock { code, .. } => {
                let lines = code.lines().count().max(1);
                height += lines as f32 * 16.0 + 30.0;
            }
            RenderBlock::ListItem(_) => height += 20.0,
            RenderBlock::Blockquote(_) => height += 20.0,
            RenderBlock::HorizontalRule => height += 20.0,
        }
        height += 4.0; // inter-block spacing
    }
    height
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SessionData { id: String, title: String, created_at: u64, updated_at: u64, messages: Vec<MessageData> }

#[derive(serde::Serialize, serde::Deserialize)]
struct MessageData { role: String, content: String }

fn new_session() -> Session {
    let id = format!("sess-{}", uuid::Uuid::new_v4());
    Session { id: id.clone(), title: "New Chat".into(), messages: vec![], updated_at: now_millis() }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn main() -> eframe::Result {
    tracing_subscriber::fmt::init();
    std::panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() { s.to_string() }
            else if let Some(s) = info.payload().downcast_ref::<String>() { s.clone() }
            else { "Unknown panic payload".to_string() };
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "unknown location".to_string());
        let report = format!("[{}] PANIC: {}\n", location, msg);
        eprintln!("{}", report);
        if let Some(data_dir) = dirs::data_dir() {
            let log_path = data_dir.join("clarity").join("panic.log");
            if let Err(e) = std::fs::create_dir_all(log_path.parent().unwrap_or(&data_dir)) { eprintln!("Failed to create panic log dir: {}", e); }
            if let Err(e) = std::fs::write(&log_path, report) { eprintln!("Failed to write panic log: {}", e); }
        }
    }));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native("Clarity", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
