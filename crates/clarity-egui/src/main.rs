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

use chrono::Utc;

mod app_state;
pub(crate) mod claw;
use crate::claw::normalize_gateway_url;
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
    /// Keyboard shortcuts reference modal (Ctrl+/).
    pub(crate) shortcuts_help_open: bool,
    /// Pretext UI command palette (Ctrl+Shift+P).
    pub(crate) command_palette: crate::widgets::command_palette::CommandPalette,
    /// Pretext UI unified view state (replaces boolean flag hell).
    pub(crate) view_state: clarity_core::ui::ViewState,
    /// Pretext text measurement backend backed by egui fonts.
    pub(crate) pretext_metrics: crate::pretext::EguiFontMetrics,
    /// Live Claw device list polled from Gateway (replaces hardcoded mock data).
    pub(crate) device_state: crate::claw::DeviceState,
    /// Active WebSocket connection to the selected Claw Gateway.
    pub(crate) claw_ws: Option<crate::claw::ClawClientHandle>,
    /// Track which device the current WebSocket is connected to.
    pub(crate) claw_ws_device_id: String,
    /// Cached Clarity device identity for OpenClaw device-paired auth.
    pub(crate) claw_device_identity: Option<clarity_claw::DeviceIdentity>,
    /// Cached paired-device token for the OpenClaw Gateway.
    pub(crate) claw_device_token: Option<clarity_claw::PairedToken>,
    /// Temporary WebSocket client used only for in-app pairing.
    pub(crate) claw_pairing_client: Option<clarity_claw::ClawClient>,
    /// Current state of the in-app pairing flow.
    pub(crate) claw_pairing_state: PairingState,
    /// OKF knowledge bundle browser state.
    pub(crate) knowledge_store: crate::stores::KnowledgeStore,
    /// Console output ring-buffer for the right-rail Console panel.
    pub(crate) console_store: crate::stores::ConsoleStore,
    /// Local file browser state for the right-rail Files panel.
    pub(crate) files_store: crate::stores::FilesStore,
    /// Export format and share options for the right-rail Share panel.
    pub(crate) share_store: crate::stores::ShareStore,
    /// Built-in and remote template library for the right-rail Templates panel.
    pub(crate) template_store: crate::stores::TemplateStore,
    /// Panel transition animation state.
    pub(crate) panel_animation: crate::animation::PanelAnimationState,
}

mod animation;
mod app_logic;
mod onboarding;

/// State of an in-app OpenClaw device-pairing flow.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum PairingState {
    /// No pairing in progress.
    #[default]
    Idle,
    /// Sending the pairing request.
    Requesting,
    /// Waiting for the user to approve the request in the Gateway UI.
    Waiting {
        /// Gateway URL being paired with.
        gateway_url: String,
        /// When the wait started.
        since: std::time::Instant,
    },
    /// Pairing approved and token saved.
    Approved {
        /// Gateway URL that was paired.
        gateway_url: String,
        /// Device token returned by the Gateway.
        token: String,
    },
    /// Pairing failed or timed out.
    Error(String),
}

/// Return true if the URL's host is localhost/127.0.0.1.
pub(crate) fn is_localhost_host(url: &str) -> bool {
    crate::claw::normalize_gateway_url(url)
        .trim_start_matches("ws://")
        .trim_start_matches("wss://")
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(':')
        .next()
        .map(|h| h == "localhost" || h == "127.0.0.1")
        .unwrap_or(false)
}

impl App {
    // ── Per-frame service methods (extracted from update() in Phase 2) ──

    /// Advance the frame counter and compute instantaneous FPS.
    fn tick_frame_counter(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.ui_store.frame_count += 1;
        if now - self.ui_store.last_fps_time >= 1.0 {
            self.ui_store.fps =
                self.ui_store.frame_count as f64 / (now - self.ui_store.last_fps_time);
            self.ui_store.frame_count = 0;
            self.ui_store.last_fps_time = now;
        }
    }

