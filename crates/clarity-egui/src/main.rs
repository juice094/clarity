#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        missing_docs,
        unsafe_code
    )
)]
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
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

mod app_state;
pub(crate) mod claw;
pub(crate) mod claw_client;
mod components;
mod design_system;
mod error;
mod handlers;
mod i18n;
mod layout;
mod llm_binder;
mod llm_loader;
mod llm_policy;
mod panels;
mod platform;
mod pretext;
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
mod window_manager;

use app_state::AppState;

use ui::types::*;

// ============================================================================
// Clarity egui Desktop — Phase A: Design System Foundation
// ============================================================================

// (layout constants moved to Theme tokens)

/// Holds app state.
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
    pub(crate) project_store: stores::ProjectStore,
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
    /// Pretext text measurement backend backed by egui fonts.
    pub(crate) pretext_metrics: crate::pretext::EguiFontMetrics,
    /// S6 navigation tree: work/chat context (GUI-local; not persisted to ViewState).
    pub(crate) nav_context: crate::settings::NavContext,
    /// Live Claw device list polled from Gateway (replaces hardcoded mock data).
    pub(crate) device_state: crate::claw::DeviceState,
    /// Active WebSocket connection to the selected Claw Gateway.
    pub(crate) claw_ws: Option<crate::claw_client::ClawClient>,
    /// Track which device the current WebSocket is connected to.
    pub(crate) claw_ws_device_id: String,
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
            let msg = format!("PANIC in panel '{}': {}", name, payload);
            tracing::error!("{}", msg);
            self.push_toast(format!("UI error in {} panel", name), ToastLevel::Error);
        }
    }

    /// Orchestrates the full layout shell.
    ///
    /// S6 (Pretext Phase A): the shell is now organised as a single-page
    /// three-column layout:
    ///   [left icon rail + expanded list] [main stage] [right utility rail]
    /// The titlebar, input bar, overlays and modals are rendered on top.
    fn render_layout_shell(&mut self, ctx: &egui::Context) {
        // Draw the unified background first so that titlebar/left-rail panels are
        // visually on top of it. This prevents the base painter from accidentally
        // overpainting the left sidebar content when panels and painter share the
        // same layer.
        self.render_safe(ctx, "main_frame", |app, ctx| {
            app.render_main_stage_border(ctx);
        });

        // ── Base chrome (always rendered) ──
        self.render_safe(ctx, "titlebar", |app, ctx| app.render_titlebar(ctx));
        self.render_safe(ctx, "left_rail", |app, ctx| app.render_left_rail(ctx));

        // Right rail is declared early so the bottom input bar and central
        // stage are sized within the remaining width and cannot overlap it.
        self.render_safe(ctx, "right_rail", |app, ctx| app.render_right_rail(ctx));

        // Input bar must be declared before the central/main stage so egui
        // reserves the correct bottom area.
        self.render_safe(ctx, "input", |app, ctx| app.render_input_panel(ctx));

        // ── Main stage (mutually exclusive) ──
        self.render_safe(ctx, "main_stage", |app, ctx| app.render_main_stage(ctx));

        // ── Overlay panels ──
        self.render_safe(ctx, "skill", |app, ctx| app.render_skill_panel(ctx));
        self.render_safe(ctx, "mcp", |app, ctx| app.render_mcp_panel(ctx));
        self.render_safe(ctx, "toast", |app, ctx| app.render_toasts(ctx));

        // ── Modals (top-most, blocking) ──
        // Dispatch exclusively through `view_state.modal` (P1.5 migration).
        if let Some(modal) = self.view_state.modal {
            use clarity_core::ui::ModalType;
            let name = match modal {
                ModalType::CronCreate => "cron_create",
                ModalType::Approval => "approval",
                ModalType::Snapshot => "snapshot",
                ModalType::TaskCreate => "task_create",
                ModalType::TaskView => "task_view",
                ModalType::SubAgentView => "subagent_view",
                ModalType::TeamCreate => "team_create",
                ModalType::KimiCodeLogin => "kimi_login",
                ModalType::ManageWebLinksChat => "manage_web_links_chat",
                ModalType::ManageWebLinksWork => "manage_web_links_work",
                ModalType::ManageWorkTemplates => "manage_work_templates",
                ModalType::Skill | ModalType::Mcp | ModalType::Login | ModalType::AddProvider => {
                    // Skill/Mcp are overlay panels; Login/AddProvider not yet wired.
                    ""
                }
            };
            if !name.is_empty() {
                self.render_safe(ctx, name, |app, ctx| match modal {
                    ModalType::CronCreate => app.render_cron_create_modal(ctx),
                    ModalType::Approval => app.render_approval_modal(ctx),
                    ModalType::Snapshot => app.render_snapshot_modal(ctx),
                    ModalType::TaskCreate => app.render_task_create_modal(ctx),
                    ModalType::TaskView => app.render_task_view_modal(ctx),
                    ModalType::SubAgentView => app.render_subagent_view_modal(ctx),
                    ModalType::TeamCreate => app.render_team_create_modal(ctx),
                    ModalType::KimiCodeLogin => {
                        crate::panels::modals::login::render_oauth_login_modal(
                            app,
                            ctx,
                            &clarity_llm::auth::OAuthDeviceFlowConfig::default(),
                        );
                    }
                    ModalType::ManageWebLinksChat => {
                        crate::panels::modals::manage_web_links::render_manage_web_links_modal(
                            app, ctx, true,
                        );
                    }
                    ModalType::ManageWebLinksWork => {
                        crate::panels::modals::manage_web_links::render_manage_web_links_modal(
                            app, ctx, false,
                        );
                    }
                    ModalType::ManageWorkTemplates => {
                        crate::panels::modals::manage_work_templates::render_manage_work_templates_modal(app, ctx);
                    }
                    _ => {}
                });
            }
        }

        self.render_safe(ctx, "onboarding", |app, ctx| {
            onboarding::render_onboarding(app, ctx);
        });
        self.render_safe(ctx, "resize", |app, ctx| {
            app.handle_window_resize(ctx);
        });
    }

    /// Returns true when an agent turn is actively loading/generating.
    pub(crate) fn is_loading(&self) -> bool {
        matches!(self.view_state.turn, clarity_core::ui::TurnState::Loading)
    }

    /// Render the left navigation tree.
    ///
    /// S6 Phase D: the left chrome is now a single fixed-width tree. The old
    /// 36px icon rail and the conditional expanded panel have been replaced by
    /// `panels::navigation_tree`.
    fn render_left_rail(&mut self, ctx: &egui::Context) {
        if self.view_state.left_rail_expanded {
            crate::panels::navigation_tree::render_left_navigation_tree(self, ctx);
        }
    }

    /// Render the unified outer border around the chat stage + right rail + input bar.
    ///
    /// The individual panels already fill themselves with `theme.bg`; this helper
    /// only draws the outer rounded stroke so there is no mismatch/black gap
    /// between a painter fill and the panel fills. The border is inset a few
    /// pixels from the window edges and from the left sidebar for a cleaner,
    /// Kimi-style floating-stage look.
    fn render_main_stage_border(&self, ctx: &egui::Context) {
        let theme = self.ui_store.theme.clone();
        let left_w = if self.view_state.left_rail_expanded {
            theme.size_sidebar
        } else {
            0.0
        };
        let right_w = if self.view_state.right_rail_visible {
            self.ui_store
                .right_rail_width
                .unwrap_or(theme.size_panel_right)
        } else {
            0.0
        };

        let screen = ctx.screen_rect();
        let titlebar_h = theme.size_titlebar;
        let inset = theme.space_4;

        // Layer 1 — base: fills the entire area below the titlebar all the way
        // to the window edges, including the left-rail region. This makes
        // titlebar and left rail share one continuous background.
        let base_rect = egui::Rect::from_min_max(
            egui::pos2(0.0, titlebar_h),
            egui::pos2(screen.max.x, screen.max.y),
        );
        if base_rect.width() <= 0.0 || base_rect.height() <= 0.0 {
            return;
        }

        // Layer 2 — inner surface: inset from the base, with rounded corners and border.
        let surface_rect = egui::Rect::from_min_max(
            egui::pos2(left_w + inset, titlebar_h + inset),
            egui::pos2(screen.max.x - inset, screen.max.y - inset),
        );

        let bg_painter = ctx.layer_painter(egui::LayerId::background());
        let radius = theme.radius_sm as u8;
        let corner_radius = egui::CornerRadius::same(radius);

        bg_painter.rect_filled(base_rect, egui::CornerRadius::ZERO, theme.bg);
        bg_painter.rect_filled(surface_rect, corner_radius, theme.bg);

        // Right-rail surface: paint the rail with the same rounded corners as
        // the main stage so its right edge seamlessly meets the outer border.
        // The divider line is drawn by the panel after its contents so the
        // native resize hover/drag line can be hidden.
        if right_w > 0.0 {
            let rail_w = right_w.min(surface_rect.width());
            let rail_rect = egui::Rect::from_min_max(
                egui::pos2(surface_rect.max.x - rail_w, surface_rect.min.y),
                surface_rect.max,
            );
            bg_painter.rect_filled(rail_rect, corner_radius, theme.bg);
        }

        // Paint the border stroke on the tooltip layer so it sits above the
        // right-rail cover that hides the native resize hover/drag line.
        let border_painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("main_stage_border"),
        ));
        border_painter.rect_stroke(
            surface_rect,
            corner_radius,
            egui::Stroke::new(1.0_f32, theme.border),
            egui::StrokeKind::Inside,
        );
    }

    /// Render the main stage (mutually exclusive central view).
    fn render_main_stage(&mut self, ctx: &egui::Context) {
        // Views that do not render their own CentralPanel would otherwise expose
        // the raw window background (black on decorated-less Windows windows).
        // Fill the stage with `theme.bg` first; Chat/TaskBoard/Work already do
        // this themselves, so skip them to avoid double CentralPanel.
        let self_renders_central = matches!(
            self.view_state.main,
            clarity_core::ui::AppView::Chat
                | clarity_core::ui::AppView::TaskBoard
                | clarity_core::ui::AppView::Work
        );
        if !self_renders_central {
            // The unified background painter already fills the main stage; only
            // guarantee a transparent central panel exists for child widgets.
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE)
                        .inner_margin(egui::Margin::ZERO)
                        .outer_margin(egui::Margin::ZERO),
                )
                .show(ctx, |_ui| {});
        }

        match self.view_state.main {
            clarity_core::ui::AppView::Chat => self.render_chat_area(ctx),
            clarity_core::ui::AppView::Settings => self.render_settings_panel(ctx),
            clarity_core::ui::AppView::Dashboard => self.render_dashboard_panel(ctx),
            clarity_core::ui::AppView::Gantt => self.render_gantt_panel(ctx),
            clarity_core::ui::AppView::TaskBoard => self.render_task_board(ctx),
            clarity_core::ui::AppView::Work => self.render_work_panel(ctx),
        }
    }

    /// Render the IDE-style right rail panel.
    ///
    /// S6 Phase D: the right rail is now a single functional panel selected by
    /// the Bot bar. The old stacked-card content lives in `panels::right_rail`
    /// and will be migrated into the new IDE panels over the next iterations.
    fn render_right_rail(&mut self, ctx: &egui::Context) {
        if !self.view_state.right_rail_visible {
            return;
        }
        crate::panels::right_ide_panel::render_right_ide_panel(self, ctx);
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
                    self.view_state.main = clarity_core::ui::AppView::Settings;
                }
                TrayAction::Quit => {
                    self.tray_quit_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    /// Render the minimal custom titlebar.
    ///
    /// S6 Phase D: the titlebar is stripped down to a single sidebar toggle on
    /// the left and the window control buttons on the right. The previous
    /// brand, session tabs, persona switcher, model indicator, and status
    /// capsules have been removed from the chrome; they will resurface in the
    /// Bot bar, the right rail, or the bottom composer.
    fn render_titlebar(&mut self, ctx: &egui::Context) {
        let theme = self.ui_store.theme.clone();

        egui::TopBottomPanel::top("titlebar")
            .exact_height(theme.size_titlebar)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme.bg)
                    .stroke(egui::Stroke::NONE)
                    .inner_margin(egui::Margin::symmetric(8, 0)),
            )
            .show(ctx, |ui| {
                let titlebar_rect = ui.max_rect();

                // Register the entire titlebar as a drag region first; buttons
                // rendered afterwards automatically override this hitbox.
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

                ui.horizontal(|ui| {
                    // Sidebar toggle.
                    let sidebar_tooltip = if self.view_state.left_rail_expanded {
                        "Collapse sidebar"
                    } else {
                        "Expand sidebar"
                    };
                    if crate::widgets::icon_button_toolbar(
                        ui,
                        crate::theme::ICON_LIST,
                        theme.text_base,
                        &theme,
                    )
                    .on_hover_text(sidebar_tooltip)
                    .clicked()
                    {
                        self.view_state.left_rail_expanded = !self.view_state.left_rail_expanded;
                    }

                    // Right-aligned window controls.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;

                        // Close.
                        let close = crate::widgets::window_control_button(
                            ui,
                            crate::theme::ICON_X,
                            &theme,
                            theme.danger.linear_multiply(0.25),
                            egui::Color32::WHITE,
                            theme.text,
                        )
                        .on_hover_text("Close window");
                        if close.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        }

                        // Maximize / restore.
                        let max_icon = if is_maximized {
                            crate::theme::ICON_COPY
                        } else {
                            crate::theme::ICON_SQUARE
                        };
                        let max = crate::widgets::window_control_button(
                            ui,
                            max_icon,
                            &theme,
                            theme.overlay_medium,
                            theme.text,
                            theme.text,
                        )
                        .on_hover_text(if is_maximized {
                            "Restore window"
                        } else {
                            "Maximize window"
                        });
                        if max.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                        }

                        // Minimize.
                        let min = crate::widgets::window_control_button(
                            ui,
                            crate::theme::ICON_MINUS,
                            &theme,
                            theme.overlay_medium,
                            theme.text,
                            theme.text,
                        )
                        .on_hover_text("Minimize to taskbar");
                        if min.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                    });
                });
            });
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
                if self.view_state.modal.is_some() {
                    self.view_state.close_modal();
                } else if self.view_state.main == clarity_core::ui::AppView::Settings {
                    self.view_state.main = clarity_core::ui::AppView::Chat;
                } else if matches!(
                    self.view_state.right,
                    Some(clarity_core::ui::SidePanel::Team)
                        | Some(clarity_core::ui::SidePanel::Task)
                ) {
                    self.view_state.right = None;
                }
                true
            }
            ids::NEW_SESSION => {
                if !self.is_loading() {
                    self.new_session();
                }
                true
            }
            ids::STOP_GENERATION => {
                self.stop();
                true
            }
            ids::SEND_MESSAGE => {
                if !self.chat_store.input.trim().is_empty() && !self.is_loading() {
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
                    self.view_state
                        .open_modal(clarity_core::ui::ModalType::Skill);
                }
                true
            }
            ids::TOGGLE_TEAM_PANEL => {
                self.view_state
                    .toggle_right(clarity_core::ui::SidePanel::Team);
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
            ids::TOGGLE_LAYOUT_DEBUG => {
                self.view_state.toggle_debug_layout_overlay();
                self.persist_layout_settings();
                true
            }
            ids::TOGGLE_SIDEBAR => {
                self.view_state.left_rail_expanded = !self.view_state.left_rail_expanded;
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
            ids::INCREASE_FONT_SCALE => {
                self.increase_font_scale();
                true
            }
            ids::DECREASE_FONT_SCALE => {
                self.decrease_font_scale();
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
        let session = self
            .session_store
            .sessions
            .iter()
            .find(|s| s.id == active_id)?;
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

    fn render_team_create_modal(&mut self, ctx: &egui::Context) {
        panels::team_create::render_team_create_modal(self, ctx);
    }

    fn render_dashboard_panel(&mut self, ctx: &egui::Context) {
        panels::dashboard::render_dashboard_panel(self, ctx);
    }

    fn render_gantt_panel(&mut self, ctx: &egui::Context) {
        panels::gantt::render_gantt_panel(self, ctx);
    }

    fn render_task_board(&mut self, ctx: &egui::Context) {
        panels::task_board::render_task_board(self, ctx);
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

    fn render_work_panel(&mut self, ctx: &egui::Context) {
        panels::work::render_work_panel(self, ctx);
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

        // Sync live Claw device list (~2 Hz — cheap snapshot of pre-fetched data).
        if self.ui_store.frame_count % 30 == 0 {
            let devices = self.device_state.snapshot();
            if !devices.is_empty() {
                self.ui_store.bot_instances = devices;
                // If the previously-active bot disappeared, reset selection.
                if !self
                    .ui_store
                    .bot_instances
                    .iter()
                    .any(|b| b.id == self.ui_store.active_bot_id)
                {
                    self.ui_store.active_bot_id = self
                        .ui_store
                        .bot_instances
                        .first()
                        .map(|b| b.id.clone())
                        .unwrap_or_default();
                }
            }

            // Manage WebSocket connection: connect/reconnect when the active
            // Claw device changes or when the connection drops.
            let active_id = &self.ui_store.active_bot_id;
            if active_id != &self.claw_ws_device_id || self.claw_ws.is_none() {
                if let Some(conn) = self.device_state.connection(active_id) {
                    if !conn.gateway_token.is_empty() {
                        let gw_url = conn.gateway_url.clone();
                        if !gw_url.starts_with("ws://") && !gw_url.starts_with("wss://") {
                            // Convert HTTP URL to WebSocket URL.
                            let ws_url = gw_url
                                .replace("http://", "ws://")
                                .replace("https://", "wss://");
                            self.claw_ws = Some(crate::claw_client::ClawClient::connect(
                                &ws_url,
                                &conn.gateway_token,
                            ));
                        } else {
                            self.claw_ws = Some(crate::claw_client::ClawClient::connect(
                                &gw_url,
                                &conn.gateway_token,
                            ));
                        }
                        self.claw_ws_device_id = active_id.clone();
                    }
                }
            }

            // Drain WebSocket responses (clone handle to avoid borrow conflicts).
            {
                let responses = self
                    .claw_ws
                    .as_ref()
                    .map(|ws| ws.drain())
                    .unwrap_or_default();
                let ws_handle = self.claw_ws.clone();

                for resp in responses {
                    match resp {
                        crate::claw_client::ClawResponse::Connected { gateway_url } => {
                            self.push_toast(
                                format!("Connected to Claw Gateway: {}", gateway_url),
                                crate::ui::types::ToastLevel::Info,
                            );
                            // Auto-fetch session history after connect.
                            if let Some(ref ws) = ws_handle {
                                ws.fetch_history("agent:main:main");
                            }
                        }
                        crate::claw_client::ClawResponse::HistoryLoaded {
                            session_key: _,
                            messages,
                        } => {
                            let count = messages.len();
                            self.push_toast(
                                format!("Loaded {} messages from session", count),
                                crate::ui::types::ToastLevel::Info,
                            );
                            self.ui_store.claw_history = messages
                                .iter()
                                .map(|m| format!("[{}] {}", m.role, m.content))
                                .collect();
                        }
                        crate::claw_client::ClawResponse::Reply { id: _, ok, payload } => {
                            if let Some(text) = extract_claw_text(&payload) {
                                if !text.trim().is_empty() {
                                    let _ = self.ui_tx.send(crate::ui::types::UiEvent::Chunk(text));
                                    let _ = self.ui_tx.send(crate::ui::types::UiEvent::Done);
                                }
                            } else if !ok {
                                let err = payload
                                    .get("error")
                                    .and_then(|v| v.as_str())
                                    .or_else(|| payload.get("message").and_then(|v| v.as_str()))
                                    .unwrap_or("OpenClaw request failed");
                                let _ = self
                                    .ui_tx
                                    .send(crate::ui::types::UiEvent::Error(err.into()));
                            }
                        }
                        crate::claw_client::ClawResponse::Event {
                            event_type,
                            payload,
                        } => {
                            if matches!(
                                event_type.as_str(),
                                "done" | "finished" | "turn_end" | "message_end"
                            ) {
                                let _ = self.ui_tx.send(crate::ui::types::UiEvent::Done);
                            } else if let Some(text) = extract_claw_text(&payload) {
                                if !text.trim().is_empty() {
                                    let _ = self.ui_tx.send(crate::ui::types::UiEvent::Chunk(text));
                                }
                            }
                        }
                        crate::claw_client::ClawResponse::Error(e) => {
                            tracing::warn!("Claw WebSocket error: {}", e);
                            let _ = self.ui_tx.send(crate::ui::types::UiEvent::Error(format!(
                                "OpenClaw connection error: {}",
                                e
                            )));
                            self.claw_ws = None;
                            self.claw_ws_device_id.clear();
                        }
                    }
                }
            }
        }

        // Refresh shell prompt (~1 Hz) to track cwd / git branch changes.
        if self.ui_store.frame_count % 60 == 0 {
            self.refresh_shell_prompt();
        }

        self.process_events();

        // Sync the layout debug overlay toggle from ViewState to egui memory.
        crate::ui::debug_overlay::sync_enabled(ctx, self.view_state.debug_layout_overlay);

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

        if self.is_loading() {
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
        if self.view_state.right == Some(clarity_core::ui::SidePanel::Task)
            && self.task_store.last_task_refresh.elapsed() > Duration::from_secs(3)
        {
            self.refresh_tasks();
        }

        // Poll parallel batch status when panel is open
        if self.view_state.right == Some(clarity_core::ui::SidePanel::Task)
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
                if self.is_loading() {
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

        // ── Layout shell: responsive geometry + collapse policy ──
        let _metrics = crate::layout::update_and_measure(self, ctx);

        ctx.style_mut(|style| {
            self.ui_store.theme.apply(style);
        });
        // Install theme into egui Context data so design_system helpers can
        // retrieve it automatically without threading `&Theme` everywhere.
        crate::design_system::install_theme(ctx, self.ui_store.theme.clone());

        // Mirror pending approvals into the modal state machine. Approval takes
        // precedence only when no other modal is currently open.
        if !self.ui_store.pending_approvals.is_empty() && !self.ui_store.kimi_conversation_style {
            if self.view_state.modal.is_none()
                || self.view_state.modal == Some(clarity_core::ui::ModalType::Approval)
            {
                self.view_state
                    .open_modal(clarity_core::ui::ModalType::Approval);
            }
        } else if self.view_state.modal == Some(clarity_core::ui::ModalType::Approval) {
            self.view_state.close_modal();
        }

        // ── Layout shell: chrome + main view + overlays + modals ──
        self.render_layout_shell(ctx);

        // Pretext PoC: measurement probe window
        if self.ui_store.pretext_probe_open {
            crate::widgets::pretext_probe::render_pretext_probe(self, ctx);
        }

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
    let startup_t0 = Instant::now();
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
                .with_inner_size([
                    theme_defaults.window_default_w,
                    theme_defaults.window_default_h,
                ])
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

    let pre_window_elapsed = startup_t0.elapsed();
    tracing::info!("Startup: pre-window init took {:?}", pre_window_elapsed);

    eframe::run_native(
        "Clarity",
        options,
        Box::new(move |cc| {
            let app_creation_t0 = Instant::now();
            #[cfg(windows)]
            let _ = platform::windows::apply_rounded_corners(cc);
            let tray_manager = crate::services::tray::TrayManager::new();
            if tray_manager.is_none() {
                tracing::warn!("Failed to initialize system tray icon");
            }
            let app = Box::new(App::new(cc, gateway_manager, tray_manager)?);
            tracing::info!(
                "Startup: App creation took {:?} (total {:?})",
                app_creation_t0.elapsed(),
                startup_t0.elapsed()
            );
            Ok(app)
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
        RenderLine::ArtifactRef {
            artifact_id,
            summary,
        } => {
            format!("{artifact_id} — {summary}")
        }
        RenderLine::CrossInstanceRef {
            target_instance,
            message,
            ..
        } => {
            format!("@{target_instance} — {message}")
        }
        RenderLine::SlashCompletion {
            command,
            description,
        } => {
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

/// Try to extract human-readable text from an OpenClaw Gateway payload.
///
/// Different Gateway implementations emit responses under different keys, so
/// this helper checks the common shapes without being tied to one schema.
fn extract_claw_text(payload: &serde_json::Value) -> Option<String> {
    // Direct string fields.
    for key in ["text", "content", "message", "delta", "answer", "output"] {
        if let Some(text) = payload.get(key).and_then(|v| v.as_str()) {
            return Some(text.into());
        }
    }
    // Nested message object.
    if let Some(content) = payload
        .get("message")
        .or_else(|| payload.get("choices"))
        .and_then(|v| v.get("content"))
        .and_then(|v| v.as_str())
    {
        return Some(content.into());
    }
    // Array of content parts (OpenAI-style).
    if let Some(parts) = payload.get("content_parts").and_then(|v| v.as_array()) {
        let text: String = parts
            .iter()
            .filter_map(|p| p.get("text").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("");
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

#[cfg(test)]
mod pretext_alignment;
