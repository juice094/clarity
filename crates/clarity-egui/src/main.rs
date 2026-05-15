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
use egui_extras::{Size, StripBuilder};
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

// (layout constants moved to Theme tokens)

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
    /// Pretext UI command palette (Ctrl+Shift+P).
    pub(crate) command_palette: crate::widgets::command_palette::CommandPalette,
    /// Pretext UI unified view state (replaces boolean flag hell).
    pub(crate) view_state: clarity_core::ui::ViewState,
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
            .exact_height(theme.size_titlebar)
            .frame(
                egui::Frame::new()
                    .fill(theme.bg)
                    .stroke(egui::Stroke::NONE)
                    .inner_margin(egui::Margin::symmetric(8, 0)),
            )
            .show(ctx, |ui| {
                let titlebar_rect = ui.max_rect();

                // Fix 3 (官方范式): 整个 titlebar 作为拖拽热区，先注册后覆盖。
                // 按钮在后续代码中渲染，其交互会自动覆盖同一 rect 上的拖拽。
                let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                let drag_resp = ui.interact(
                    titlebar_rect,
                    ui.id().with("titlebar_drag"),
                    egui::Sense::click_and_drag(),
                );
                if drag_resp.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag_resp.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                // ── TitleBar three-zone layout via egui_extras::StripBuilder ──
                //   LEFT  : sidebar toggle (when collapsed) + brand
                //   CENTER: session tabs + model indicator (drag handled globally above)
                //   RIGHT : window controls + status capsules
                let show_status_labels = ctx.screen_rect().width() >= theme.breakpoint_compact;
                let right_w = self.ui_store.titlebar_right_width.max(180.0);
                // 溢出保护：RIGHT zone 不得超过当前可用宽度的 45%，防止挤压 CENTER
                let max_right = ui.available_width() * 0.45;
                let right_w = right_w.min(max_right.max(180.0));
                // LEFT zone 动态化：sidebar 折叠时需要 toggle + brand，否则仅 brand
                let left_w = if self.ui_store.sidebar_collapsed {
                    theme.titlebar_left_w
                } else {
                    68.0
                };

                StripBuilder::new(ui)
                    .size(Size::exact(left_w))
                    .size(Size::remainder().at_least(40.0))
                    .size(Size::exact(right_w))
                    .horizontal(|mut strip| {
                        // ── LEFT zone: sidebar toggle + brand ──
                        strip.cell(|ui| {
                            ui.set_min_height(theme.size_titlebar);
                            ui.horizontal_centered(|ui| {
                                if self.ui_store.sidebar_collapsed {
                                    if crate::widgets::icon_button_toolbar(
                                        ui,
                                        crate::theme::ICON_LIST,
                                        theme.text_base,
                                        &theme,
                                    )
                                    .on_hover_text("Expand sidebar")
                                    .clicked()
                                    {
                                        self.ui_store.sidebar_collapsed = false;
                                    }
                                    ui.add_space(8.0);
                                }
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new("Clarity")
                                            .size(theme.text_base)
                                            .color(theme.text_strong),
                                    )
                                    .sense(egui::Sense::empty()),
                                );
                            });
                        });

                        // ── CENTER zone: tabs + model ──
                        // Drag 已由 titlebar 全局 interact 处理，不再需要在 CENTER
                        // 内部放置 drag filler，避免与 RIGHT zone 按钮产生 hitbox 重叠。
                        strip.cell(|ui| {
                            ui.set_min_height(theme.size_titlebar);
                            ui.horizontal_centered(|ui| {
                                // S8 P3B.1: Persona switcher pill (Top Bar per ADR-014).
                                self.render_persona_switcher(ui, &theme);
                                ui.add_space(theme.space_8);

                                crate::panels::chat::header::render_session_tabs(self, ui);

                                // Model context indicator (Pretext UI mid-zone)
                                // 仅当剩余空间充足时才渲染，防止与 RIGHT zone 按钮溢出重叠。
                                let model_name =
                                    self.settings_store.settings_edit.model.trim();
                                if !model_name.is_empty() {
                                    let remaining = ui.available_width();
                                    if remaining >= 60.0 {
                                        ui.add_space(8.0);
                                        let label_w = remaining.min(120.0);
                                        ui.add_sized(
                                            egui::vec2(label_w, theme.size_titlebar),
                                            egui::Label::new(
                                                egui::RichText::new(model_name)
                                                    .size(theme.text_xs)
                                                    .color(theme.text_muted),
                                            )
                                            .truncate()
                                            .sense(egui::Sense::empty()),
                                        );
                                    }
                                }
                            });
                        });

                        // ── RIGHT zone: window controls + status capsules ──
                        strip.cell(|ui| {
                            ui.set_min_height(theme.size_titlebar);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    self.render_titlebar_right(
                                        ui,
                                        ctx,
                                        show_status_labels,
                                        is_maximized,
                                        &theme,
                                    );
                                },
                            );
                        });
                    });
            });
    }

    /// S8 P3B.1: Render the persona switcher pill in the titlebar's CENTER
    /// zone (left edge, before session tabs). Mutates `ui_store.active_persona_id`
    /// on selection and persists to `settings_edit.active_persona_id`.
    fn render_persona_switcher(&mut self, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
        let resp = crate::widgets::persona_switcher(
            ui,
            theme,
            &self.ui_store.endpoint_registry,
            &self.ui_store.active_persona_id,
            self.ui_store.persona_switcher_open,
        );
        if resp.toggle_clicked {
            self.ui_store.persona_switcher_open = !self.ui_store.persona_switcher_open;
        }
        if let Some(new_id) = resp.selected {
            if new_id != self.ui_store.active_persona_id {
                self.ui_store.active_persona_id = new_id.clone();
                self.settings_store.settings_edit.active_persona_id = Some(new_id.clone());
                if let Err(e) = self.settings_store.settings_edit.save() {
                    tracing::warn!("failed to persist active_persona_id: {}", e);
                }
                let descriptor_name = self
                    .ui_store
                    .endpoint_registry
                    .get(&new_id)
                    .map(|d| d.display_name.as_str().to_string())
                    .unwrap_or_else(|| new_id.clone());
                self.push_toast(
                    format!("Switched persona: {}", descriptor_name),
                    crate::ui::types::ToastLevel::Info,
                );
            }
        }
        if resp.close_requested {
            self.ui_store.persona_switcher_open = false;
        }
    }

    /// Render the right section of the titlebar (window controls + status capsules).
    /// Returns the actual width consumed in right-to-left layout.
    fn render_titlebar_right(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        show_status_labels: bool,
        is_maximized: bool,
        theme: &crate::theme::Theme,
    ) -> f32 {
        // In RTL, cursor.max.x shrinks leftward as widgets are placed.
        // Width = start_max_x - end_max_x.
        let start_max_x = ui.cursor().max.x;
        let available_w = ui.available_width();
        // 严格优先级：空间不足时从低优先级开始隐藏，确保 close/max/min/settings 始终可见
        let show_labels = show_status_labels && available_w >= 240.0;
        let show_capsules = available_w >= 160.0;
        let show_settings = available_w >= 120.0;
                    ui.spacing_mut().item_spacing.x = 0.0;
                    // Close (P0 — never hide)
                    let close_resp = crate::widgets::window_control_button(
                        ui,
                        crate::theme::ICON_X,
                        theme,
                        theme.danger.linear_multiply(0.25),
                        egui::Color32::WHITE,
                        theme.text,
                    )
                    .on_hover_text("Close window");
                    if close_resp.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    }

                    // Maximize / Restore (P0)
                    let max_icon = if is_maximized {
                        crate::theme::ICON_COPY
                    } else {
                        crate::theme::ICON_SQUARE
                    };
                    let max_resp = crate::widgets::window_control_button(
                        ui,
                        max_icon,
                        theme,
                        theme.overlay_medium,
                        theme.text,
                        theme.text,
                    )
                    .on_hover_text(if is_maximized { "Restore window" } else { "Maximize window" });
                    if max_resp.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    }

                    // Minimize (P0)
                    let min_resp = crate::widgets::window_control_button(
                        ui,
                        crate::theme::ICON_MINUS,
                        theme,
                        theme.overlay_medium,
                        theme.text,
                        theme.text,
                    )
                    .on_hover_text("Minimize to taskbar");
                    if min_resp.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }

                    if show_settings {
                        // Separator between system buttons and indicators
                        ui.add_space(8.0);

                        // Settings button (P1)
                        let settings_resp = crate::widgets::window_control_button(
                            ui,
                            crate::theme::ICON_SETTINGS,
                            theme,
                            theme.overlay_medium,
                            theme.text,
                            theme.text,
                        )
                        .on_hover_text("Open Settings (Esc to close)");
                        if settings_resp.clicked() {
                            self.view_state.main = clarity_core::ui::AppView::Settings;
                        }
                    }

                    if show_capsules {
                        // Connection status capsule (P2)
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
                            if show_labels { conn_label } else { "" },
                            conn_color,
                            false,
                            theme,
                        );
                        conn_resp.on_hover_text("Agent connection status");
                        ui.add_space(4.0);

                        // Gateway capsule (P2)
                        let gw_dot_color = match self.chat_store.gateway_status {
                            crate::ui::types::GatewayStatus::Online => theme.status_online,
                            crate::ui::types::GatewayStatus::Offline => theme.status_offline,
                            crate::ui::types::GatewayStatus::Checking => theme.status_busy,
                        };
                        let gw_resp = crate::widgets::status_capsule(
                            ui,
                            gw_dot_color,
                            if show_labels { "Gateway" } else { "" },
                            theme.text_muted,
                            true,
                            theme,
                        )
                        .on_hover_text("Click to start/stop Gateway");
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
                    }

        // RTL layout: cursor.max.x moved leftward; consumed width = start - end.
        let measured = start_max_x - ui.cursor().max.x;
        self.ui_store.titlebar_right_width = measured;
        measured
    }

    fn handle_window_resize(&mut self, ctx: &egui::Context) {
        let screen_rect = ctx.screen_rect();
        let edge = self.ui_store.theme.window_edge_zone;

        // Skip resize when maximized; it may not work properly and conflicts with restore logic.
        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if is_maximized {
            return;
        }

        if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
            // Do not trigger edge resize inside the titlebar area; it conflicts with drag-to-move.
            if pos.y < screen_rect.min.y + self.ui_store.theme.size_titlebar + edge {
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

    /// Unified command dispatcher (P0.5.C.1).
    ///
    /// Both the keyboard shortcut layer (`ShortcutAction::command_id()`) and
    /// the `CommandPalette` route through this method. Adding a new command
    /// means: (1) add a string constant in `clarity_core::ui::ids`,
    /// (2) match it here, (3) optionally bind a shortcut in `shortcuts/mod.rs`,
    /// (4) optionally surface in `built_in::all()`.
    ///
    /// Returns `true` when the command id is recognised.
    fn dispatch_command(&mut self, cmd_id: &str) -> bool {
        use clarity_core::ui::ids;
        match cmd_id {
            ids::CLOSE_MODAL => {
                if self.team_store.create_modal_open {
                    self.team_store.create_modal_open = false;
                } else if self.view_state.main != clarity_core::ui::AppView::Chat {
                    self.view_state.main = clarity_core::ui::AppView::Chat;
                } else if matches!(
                    self.view_state.modal,
                    Some(clarity_core::ui::ModalType::Skill)
                ) {
                    self.view_state.close_modal();
                } else if matches!(
                    self.view_state.right,
                    Some(clarity_core::ui::SidePanel::Team)
                        | Some(clarity_core::ui::SidePanel::Task)
                ) {
                    self.view_state.right = None;
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
                true
            }
            ids::NEW_SESSION => {
                if !self.chat_store.is_loading {
                    self.new_session();
                }
                true
            }
            ids::STOP_GENERATION => {
                self.stop();
                true
            }
            ids::SEND_MESSAGE => {
                if !self.chat_store.input.trim().is_empty() && !self.chat_store.is_loading {
                    self.chat_store.stick_to_bottom = true;
                    self.send();
                }
                true
            }
            ids::TOGGLE_SKILL_PANEL => {
                if matches!(
                    self.view_state.modal,
                    Some(clarity_core::ui::ModalType::Skill)
                ) {
                    self.view_state.close_modal();
                } else {
                    self.view_state.open_modal(clarity_core::ui::ModalType::Skill);
                }
                true
            }
            ids::TOGGLE_TEAM_PANEL => {
                self.view_state.toggle_right(clarity_core::ui::SidePanel::Team);
                true
            }
            ids::FOCUS_INPUT => {
                self.ui_store.focus_input_requested = true;
                true
            }
            ids::TOGGLE_COMMAND_PALETTE => {
                self.command_palette.open = true;
                self.command_palette.query.clear();
                self.command_palette.selected = 0;
                true
            }
            ids::TOGGLE_DASHBOARD => {
                self.view_state.main =
                    if self.view_state.main == clarity_core::ui::AppView::Dashboard {
                        clarity_core::ui::AppView::Chat
                    } else {
                        clarity_core::ui::AppView::Dashboard
                    };
                true
            }
            ids::TOGGLE_SIDEBAR => {
                self.ui_store.sidebar_collapsed = !self.ui_store.sidebar_collapsed;
                true
            }
            ids::OPEN_SETTINGS => {
                self.view_state.main = clarity_core::ui::AppView::Settings;
                true
            }
            ids::NAVIGATE_DOWN => {
                self.navigate_line(1);
                true
            }
            ids::NAVIGATE_UP => {
                self.navigate_line(-1);
                true
            }
            ids::NAVIGATE_TOP => {
                self.ui_store.line_cursor_selected = Some(0);
                true
            }
            ids::NAVIGATE_BOTTOM => {
                let total = self.ui_store.line_cursor_total_lines;
                if total > 0 {
                    self.ui_store.line_cursor_selected = Some(total.saturating_sub(1));
                }
                true
            }
            ids::COPY_LINE => {
                // Actual copy is handled in App::update() where egui::Context is available.
                true
            }
            other => {
                tracing::warn!("dispatch_command: unknown command id '{}'", other);
                false
            }
        }
    }

    /// S7 Phase 2D: navigate line cursor by `delta` lines (-1 = up, +1 = down).
    fn navigate_line(&mut self, delta: isize) {
        let total = self.ui_store.line_cursor_total_lines;
        if total == 0 {
            return;
        }
        let current = self.ui_store.line_cursor_selected.unwrap_or(0);
        let new_idx = if delta > 0 {
            (current + delta as usize).min(total.saturating_sub(1))
        } else {
            current.saturating_sub((-delta) as usize)
        };
        self.ui_store.line_cursor_selected = Some(new_idx);
    }

    /// S7 Phase 2D: return the text of the currently selected line (if any).
    fn selected_line_text(&self) -> Option<String> {
        let global_idx = self.ui_store.line_cursor_selected?;
        let active_id = self.session_store.active_session_id.clone();
        let session = self.session_store.sessions.iter().find(|s| s.id == active_id)?;
        let mut acc = 0;
        for msg in &session.messages {
            let msg_lines = msg.lines.len();
            if global_idx >= acc && global_idx < acc + msg_lines {
                let local_idx = global_idx - acc;
                return msg.lines.get(local_idx).map(render_line_text);
            }
            acc += msg_lines;
        }
        None
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

        // Refresh shell prompt (~1 Hz) to track cwd / git branch changes.
        if self.ui_store.frame_count % 60 == 0 {
            self.refresh_shell_prompt();
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

        // ── Global keyboard shortcuts (P0.5.C.1: unified dispatch) ──
        // All shortcut actions and CommandPalette entries route through
        // App::dispatch_command(&str) using ids from clarity_core::ui::ids.
        for action in shortcuts::collect_actions(ctx, self) {
            if action == shortcuts::ShortcutAction::CopyLine {
                if let Some(text) = self.selected_line_text() {
                    ctx.copy_text(text);
                    self.push_toast("Copied to clipboard", ToastLevel::Info);
                }
            } else {
                self.dispatch_command(action.command_id());
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
            if last_width >= self.ui_store.theme.breakpoint_medium && current_width < self.ui_store.theme.breakpoint_medium {
                // Dashboard is controlled by view_state.main (AppView), not right panel.
                // Team / Task right panels are collapsed via view_state.right.
                self.view_state.right = None;
            }
            if last_width >= self.ui_store.theme.breakpoint_compact && current_width < self.ui_store.theme.breakpoint_compact {
                self.ui_store.sidebar_collapsed = true;
            }
        }
        self.last_frame_width = Some(current_width);

        // ── Content-area guard: ensure chat area never drops below 480px ──
        // This catches cases where user manually resized side panels narrower
        // than the window-width breakpoints above.
        let sidebar_w = if self.ui_store.sidebar_collapsed {
            self.ui_store.theme.size_sidebar_collapsed
        } else {
            self.ui_store.theme.size_sidebar
        };
        let workspace_w = self.ui_store.theme.size_workspace; // always present
        let dashboard_w = if self.ui_store.dashboard_panel_open {
            self.ui_store.theme.size_panel_right
        } else {
            0.0
        };
        let team_w = if self.team_store.team_panel_open {
            self.ui_store.theme.size_panel_right
        } else {
            0.0
        };
        let task_w = if self.task_store.task_panel_open {
            self.ui_store.theme.size_panel_right
        } else {
            0.0
        };
        let content_w = current_width - sidebar_w - workspace_w - dashboard_w - team_w - task_w;
        if content_w < self.ui_store.theme.content_min_width {
            // Collapse Team or Task right panel if open (dashboard is AppView, not right panel).
            if matches!(
                self.view_state.right,
                Some(clarity_core::ui::SidePanel::Team)
                    | Some(clarity_core::ui::SidePanel::Task)
            ) {
                self.view_state.right = None;
            }
        }

        ctx.style_mut(|style| {
            self.ui_store.theme.apply(style);
        });

        // Sync legacy boolean flags with ViewState (compatibility layer).
        // This ensures render_* methods that still check their private booleans
        // stay consistent with the unified view state machine.
        self.settings_store.settings_open =
            self.view_state.main == clarity_core::ui::AppView::Settings;
        self.ui_store.dashboard_panel_open =
            self.view_state.main == clarity_core::ui::AppView::Dashboard;
        self.ui_store.gantt_panel_open =
            self.view_state.main == clarity_core::ui::AppView::Gantt;

        // Forward-direction sync (P1.5.4 bridge reversal — ADR-014):
        // ViewState is the authoritative source for side-panel and modal state.
        // Legacy booleans are read-only mirrors used by render_* methods that
        // have not yet been migrated.  turn/expansions remain on reverse-sync
        // until their respective P1.5.x subtasks complete.
        self.team_store.team_panel_open = matches!(
            self.view_state.right,
            Some(clarity_core::ui::SidePanel::Team)
        );
        self.task_store.task_panel_open = matches!(
            self.view_state.right,
            Some(clarity_core::ui::SidePanel::Task)
        );
        self.ui_store.skill_panel_open = matches!(
            self.view_state.modal,
            Some(clarity_core::ui::ModalType::Skill)
        );
        self.mcp_store.mcp_panel_open = matches!(
            self.view_state.modal,
            Some(clarity_core::ui::ModalType::Mcp)
        );

        // Reverse-direction sync — remaining booleans not yet reversed.
        self.view_state.turn = clarity_core::ui::TurnState::from_legacy(
            self.chat_store.is_loading,
            self.chat_store.compacting,
            self.chat_store.stopping,
            self.snapshot_store.restoring,
        );
        self.view_state.expansions = clarity_core::ui::PanelExpansion::from_legacy_flags(
            self.cron_store.cron_expanded,
            self.ui_store.web_tabs_expanded,
            self.ui_store.thinking_log_expanded,
            self.ui_store.tools_expanded,
            self.ui_store.subagents_expanded,
            self.ui_store.workspace_plan_expanded,
            self.ui_store.workspace_plan_manually_collapsed,
        );

        // ── Base chrome (always rendered) ──
        self.render_safe(ctx, "titlebar", |app, ctx| app.render_titlebar(ctx));
        self.render_safe(ctx, "sidebar", |app, ctx| app.render_sidebar(ctx));
        self.render_safe(ctx, "workspace", |app, ctx| app.render_workspace_panel(ctx));
        self.render_safe(ctx, "input", |app, ctx| app.render_input_panel(ctx));

        // ── Main view (mutually exclusive) ──
        match self.view_state.main {
            clarity_core::ui::AppView::Chat => {
                self.render_safe(ctx, "chat", |app, ctx| app.render_chat_area(ctx));
            }
            clarity_core::ui::AppView::Settings => {
                self.render_safe(ctx, "settings", |app, ctx| app.render_settings_panel(ctx));
            }
            clarity_core::ui::AppView::Dashboard => {
                self.render_safe(ctx, "dashboard", |app, ctx| app.render_dashboard_panel(ctx));
            }
            clarity_core::ui::AppView::Gantt => {
                self.render_safe(ctx, "gantt", |app, ctx| app.render_gantt_panel(ctx));
            }
            clarity_core::ui::AppView::TaskBoard => {
                // TODO: task board main view
            }
        }

        // ── Overlay panels ──
        self.render_safe(ctx, "skill", |app, ctx| app.render_skill_panel(ctx));
        self.render_safe(ctx, "mcp", |app, ctx| app.render_mcp_panel(ctx));
        self.render_safe(ctx, "toast", |app, ctx| app.render_toasts(ctx));

        // ── Modals (top-most, blocking) ──
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

        // Command Palette (top-most layer)
        if self.command_palette.open {
            let commands = clarity_core::ui::commands::built_in::all();
            let theme = self.ui_store.theme.clone();
            // P0.5.C.2: palette returns the activated command id (if any),
            // which we forward to the unified dispatcher.
            if let Some(cmd_id) = self.command_palette.show(ctx, &theme, &commands) {
                self.dispatch_command(&cmd_id);
            }
        }
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
        viewport: {
            let theme_defaults = crate::theme::Theme::default();
            egui::ViewportBuilder::default()
                .with_inner_size([theme_defaults.window_default_w, theme_defaults.window_default_h])
                .with_min_inner_size([theme_defaults.window_min_w, theme_defaults.window_min_h])
                .with_decorations(false)
        },
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

/// S7 Phase 2D: extract plain text from a RenderLine for copy-to-clipboard.
fn render_line_text(line: &clarity_core::ui::RenderLine) -> String {
    use clarity_core::ui::RenderLine;
    match line {
        RenderLine::Text { spans, .. } => spans.iter().map(|s| s.text.as_str()).collect(),
        RenderLine::CodeLine { content, .. } => content.to_string(),
        RenderLine::ToolCallHeader { name, .. } => format!("🔧 {name}"),
        RenderLine::ToolCallArg { key, value } => format!("{key}: {value}"),
        RenderLine::Thinking { content, .. } => content.to_string(),
        RenderLine::ApprovalPrompt { options } => options
            .iter()
            .map(|o| format!("{:?}", o))
            .collect::<Vec<_>>()
            .join(" | "),
        RenderLine::StatusLine { content, .. } => content.to_string(),
        RenderLine::ArtifactRef { artifact_id, summary } => {
            format!("{artifact_id} — {summary}")
        }
        RenderLine::CrossInstanceRef {
            target_instance,
            message,
            ..
        } => {
            format!("@{target_instance} — {message}")
        }
        RenderLine::SlashCompletion { command, description } => {
            format!("/{command}  {description}")
        }
        RenderLine::StreamingCursor => "▌".to_string(),
        RenderLine::Divider => "───".to_string(),
        RenderLine::Empty => String::new(),
        RenderLine::BlockSlot {
            block_id,
            line_count,
        } => {
            format!("⤢ Block {block_id} ({line_count} lines)")
        }
    }
}
