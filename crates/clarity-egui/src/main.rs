use eframe::egui;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod app_state;
mod settings;

use app_state::{ensure_llm, reload_llm, AppState};
use settings::GuiSettings;

// ============================================================================
// Clarity egui Desktop — Phase 2: Real LLM Integration
// ============================================================================

const COLOR_BG: egui::Color32 = egui::Color32::from_rgb(26, 26, 26);
const COLOR_SIDEBAR_BG: egui::Color32 = egui::Color32::from_rgb(30, 30, 30);
const COLOR_SURFACE: egui::Color32 = egui::Color32::from_rgb(45, 45, 45);
const COLOR_BORDER: egui::Color32 = egui::Color32::from_rgb(60, 60, 60);
const COLOR_TEXT: egui::Color32 = egui::Color32::from_rgb(230, 230, 230);
const COLOR_TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(150, 150, 150);
const COLOR_USER_BUBBLE: egui::Color32 = egui::Color32::from_rgb(0, 120, 255);
const COLOR_AI_BUBBLE: egui::Color32 = egui::Color32::from_rgb(45, 45, 45);
const COLOR_STATUS_ONLINE: egui::Color32 = egui::Color32::from_rgb(34, 197, 94);
const COLOR_STATUS_BUSY: egui::Color32 = egui::Color32::from_rgb(234, 179, 8);
const COLOR_STATUS_OFFLINE: egui::Color32 = egui::Color32::from_rgb(239, 68, 68);
const SIDEBAR_WIDTH: f32 = 220.0;
const SIDEBAR_COLLAPSED_WIDTH: f32 = 0.0;

#[derive(Debug, Clone)]
enum UiEvent {
    Chunk(String),
    ToolStart { id: String, name: String, arguments: serde_json::Value },
    ToolResult { id: String, result: String },
    StepBegin { tool_name: String },
    CompactionBegin,
    CompactionEnd,
    Done,
    Error(String),
    Fallback { fallback: bool, reason: String },
    FallbackError { message: String },
}

#[derive(Clone)]
struct Session { id: String, title: String, messages: Vec<Message>, updated_at: u64 }

