//! Right rail — Memory / Teams card.

use crate::App;
use crate::design_system::{self, ButtonStyle, Space, Surface, Text};
use crate::ui::types::ToastLevel;
use clarity_core::tools::Tool;

/// Render teams and memory context into the right rail.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    design_system::text(ui, "Teams & Memory", Text::BodyStrong);
    design_system::gap(ui, Space::S2);

    // New team button
    let mut new_team_clicked = false;
    ui.horizontal(|ui| {
        design_system::push_right(ui);
        new_team_clicked = design_system::btn(ui, "+ New Team", ButtonStyle::Secondary).clicked();
    });
    if new_team_clicked {
        app.view_state
            .open_modal(clarity_core::ui::ModalType::TeamCreate);
    }
    design_system::gap(ui, Space::S1);

    if app.team_store.teams.is_empty() {
        design_system::center(ui, |ui| {
            design_system::gap(ui, Space::S6);
            design_system::text(ui, "No teams yet", Text::BodyMuted);
        });
        return;
    }

    let mut to_delete: Option<usize> = None;
    let mut to_run: Option<usize> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, team) in app.team_store.teams.iter().enumerate() {
            design_system::surface(ui, Surface::Card, |ui| {
                ui.set_min_width(ui.available_width());

                ui.horizontal(|ui| {
                    design_system::text(ui, &team.name, Text::BodyStrong);
                    design_system::push_right(ui);
                    if design_system::btn(ui, "Run", ButtonStyle::Primary).clicked() {
                        to_run = Some(i);
                    }
                    design_system::gap(ui, Space::S0);
                    if design_system::btn(ui, "Delete", ButtonStyle::Danger).clicked() {
                        to_delete = Some(i);
                    }
                });

                if !team.goal.is_empty() {
                    design_system::text(ui, &team.goal, Text::BodyMuted);
                }

                design_system::row(ui, |ui| {
                    design_system::text(ui, format!("{} members", team.members.len()), Text::Small);
                    design_system::gap(ui, Space::S0);
                    design_system::text(ui, "•", Text::Small);
                    design_system::gap(ui, Space::S0);
                    design_system::text(
                        ui,
                        format!("max {} concurrent", team.max_concurrency),
                        Text::Small,
                    );
                    design_system::gap(ui, Space::S0);
                    design_system::text(ui, "•", Text::Small);
                    design_system::gap(ui, Space::S0);
                    design_system::text(ui, format!("{}s timeout", team.timeout_secs), Text::Small);
                });

                design_system::gap(ui, Space::S1);

                ui.collapsing(
                    egui::RichText::new(format!("Members ({})", team.members.len()))
                        .size(theme.text_sm)
                        .color(theme.text),
                    |ui| {
                        design_system::gap(ui, Space::S0);
                        for member in &team.members {
                            egui::Frame::new()
                                .fill(theme.glass)
                                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                                .inner_margin(egui::Margin::same(8))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        design_system::text(ui, &member.name, Text::BodyStrong);
                                        design_system::gap(ui, Space::S0);
                                        design_system::text(
                                            ui,
                                            format!("({})", member.agent_type),
                                            Text::Small,
                                        );
                                    });
                                    if !member.description.is_empty() {
                                        design_system::text(ui, &member.description, Text::Caption);
                                    }
                                });
                            design_system::gap(ui, Space::S0);
                        }
                    },
                );
            });
            design_system::gap(ui, Space::S1);
        }
    });

    if let Some(i) = to_delete {
        let team = app.team_store.teams[i].clone();
        let tool = clarity_core::tools::team::TeamDeleteTool::new();
        let args = serde_json::json!({ "team_name": team.name });
        app.runtime.spawn(async move {
            let ctx = clarity_core::tools::ToolContext::new();
            match tool.execute(args, ctx).await {
                Ok(_) => tracing::info!("Team deleted: {}", team.name),
                Err(e) => tracing::warn!("Failed to delete team: {}", e),
            }
        });
        app.team_store.teams.remove(i);
        app.push_toast("Team deleted".to_string(), ToastLevel::Info);
    }
    if let Some(i) = to_run {
        let team = app.team_store.teams[i].clone();
        let agent = app.state.agent.clone();
        let tx = app.ui_tx.clone();
        app.runtime.spawn(async move {
            let specs: Vec<_> = team
                .members
                .iter()
                .map(|m| clarity_contract::subagent::RunSpec::new(&m.name, &m.description))
                .collect();
            let team_config = clarity_contract::subagent::AgentTeam::new(&team.name, &team.goal)
                .with_members(specs)
                .with_config(clarity_contract::subagent::ParallelConfig {
                    max_concurrency: team.max_concurrency,
                    timeout_secs: Some(team.timeout_secs),
                    cancel_on_error: false,
                    enable_aggregation: true,
                });
            match agent.run_team(team_config).await {
                Ok(result) => {
                    let text = format!(
                        "Team {} completed: {} results",
                        team.name,
                        result.parallel.results.len()
                    );
                    let _ = tx.send(crate::ui::types::UiEvent::Chunk(text));
                }
                Err(e) => {
                    let _ = tx.send(crate::ui::types::UiEvent::Error(format!(
                        "Team execution failed: {}",
                        e
                    )));
                }
            }
        });
    }
}
