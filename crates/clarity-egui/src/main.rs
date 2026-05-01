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

use app_state::AppState;

use settings::GuiSettings;
use theme::Theme;
use ui::types::*;

// ============================================================================
// Clarity egui Desktop — Phase A: Design System Foundation
// ============================================================================

const SIDEBAR_WIDTH: f32 = 240.0;
const TITLEBAR_HEIGHT: f32 = 36.0;

pub(crate) struct App {
    pub(crate) state: Arc<AppState>,
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) ui_tx: Sender<UiEvent>,
    pub(crate) ui_rx: Receiver<UiEvent>,
    pub(crate) sessions: Vec<Session>,
    pub(crate) active_session_id: String,
    pub(crate) sidebar_collapsed: bool,
    pub(crate) input: String,
    /// Per-session draft buffer. Key = session_id.
    ///
    /// FIXME-WEEK1-RISK: Drafts are memory-only; process restart loses them.
    ///   Optimize: Persist alongside session files or as a JSON sidecar.
    /// FIXME-WEEK1-RISK: No upper bound on HashMap size; long-lived sessions
    ///   may accumulate memory. Optimize: Add LRU eviction for stale drafts.
    pub(crate) drafts: std::collections::HashMap<String, String>,
    pub(crate) is_loading: bool,
    pub(crate) agent_status: AgentStatus,
    pub(crate) network_banner: Option<String>,
    pub(crate) tool_calls: Vec<ToolCallInfo>,
    pub(crate) compacting: bool,
    pub(crate) settings_open: bool,
    pub(crate) settings_edit: GuiSettings,
    #[allow(dead_code)]
    pub(crate) settings_vm: clarity_core::view_models::settings::SettingsViewModel,
    #[allow(dead_code)]
    pub(crate) wire: Arc<clarity_wire::Wire>,
    pub(crate) frame_count: u64,
    pub(crate) last_fps_time: f64,
    pub(crate) fps: f64,
    #[allow(dead_code)]
    pub(crate) start: Instant,
    pub(crate) locale: i18n::Locale,
    pub(crate) theme: Theme,
    pub(crate) provider_registry: provider::ProviderRegistry,
    pub(crate) settings_active_tab: u8,
    pub(crate) show_add_provider: bool,
    pub(crate) add_provider_name: String,
    pub(crate) add_provider_url: String,
    pub(crate) add_provider_key: String,
    pub(crate) add_provider_format: String,
    pub(crate) attachments: Vec<Attachment>,
    pub(crate) task_panel_open: bool,
    pub(crate) tasks: Vec<clarity_core::background::TaskInfo>,
    pub(crate) last_task_refresh: Instant,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) mcp_panel_open: bool,
    pub(crate) mcp_config: Option<clarity_core::mcp::config::McpConfig>,
    pub(crate) mcp_changed: bool,
    /// Last frame's scroll offset for virtual list culling.
    pub(crate) last_scroll_offset: f32,
    /// File preview: (file_name, content_text).
    pub(crate) preview_file: Option<(String, String)>,
    /// Queued message to auto-send when current streaming finishes.
    pub(crate) pending_send: Option<(String, Vec<Attachment>)>,
    /// Timestamp of the most recent input modification (used to detect IME
    /// composition activity and suppress premature Enter-send).
    pub(crate) last_input_modified: Instant,
    /// Pending approval requests from the agent runtime (populated each frame).
    pub(crate) pending_approvals: Vec<clarity_core::approval::ApprovalRequest>,
    /// Latest token usage for the active session.
    pub(crate) last_usage: Option<(u32, u32, u32)>,
    /// Pending plan for user review (Plan mode).
    pub(crate) pending_plan: Option<clarity_core::agent::Plan>,
    /// Live execution tracker for an active plan.
    pub(crate) plan_tracker: Option<crate::ui::types::PlanExecutionTracker>,
    /// Skill panel open state.
    pub(crate) skill_panel_open: bool,
    /// Right toolbar (generic tools panel) open state.
    pub(crate) toolbar_open: bool,
    /// Active session category: emotion / knowledge / engineering / tools.
    pub(crate) active_category: String,
    /// Task creation modal state.
    pub(crate) task_create_modal_open: bool,
    /// Task creation form fields.
    pub(crate) task_create_name: String,
    pub(crate) task_create_desc: String,
    pub(crate) task_create_prompt: String,
    pub(crate) task_create_priority: u8,
    /// First-run onboarding state.
    pub(crate) onboarding_state: onboarding::OnboardingState,
    /// Progress receiver for model download (std channel bridged from tokio).
    pub(crate) onboarding_progress_rx:
        Option<std::sync::mpsc::Receiver<clarity_core::model_download::ModelDownloadProgress>>,
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
    fn render_titlebar(&mut self, ctx: &egui::Context) {
        let theme = &self.theme;
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
                    if self.sidebar_collapsed {
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new("☰").size(14.0))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                            )
                            .clicked()
                        {
                            self.sidebar_collapsed = false;
                        }
                        ui.add_space(8.0);
                    }

                    ui.label(
                        egui::RichText::new("Clarity")
                            .size(13.0)
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
                            egui::Button::new(egui::RichText::new("×").size(14.0).color(theme.text_dim))
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
                                "×", egui::FontId::proportional(14.0), text_col);
                        }

                        // Maximize
                        let max_resp = ui.add_sized(btn_size,
                            egui::Button::new(egui::RichText::new("□").size(11.0).color(theme.text_dim))
                                .fill(egui::Color32::TRANSPARENT)
                        );
                        if max_resp.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                        } else if max_resp.hovered() {
                            ui.painter().rect_filled(max_resp.rect, egui::CornerRadius::ZERO, theme.overlay_medium);
                        }

                        // Minimize
                        let min_resp = ui.add_sized(btn_size,
                            egui::Button::new(egui::RichText::new("─").size(11.0).color(theme.text_dim))
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
        self.frame_count += 1;
        if now - self.last_fps_time >= 1.0 {
            self.fps = self.frame_count as f64 / (now - self.last_fps_time);
            self.frame_count = 0;
            self.last_fps_time = now;
        }

        self.process_events();

        // Drain batch-grant auto-approval notifications and show toasts.
        for msg in self.state.mode_aware_approval_runtime.drain_auto_approval_notifications() {
            self.push_toast(msg, ToastLevel::Info);
        }

        if self.is_loading {
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
                    self.attachments.push(Attachment { path, name });
                }
            }
        }

        // Check if approval modal is active — if so, suppress main-UI shortcuts.
        let approval_active = !self.pending_approvals.is_empty();

        // ESC closes modals (but not when approval modal is open).
        if !approval_active && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.settings_open {
                self.settings_open = false;
            } else if self.skill_panel_open {
                self.skill_panel_open = false;
            }
        }

        if !self.settings_open
            && !self.is_loading
            && !approval_active
            && ctx.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.ctrl)
        {
            self.new_session();
        }

        // Ctrl+C stops the running agent turn (only when generating).
        if self.is_loading
            && ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl)
        {
            self.stop();
        }

        // Refresh task list periodically when panel is open
        if self.task_panel_open && self.last_task_refresh.elapsed() > Duration::from_secs(3) {
            self.refresh_tasks();
        }

        use clarity_core::agent::AgentState;
        self.agent_status = match self.state.agent.state() {
            AgentState::Unconfigured => AgentStatus::Unconfigured,
            AgentState::Idle => {
                if self.is_loading {
                    AgentStatus::Busy
                } else {
                    AgentStatus::Online
                }
            }
            AgentState::Running { .. } => AgentStatus::Busy,
            AgentState::Stalled => AgentStatus::Offline,
        };

        ctx.style_mut(|style| {
            self.theme.apply(style);
        });

        self.render_titlebar(ctx);

        self.render_sidebar(ctx);

        self.render_task_panel(ctx);

        self.render_toolbar(ctx);

        self.render_chat_area(ctx);

        self.render_settings_panel(ctx);

        self.render_skill_panel(ctx);

        self.render_mcp_panel(ctx);

        self.render_toasts(ctx);

        self.render_approval_modal(ctx);

        self.render_task_create_modal(ctx);

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