#[derive(Clone)]
struct Message { role: Role, content: String, #[allow(dead_code)] timestamp: Instant }

#[derive(Clone, Copy, PartialEq, Debug)]
enum Role { User, Agent }

#[derive(Clone, Copy, PartialEq, Debug)]
enum AgentStatus { Online, Busy, Unconfigured, Offline }

#[derive(Clone)]
struct ToolCallInfo { id: String, name: String, status: ToolCallStatus, result: Option<String> }

#[derive(Clone)]
enum ToolCallStatus { Running, Done }

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
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
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
                        if let Err(e) = ensure_llm(&state_for_monitor).await {
                            let _ = tx_for_monitor.send(UiEvent::FallbackError { message: e });
                        } else {
                            let _ = tx_for_monitor.send(UiEvent::Fallback { fallback: true, reason: "offline".into() });
                        }
                    } else if available && !prev {
                        if let Err(e) = ensure_llm(&state_for_monitor).await {
                            let _ = tx_for_monitor.send(UiEvent::FallbackError { message: e });
                        } else {
                            let _ = tx_for_monitor.send(UiEvent::Fallback { fallback: false, reason: "online".into() });
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

        Self {
            state, runtime, ui_tx, ui_rx, sessions, active_session_id: active_id,
            sidebar_collapsed: false, input: String::new(), is_loading: false,
            agent_status: AgentStatus::Unconfigured, network_banner: None,
            tool_calls: vec![], compacting: false,
            settings_open: false,
            settings_edit: GuiSettings::load(),
            frame_count: 0, last_fps_time: cc.egui_ctx.input(|i| i.time),
            fps: 0.0, start: now,
        }
    }

    fn active_session(&self) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == self.active_session_id)
    }
    fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.id == self.active_session_id)
    }

    fn send(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() || self.is_loading { return; }

        if let Some(session) = self.active_session_mut() {
            session.messages.push(Message { role: Role::User, content: text.clone(), timestamp: Instant::now() });
            session.updated_at = now_millis();
        }
        self.input.clear();
        self.is_loading = true;
        self.agent_status = AgentStatus::Busy;
        self.tool_calls.clear();

        let state = self.state.clone();
        let tx = self.ui_tx.clone();
        let query = text;

        self.runtime.spawn(async move {
            if let Err(e) = ensure_llm(&state).await {
                let _ = tx.send(UiEvent::Error(e));
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
                    if let Some(ev) = event { let _ = tx_wire.send(ev); }
                }
            });

            let tx_chunk = tx.clone();
            let result = agent.run_streaming(&query, move |chunk: &str| {
                let _ = tx_chunk.send(UiEvent::Chunk(chunk.to_string()));
            }).await;

            match result {
                Ok(_) => { let _ = tx.send(UiEvent::Done); }
                Err(clarity_core::AgentError::Cancelled) => { let _ = tx.send(UiEvent::Done); }
                Err(e) => { let _ = tx.send(UiEvent::Error(format!("Agent error: {}", e))); }
            }
        });
    }

    fn process_events(&mut self) {
        while let Ok(event) = self.ui_rx.try_recv() {
            match event {
                UiEvent::Chunk(text) => {
                    if let Some(session) = self.active_session_mut() {
                        if let Some(last) = session.messages.last_mut() {
                            if last.role == Role::Agent { last.content.push_str(&text); continue; }
                        }
                        session.messages.push(Message { role: Role::Agent, content: text, timestamp: Instant::now() });
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
                    self.save_current_session();
                }
                UiEvent::Error(msg) => {
                    self.is_loading = false; self.agent_status = AgentStatus::Online;
                    if let Some(session) = self.active_session_mut() {
                        session.messages.push(Message { role: Role::Agent, content: format!("Error: {}", msg), timestamp: Instant::now() });
                    }
                }
                UiEvent::Fallback { fallback, reason } => {
                    self.network_banner = Some(if fallback {
                        format!("Network unavailable — switched to local ({})", reason)
                    } else {
                        format!("Network restored — switched back to preferred provider ({})", reason)
                    });
                }
                UiEvent::FallbackError { message } => {
                    self.network_banner = Some(format!("Fallback error: {}", message));
                }
            }
        }
    }

    fn save_current_session(&self) {
        if let Some(session) = self.active_session() { let _ = save_session_internal(session); }
    }
    fn new_session(&mut self) {
        self.save_current_session();
        let s = new_session(); let id = s.id.clone();
        self.sessions.push(s); self.active_session_id = id;
    }
    fn delete_session(&mut self, id: String) {
        self.sessions.retain(|s| s.id != id);
        let _ = std::fs::remove_file(session_path(&id));
        if self.sessions.is_empty() { self.new_session(); }
        else if self.active_session_id == id { self.active_session_id = self.sessions[0].id.clone(); }
    }

    fn render_settings_panel(&mut self, ctx: &egui::Context) {
        if !self.settings_open { return; }

        egui::Window::new("Settings")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .frame(egui::Frame::window(&ctx.style()).fill(COLOR_SURFACE).corner_radius(egui::CornerRadius::same(12)))
            .show(ctx, |ui| {
                ui.set_min_width(400.0);
                ui.add_space(8.0);

                // Provider
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Provider").size(13.0).color(COLOR_TEXT));
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
                    ui.label(egui::RichText::new("Model").size(13.0).color(COLOR_TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_sized(egui::vec2(200.0, 28.0), egui::TextEdit::singleline(&mut self.settings_edit.model).text_color(COLOR_TEXT));
                    });
                });
                ui.add_space(8.0);

                // API Key
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("API Key").size(13.0).color(COLOR_TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut key = self.settings_edit.api_key.clone().unwrap_or_default();
                        let response = ui.add_sized(egui::vec2(200.0, 28.0), egui::TextEdit::singleline(&mut key).password(true).text_color(COLOR_TEXT));
                        if response.changed() {
                            self.settings_edit.api_key = if key.is_empty() { None } else { Some(key) };
                        }
                    });
                });
                ui.add_space(8.0);

                // Local model path
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Local Model Path").size(13.0).color(COLOR_TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut path = self.settings_edit.local_model_path.clone().unwrap_or_default();
                        let response = ui.add_sized(egui::vec2(200.0, 28.0), egui::TextEdit::singleline(&mut path).text_color(COLOR_TEXT));
                        if response.changed() {
                            self.settings_edit.local_model_path = if path.is_empty() { None } else { Some(path) };
                        }
                    });
                });
                ui.add_space(8.0);

                // Approval mode
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Approval Mode").size(13.0).color(COLOR_TEXT));
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
                        if ui.add(egui::Button::new(egui::RichText::new("Save").size(13.0).color(COLOR_TEXT)).fill(COLOR_USER_BUBBLE).min_size(egui::vec2(80.0, 32.0))).clicked() {
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
                        if ui.add(egui::Button::new(egui::RichText::new("Cancel").size(13.0).color(COLOR_TEXT)).fill(COLOR_BORDER).min_size(egui::vec2(80.0, 32.0))).clicked() {
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

        use clarity_core::agent::AgentState;
        self.agent_status = match self.state.agent.state() {
            AgentState::Unconfigured => AgentStatus::Unconfigured,
            AgentState::Idle => if self.is_loading { AgentStatus::Busy } else { AgentStatus::Online },
            AgentState::Running { .. } => AgentStatus::Busy,
            AgentState::Stalled => AgentStatus::Offline,
        };

        ctx.style_mut(|style| {
            style.visuals.override_text_color = Some(COLOR_TEXT);
            style.visuals.panel_fill = COLOR_BG;
            style.visuals.window_fill = COLOR_SURFACE;
            style.visuals.extreme_bg_color = COLOR_SIDEBAR_BG;
            style.visuals.widgets.inactive.weak_bg_fill = COLOR_SURFACE;
            style.visuals.widgets.inactive.bg_fill = COLOR_SURFACE;
            style.visuals.widgets.hovered.weak_bg_fill = COLOR_BORDER;
            style.visuals.widgets.hovered.bg_fill = COLOR_BORDER;
            style.visuals.widgets.active.bg_fill = COLOR_BORDER;
            style.visuals.selection.bg_fill = COLOR_USER_BUBBLE;
            style.visuals.selection.stroke = egui::Stroke::NONE;
            style.visuals.window_corner_radius = egui::CornerRadius::same(8);
            style.visuals.window_shadow = egui::Shadow::NONE;
            style.visuals.popup_shadow = egui::Shadow::NONE;
        });

        let sidebar_width = if self.sidebar_collapsed { SIDEBAR_COLLAPSED_WIDTH } else { SIDEBAR_WIDTH };

        if !self.sidebar_collapsed {
            egui::SidePanel::left("sidebar")
                .exact_width(sidebar_width)
                .resizable(false)
                .frame(egui::Frame::side_top_panel(&ctx.style()).fill(COLOR_SIDEBAR_BG))
                .show(ctx, |ui| {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Clarity").size(18.0).strong().color(COLOR_TEXT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("⬅").clicked() { self.sidebar_collapsed = true; }
                        });
                    });
                    ui.add_space(16.0);

                    if ui.add(egui::Button::new(egui::RichText::new("+ New Chat").size(13.0).color(COLOR_TEXT))
                        .fill(COLOR_SURFACE).corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(ui.available_width(), 36.0))).clicked() { self.new_session(); }
                    ui.add_space(12.0);

                    ui.label(egui::RichText::new("Sessions").size(11.0).color(COLOR_TEXT_DIM).weak());
                    ui.add_space(4.0);

                    let mut to_delete: Option<String> = None;
                    let sessions_clone: Vec<(String, String, bool)> = self.sessions.iter().map(|s| (s.id.clone(), s.title.clone(), s.id == self.active_session_id)).collect();
                    for (id, title, is_active) in sessions_clone {
                        let bg = if is_active { COLOR_SURFACE } else { COLOR_SIDEBAR_BG };
                        let text_color = if is_active { COLOR_TEXT } else { COLOR_TEXT_DIM };
                        ui.horizontal(|ui| {
                            let response = ui.add(egui::Button::new(egui::RichText::new(&title).size(13.0).color(text_color))
                                .fill(bg).corner_radius(egui::CornerRadius::same(6))
                                .min_size(egui::vec2(ui.available_width() - 28.0, 36.0)));
                            if response.clicked() { self.save_current_session(); self.active_session_id = id.clone(); }
                            if ui.add(egui::Button::new("🗑").fill(egui::Color32::TRANSPARENT)).clicked() { to_delete = Some(id); }
                        });
                    }
                    if let Some(id) = to_delete { self.delete_session(id); }

                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("FPS: {:.0}", self.fps)).size(10.0).color(COLOR_TEXT_DIM));
                        });
                    });
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).fill(COLOR_BG))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.sidebar_collapsed { if ui.button("➡").clicked() { self.sidebar_collapsed = false; } }
                    ui.label(egui::RichText::new("Chat").size(16.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("⚙").clicked() {
                            self.settings_open = true;
                            self.settings_edit = {
                                let guard = self.state.cached_settings.lock().unwrap();
                                guard.clone()
                            };
                        }
                        let (status_color, status_label) = match self.agent_status {
                            AgentStatus::Online => (COLOR_STATUS_ONLINE, "Online"),
                            AgentStatus::Busy => (COLOR_STATUS_BUSY, "Busy"),
                            AgentStatus::Unconfigured => (COLOR_STATUS_OFFLINE, "Unconfigured"),
                            AgentStatus::Offline => (COLOR_STATUS_OFFLINE, "Offline"),
                        };
                        ui.label(egui::RichText::new(status_label).size(12.0).color(COLOR_TEXT_DIM));
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, status_color);
                    });
                });
                ui.separator();

                let banner_text = self.network_banner.clone();
                if let Some(banner) = banner_text {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&banner).size(12.0).color(COLOR_STATUS_BUSY));
                        if ui.button("✕").clicked() { self.network_banner = None; }
                    });
                    ui.separator();
                }

                if self.compacting {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Compacting conversation history…").size(12.0).color(COLOR_TEXT_DIM));
                    });
                    ui.separator();
                }

                let available_height = ui.available_height() - 70.0;
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(available_height)
                    .show(ui, |ui| {
                        if let Some(session) = self.active_session() {
                            if session.messages.is_empty() && !self.is_loading {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(120.0);
                                    ui.label(egui::RichText::new("Clarity").size(32.0).strong().color(COLOR_TEXT_DIM));
                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new("Local-first AI agent runtime").size(14.0).color(COLOR_TEXT_DIM));
                                    ui.add_space(24.0);
                                    if ui.add(egui::Button::new(egui::RichText::new("Configure Settings").size(13.0).color(COLOR_TEXT))
                                        .fill(COLOR_SURFACE).corner_radius(egui::CornerRadius::same(6))
                                        .min_size(egui::vec2(180.0, 40.0))).clicked() { self.agent_status = AgentStatus::Unconfigured; }
                                });
                            } else {
                                for msg in &session.messages { message_bubble(ui, msg); }
                                for tc in &self.tool_calls { tool_call_bubble(ui, tc); }
                                if self.is_loading && self.active_session().map_or(true, |s| s.messages.last().map_or(true, |m| m.role == Role::User)) && self.tool_calls.is_empty() {
                                    typing_indicator(ui);
                                }
                            }
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    let available_width = ui.available_width();
                    let input_width = available_width - 60.0;
                    ui.allocate_ui_with_layout(
                        egui::vec2(input_width, 50.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            let text_edit = egui::TextEdit::multiline(&mut self.input)
                                .desired_rows(1).hint_text("Type a message...").margin(egui::vec2(10.0, 10.0));
                            ui.add_sized(egui::vec2(input_width, 50.0), text_edit);
                        },
                    );
                    ui.vertical_centered(|ui| {
                        let btn = ui.add_sized(
                            egui::vec2(44.0, 44.0),
                            egui::Button::new(egui::RichText::new("➤").size(18.0).color(COLOR_TEXT))
                                .fill(COLOR_USER_BUBBLE).corner_radius(egui::CornerRadius::same(8)),
                        );
                        if btn.clicked() { self.send(); }
                    });
                });
            });

        self.render_settings_panel(ctx);
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
                            messages: data.messages.into_iter().map(|m| Message {
                                role: if m.role == "user" { Role::User } else { Role::Agent },
                                content: m.content, timestamp: Instant::now(),
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

fn message_bubble(ui: &mut egui::Ui, msg: &Message) {
    let (align, bg, text_color) = match msg.role {
        Role::User => (egui::Align::RIGHT, COLOR_USER_BUBBLE, egui::Color32::WHITE),
        Role::Agent => (egui::Align::LEFT, COLOR_AI_BUBBLE, COLOR_TEXT),
    };
    ui.with_layout(egui::Layout::top_down(align), |ui| {
        egui::Frame::group(ui.style()).fill(bg).corner_radius(egui::CornerRadius::same(12))
            .stroke(egui::Stroke::NONE).inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(&msg.content).color(text_color).size(14.0));
            });
    });
    ui.add_space(8.0);
}

