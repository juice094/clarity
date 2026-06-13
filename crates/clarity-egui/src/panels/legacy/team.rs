use crate::App;
use crate::ui::types::ToastLevel;
use clarity_core::tools::Tool;

/// Renders the team panel UI.
pub fn render_team_panel(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();

    egui::SidePanel::right("team_panel")
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
                    egui::RichText::new("Teams")
                        .size(theme.text_lg)
                        .strong()
                        .color(theme.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            egui::RichText::new("+ New")
                                .size(theme.text_sm)
                                .color(theme.text),
                        )
                        .clicked()
                    {
                        app.team_store.create_modal_open = true;
                    }
                });
            });

            ui.add_space(theme.space_12);

            if app.team_store.teams.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(theme.space_40);
                    ui.label(
                        egui::RichText::new("No teams yet")
                            .size(theme.text_base)
                            .color(theme.text_dim),
                    );
                });
                return;
            }

            let mut to_delete: Option<usize> = None;
            let mut to_run: Option<usize> = None;

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, team) in app.team_store.teams.iter().enumerate() {
                    egui::Frame::new()
                        .fill(theme.surface)
                        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                        .stroke(egui::Stroke::new(1.0_f32, theme.border))
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());

                            // Header: name + actions
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&team.name)
                                        .size(theme.text_sm)
                                        .strong()
                                        .color(theme.text),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    egui::RichText::new("Delete")
                                                        .size(theme.text_xs),
                                                )
                                                .fill(theme.danger.linear_multiply(0.25))
                                                .corner_radius(egui::CornerRadius::same(
                                                    theme.radius_sm as u8,
                                                )),
                                            )
                                            .clicked()
                                        {
                                            to_delete = Some(i);
                                        }
                                        ui.add_space(theme.space_4);
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    egui::RichText::new("Run").size(theme.text_xs),
                                                )
                                                .fill(theme.accent.linear_multiply(0.25))
                                                .corner_radius(egui::CornerRadius::same(
                                                    theme.radius_sm as u8,
                                                )),
                                            )
                                            .clicked()
                                        {
                                            to_run = Some(i);
                                        }
                                    },
                                );
                            });

                            // Goal
                            if !team.goal.is_empty() {
                                ui.label(
                                    egui::RichText::new(&team.goal)
                                        .size(theme.text_sm)
                                        .color(theme.text_dim),
                                );
                            }

                            // Meta row
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{} members", team.members.len()))
                                        .size(theme.text_xs)
                                        .color(theme.text_muted),
                                );
                                ui.label(
                                    egui::RichText::new("\u{2022}")
                                        .size(theme.text_xs)
                                        .color(theme.text_dim),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "max {} concurrent",
                                        team.max_concurrency
                                    ))
                                    .size(theme.text_xs)
                                    .color(theme.text_muted),
                                );
                                ui.label(
                                    egui::RichText::new("\u{2022}")
                                        .size(theme.text_xs)
                                        .color(theme.text_dim),
                                );
                                ui.label(
                                    egui::RichText::new(format!("{}s timeout", team.timeout_secs))
                                        .size(theme.text_xs)
                                        .color(theme.text_muted),
                                );
                            });

                            ui.add_space(theme.space_8);

                            // Expandable member list
                            ui.collapsing(
                                egui::RichText::new(format!("Members ({})", team.members.len()))
                                    .size(theme.text_sm)
                                    .color(theme.text),
                                |ui| {
                                    ui.add_space(theme.space_4);
                                    for member in &team.members {
                                        egui::Frame::new()
                                            .fill(theme.glass)
                                            .corner_radius(egui::CornerRadius::same(
                                                theme.radius_sm as u8,
                                            ))
                                            .inner_margin(egui::Margin::same(8))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(&member.name)
                                                            .size(theme.text_sm)
                                                            .strong()
                                                            .color(theme.text),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(format!(
                                                            "({})",
                                                            member.agent_type
                                                        ))
                                                        .size(theme.text_xs)
                                                        .color(theme.text_muted),
                                                    );
                                                });
                                                if !member.description.is_empty() {
                                                    ui.label(
                                                        egui::RichText::new(&member.description)
                                                            .size(theme.text_xs)
                                                            .color(theme.text_dim),
                                                    );
                                                }
                                            });
                                        ui.add_space(theme.space_4);
                                    }
                                },
                            );
                        });
                    ui.add_space(theme.space_8);
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
                    let team_config =
                        clarity_contract::subagent::AgentTeam::new(&team.name, &team.goal)
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
        });
}