    /// Mirror agent runtime state into the UI status indicator.
    fn sync_agent_status(&mut self) {
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
    }

    /// Sync the system-tray icon colour with the current runtime state.
    fn sync_tray_status(&mut self) {
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
    }

    /// Handle OS-level file drops into the window.
    fn handle_file_drops(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped_files.is_empty() {
            return;
        }
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

    /// Periodic poll: tasks, parallel-batch status, Gateway health.
    fn poll_periodic_checks(&mut self) {
        let task_visible = self.view_state.right_rail_panel
            == clarity_core::ui::RightRailPanel::Task
            && self.view_state.right_rail_visible;
        if task_visible && self.task_store.last_task_refresh.elapsed() > Duration::from_secs(3) {
            self.refresh_tasks();
        }
        if task_visible
            && !self.subagent_store.parallel_batches.is_empty()
            && self.subagent_store.last_parallel_poll.elapsed() > Duration::from_secs(2)
        {
            self.poll_parallel_batches();
        }
        if self.subagent_store.last_gateway_health_poll.elapsed() > Duration::from_secs(5) {
            self.subagent_store.last_gateway_health_poll = Instant::now();
            self.poll_gateway_health();
        }
    }

    /// Apply responsive layout, theme, and approval modal state.
    fn apply_frame_state(&mut self, ctx: &egui::Context) {
        let _metrics = crate::layout::update_and_measure(self, ctx);
        ctx.style_mut(|style| {
            self.ui_store.theme.apply(style);
        });
        crate::design_system::install_theme(ctx, self.ui_store.theme.clone());
        // Mirror pending approvals into the modal state machine.
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
    }

    /// Render the keyboard shortcuts reference modal.
    fn render_shortcuts_help(&mut self, ctx: &egui::Context) {
        if !self.shortcuts_help_open {
            return;
        }
        let theme = self.ui_store.theme.clone();
        let mut open = self.shortcuts_help_open;
        egui::Window::new("Keyboard Shortcuts")
            .id(egui::Id::new("shortcuts_help"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_width(520.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(480.0)
                    .show(ui, |ui| {
                        let actions: &[(&str, &[crate::shortcuts::ShortcutAction])] = &[
                            (
                                "General",
                                &[
                                    crate::shortcuts::ShortcutAction::NewSession,
                                    crate::shortcuts::ShortcutAction::SendMessage,
                                    crate::shortcuts::ShortcutAction::StopGeneration,
                                    crate::shortcuts::ShortcutAction::CloseModal,
                                    crate::shortcuts::ShortcutAction::ShowShortcuts,
                                ],
                            ),
                            (
                                "Panels",
                                &[
                                    crate::shortcuts::ShortcutAction::ToggleCommandPalette,
                                    crate::shortcuts::ShortcutAction::FocusInput,
                                    crate::shortcuts::ShortcutAction::ToggleConsole,
                                    crate::shortcuts::ShortcutAction::ToggleFiles,
                                    crate::shortcuts::ShortcutAction::ToggleShare,
                                    crate::shortcuts::ShortcutAction::ToggleSkillPanel,
                                    crate::shortcuts::ShortcutAction::ToggleTeamPanel,
                                    crate::shortcuts::ShortcutAction::ToggleDashboardPanel,
                                ],
                            ),
                            (
                                "View",
                                &[
                                    crate::shortcuts::ShortcutAction::IncreaseFontScale,
                                    crate::shortcuts::ShortcutAction::DecreaseFontScale,
                                    crate::shortcuts::ShortcutAction::ToggleLayoutDebug,
                                ],
                            ),
                        ];
                        for (group, items) in actions {
                            crate::design_system::gap(ui, crate::design_system::Space::S1);
                            ui.label(
                                egui::RichText::new(*group)
                                    .size(theme.text_sm)
                                    .color(theme.text_muted)
                                    .strong(),
                            );
                            for action in items.iter() {
                                ui.horizontal(|ui| {
                                    ui.add_sized(
                                        [140.0, theme.text_base],
                                        egui::Label::new(
                                            egui::RichText::new(action.keybinding())
                                                .size(theme.text_sm)
                                                .monospace()
                                                .color(theme.accent),
                                        ),
                                    );
                                    ui.label(
                                        egui::RichText::new(action.description())
                                            .size(theme.text_sm)
                                            .color(theme.text),
                                    );
                                });
                            }
                        }
                    });
            });
        self.shortcuts_help_open = open;
    }

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
    /// Render a full-screen scrim that blocks background interaction when a
    /// modal is open. Clicks are absorbed by the scrim; Tab without modifiers
    /// requests focus on the first focusable element inside the modal to keep
    /// keyboard navigation from leaking into background panels.
    fn render_modal_scrim(&self, ctx: &egui::Context) {
        let theme = self.ui_store.theme.clone();
        let screen = ctx.screen_rect();
        let scrim_id = egui::Id::new("modal_scrim");

        // Absorb Tab / Shift+Tab so they cannot cycle focus into background
        // panels. The modal itself handles Tab via its own widget hierarchy.
        let tab_pressed = ctx.input(|i| i.key_pressed(egui::Key::Tab));
        if tab_pressed {
            // Let the modal's natural focus order handle it — just mark the
            // scrim as having consumed the event so egui's default focus
            // navigation doesn't move outside the modal area.
            ctx.memory_mut(|m| {
                m.request_focus(scrim_id);
            });
        }

        // Close-on-Escape is already handled by the ShortcutAction::CloseModal
        // dispatch path; the scrim does not duplicate that logic.

        egui::Area::new(scrim_id)
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // Absorb all pointer events — clicks on the scrim are ignored.
                ui.allocate_rect(screen, egui::Sense::click_and_drag());
                ui.painter()
                    .rect_filled(screen, egui::CornerRadius::ZERO, theme.overlay);
            });
    }

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

