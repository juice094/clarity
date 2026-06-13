use crate::ui::types::{AgentStatus, GatewayStatus};
use crate::App;

/// Render the Agent execution metrics dashboard as a right-side panel.
pub fn render_dashboard_panel(app: &mut App, ctx: &egui::Context) {
    if !app.ui_store.dashboard_panel_open {
        return;
    }

    let theme = app.ui_store.theme.clone();

    egui::SidePanel::right("dashboard_panel")
        .default_width(280.0)
        .min_width(180.0)
        .max_width(400.0)
        .resizable(true)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(theme.bg)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            ui.add_space(theme.space_12);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Dashboard")
                        .size(theme.text_lg)
                        .strong()
                        .color(theme.text),
                );
            });
            ui.add_space(theme.space_12);

            // ── Session Stats ──
            let msg_count = app
                .session_store
                .active_session()
                .map(|s| s.messages.len())
                .unwrap_or(0);
            let token_str = app
                .chat_store
                .last_usage
                .map(|(_, _, t)| format!("{}", t))
                .unwrap_or_else(|| "—".to_string());

            metric_card_pair(
                ui,
                &theme,
                "Session Messages",
                &format!("{}", msg_count),
                "Session Tokens",
                &token_str,
            );

            ui.add_space(theme.space_8);

            // ── Agent Status ──
            let (status_label, status_color) = match app.chat_store.agent_status {
                AgentStatus::Online => ("Online", theme.status_online),
                AgentStatus::Busy => ("Busy", theme.status_busy),
                AgentStatus::Offline => ("Offline", theme.status_offline),
                AgentStatus::Unconfigured => ("Unconfigured", theme.text_dim),
            };
            status_card(ui, &theme, "Agent Status", status_label, status_color);

            ui.add_space(theme.space_8);

            // ── Tool Calls ──
            let tool_count = app.chat_store.tool_calls.len();
            metric_card(
                ui,
                &theme,
                "Tool Calls (Session)",
                &format!("{}", tool_count),
            );

            ui.add_space(theme.space_8);

            // ── Subagents ──
            let running = app.subagent_store.running_agents.len();
            let batches = app.subagent_store.parallel_batches.len();
            metric_card_pair(
                ui,
                &theme,
                "Running Subagents",
                &format!("{}", running),
                "Parallel Batches",
                &format!("{}", batches),
            );

            ui.add_space(theme.space_8);

            // ── Teams ──
            metric_card(
                ui,
                &theme,
                "Active Teams",
                &format!("{}", app.team_store.teams.len()),
            );

            ui.add_space(theme.space_8);

            // ── Background Tasks ──
            metric_card(
                ui,
                &theme,
                "Background Tasks",
                &format!("{}", app.task_store.tasks.len()),
            );

            ui.add_space(theme.space_8);

            // ── Gateway Status ──
            let (gw_label, gw_color) = match app.chat_store.gateway_status {
                GatewayStatus::Online => ("Online", theme.status_online),
                GatewayStatus::Offline => ("Offline", theme.status_offline),
                GatewayStatus::Checking => ("Checking", theme.status_busy),
            };
            status_card(ui, &theme, "Gateway Status", gw_label, gw_color);

            ui.add_space(theme.space_8);

            // ── FPS ──
            metric_card(ui, &theme, "FPS", &format!("{:.1}", app.ui_store.fps));
        });
}

// ---------------------------------------------------------------------------
// Card helpers
// ---------------------------------------------------------------------------

fn metric_card(ui: &mut egui::Ui, theme: &crate::theme::Theme, title: &str, value: &str) {
    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .stroke(egui::Stroke::new(1.0_f32, theme.border))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .size(theme.text_xs)
                    .color(theme.text_muted),
            );
            ui.add_space(theme.space_4);
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
    theme: &crate::theme::Theme,
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
        ui.add_space(theme.space_8);
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
    theme: &crate::theme::Theme,
    title: &str,
    label: &str,
    dot_color: egui::Color32,
) {
    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .stroke(egui::Stroke::new(1.0_f32, theme.border))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .size(theme.text_xs)
                    .color(theme.text_muted),
            );
            ui.add_space(theme.space_4);
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
