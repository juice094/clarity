use crate::ui::types::ToastLevel;
use crate::App;

pub fn render_team_panel(app: &mut App, ctx: &egui::Context) {
    if !app.team_store.team_panel_open {
        return;
    }

    let theme = app.ui_store.theme.clone();

    egui::SidePanel::right("team_panel")
        .default_width(320.0)
        .min_width(240.0)
        .max_width(400.0)
        .resizable(true)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(theme.bg)
                .stroke(egui::Stroke::new(1.0, theme.border))
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
                        .stroke(egui::Stroke::new(1.0, theme.border))
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
                // TODO: backend integration — call TeamDeleteTool
                app.team_store.teams.remove(i);
                app.push_toast("Team deleted".to_string(), ToastLevel::Info);
            }
            if let Some(i) = to_run {
                let team_name = app.team_store.teams[i].name.clone();
                // TODO: backend integration — call TeamCoordinator::execute_team
                app.push_toast(format!("Running team: {}", team_name), ToastLevel::Info);
            }
        });
}
