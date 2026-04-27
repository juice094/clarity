use eframe::egui;
use std::time::{Duration, Instant};

// ============================================================================
// Clarity egui Desktop — Replaces Tauri
// ============================================================================
// Design goal: Pixel-for-pixel reproduction of Tauri frontend visual style
// in immediate-mode Rust. Dark-first theme matching App.tsx CSS.
//
// Phase 1 (this file): Static mock — no backend integration
// Phase 2: Wire clarity-core / clarity-gateway for real LLM chat
// ============================================================================

const COLOR_BG: egui::Color32 = egui::Color32::from_rgb(26, 26, 26);          // #1a1a1a
const COLOR_SIDEBAR_BG: egui::Color32 = egui::Color32::from_rgb(30, 30, 30);   // #1e1e1e
const COLOR_SURFACE: egui::Color32 = egui::Color32::from_rgb(45, 45, 45);      // #2d2d2d
const COLOR_BORDER: egui::Color32 = egui::Color32::from_rgb(60, 60, 60);       // #3c3c3c
const COLOR_TEXT: egui::Color32 = egui::Color32::from_rgb(230, 230, 230);      // #e6e6e6
const COLOR_TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(150, 150, 150);  // #969696
const COLOR_USER_BUBBLE: egui::Color32 = egui::Color32::from_rgb(0, 120, 255); // #0078ff
const COLOR_AI_BUBBLE: egui::Color32 = egui::Color32::from_rgb(45, 45, 45);    // #2d2d2d
const COLOR_STATUS_ONLINE: egui::Color32 = egui::Color32::from_rgb(34, 197, 94);
const COLOR_STATUS_BUSY: egui::Color32 = egui::Color32::from_rgb(234, 179, 8);
const COLOR_STATUS_OFFLINE: egui::Color32 = egui::Color32::from_rgb(239, 68, 68);
const SIDEBAR_WIDTH: f32 = 220.0;
const SIDEBAR_COLLAPSED_WIDTH: f32 = 0.0;

struct Session {
    id: String,
    title: String,
    messages: Vec<Message>,
    updated_at: Instant,
}

struct Message {
    role: Role,
    content: String,
    #[allow(dead_code)]
    timestamp: Instant,
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    User,
    Agent,
}

#[allow(dead_code)]
enum AgentStatus {
    Online,
    Busy,
    Unconfigured,
    Offline,
}

struct App {
    sessions: Vec<Session>,
    active_session_id: String,
    sidebar_collapsed: bool,
    input: String,
    is_loading: bool,
    agent_status: AgentStatus,
    // Streaming simulation
    pending_ai: bool,
    ai_timer: Option<Instant>,
    ai_target: String,
    // FPS / debug
    frame_count: u64,
    last_fps_time: f64,
    fps: f64,
    #[allow(dead_code)]
    start: Instant,
}

