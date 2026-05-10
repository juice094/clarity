//! Subagent output view modal — displays the live output of a completed subagent.

use crate::App;

pub fn render_subagent_view_modal(app: &mut App, ctx: &egui::Context) {
    if !app.subagent_store.subagent_view_modal_open {
        return;
    }

    let mut close_requested = false;
    let theme = &app.ui_store.theme;

    let agent_id = app
        .subagent_store
        .viewing_subagent_id
        .clone()
        .unwrap_or_default();
    let agent_opt = app.subagent_store.running_agents.get(&agent_id).cloned();

    let title = if let Some(ref agent) = agent_opt {
        format!("{} Output", agent.agent_type)
    } else {
        "Subagent Output".to_string()
    };

    egui::Window::new(title)
        .collapsible(false)
        .resizable(true)
        .min_width(480.0)
        .min_height(240.0)
        .max_width(800.0)
        .max_height(600.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_md as u8)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(480.0);

            if let Some(ref agent) = agent_opt {
                // Header: status + elapsed + steps
                ui.horizontal(|ui| {
                    let (status_icon, status_color) = if agent.status == "Completed" {
                        (crate::theme::ICON_CHECK, theme.status_online)
                    } else if agent.status == "Failed" {
                        (crate::theme::ICON_X, theme.danger)
                    } else {
                        (crate::theme::ICON_HOURGLASS, theme.status_busy)
                    };
                    ui.label(
                        egui::RichText::new(status_icon).font(theme.font_icon(theme.text_base)),
                    );
                    ui.label(
                        egui::RichText::new(&agent.status)
                            .size(theme.text_base)
                            .strong()
                            .color(status_color),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if agent.max_steps > 0 {
                            ui.label(
                                egui::RichText::new(format!("{}/{}", agent.steps, agent.max_steps))
                                    .size(theme.text_sm)
                                    .color(theme.text_dim),
                            );
                        }
                        if let Some(completed) = agent.completed_at {
                            let elapsed = completed.duration_since(agent.started_at).as_secs();
                            ui.label(
                                egui::RichText::new(format!("{}s", elapsed))
                                    .size(theme.text_sm)
                                    .color(theme.text_dim),
                            );
                        }
                    });
                });
                ui.add_space(theme.space_8);

                // Output lines
                egui::Frame::new()
                    .fill(theme.bg)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(400.0)
                            .show(ui, |ui| {
                                for line in &agent.output_lines {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(line)
                                                .size(theme.text_sm)
                                                .color(theme.text)
                                                .monospace(),
                                        )
                                        .wrap(),
                                    );
                                }
                            });
                    });
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(theme.space_40);
                    ui.label(
                        egui::RichText::new(
                            "Output no longer available (agent cleaned up after 30s)",
                        )
                        .size(theme.text_base)
                        .color(theme.text_dim),
                    );
                });
            }

            ui.add_space(theme.space_12);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            egui::vec2(80.0, 32.0),
                            egui::Button::new(
                                egui::RichText::new("Close")
                                    .size(theme.text_base)
                                    .color(theme.text),
                            )
                            .fill(theme.border),
                        )
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        app.subagent_store.subagent_view_modal_open = false;
        app.subagent_store.viewing_subagent_id = None;
    }
}
