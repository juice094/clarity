//! Subagent output view modal — displays the live output of a completed subagent.

use crate::App;
use clarity_ui::design_system::{Space, TextStyle, code_frame, gap, text};
use clarity_ui::theme::{ICON_CHECK, ICON_HOURGLASS, ICON_X};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::modal::Modal;

/// Renders the subagent view modal UI using the Clarity Design Protocol.
///
/// The modal shell itself (scrim + frame + centering) is owned by
/// `clarity_ui::widgets::modal`; this function only renders the content.
pub fn render_subagent_view_modal(app: &mut App, ctx: &egui::Context) {
    if app.current_modal() != Some(&clarity_core::ui::ModalType::SubAgentView) {
        return;
    }

    let mut close_requested = false;
    let theme = &app.context.ui_store.theme;

    let agent_id = app
        .subagent_store()
        .viewing_subagent_id
        .clone()
        .unwrap_or_default();
    let agent_opt = app.subagent_store().running_agents.get(&agent_id).cloned();

    let title = if let Some(ref agent) = agent_opt {
        format!("{} Output", agent.agent_type)
    } else {
        "Subagent Output".to_string()
    };

    Modal::new(("subagent_view", &agent_id))
        .width(420.0)
        .show(ctx, |ui| {
            text(ui, &title, TextStyle::Title);
            gap(ui, Space::S2);

            if let Some(ref agent) = agent_opt {
                // Header: status + elapsed + steps
                ui.horizontal(|ui| {
                    let (status_icon, status_color) = if agent.status == "Completed" {
                        (ICON_CHECK, theme.status_online)
                    } else if agent.status == "Failed" {
                        (ICON_X, theme.danger)
                    } else {
                        (ICON_HOURGLASS, theme.status_busy)
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
                gap(ui, Space::S1);

                // Output lines
                code_frame(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(400.0)
                        .show(ui, |ui| {
                            for line in &agent.output_lines {
                                text(ui, line, TextStyle::Mono);
                            }
                        });
                });
            } else {
                ui.vertical_centered(|ui| {
                    gap(ui, Space::S6);
                    text(
                        ui,
                        "Output no longer available (agent cleaned up after 30s)",
                        TextStyle::Body,
                    );
                });
            }

            gap(ui, Space::S2);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(Button::new("Close").width(80.0)).clicked() {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        app.close_modal();
        app.subagent_store_mut().viewing_subagent_id = None;
    }
}