impl App {
    fn new() -> Self {
        let now = Instant::now();
        let session = Session {
            id: "sess-1".into(),
            title: "New Chat".into(),
            messages: vec![
                Message {
                    role: Role::User,
                    content: "Hello, can you explain what Clarity is?".into(),
                    timestamp: now,
                },
                Message {
                    role: Role::Agent,
                    content: "Clarity is a local-first AI agent runtime. It runs entirely on your machine, with optional cloud fallback.".into(),
                    timestamp: now,
                },
            ],
            updated_at: now,
        };
        let id = session.id.clone();
        Self {
            sessions: vec![session],
            active_session_id: id,
            sidebar_collapsed: false,
            input: String::new(),
            is_loading: false,
            agent_status: AgentStatus::Online,
            pending_ai: false,
            ai_timer: None,
            ai_target: String::new(),
            frame_count: 0,
            last_fps_time: 0.0,
            fps: 0.0,
            start: now,
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
        if text.is_empty() || self.is_loading {
            return;
        }
        if let Some(session) = self.active_session_mut() {
            session.messages.push(Message {
                role: Role::User,
                content: text.clone(),
                timestamp: Instant::now(),
            });
            session.updated_at = Instant::now();
        }
        self.input.clear();
        self.is_loading = true;
        self.agent_status = AgentStatus::Busy;

        // Simulate AI response
        self.pending_ai = true;
        self.ai_timer = Some(Instant::now());
        self.ai_target = format!("Echo: {}", text);
    }

    fn tick_ai(&mut self) {
        if !self.pending_ai {
            return;
        }
        let now = Instant::now();
        if let Some(timer) = self.ai_timer {
            if timer.elapsed() >= Duration::from_millis(40) {
                let chars: Vec<char> = self.ai_target.chars().collect();
                if let Some(session) = self.active_session_mut() {
                    if let Some(last) = session.messages.last_mut() {
                        if last.role == Role::Agent {
                            let current_len = last.content.chars().count();
                            if current_len < chars.len() {
                                let next = chars[current_len..].iter().take(4).collect::<String>();
                                last.content.push_str(&next);
                                self.ai_timer = Some(now);
                                return;
                            }
                        }
                    }
                    // Start new agent message
                    if session.messages.last().map_or(true, |m| m.role == Role::User) {
                        session.messages.push(Message {
                            role: Role::Agent,
                            content: chars.get(0..1).unwrap_or_default().iter().collect(),
                            timestamp: now,
                        });
                    } else if session.messages.last().unwrap().content.len() >= self.ai_target.len() {
                        self.pending_ai = false;
                        self.ai_timer = None;
                        self.is_loading = false;
                        self.agent_status = AgentStatus::Online;
                        return;
                    }
                }
                self.ai_timer = Some(now);
            }
        }
    }

    fn new_session(&mut self) {
        let id = format!("sess-{}", self.sessions.len() + 1);
        let session = Session {
            id: id.clone(),
            title: "New Chat".into(),
            messages: vec![],
            updated_at: Instant::now(),
        };
        self.sessions.push(session);
        self.active_session_id = id;
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // FPS counter
        let now = ctx.input(|i| i.time);
        self.frame_count += 1;
        if now - self.last_fps_time >= 1.0 {
            self.fps = self.frame_count as f64 / (now - self.last_fps_time);
            self.frame_count = 0;
            self.last_fps_time = now;
        }

        self.tick_ai();
        if self.pending_ai {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        // Global dark theme overrides
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

        // ---- Sidebar ----
        let sidebar_width = if self.sidebar_collapsed {
            SIDEBAR_COLLAPSED_WIDTH
        } else {
            SIDEBAR_WIDTH
        };

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
                            if ui.button("⬅").clicked() {
                                self.sidebar_collapsed = true;
                            }
                        });
                    });
                    ui.add_space(16.0);

                    // New chat button
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("+ New Chat").size(13.0).color(COLOR_TEXT),
                            )
                            .fill(COLOR_SURFACE)
                            .corner_radius(egui::CornerRadius::same(6))
                            .min_size(egui::vec2(ui.available_width(), 36.0)),
                        )
                        .clicked()
                    {
                        self.new_session();
                    }
                    ui.add_space(12.0);

                    ui.label(
                        egui::RichText::new("Sessions")
                            .size(11.0)
                            .color(COLOR_TEXT_DIM)
                            .weak(),
                    );
                    ui.add_space(4.0);

                    // Session list
                    for session in &self.sessions {
                        let is_active = session.id == self.active_session_id;
                        let bg = if is_active { COLOR_SURFACE } else { COLOR_SIDEBAR_BG };
                        let text_color = if is_active { COLOR_TEXT } else { COLOR_TEXT_DIM };

                        let response = ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(&session.title)
                                        .size(13.0)
                                        .color(text_color),
                                )
                                .fill(bg)
                                .corner_radius(egui::CornerRadius::same(6))
                                .min_size(egui::vec2(ui.available_width(), 36.0)),
                            );
                        if response.clicked() {
                            self.active_session_id = session.id.clone();
                        }
                    }

                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("FPS: {:.0}", self.fps))
                                    .size(10.0)
                                    .color(COLOR_TEXT_DIM),
                            );
                        });
                    });
                });
        }

        // ---- Main Chat Area ----
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).fill(COLOR_BG))
            .show(ctx, |ui| {
                // Top header
                ui.horizontal(|ui| {
                    if self.sidebar_collapsed {
                        if ui.button("➡").clicked() {
                            self.sidebar_collapsed = false;
                        }
                    }
                    ui.label(egui::RichText::new("Chat").size(16.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Status dot
                        let (status_color, status_label) = match self.agent_status {
                            AgentStatus::Online => (COLOR_STATUS_ONLINE, "Online"),
                            AgentStatus::Busy => (COLOR_STATUS_BUSY, "Busy"),
                            AgentStatus::Unconfigured => (COLOR_STATUS_OFFLINE, "Unconfigured"),
                            AgentStatus::Offline => (COLOR_STATUS_OFFLINE, "Offline"),
                        };
                        ui.label(
                            egui::RichText::new(status_label)
                                .size(12.0)
                                .color(COLOR_TEXT_DIM),
                        );
                        let (rect, _response) = ui.allocate_exact_size(
                            egui::vec2(8.0, 8.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().circle_filled(rect.center(), 4.0, status_color);
                    });
                });
                ui.separator();

                // Messages area
                let available_height = ui.available_height() - 70.0;
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(available_height)
                    .show(ui, |ui| {
                        if let Some(session) = self.active_session() {
                            if session.messages.is_empty() && !self.is_loading {
                                // Welcome screen
                                ui.vertical_centered(|ui| {
                                    ui.add_space(120.0);
                                    ui.label(
                                        egui::RichText::new("Clarity")
                                            .size(32.0)
                                            .strong()
                                            .color(COLOR_TEXT_DIM),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(
                                        egui::RichText::new("Local-first AI agent runtime")
                                            .size(14.0)
                                            .color(COLOR_TEXT_DIM),
                                    );
                                    ui.add_space(24.0);
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new("Configure Settings")
                                                    .size(13.0)
                                                    .color(COLOR_TEXT),
                                            )
                                            .fill(COLOR_SURFACE)
                                            .corner_radius(egui::CornerRadius::same(6))
                                            .min_size(egui::vec2(180.0, 40.0)),
                                        )
                                        .clicked()
                                    {
                                        self.agent_status = AgentStatus::Unconfigured;
                                    }
                                });
                            } else {
                                for msg in &session.messages {
                                    message_bubble(ui, msg);
                                }
                                if self.is_loading
                                    && session.messages.last().map_or(true, |m| m.role == Role::User)
                                {
                                    typing_indicator(ui);
                                }
                            }
                        }
                    });

                // Input area
                ui.separator();
                ui.horizontal(|ui| {
                    let available_width = ui.available_width();
                    let input_width = available_width - 60.0;

                    ui.allocate_ui_with_layout(
                        egui::vec2(input_width, 50.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            let text_edit = egui::TextEdit::multiline(&mut self.input)
                                .desired_rows(1)
                                .hint_text("Type a message...")
                                .margin(egui::vec2(10.0, 10.0));
                            ui.add_sized(egui::vec2(input_width, 50.0), text_edit);
                        },
                    );

                    ui.vertical_centered(|ui| {
                        let btn = ui.add_sized(
                            egui::vec2(44.0, 44.0),
                            egui::Button::new(
                                egui::RichText::new("➤").size(18.0).color(COLOR_TEXT),
                            )
                            .fill(COLOR_USER_BUBBLE)
                            .corner_radius(egui::CornerRadius::same(8)),
                        );
                        if btn.clicked() {
                            self.send();
                        }
                    });
                });
            });
    }
}

fn message_bubble(ui: &mut egui::Ui, msg: &Message) {
    let (align, bg, text_color) = match msg.role {
        Role::User => (
            egui::Align::RIGHT,
            COLOR_USER_BUBBLE,
            egui::Color32::WHITE,
        ),
        Role::Agent => (egui::Align::LEFT, COLOR_AI_BUBBLE, COLOR_TEXT),
    };

    ui.with_layout(egui::Layout::top_down(align), |ui| {
        egui::Frame::group(ui.style())
            .fill(bg)
            .corner_radius(egui::CornerRadius::same(12))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(&msg.content)
                        .color(text_color)
                        .size(14.0),
                );
            });
    });
    ui.add_space(8.0);
}

fn typing_indicator(ui: &mut egui::Ui) {
    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        egui::Frame::group(ui.style())
            .fill(COLOR_AI_BUBBLE)
            .corner_radius(egui::CornerRadius::same(12))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("● ● ●")
                        .size(12.0)
                        .color(COLOR_TEXT_DIM),
                );
            });
    });
    ui.add_space(8.0);
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Clarity",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
