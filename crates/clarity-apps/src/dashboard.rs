//! Dashboard app — system dashboard surface.
//!
//! ponytail: card helpers are co-located with the app so `clarity-apps` does not
//! depend on `clarity-egui` panel code. They only use `clarity_ui` theme and
//! design-system tokens.

use clarity_shell::{ClarityApp, ClarityAppContext, ClarityAppResponse};
use clarity_ui::design_system::{self, Space, TextStyle};
use clarity_ui::theme::Theme;
use std::collections::HashMap;
use std::time::Instant;

/// Holds task UI state.
// ponytail: field docs inherited from legacy `clarity-egui`; add per-field docs
// in a future documentation pass.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct TaskStore {
    pub tasks: Vec<clarity_core::background::TaskInfo>,
    pub last_task_refresh: Instant,
    pub task_create_name: String,
    pub task_create_desc: String,
    pub task_create_prompt: String,
    pub task_create_priority: u8,
    /// ID of the task whose result is being viewed.
    pub viewing_task_id: Option<String>,
    /// Fetched result for the viewing task.
    pub viewing_task_result: Option<clarity_core::background::TaskResult>,
}

/// Holds cron UI state.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct CronStore {
    pub tasks: Vec<clarity_core::background::cron::CronTask>,
    #[allow(dead_code)]
    pub last_refresh: Instant,
    pub create_name: String,
    pub create_desc: String,
    pub create_prompt: String,
    pub create_expr: String,
    pub create_priority: u8,
}

/// Holds team member state.
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub struct TeamMember {
    pub name: String,
    pub description: String,
    pub agent_type: String,
}

/// Holds team state.
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub struct Team {
    pub name: String,
    pub goal: String,
    pub members: Vec<TeamMember>,
    pub max_concurrency: usize,
    pub timeout_secs: u64,
}

/// Holds team UI state.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct TeamStore {
    pub teams: Vec<Team>,
    pub create_name: String,
    pub create_goal: String,
    pub create_members: Vec<TeamMember>,
    pub create_max_concurrency: usize,
    pub create_timeout_secs: u64,
}

/// Holds sub agent UI state.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct SubAgentStore {
    pub parallel_batches: Vec<clarity_contract::subagent::SubAgentProgress>,
    pub last_parallel_poll: Instant,
    /// Live single-agent progress tracked via channel (IS-1 Sprint 30).
    pub running_agents: HashMap<String, clarity_contract::subagent::SingleSubagentProgress>,
    /// Last Gateway health check poll time.
    pub last_gateway_health_poll: Instant,
    /// ID of the subagent whose output is being viewed.
    pub viewing_subagent_id: Option<String>,
}

impl Default for SubAgentStore {
    fn default() -> Self {
        Self {
            parallel_batches: Vec::new(),
            last_parallel_poll: Instant::now(),
            running_agents: HashMap::new(),
            last_gateway_health_poll: Instant::now(),
            viewing_subagent_id: None,
        }
    }
}

impl Default for TaskStore {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            last_task_refresh: Instant::now(),
            task_create_name: String::new(),
            task_create_desc: String::new(),
            task_create_prompt: String::new(),
            task_create_priority: 0,
            viewing_task_id: None,
            viewing_task_result: None,
        }
    }
}

impl Default for CronStore {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            last_refresh: Instant::now(),
            create_name: String::new(),
            create_desc: String::new(),
            create_prompt: String::new(),
            create_expr: String::new(),
            create_priority: 0,
        }
    }
}

impl Default for TeamStore {
    fn default() -> Self {
        Self {
            teams: Vec::new(),
            create_name: String::new(),
            create_goal: String::new(),
            create_members: Vec::new(),
            create_max_concurrency: 1,
            create_timeout_secs: 60,
        }
    }
}

/// System dashboard sub-application.
#[derive(Debug, Default)]
pub struct DashboardApp {
    /// Task UI state owned by this sub-application.
    pub task_store: TaskStore,
    /// Cron UI state owned by this sub-application.
    pub cron_store: CronStore,
    /// Team UI state owned by this sub-application.
    pub team_store: TeamStore,
    /// Subagent UI state owned by this sub-application.
    pub subagent_store: SubAgentStore,
}

