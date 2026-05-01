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
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

mod app_state;
mod error;
mod i18n;
mod llm_binder;
mod llm_loader;
mod llm_policy;
mod panels;
mod provider;
mod session;
mod settings;
mod theme;
mod ui;
mod stores;
mod services;
mod handlers;

use app_state::AppState;


use ui::types::*;

// ============================================================================
// Clarity egui Desktop — Phase A: Design System Foundation
// ============================================================================

const SIDEBAR_WIDTH: f32 = 240.0;
const TITLEBAR_HEIGHT: f32 = 36.0;

pub(crate) struct App {
    // === Core Runtime ===
    pub(crate) state: Arc<AppState>,
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) ui_tx: Sender<UiEvent>,
    pub(crate) ui_rx: Receiver<UiEvent>,
    // === Domain Stores (Zustand-style slices) ===
    pub(crate) session_store: stores::SessionStore,
    pub(crate) chat_store: stores::ChatStore,
    pub(crate) settings_store: stores::SettingsStore,
    pub(crate) task_store: stores::TaskStore,
    pub(crate) ui_store: stores::UiStore,
    pub(crate) subagent_store: stores::SubAgentStore,
    pub(crate) mcp_store: stores::McpStore,
    pub(crate) onboarding_store: stores::OnboardingStore,
}

mod app_logic;
mod onboarding;