fn tool_call_bubble(ui: &mut egui::Ui, tc: &ToolCallInfo) {
    let bg = egui::Color32::from_rgb(50, 50, 50);
    let icon = match tc.status { ToolCallStatus::Running => "⏳", ToolCallStatus::Done => "✅" };
    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        egui::Frame::group(ui.style()).fill(bg).corner_radius(egui::CornerRadius::same(8))
            .stroke(egui::Stroke::new(1.0, COLOR_BORDER)).inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).size(14.0));
                    ui.label(egui::RichText::new(&tc.name).size(12.0).strong().color(COLOR_TEXT_DIM));
                });
                if let Some(ref result) = tc.result {
                    ui.label(egui::RichText::new(truncate(result, 200)).size(11.0).color(COLOR_TEXT_DIM));
                }
            });
    });
    ui.add_space(6.0);
}

fn typing_indicator(ui: &mut egui::Ui) {
    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        egui::Frame::group(ui.style()).fill(COLOR_AI_BUBBLE).corner_radius(egui::CornerRadius::same(12))
            .stroke(egui::Stroke::NONE).inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("● ● ●").size(12.0).color(COLOR_TEXT_DIM));
            });
    });
    ui.add_space(8.0);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len { s.to_string() } else { format!("{}…", &s[..max_len]) }
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
            let _ = std::fs::create_dir_all(log_path.parent().unwrap_or(&data_dir));
            let _ = std::fs::write(&log_path, report);
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
