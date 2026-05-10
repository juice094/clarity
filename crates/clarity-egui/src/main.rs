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
use std::time::{Duration, Instant};

mod app_state;
mod components;
mod error;
mod handlers;
mod i18n;
mod llm_binder;
mod llm_loader;
mod llm_policy;
mod panels;
mod platform;
mod provider;
mod render;
mod services;
mod session;
mod settings;
mod shortcuts;
mod stores;
mod theme;
mod ui;
mod widgets;

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
    pub(crate) cron_store: stores::CronStore,
    pub(crate) ui_store: stores::UiStore,
    pub(crate) subagent_store: stores::SubAgentStore,
    pub(crate) mcp_store: stores::McpStore,
    pub(crate) onboarding_store: stores::OnboardingStore,
    pub(crate) team_store: stores::TeamStore,
    pub(crate) snapshot_store: stores::SnapshotStore,
    /// Gateway process manager (auto-start + manual control).
    #[allow(dead_code)]
    pub(crate) gateway_manager: Option<crate::services::gateway_manager::GatewayManager>,
    /// File-system watcher for live skill reloading.
    #[allow(dead_code)]
    pub(crate) skill_watcher: Option<clarity_core::skills::SkillWatcher>,
    /// System tray manager (minimize-to-tray + context menu).
    pub(crate) tray_manager: Option<crate::services::tray::TrayManager>,
    /// When true, the next close request should be honoured (Quit from tray menu).
    pub(crate) tray_quit_requested: bool,
    /// Last tray status to avoid redundant icon updates every frame.
    pub(crate) last_tray_status: Option<crate::services::tray::TrayIconStatus>,
    /// Last frame's screen width for responsive breakpoint detection.
    last_frame_width: Option<f32>,
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
        if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render(self, ctx)))
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

    /// Handle system tray events: show/hide window and menu actions.
    fn handle_tray_events(&mut self, ctx: &egui::Context) {
        let Some(tray) = self.tray_manager.as_ref() else {
            return;
        };

        // Tray icon clicks (double-click → show)
        for event in tray.poll_tray_events() {
            use tray_icon::TrayIconEvent;
            match event {
                TrayIconEvent::DoubleClick { .. } | TrayIconEvent::Click { .. } => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                }
                _ => {}
            }
        }

        // Context menu actions
        for action in tray.poll_menu_events() {
            use crate::services::tray::TrayAction;
            match action {
                TrayAction::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                }
                TrayAction::CopySessionLink => {
                    if let Some(session) = self.session_store.active_session() {
                        let link = format!("clarity://session/{}", session.id);
                        ctx.copy_text(link);
                        self.push_toast("Session link copied".to_string(), ToastLevel::Info);
                    }
                }
                TrayAction::Pause => {
                    self.stop();
                    self.push_toast("Agent paused".to_string(), ToastLevel::Info);
                }
                TrayAction::Settings => {
                    self.settings_store.settings_open = true;
                }
                TrayAction::Quit => {
                    self.tray_quit_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn render_titlebar(&mut self, ctx: &egui::Context) {
        let theme = self.ui_store.theme.clone();

        egui::TopBottomPanel::top("titlebar")
            .min_height(TITLEBAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(theme.bg)
                    .stroke(egui::Stroke::NONE)
                    .inner_margin(egui::Margin::symmetric(8, 0)),
            )
            .show(ctx, |ui| {
                ui.set_min_height(TITLEBAR_HEIGHT);

                // Single horizontal row:
                // [toggle?] [title] [sessions] [tabs] [elastic drag] [status] [settings] [buttons]
                ui.horizontal_centered(|ui| {
                    ui.set_min_height(TITLEBAR_HEIGHT);

                    // Sidebar toggle when collapsed
                    if self.ui_store.sidebar_collapsed {
                        if crate::widgets::icon_button_toolbar(
                            ui,
                            crate::theme::ICON_LIST,
                            theme.text_base,
                            &theme,
                        )
                        .clicked()
                        {
                            self.ui_store.sidebar_collapsed = false;
                        }
                        ui.add_space(8.0);
                    }

                    // Brand
                    ui.label(
                        egui::RichText::new("Clarify")
                            .size(theme.text_base)
                            .color(theme.text_strong),
                    );
                    ui.add_space(8.0);

                    // Session tabs moved from chat header to titlebar
                    crate::panels::chat::header::render_session_tabs(self, ui);

                    // Elastic filler — drag to move window; double-click toggles maximize.
                    let drag_w = ui.available_size().x.max(40.0);
                    let (_drag_id, drag_resp) = ui.allocate_exact_size(
                        egui::vec2(drag_w, TITLEBAR_HEIGHT),
                        egui::Sense::click_and_drag(),
                    );
                    if drag_resp.drag_started_by(egui::PointerButton::Primary) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if drag_resp.double_clicked() {
                        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    }

                    // Right section: status indicators + settings + window controls.
                    // Rendered right-to-left so interactive elements have higher z-order
                    // than the drag region — clicks are not swallowed.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Close
                        let close_resp = crate::widgets::window_control_button(
                            ui,
                            crate::theme::ICON_X,
                            &theme,
                            theme.danger.linear_multiply(0.25),
                            egui::Color32::WHITE,
                            theme.text_dim,
                        );
                        if close_resp.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        }

                        // Maximize / Restore
                        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        let max_icon = if is_maximized {
                            crate::theme::ICON_COPY
                        } else {
                            crate::theme::ICON_SQUARE
                        };
                        let max_resp = crate::widgets::window_control_button(
                            ui,
                            max_icon,
                            &theme,
                            theme.overlay_medium,
                            theme.text,
                            theme.text_dim,
                        );
                        if max_resp.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                        }

                        // Minimize
                        let min_resp = crate::widgets::window_control_button(
                            ui,
                            crate::theme::ICON_MINUS,
                            &theme,
                            theme.overlay_medium,
                            theme.text,
                            theme.text_dim,
                        );
                        if min_resp.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        }

                        // Separator between system buttons and indicators
                        ui.add_space(8.0);

                        // Settings button
                        let settings_resp = crate::widgets::window_control_button(
                            ui,
                            crate::theme::ICON_SETTINGS,
                            &theme,
                            theme.overlay_medium,
                            theme.text,
                            theme.text_dim,
                        );
                        if settings_resp.clicked() {
                            self.settings_store.settings_open = true;
                        }

                        // Connection status capsule
                        let (conn_label, conn_color) = match self.chat_store.agent_status {
                            AgentStatus::Online => ("Online", theme.status_online),
                            AgentStatus::Busy => ("Busy", theme.status_busy),
                            AgentStatus::Offline | AgentStatus::Unconfigured => {
                                ("断开", theme.status_offline)
                            }
                        };
                        let conn_resp = crate::widgets::status_capsule(
                            ui,
                            conn_color,
                            conn_label,
                            conn_color,
                            false,
                            &theme,
                        );
                        if conn_resp.hovered() {
                            let _ = conn_resp.on_hover_text("Agent connection status");
                        }
                        ui.add_space(4.0);

                        // Gateway capsule (clickable)
                        let gw_dot_color = match self.chat_store.gateway_status {
                            crate::ui::types::GatewayStatus::Online => theme.status_online,
                            crate::ui::types::GatewayStatus::Offline => theme.status_offline,
                            crate::ui::types::GatewayStatus::Checking => theme.status_busy,
                        };
                        let gw_resp = crate::widgets::status_capsule(
                            ui,
                            gw_dot_color,
                            "Gateway",
                            theme.text_muted,
                            true,
                            &theme,
                        );
                        let gw_resp = if gw_resp.hovered() {
                            gw_resp.on_hover_text("Click to start/stop Gateway")
                        } else {
                            gw_resp
                        };
                        if gw_resp.clicked() {
                            match self.chat_store.gateway_status {
                                crate::ui::types::GatewayStatus::Online => {
                                    if let Some(ref gm) = self.gateway_manager {
                                        match gm.stop() {
                                            Ok(_) => self.push_toast(
                                                "Gateway stopping...".to_string(),
                                                crate::ui::types::ToastLevel::Info,
                                            ),
                                            Err(e) => self.push_toast(
                                                format!("Gateway stop failed: {}", e),
                                                crate::ui::types::ToastLevel::Error,
                                            ),
                                        }
                                    } else {
                                        self.push_toast(
                                            "Gateway manager not available".to_string(),
                                            crate::ui::types::ToastLevel::Warn,
                                        );
                                    }
                                }
                                _ => {
                                    if let Some(ref gm) = self.gateway_manager {
                                        match gm.start_if_needed() {
                                            Ok(_) => self.push_toast(
                                                "Gateway starting...".to_string(),
                                                crate::ui::types::ToastLevel::Info,
                                            ),
                                            Err(e) => self.push_toast(
                                                format!("Gateway start failed: {}", e),
                                                crate::ui::types::ToastLevel::Error,
                                            ),
                                        }
                                    } else {
                                        self.push_toast(
                                            "Gateway manager not available".to_string(),
                                            crate::ui::types::ToastLevel::Warn,
                                        );
                                    }
                                }
                            }
                        }
                    });
                });
            });
    }

    fn handle_window_resize(&mut self, ctx: &egui::Context) {
        let screen_rect = ctx.screen_rect();
        let edge = 10.0;

        // Skip resize when maximized; it may not work properly and conflicts with restore logic.
        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if is_maximized {
            return;
        }

        if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
            // Do not trigger edge resize inside the titlebar area; it conflicts with drag-to-move.
            if pos.y < screen_rect.min.y + TITLEBAR_HEIGHT + edge {
                return;
            }

            let on_left = pos.x < screen_rect.min.x + edge;
            let on_right = pos.x > screen_rect.max.x - edge;
            let on_top = pos.y < screen_rect.min.y + edge;
            let on_bottom = pos.y > screen_rect.max.y - edge;

            let (direction, cursor) = if on_top && on_left {
                (
                    Some(egui::ResizeDirection::NorthWest),
                    egui::CursorIcon::ResizeNorthWest,
                )
            } else if on_top && on_right {
                (
                    Some(egui::ResizeDirection::NorthEast),
                    egui::CursorIcon::ResizeNorthEast,
                )
            } else if on_bottom && on_left {
                (
                    Some(egui::ResizeDirection::SouthWest),
                    egui::CursorIcon::ResizeSouthWest,
                )
            } else if on_bottom && on_right {
                (
                    Some(egui::ResizeDirection::SouthEast),
                    egui::CursorIcon::ResizeSouthEast,
                )
            } else if on_left {
                (
                    Some(egui::ResizeDirection::West),
                    egui::CursorIcon::ResizeHorizontal,
                )
            } else if on_right {
                (
                    Some(egui::ResizeDirection::East),
                    egui::CursorIcon::ResizeHorizontal,
                )
            } else if on_top {
                (
                    Some(egui::ResizeDirection::North),
                    egui::CursorIcon::ResizeVertical,
                )
            } else if on_bottom {
                (
                    Some(egui::ResizeDirection::South),
                    egui::CursorIcon::ResizeVertical,
                )
            } else {
                (None, egui::CursorIcon::Default)
            };

            if let Some(dir) = direction {
                ctx.set_cursor_icon(cursor);
                if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                }
            }
        }
    }

    fn render_settings_panel(&mut self, ctx: &egui::Context) {
        components::settings::render_settings_panel(self, ctx);
    }

    fn render_chat_area(&mut self, ctx: &egui::Context) {
        panels::chat::render_chat_area(self, ctx);
    }

    fn render_input_panel(&mut self, ctx: &egui::Context) {
        panels::chat::render_input_panel(self, ctx);
    }

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        panels::sidebar::render_sidebar(self, ctx);
    }

    fn render_workspace_panel(&mut self, ctx: &egui::Context) {
        panels::workspace::render_workspace_panel(self, ctx);
    }

    fn render_mcp_panel(&mut self, ctx: &egui::Context) {
        panels::mcp::render_mcp_panel(self, ctx);
    }

    fn render_task_create_modal(&mut self, ctx: &egui::Context) {
        panels::task_create::render_task_create_modal(self, ctx);
    }

    fn render_task_view_modal(&mut self, ctx: &egui::Context) {
        panels::task_view::render_task_view_modal(self, ctx);
    }

    fn render_subagent_view_modal(&mut self, ctx: &egui::Context) {
        panels::subagent_view::render_subagent_view_modal(self, ctx);
    }

    fn render_team_panel(&mut self, ctx: &egui::Context) {
        panels::team::render_team_panel(self, ctx);
    }

    fn render_team_create_modal(&mut self, ctx: &egui::Context) {
        panels::team_create::render_team_create_modal(self, ctx);
    }

    fn render_dashboard_panel(&mut self, ctx: &egui::Context) {
        panels::dashboard::render_dashboard_panel(self, ctx);
    }

    fn render_gantt_panel(&mut self, ctx: &egui::Context) {
        panels::gantt::render_gantt_panel(self, ctx);
    }

    fn render_cron_create_modal(&mut self, ctx: &egui::Context) {
        panels::cron_create::render_cron_create_modal(self, ctx);
    }

    fn render_skill_panel(&mut self, ctx: &egui::Context) {
        panels::skill::render_skill_panel(self, ctx);
    }

    fn render_toasts(&mut self, ctx: &egui::Context) {
        panels::toast::render_toasts(self, ctx);
    }

    fn render_approval_modal(&mut self, ctx: &egui::Context) {
        panels::approval::render_approval_modal(self, ctx);
    }

    fn render_snapshot_modal(&mut self, ctx: &egui::Context) {
        panels::snapshot::render_snapshot_modal(self, ctx);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Intercept system close (Alt+F4 / taskbar close) → hide to tray ──
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.tray_quit_requested {
                // Allow the close to proceed (Quit from tray menu).
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        // ── Poll system tray events ──
        self.handle_tray_events(ctx);

        let now = ctx.input(|i| i.time);
        self.ui_store.frame_count += 1;
        if now - self.ui_store.last_fps_time >= 1.0 {
            self.ui_store.fps =
                self.ui_store.frame_count as f64 / (now - self.ui_store.last_fps_time);
            self.ui_store.frame_count = 0;
            self.ui_store.last_fps_time = now;
        }

        self.process_events();

        // Poll MCP config for external changes (hot-reload).
        self.check_mcp_config_reload();

        // Drain batch-grant auto-approval notifications and show toasts.
        for msg in self
            .state
            .mode_aware_approval_runtime
            .drain_auto_approval_notifications()
        {
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

        // ── Global keyboard shortcuts ──
        for action in shortcuts::collect_actions(ctx, self) {
            match action {
                shortcuts::ShortcutAction::CloseModal => {
                    if self.team_store.create_modal_open {
                        self.team_store.create_modal_open = false;
                    } else if self.settings_store.settings_open {
                        self.settings_store.settings_open = false;
                    } else if self.ui_store.skill_panel_open {
                        self.ui_store.skill_panel_open = false;
                    } else if self.team_store.team_panel_open {
                        self.team_store.team_panel_open = false;
                    }
                    if self.cron_store.create_modal_open {
                        self.cron_store.create_modal_open = false;
                    }
                    if self.snapshot_store.modal_open {
                        self.snapshot_store.modal_open = false;
                    }
                    if self.task_store.task_create_modal_open {
                        self.task_store.task_create_modal_open = false;
                    }
                }
                shortcuts::ShortcutAction::NewSession => {
                    if !self.chat_store.is_loading {
                        self.new_session();
                    }
                }
                shortcuts::ShortcutAction::StopGeneration => {
                    self.stop();
                }
                shortcuts::ShortcutAction::SendMessage => {
                    if !self.chat_store.input.trim().is_empty() && !self.chat_store.is_loading {
                        self.chat_store.stick_to_bottom = true;
                        self.send();
                    }
                }
                shortcuts::ShortcutAction::ToggleSkillPanel => {
                    self.ui_store.skill_panel_open = !self.ui_store.skill_panel_open;
                }
                shortcuts::ShortcutAction::ToggleTeamPanel => {
                    self.team_store.team_panel_open = !self.team_store.team_panel_open;
                }
                shortcuts::ShortcutAction::FocusInput => {
                    self.ui_store.focus_input_requested = true;
                }
                shortcuts::ShortcutAction::ToggleCommandPalette => {
                    // Placeholder: command palette skeleton
                    self.push_toast(
                        "Command Palette (Ctrl+Shift+P) — coming in v0.3.2".to_string(),
                        crate::ui::types::ToastLevel::Info,
                    );
                }
                shortcuts::ShortcutAction::ToggleDashboardPanel => {
                    self.ui_store.dashboard_panel_open = !self.ui_store.dashboard_panel_open;
                }
            }
        }

        // Refresh task list periodically when panel is open
        if self.task_store.task_panel_open
            && self.task_store.last_task_refresh.elapsed() > Duration::from_secs(3)
        {
            self.refresh_tasks();
        }

        // Poll parallel batch status when panel is open
        if self.task_store.task_panel_open
            && !self.subagent_store.parallel_batches.is_empty()
            && self.subagent_store.last_parallel_poll.elapsed() > Duration::from_secs(2)
        {
            self.poll_parallel_batches();
        }

        // Poll Gateway health every 5 seconds
        if self.subagent_store.last_gateway_health_poll.elapsed() > Duration::from_secs(5) {
            self.subagent_store.last_gateway_health_poll = Instant::now();
            self.poll_gateway_health();
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

        // ── Sync tray icon colour with runtime state ──
        if let Some(ref mut tray) = self.tray_manager {
            let new_status = if !self.ui_store.pending_approvals.is_empty() {
                crate::services::tray::TrayIconStatus::Message
            } else {
                match self.chat_store.agent_status {
                    AgentStatus::Online | AgentStatus::Unconfigured => {
                        crate::services::tray::TrayIconStatus::Idle
                    }
                    AgentStatus::Busy => crate::services::tray::TrayIconStatus::Active,
                    AgentStatus::Offline => crate::services::tray::TrayIconStatus::Error,
                }
            };
            if self.last_tray_status != Some(new_status) {
                tray.set_status(new_status);
                self.last_tray_status = Some(new_status);
            }
        }

        // ── Responsive breakpoints: auto-collapse panels when window is too narrow ──
        let current_width = ctx.screen_rect().width();
        if let Some(last_width) = self.last_frame_width {
            // One-way collapse: only trigger when window becomes narrower.
            // Do NOT auto-restore on widen to avoid fighting user intent.
            if last_width >= 1100.0 && current_width < 1100.0 {
                self.ui_store.dashboard_panel_open = false;
                self.team_store.team_panel_open = false;
                self.task_store.task_panel_open = false;
            }
            if last_width >= 768.0 && current_width < 768.0 {
                self.ui_store.sidebar_collapsed = true;
            }
        }
        self.last_frame_width = Some(current_width);

        // ── Content-area guard: ensure chat area never drops below 480px ──
        // This catches cases where user manually resized side panels narrower
        // than the window-width breakpoints above.
        let sidebar_w = if self.ui_store.sidebar_collapsed {
            36.0
        } else {
            220.0
        };
        let workspace_w = 280.0; // always present
        let dashboard_w = if self.ui_store.dashboard_panel_open {
            240.0
        } else {
            0.0
        };
        let team_w = if self.team_store.team_panel_open {
            240.0
        } else {
            0.0
        };
        let task_w = if self.task_store.task_panel_open {
            240.0
        } else {
            0.0
        };
        let content_w = current_width - sidebar_w - workspace_w - dashboard_w - team_w - task_w;
        const CONTENT_MIN: f32 = 480.0;
        if content_w < CONTENT_MIN {
            // Priority: dashboard → team → task (sidebar handled by 768px breakpoint)
            if self.ui_store.dashboard_panel_open {
                self.ui_store.dashboard_panel_open = false;
            } else if self.team_store.team_panel_open {
                self.team_store.team_panel_open = false;
            } else if self.task_store.task_panel_open {
                self.task_store.task_panel_open = false;
            }
        }

        ctx.style_mut(|style| {
            self.ui_store.theme.apply(style);
        });

        self.render_safe(ctx, "titlebar", |app, ctx| app.render_titlebar(ctx));
        self.render_safe(ctx, "sidebar", |app, ctx| app.render_sidebar(ctx));
        self.render_safe(ctx, "workspace", |app, ctx| app.render_workspace_panel(ctx));
        self.render_safe(ctx, "input", |app, ctx| app.render_input_panel(ctx));
        self.render_safe(ctx, "chat", |app, ctx| app.render_chat_area(ctx));
        self.render_safe(ctx, "settings", |app, ctx| app.render_settings_panel(ctx));
        self.render_safe(ctx, "skill", |app, ctx| app.render_skill_panel(ctx));
        self.render_safe(ctx, "mcp", |app, ctx| app.render_mcp_panel(ctx));
        self.render_safe(ctx, "toast", |app, ctx| app.render_toasts(ctx));
        self.render_safe(ctx, "cron_create", |app, ctx| {
            app.render_cron_create_modal(ctx)
        });
        self.render_safe(ctx, "approval", |app, ctx| app.render_approval_modal(ctx));
        self.render_safe(ctx, "snapshot", |app, ctx| app.render_snapshot_modal(ctx));
        self.render_safe(ctx, "task_create", |app, ctx| {
            app.render_task_create_modal(ctx)
        });
        self.render_safe(ctx, "task_view", |app, ctx| app.render_task_view_modal(ctx));
        self.render_safe(ctx, "subagent_view", |app, ctx| {
            app.render_subagent_view_modal(ctx)
        });
        self.render_safe(ctx, "team", |app, ctx| app.render_team_panel(ctx));
        self.render_safe(ctx, "team_create", |app, ctx| {
            app.render_team_create_modal(ctx)
        });
        self.render_safe(ctx, "dashboard", |app, ctx| app.render_dashboard_panel(ctx));
        self.render_safe(ctx, "gantt", |app, ctx| app.render_gantt_panel(ctx));
        self.render_safe(ctx, "kimi_login", |app, ctx| {
            crate::components::login_modal::render_oauth_login_modal(
                app,
                ctx,
                &clarity_llm::auth::OAuthDeviceFlowConfig::default(),
            );
        });
        self.render_safe(ctx, "onboarding", |app, ctx| {
            onboarding::render_onboarding(app, ctx);
        });
        self.render_safe(ctx, "resize", |app, ctx| {
            app.handle_window_resize(ctx);
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_current_session();
    }
}

fn main() -> eframe::Result {
    clarity_core::logging::init();
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

    // ------------------------------------------------------------------
    // Auto-start Gateway if not already running
    // ------------------------------------------------------------------
    let gateway_manager = crate::services::gateway_manager::GatewayManager::new();
    let gateway_manager = match gateway_manager.start_if_needed() {
        Ok(true) => {
            tracing::info!("Auto-started Gateway");
            Some(gateway_manager)
        }
        Ok(false) => {
            tracing::info!("Gateway already running — no action needed");
            None
        }
        Err(e) => {
            tracing::warn!("Could not auto-start Gateway: {}", e);
            None
        }
    };

    eframe::run_native(
        "Clarity",
        options,
        Box::new(move |cc| {
            #[cfg(windows)]
            let _ = platform::windows::apply_rounded_corners(cc);
            let tray_manager = crate::services::tray::TrayManager::new();
            if tray_manager.is_none() {
                tracing::warn!("Failed to initialize system tray icon");
            }
            Ok(Box::new(App::new(cc, gateway_manager, tray_manager)))
        }),
    )
}