impl App {
    /// Render a custom titlebar with window drag and control buttons.
    ///
    /// LAYOUT (two independent sub-layouts at the same vertical origin):
    ///   ┌─ left_to_right ──────────────────────────┐  ┌─ right_to_left ─┐
    ///   │ [☰] Clarity  [drag region ─── elastic]  │  │ [─] [□] [✕]    │
    ///   └──────────────────────────────────────────┘  └─────────────────┘
    ///
    /// ARCHITECTURE NOTE:
    ///   The drag region uses `allocate_exact_size` ONLY inside a horizontal
    ///   sub-layout, so `avail` is REMAINING WIDTH — not the full panel height.
    ///   This avoids the layout feedback loop where the drag region consumed
    ///   the entire panel, forcing content below and causing panel growth
    ///   every frame.
    ///
    ///   Button sub-layout (right_to_left) is rendered second, so its buttons
    ///   have higher z-order than the drag region — clicks on buttons are
    ///   NOT swallowed by the drag.
    /// Render a panel with panic isolation (error boundary).
    /// Mimics React ErrorBoundary: a child panel panic does not crash the entire app.
    fn render_safe<F>(&mut self, ctx: &egui::Context, name: &str, mut render: F)
    where
        F: FnMut(&mut Self, &egui::Context),
    {
        if let Err(e) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render(self, ctx)))
        {
            let payload = if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            tracing::error!("Panel '{}' panicked: {}", name, payload);
            self.push_toast(format!("UI error in {} panel", name), ToastLevel::Error);
        }
    }

    fn render_titlebar(&mut self, ctx: &egui::Context) {
        let theme = &self.ui_store.theme;
        let btn_size = egui::vec2(36.0, TITLEBAR_HEIGHT);

        egui::TopBottomPanel::top("titlebar")
            .min_height(TITLEBAR_HEIGHT)
            .frame(egui::Frame::new()
                .fill(theme.bg)
                .inner_margin(egui::Margin::symmetric(8, 0)))
            .show(ctx, |ui| {
                ui.set_min_height(TITLEBAR_HEIGHT);

                // Single horizontal row: [toggle?] [title] [elastic drag] [buttons]
                ui.horizontal_centered(|ui| {
                    ui.set_min_height(TITLEBAR_HEIGHT);

                    // Sidebar toggle when collapsed
                    if self.ui_store.sidebar_collapsed {
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new(crate::theme::ICON_LIST).font(theme.font_icon(theme.text_base)))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                            )
                            .clicked()
                        {
                            self.ui_store.sidebar_collapsed = false;
                        }
                        ui.add_space(8.0);
                    }

                    ui.label(
                        egui::RichText::new("Clarity")
                            .size(theme.text_base)
                            .strong()
                            .color(theme.text_muted),
                    );

                    // Elastic filler — drag to move window.
                    // Using `allocate_exact_size` with remaining horizontal space
                    // creates a click-and-drag region that fills the titlebar.
                    let drag_w = ui.available_size().x.max(40.0);
                    let (_drag_id, drag_resp) = ui.allocate_exact_size(
                        egui::vec2(drag_w, TITLEBAR_HEIGHT),
                        egui::Sense::click_and_drag(),
                    );
                    if drag_resp.drag_started_by(egui::PointerButton::Primary) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }

                    // Window control buttons (right-aligned)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Close
                        let close_resp = ui.add_sized(btn_size,
                            egui::Button::new(egui::RichText::new(crate::theme::ICON_X).font(theme.font_icon(theme.text_base)).color(theme.text_dim))
                                .fill(egui::Color32::TRANSPARENT)
                        );
                        if close_resp.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        } else {
                            let fill = if close_resp.hovered() {
                                theme.danger.linear_multiply(0.25)
                            } else {
                                egui::Color32::TRANSPARENT
                            };
                            let text_col = if close_resp.hovered() {
                                egui::Color32::WHITE
                            } else {
                                theme.text_dim
                            };
                            ui.painter().rect_filled(close_resp.rect, egui::CornerRadius::ZERO, fill);
                            ui.painter().text(close_resp.rect.center(), egui::Align2::CENTER_CENTER,
                                crate::theme::ICON_X, theme.font_icon(14.0), text_col);
                        }

                        // Maximize
                        let max_resp = ui.add_sized(btn_size,
                            egui::Button::new(egui::RichText::new("□").size(theme.text_sm).color(theme.text_dim))
                                .fill(egui::Color32::TRANSPARENT)
                        );
                        if max_resp.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                        } else if max_resp.hovered() {
                            ui.painter().rect_filled(max_resp.rect, egui::CornerRadius::ZERO, theme.overlay_medium);
                        }

                        // Minimize
                        let min_resp = ui.add_sized(btn_size,
                            egui::Button::new(egui::RichText::new("─").size(theme.text_sm).color(theme.text_dim))
                                .fill(egui::Color32::TRANSPARENT)
                        );
                        if min_resp.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        } else if min_resp.hovered() {
                            ui.painter().rect_filled(min_resp.rect, egui::CornerRadius::ZERO, theme.overlay_medium);
                        }
                    });
                });
            });
    }

    fn render_settings_panel(&mut self, ctx: &egui::Context) {
        panels::settings::render_settings_panel(self, ctx);
    }

    fn render_chat_area(&mut self, ctx: &egui::Context) {
        panels::chat::render_chat_area(self, ctx);
    }

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        panels::sidebar::render_sidebar(self, ctx);
    }

    fn render_task_panel(&mut self, ctx: &egui::Context) {
        panels::task::render_task_panel(self, ctx);
    }

    fn render_mcp_panel(&mut self, ctx: &egui::Context) {
        panels::mcp::render_mcp_panel(self, ctx);
    }

    fn render_task_create_modal(&mut self, ctx: &egui::Context) {
        panels::task_create::render_task_create_modal(self, ctx);
    }

    fn render_skill_panel(&mut self, ctx: &egui::Context) {
        panels::skill::render_skill_panel(self, ctx);
    }

    fn render_toolbar(&mut self, ctx: &egui::Context) {
        panels::toolbar::render_toolbar(self, ctx);
    }

    fn render_toasts(&mut self, ctx: &egui::Context) {
        panels::toast::render_toasts(self, ctx);
    }

    fn render_approval_modal(&mut self, ctx: &egui::Context) {
        panels::approval::render_approval_modal(self, ctx);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        self.ui_store.frame_count += 1;
        if now - self.ui_store.last_fps_time >= 1.0 {
            self.ui_store.fps = self.ui_store.frame_count as f64 / (now - self.ui_store.last_fps_time);
            self.ui_store.frame_count = 0;
            self.ui_store.last_fps_time = now;
        }

        self.process_events();

        // Drain batch-grant auto-approval notifications and show toasts.
        for msg in self.state.mode_aware_approval_runtime.drain_auto_approval_notifications() {
            self.push_toast(msg, ToastLevel::Info);
        }

        if self.chat_store.is_loading {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        // File drag-and-drop
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            for file in dropped_files {
                if let Some(path) = file.path {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    self.chat_store.attachments.push(Attachment { path, name });
                }
            }
        }

        // Check if approval modal is active — if so, suppress main-UI shortcuts.
        let approval_active = !self.ui_store.pending_approvals.is_empty();

        // ESC closes modals (but not when approval modal is open).
        if !approval_active && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.settings_store.settings_open {
                self.settings_store.settings_open = false;
            } else if self.ui_store.skill_panel_open {
                self.ui_store.skill_panel_open = false;
            }
        }

        if !self.settings_store.settings_open
            && !self.chat_store.is_loading
            && !approval_active
            && ctx.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.ctrl)
        {
            self.new_session();
        }

        // Ctrl+C stops the running agent turn (only when generating).
        if self.chat_store.is_loading
            && ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl)
        {
            self.stop();
        }

        // Refresh task list periodically when panel is open
        if self.task_store.task_panel_open && self.task_store.last_task_refresh.elapsed() > Duration::from_secs(3) {
            self.refresh_tasks();
        }

        // Poll parallel batch status when panel is open
        if self.task_store.task_panel_open
            && !self.subagent_store.parallel_batches.is_empty()
            && self.subagent_store.last_parallel_poll.elapsed() > Duration::from_secs(2)
        {
            self.poll_parallel_batches();
        }

        use clarity_core::agent::AgentState;
        self.chat_store.agent_status = match self.state.agent.state() {
            AgentState::Unconfigured => AgentStatus::Unconfigured,
            AgentState::Idle => {
                if self.chat_store.is_loading {
                    AgentStatus::Busy
                } else {
                    AgentStatus::Online
                }
            }
            AgentState::Running { .. } => AgentStatus::Busy,
            AgentState::Stalled => AgentStatus::Offline,
        };

        ctx.style_mut(|style| {
            self.ui_store.theme.apply(style);
        });

        self.render_safe(ctx, "titlebar", |app, ctx| app.render_titlebar(ctx));
        self.render_safe(ctx, "sidebar", |app, ctx| app.render_sidebar(ctx));
        self.render_safe(ctx, "task", |app, ctx| app.render_task_panel(ctx));
        self.render_safe(ctx, "toolbar", |app, ctx| app.render_toolbar(ctx));
        self.render_safe(ctx, "chat", |app, ctx| app.render_chat_area(ctx));
        self.render_safe(ctx, "settings", |app, ctx| app.render_settings_panel(ctx));
        self.render_safe(ctx, "skill", |app, ctx| app.render_skill_panel(ctx));
        self.render_safe(ctx, "mcp", |app, ctx| app.render_mcp_panel(ctx));
        self.render_safe(ctx, "toast", |app, ctx| app.render_toasts(ctx));
        self.render_safe(ctx, "approval", |app, ctx| app.render_approval_modal(ctx));
        self.render_safe(ctx, "task_create", |app, ctx| app.render_task_create_modal(ctx));
        onboarding::render_onboarding(self, ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_current_session();
    }
}

fn main() -> eframe::Result {
    tracing_subscriber::fmt::init();
    std::panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        let report = format!("[{}] PANIC: {}\n", location, msg);
        eprintln!("{}", report);
        if let Some(data_dir) = dirs::data_dir() {
            let log_path = data_dir.join("clarity").join("panic.log");
            if let Err(e) = std::fs::create_dir_all(log_path.parent().unwrap_or(&data_dir)) {
                eprintln!("Failed to create panic log dir: {}", e);
            }
            if let Err(e) = std::fs::write(&log_path, report) {
                eprintln!("Failed to write panic log: {}", e);
            }
        }
    }));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([900.0, 600.0])
            .with_decorations(false),
        ..Default::default()
    };

    eframe::run_native(
        "Clarity",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