impl DashboardApp {
    /// Create a new dashboard app instance.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ClarityApp for DashboardApp {
    fn id(&self) -> &'static str {
        "dashboard"
    }

    fn title(&self, ctx: &ClarityAppContext<'_>) -> String {
        ctx.state.t("Dashboard").to_string()
    }

    fn render(
        &mut self,
        ctx: &mut ClarityAppContext<'_>,
        ui: &mut egui::Ui,
        _egui_ctx: &egui::Context,
    ) -> ClarityAppResponse {
        let theme = ctx.state.theme().clone();

        // ponytail: P1d — the host already places this renderer inside the
        // central strip. Do not create another CentralPanel here; the background
        // is provided by the chrome's `render_main_stage_border`.
        egui::Panel::right("dashboard_panel")
            .default_size(280.0)
            .min_size(180.0)
            .max_size(400.0)
            .resizable(true)
            .frame(
                egui::Frame::side_top_panel(ui.style())
                    .fill(theme.bg)
                    .stroke(egui::Stroke::NONE)
                    .inner_margin(egui::Margin::symmetric(12, 16)),
            )
            .show(ui, |ui| {
                design_system::gap(ui, Space::S2);
                ui.horizontal(|ui| {
                    design_system::text(ui, "Dashboard", TextStyle::Title);
                });
                design_system::gap(ui, Space::S2);

                // ── Session Stats ──
                let msg_count = ctx.state.session_message_count();
                let token_str = ctx
                    .state
                    .session_token_count()
                    .map(|t| format!("{}", t))
                    .unwrap_or_else(|| "—".to_string());

                metric_card_pair(
                    ui,
                    &theme,
                    "Session Messages",
                    &format!("{}", msg_count),
                    "Session Tokens",
                    &token_str,
                );

                design_system::gap(ui, Space::S1);

                // ── Agent Status ──
                let status_label = ctx.state.agent_status_label();
                let status_color = ctx.state.agent_status_color();
                status_card(ui, &theme, "Agent Status", status_label, status_color);

                design_system::gap(ui, Space::S1);

                // ── Tool Calls ──
                let tool_count = ctx.state.session_tool_call_count();
                metric_card(
                    ui,
                    &theme,
                    "Tool Calls (Session)",
                    &format!("{}", tool_count),
                );

                design_system::gap(ui, Space::S1);

                // ── Subagents ──
                let running = self.subagent_store.running_agents.len();
                let batches = self.subagent_store.parallel_batches.len();
                metric_card_pair(
                    ui,
                    &theme,
                    "Running Subagents",
                    &format!("{}", running),
                    "Parallel Batches",
                    &format!("{}", batches),
                );

                design_system::gap(ui, Space::S1);

                // ── Teams ──
                metric_card(
                    ui,
                    &theme,
                    "Active Teams",
                    &format!("{}", self.team_store.teams.len()),
                );

                design_system::gap(ui, Space::S1);

                // ── Background Tasks ──
                metric_card(
                    ui,
                    &theme,
                    "Background Tasks",
                    &format!("{}", self.task_store.tasks.len()),
                );

                design_system::gap(ui, Space::S1);

                // ── Gateway Status ──
                let gw_label = ctx.state.gateway_status_label();
                let gw_color = ctx.state.gateway_status_color();
                status_card(ui, &theme, "Gateway Status", gw_label, gw_color);

                design_system::gap(ui, Space::S1);

                // ── FPS ──
                metric_card(ui, &theme, "FPS", &format!("{:.1}", ctx.state.fps()));
            });

        ClarityAppResponse::None
    }
}

// ---------------------------------------------------------------------------
// Dashboard card helpers
// ---------------------------------------------------------------------------

fn metric_card(ui: &mut egui::Ui, theme: &Theme, title: &str, value: &str) {
    theme
        .frame_card()
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .size(theme.text_xs)
                    .color(theme.text_muted),
            );
            design_system::gap(ui, Space::S0);
            ui.label(
                egui::RichText::new(value)
                    .size(theme.text_2xl)
                    .strong()
                    .color(theme.text_strong),
            );
        });
}

fn metric_card_pair(
    ui: &mut egui::Ui,
    theme: &Theme,
    title_a: &str,
    value_a: &str,
    title_b: &str,
    value_b: &str,
) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width() * 0.5 - theme.space_4, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                metric_card(ui, theme, title_a, value_a);
            },
        );
        design_system::gap(ui, Space::S1);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                metric_card(ui, theme, title_b, value_b);
            },
        );
    });
}

fn status_card(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    label: &str,
    dot_color: egui::Color32,
) {
    theme
        .frame_card()
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .size(theme.text_xs)
                    .color(theme.text_muted),
            );
            design_system::gap(ui, Space::S0);
            ui.horizontal(|ui| {
                let (rect, _response) =
                    ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                let dot_center = rect.center();
                ui.painter().circle_filled(dot_center, 4.5, dot_color);
                ui.label(
                    egui::RichText::new(label)
                        .size(theme.text_xl)
                        .strong()
                        .color(theme.text_strong),
                );
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_shell::{AppState, ClarityApp};
    use clarity_ui::theme::Theme;

    struct TestState {
        theme: Theme,
    }

    impl AppState for TestState {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
        fn theme(&self) -> &Theme {
            &self.theme
        }
        fn theme_mut(&mut self) -> &mut Theme {
            &mut self.theme
        }
    }

    fn test_context<'a>(theme: &'a mut Theme, state: &'a mut TestState) -> ClarityAppContext<'a> {
        ClarityAppContext {
            theme,
            app_name: "Clarity",
            app_version: "0.0.0",
            app_description: "Test",
            app_license: "AGPL-3.0-or-later",
            state,
        }
    }

    #[test]
    fn dashboard_app_id_and_title() {
        let mut theme = Theme::dark();
        let mut state = TestState {
            theme: Theme::dark(),
        };
        let ctx = &mut test_context(&mut theme, &mut state);
        let dashboard = DashboardApp::new();
        assert_eq!(dashboard.id(), "dashboard");
        assert_eq!(dashboard.title(ctx), "Dashboard");
    }

    #[test]
    fn dashboard_app_renders_without_panic() {
        let egui_ctx = egui::Context::default();
        let mut theme = Theme::dark();
        let mut state = TestState {
            theme: Theme::dark(),
        };
        let mut dashboard = DashboardApp::new();

        let _output = egui_ctx.run_ui(egui::RawInput::default(), |egui_ctx| {
            egui::Area::new("dashboard_test".into()).show(egui_ctx, |ui| {
                let mut ctx = test_context(&mut theme, &mut state);
                let response = dashboard.render(&mut ctx, ui, egui_ctx);
                assert_eq!(response, ClarityAppResponse::None);
            });
        });
    }
}