    /// Find a Claw session by its session_key.
    fn claw_session_id_by_key(&self, session_key: &str) -> Option<String> {
        self.session_store
            .sessions
            .iter()
            .find(|s| {
                matches!(
                    &s.context,
                    crate::ui::types::SessionContext::Claw {
                        session_key: key,
                        ..
                    } if key == session_key
                )
            })
            .map(|s| s.id.clone())
    }

    /// Find all Claw sessions belonging to a role.
    fn claw_session_ids_by_role(&self, role: &str) -> Vec<String> {
        self.session_store
            .sessions
            .iter()
            .filter(|s| {
                matches!(
                    &s.context,
                    crate::ui::types::SessionContext::Claw {
                        role: r,
                        ..
                    } if r == role
                )
            })
            .map(|s| s.id.clone())
            .collect()
    }

    /// Subscribe to and fetch history for the given Claw session_key over an
    /// already-connected Gateway WebSocket.
    fn subscribe_claw_session(&self, session_key: &str) {
        let Some(ref ws) = self.claw_ws else {
            return;
        };
        // OpenClaw/KimiClaw Gateways do not support the Gateway-native
        // subscribe/history WebSocket commands.
        let is_openclaw =
            self.active_claw_protocol() == Some(crate::claw::ClawProtocol::OpenClawJsonRpc);
        if is_openclaw {
            return;
        }
        ws.subscribe_session(session_key);
        ws.subscribe_messages(session_key);
        ws.get_history(session_key);
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

        // Status bar sits at the very bottom (declared first so it consumes the
        // bottom-most slot). Shows git branch, agent status, and current model.
        self.render_safe(ctx, "status_bar", |app, ctx| app.render_status_bar(ctx));

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
            // Render a scrim overlay that blocks background interaction and
            // traps keyboard focus inside the modal. Clicks on the scrim are
            // absorbed; Tab/Shift+Tab cycle within the modal boundary.
            self.render_modal_scrim(ctx);

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
                ModalType::ManageWebLinks => "manage_web_links",
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
                    ModalType::ManageWebLinks => {
                        crate::panels::modals::manage_web_links::render_manage_web_links_modal(
                            app, ctx,
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
    /// Render a minimal status bar at the bottom of the window.
    ///
    /// Shows git branch (left), agent status + model name (right). Uses
    /// `size_statusbar` from the theme so it respects font scaling.
    fn render_status_bar(&mut self, ctx: &egui::Context) {
        let theme = self.ui_store.theme.clone();
        let left_w = if self.view_state.left_rail_expanded {
            theme.size_sidebar
        } else {
            theme.size_sidebar_collapsed
        };
        let right_w = if self.view_state.right_rail_visible {
            self.ui_store
                .right_rail_width
                .unwrap_or(theme.size_panel_right)
        } else {
            0.0
        };

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(theme.size_statusbar)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme.bg_accent)
                    .stroke(egui::Stroke::new(1.0, theme.border))
                    .inner_margin(egui::Margin::symmetric(
                        theme.space_8 as i8,
                        (theme.space_4 / 2.0) as i8,
                    )),
            )
            .show(ctx, |ui| {
                // Inset horizontally to match the main stage bounds.
                let content_w = ui.available_width() - left_w - right_w;
                ui.allocate_ui_with_layout(
                    egui::vec2(content_w, theme.size_statusbar),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        // Clip content so long labels don't bleed into other
                        // chrome elements on very narrow windows.
                        ui.set_clip_rect(ui.max_rect());
                        // Left: git branch / shell prompt.
                        let prompt = &self.ui_store.shell_prompt;
                        if !prompt.is_empty() {
                            ui.label(
                                egui::RichText::new(format!("⎇ {}", prompt))
                                    .size(theme.text_xs)
                                    .color(theme.text_dim),
                            );
                        }

                        // Right: agent status + model.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = theme.space_8;

                            // Model name.
                            let model = &self.settings_store.settings_edit.model;
                            if !model.is_empty() && model != "auto" {
                                ui.label(
                                    egui::RichText::new(model.as_str())
                                        .size(theme.text_xs)
                                        .color(theme.text_dim),
                                );
                            }

                            // Agent status dot + label.
                            let (dot_color, label) = match self.chat_store.agent_status {
                                crate::ui::types::AgentStatus::Online
                                | crate::ui::types::AgentStatus::Unconfigured => {
                                    (theme.status_online, "Ready")
                                }
                                crate::ui::types::AgentStatus::Busy => (theme.status_busy, "Busy"),
                                crate::ui::types::AgentStatus::Offline => {
                                    (theme.status_offline, "Offline")
                                }
                            };
                            let dot_radius = theme.space_4 / 2.0;
                            let (dot_rect, _) = ui.allocate_exact_size(
                                egui::vec2(dot_radius * 2.0, dot_radius * 2.0),
                                egui::Sense::hover(),
                            );
                            ui.painter()
                                .circle_filled(dot_rect.center(), dot_radius, dot_color);
                            ui.label(
                                egui::RichText::new(label)
                                    .size(theme.text_xs)
                                    .color(theme.text_muted),
                            );
                        });
                    },
                );
            });
    }

    fn render_main_stage(&mut self, ctx: &egui::Context) {
        // Views that do not render their own CentralPanel would otherwise expose
        // the raw window background (black on decorated-less Windows windows).
        // Fill the stage with `theme.bg` first; Chat already does this itself,
        // so skip it to avoid double CentralPanel.
        let self_renders_central = matches!(self.view_state.main, clarity_core::ui::AppView::Chat);
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
                } else if self.view_state.right_rail_visible {
                    self.view_state.right_rail_visible = false;
                    self.view_state.right_rail_panel = clarity_core::ui::RightRailPanel::None;
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
                    .toggle_right_rail_panel(clarity_core::ui::RightRailPanel::Team);
                true
            }
            ids::FOCUS_INPUT => {
                self.ui_store.focus_target = Some(FocusTarget::ChatInput);
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
            ids::TOGGLE_CONSOLE => {
                self.view_state
                    .set_right_rail_context(clarity_core::ui::RightRailContext::Session);
                self.view_state
                    .toggle_right_rail_panel(clarity_core::ui::RightRailPanel::Console);
                true
            }
            ids::TOGGLE_FILES => {
                self.view_state
                    .set_right_rail_context(clarity_core::ui::RightRailContext::Session);
                self.view_state
                    .toggle_right_rail_panel(clarity_core::ui::RightRailPanel::Files);
                true
            }
            ids::SHOW_SHORTCUTS => {
                self.shortcuts_help_open = true;
                true
            }
            ids::TOGGLE_SHARE => {
                self.view_state
                    .set_right_rail_context(clarity_core::ui::RightRailContext::Session);
                self.view_state
                    .toggle_right_rail_panel(clarity_core::ui::RightRailPanel::Share);
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
        if self.chat_store.find_open {
            self.update_find_matches();
        }
        panels::chat::render_chat_area(self, ctx);
    }

    /// Search the active session's messages for `find_query` and populate
    /// `find_matches` with the indices of matching messages.
    fn update_find_matches(&mut self) {
        // Skip recomputation when the query hasn't changed — avoids an O(n)
        // scan over all messages every frame while the find bar is open.
        if self.chat_store.find_query == self.chat_store.find_last_query {
            return;
        }
        self.chat_store.find_last_query = self.chat_store.find_query.clone();
        self.chat_store.find_matches.clear();
        if self.chat_store.find_query.is_empty() {
            self.chat_store.find_current = 0;
            return;
        }
        let query_lower = self.chat_store.find_query.to_lowercase();
        if let Some(session) = self.session_store.active_session() {
            for (i, msg) in session.messages.iter().enumerate() {
                if msg.content.to_lowercase().contains(&query_lower) {
                    self.chat_store.find_matches.push(i);
                }
            }
        }
        if self.chat_store.find_current >= self.chat_store.find_matches.len() {
            self.chat_store.find_current = self.chat_store.find_matches.len().saturating_sub(1);
        }
    }

    /// Render the find-in-session bar above the chat message list.
    fn render_find_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.ui_store.theme.clone();
        let total = self.chat_store.find_matches.len();
        let current = if total > 0 {
            self.chat_store.find_current + 1
        } else {
            0
        };

        egui::Frame::new()
            .fill(theme.bg_accent)
            .stroke(egui::Stroke::new(1.0, theme.border))
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .inner_margin(egui::Margin::symmetric(
                theme.space_8 as i8,
                theme.space_4 as i8,
            ))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Search input.
                    let text_edit = ui.add(
                        egui::TextEdit::singleline(&mut self.chat_store.find_query)
                            .hint_text("Find in session…")
                            .font(theme.font(theme.text_sm))
                            .desired_width(ui.available_width() - 120.0)
                            .frame(false),
                    );
                    if text_edit.changed() {
                        self.chat_store.find_current = 0;
                    }
                    if text_edit.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && total > 0
                    {
                        self.chat_store.find_current = (self.chat_store.find_current + 1) % total;
                    }

                    // Match counter: "2 of 5"
                    let count_text = if self.chat_store.find_query.is_empty() {
                        String::new()
                    } else {
                        format!("{} of {}", current, total)
                    };
                    if !count_text.is_empty() {
                        ui.label(
                            egui::RichText::new(count_text)
                                .size(theme.text_xs)
                                .color(theme.text_muted),
                        );
                    }

                    // Prev / Next buttons.
                    if ui
                        .add_enabled(
                            total > 0,
                            egui::Button::new(
                                egui::RichText::new("▲")
                                    .size(theme.text_xs)
                                    .color(theme.text),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .corner_radius(egui::CornerRadius::same(4)),
                        )
                        .clicked()
                    {
                        self.chat_store.find_current = if self.chat_store.find_current > 0 {
                            self.chat_store.find_current - 1
                        } else {
                            total.saturating_sub(1)
                        };
                    }
                    if ui
                        .add_enabled(
                            total > 0,
                            egui::Button::new(
                                egui::RichText::new("▼")
                                    .size(theme.text_xs)
                                    .color(theme.text),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .corner_radius(egui::CornerRadius::same(4)),
                        )
                        .clicked()
                    {
                        self.chat_store.find_current =
                            (self.chat_store.find_current + 1) % total.max(1);
                    }

                    // Close.
                    if crate::widgets::icon_button_toolbar(
                        ui,
                        crate::theme::ICON_X,
                        theme.text_sm,
                        &theme,
                    )
                    .on_hover_text("Close find")
                    .clicked()
                    {
                        self.chat_store.find_open = false;
                    }
                });
            });
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

    /// Start an in-app OpenClaw pairing flow for the connection at `conn_idx`.
    pub(crate) fn start_openclaw_pairing(&mut self, conn_idx: usize) {
        let Some(conn) = self
            .settings_store
            .settings_edit
            .openclaw_connections
            .get(conn_idx)
        else {
            self.claw_pairing_state = PairingState::Error("Connection not found".to_string());
            return;
        };

        if conn.token.is_empty() {
            self.claw_pairing_state =
                PairingState::Error("Gateway token is required to request pairing".to_string());
            return;
        }

        let identity = self
            .claw_device_identity
            .clone()
            .or_else(|| clarity_claw::DeviceIdentity::load_or_generate().ok());
        let Some(identity) = identity else {
            self.claw_pairing_state =
                PairingState::Error("Failed to load or generate device identity".to_string());
            return;
        };
        self.claw_device_identity = Some(identity.clone());

        let ws_url = crate::claw::to_ws_url(&conn.gateway_url);

        let token = crate::settings::GuiSettings::resolve_api_key(&Some(conn.token.clone()))
            .unwrap_or_default();
        self.claw_pairing_state = PairingState::Requesting;
        let client = clarity_claw::ClawClient::connect(&ws_url, &token);
        let scopes = vec![
            "operator.admin".into(),
            "operator.read".into(),
            "operator.write".into(),
            "operator.approvals".into(),
            "operator.pairing".into(),
            "operator.talk.secrets".into(),
        ];
        client.request_pairing(
            &identity.device_id(),
            &identity.public_key(),
            "openclaw-control-ui",
            "webchat",
            "windows",
            "operator",
            &scopes,
        );

        self.claw_pairing_client = Some(client);
        self.claw_pairing_state = PairingState::Waiting {
            gateway_url: ws_url,
            since: std::time::Instant::now(),
        };
        self.push_toast(
            "Pairing request sent. Approve it in the Gateway UI.".to_string(),
            ToastLevel::Info,
        );
    }

    /// Cancel an in-progress pairing flow.
    pub(crate) fn cancel_openclaw_pairing(&mut self) {
        self.claw_pairing_client = None;
        self.claw_pairing_state = PairingState::Idle;
    }

    /// Finish a successful pairing: save the device token to the matching
    /// settings connection and optionally persist a global paired token.
    fn finish_openclaw_pairing(&mut self, device_id: &str, token: &str, scopes: &[String]) {
        let gateway_url = match &self.claw_pairing_state {
            PairingState::Waiting { gateway_url, .. } => gateway_url.clone(),
            _ => String::new(),
        };

        let mut saved = false;
        for conn in &mut self.settings_store.settings_edit.openclaw_connections {
            let conn_ws = if conn.gateway_url.starts_with("ws://")
                || conn.gateway_url.starts_with("wss://")
            {
                conn.gateway_url.clone()
            } else {
                conn.gateway_url
                    .replace("http://", "ws://")
                    .replace("https://", "wss://")
            };
            if normalize_gateway_url(&conn_ws) == normalize_gateway_url(&gateway_url) {
                conn.auth_mode = crate::settings::OpenClawAuthMode::DevicePaired;
                conn.device_token = Some(token.to_string());
                saved = true;
                break;
            }
        }

        // Also persist a global paired-token file as a fallback for discovery.
        let paired = clarity_claw::PairedToken {
            gateway_url: gateway_url.clone(),
            token: token.into(),
            device_token: Some(token.into()),
            role: "operator".into(),
            scopes: scopes.to_vec(),
            paired_at_ms: Utc::now().timestamp_millis(),
        };
        if let Err(e) = clarity_claw::save_paired_token(&paired) {
            tracing::warn!("Failed to save paired token: {}", e);
        } else {
            self.claw_device_token = Some(paired);
        }

        if saved {
            self.auto_save_settings();
            self.push_toast(
                format!(
                    "Device {} paired successfully (scopes: {})",
                    &device_id[..device_id.len().min(8)],
                    scopes.join(",")
                ),
                crate::ui::types::ToastLevel::Info,
            );
        } else {
            self.push_toast(
                "Pairing approved, but no matching settings connection was found.".to_string(),
                crate::ui::types::ToastLevel::Warn,
            );
        }

        self.claw_pairing_state = PairingState::Approved {
            gateway_url,
            token: token.into(),
        };
        self.claw_pairing_client = None;
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

        self.tick_frame_counter(ctx);

        // Sync live Claw device list and manage Claw WebSocket lifecycle
        // (~2 Hz — device snapshot, reconnect, response draining, pairing).
        if self.ui_store.frame_count % 30 == 0 {
            self.manage_claw_connection();
            self.drain_claw_ws_responses();
            self.drain_pairing_responses();
            self.timeout_claw_pairing();
        }

        // Persist window position every ~5 s so it survives crashes.
        if self.ui_store.frame_count % 300 == 0 {
            if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                let pos = rect.min;
                let dirty = self.settings_store.settings_edit.window_x != Some(pos.x)
                    || self.settings_store.settings_edit.window_y != Some(pos.y);
                if dirty {
                    self.settings_store.settings_edit.window_x = Some(pos.x);
                    self.settings_store.settings_edit.window_y = Some(pos.y);
                    let _ = self.commit_settings();
                }
            }
        }

        // Refresh shell prompt (~1 Hz) to track cwd / git branch changes.
        if self.ui_store.frame_count % 60 == 0 {
            self.refresh_shell_prompt();
        }

        self.process_events();

        // Detect session switches and reset chat-local transient state so the
        // new session's message list / input / scroll are rendered fresh.
        let active_id = self.session_store.active_session_id.clone();
        if self.ui_store.last_active_session_id != active_id {
            self.ui_store.last_active_session_id = active_id;
            self.ui_store.last_scroll_offset = 0.0;
            self.chat_store.stick_to_bottom = true;
            self.chat_store.editing_message_idx = None;
            self.chat_store.edit_buffer.clear();
            // Close find bar and clear query so stale matches from the
            // previous session don't persist into the new one.
            self.chat_store.find_open = false;
            self.chat_store.find_query.clear();
            self.chat_store.find_matches.clear();
            self.chat_store.find_current = 0;
            self.ui_store.focus_target = Some(FocusTarget::ChatInput);
            // Stateful providers (e.g. deepseek-device) must not carry
            // conversation context across clarity sessions.
            if let Some(ref llm) = self.state.agent.llm() {
                llm.reset_conversation_context();
            }
            ctx.request_repaint();
        }
        if self.ui_store.request_repaint {
            self.ui_store.request_repaint = false;
            ctx.request_repaint();
        }

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

        self.handle_file_drops(ctx);

        // ── Find-in-session (Ctrl+F) — lightweight toggle, not routed through
        //    the ShortcutAction system to keep the dispatch path simple. ──
        if ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.ctrl)
            && !shortcuts::is_modal_open(self)
        {
            self.chat_store.find_open = !self.chat_store.find_open;
            if self.chat_store.find_open {
                self.chat_store.find_query.clear();
                self.chat_store.find_matches.clear();
                self.chat_store.find_current = 0;
                self.ui_store.focus_target = Some(FocusTarget::ChatInput);
            }
        }
        // Close find bar on Escape (before CloseModal for layering reasons).
        if self.chat_store.find_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.chat_store.find_open = false;
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

        self.poll_periodic_checks();

        // ── Auto-save: persist active session every 30 frames (~0.5 s) when
        //    modified. Guards against data loss on crash between explicit saves.
        if self.ui_store.frame_count % 30 == 0 {
            let needs_save = self
                .session_store
                .active_session()
                .map(|s| s.updated_at > s.last_saved_at)
                .unwrap_or(false);
            if needs_save {
                self.save_current_session();
            }
        }

        // ── Stuck-turn guard: if a turn has been in_flight for > 5 min
        //    without a Done or Error event, force-reset so the user isn't
        //    permanently blocked. ──
        if let Some(since) = self.chat_store.in_flight_since {
            if since.elapsed() > std::time::Duration::from_secs(300) {
                tracing::warn!("Turn stuck in_flight for > 5 min — force-resetting");
                if let Some(session) = self.session_store.active_session_mut() {
                    session.in_flight = false;
                }
                self.chat_store.in_flight_since = None;
                self.view_state.turn = clarity_core::ui::TurnState::Idle;
                self.chat_store.agent_status = AgentStatus::Online;
                self.push_toast(
                    "Agent turn timed out — you can retry your last message.",
                    ToastLevel::Warn,
                );
            }
        }

        self.sync_agent_status();
        self.sync_tray_status();
        self.apply_frame_state(ctx);

        // ── Layout shell: chrome + main view + overlays + modals ──
        self.render_layout_shell(ctx);

        // Pretext PoC: measurement probe window
        if self.ui_store.pretext_probe_open {
            crate::widgets::pretext_probe::render_pretext_probe(self, ctx);
        }

        // Keyboard shortcuts reference (Ctrl+/)
        self.render_shortcuts_help(ctx);

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

        // ── Theme-switch fade transition (250 ms) ──
        if let Some(start) = self.ui_store.theme_transition_start {
            let elapsed = start.elapsed().as_secs_f32();
            let duration = 0.25;
            if elapsed >= duration {
                self.ui_store.theme_transition_start = None;
            } else {
                // Ease-out: alpha goes from opaque to transparent.
                let t = elapsed / duration;
                let alpha = 1.0 - crate::animation::ease_out_cubic(t);
                let screen = ctx.screen_rect();
                let fg = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("theme_transition_overlay"),
                ));
                fg.rect_filled(
                    screen,
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_black_alpha((alpha * 255.0) as u8),
                );
            }
            ctx.request_repaint();
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

    // Load settings early so we can restore the saved window position before
    // the window is created.
    let settings_early = crate::settings::GuiSettings::load();
    let options = eframe::NativeOptions {
        viewport: {
            let theme_defaults = crate::theme::Theme::default();
            let mut builder = egui::ViewportBuilder::default()
                .with_inner_size([
                    theme_defaults.window_default_w,
                    theme_defaults.window_default_h,
                ])
                .with_min_inner_size([theme_defaults.window_min_w, theme_defaults.window_min_h])
                .with_decorations(false);
            if let (Some(x), Some(y)) = (settings_early.window_x, settings_early.window_y) {
                builder = builder.with_position([x, y]);
            }
            builder
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

#[cfg(test)]
mod pretext_alignment;
