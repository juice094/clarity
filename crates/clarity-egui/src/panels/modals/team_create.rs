use crate::stores::TeamMember;
use crate::App;
use clarity_core::tools::Tool;

pub fn render_team_create_modal(app: &mut App, ctx: &egui::Context) {
    if !app.team_store.create_modal_open {
        return;
    }

    let mut created = false;
    let mut close_requested = false;
    let mut members_to_remove: Vec<usize> = Vec::new();

    egui::Window::new("Create Team")
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(app.ui_store.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.set_max_width(520.0);
            ui.heading(egui::RichText::new("New Team").color(app.ui_store.theme.text));
            ui.add_space(app.ui_store.theme.space_12);

            // Team Name
            ui.label(
                egui::RichText::new("Team Name")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.team_store.create_name)
                    .hint_text("e.g. Code Review Squad"),
            );
            ui.add_space(app.ui_store.theme.space_8);

            // Goal
            ui.label(
                egui::RichText::new("Goal")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            ui.add_sized(
                egui::vec2(ui.available_width(), 60.0),
                egui::TextEdit::multiline(&mut app.team_store.create_goal)
                    .hint_text("What this team aims to accomplish..."),
            );
            ui.add_space(app.ui_store.theme.space_8);

            // Max Concurrency
            ui.label(
                egui::RichText::new("Max Concurrency")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            ui.add(
                egui::DragValue::new(&mut app.team_store.create_max_concurrency)
                    .speed(1)
                    .range(1..=32),
            );
            ui.add_space(app.ui_store.theme.space_8);

            // Timeout
            ui.label(
                egui::RichText::new("Timeout (seconds)")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            ui.add(
                egui::DragValue::new(&mut app.team_store.create_timeout_secs)
                    .speed(10)
                    .range(10..=3600)
                    .suffix("s"),
            );
            ui.add_space(app.ui_store.theme.space_12);

            // Members
            ui.label(
                egui::RichText::new("Members")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            ui.add_space(app.ui_store.theme.space_8);

            for (idx, member) in app.team_store.create_members.iter_mut().enumerate() {
                egui::Frame::new()
                    .fill(app.ui_store.theme.glass)
                    .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("Member {}", idx + 1))
                                    .size(app.ui_store.theme.text_sm)
                                    .strong()
                                    .color(app.ui_store.theme.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new("Remove")
                                                    .size(app.ui_store.theme.text_xs),
                                            )
                                            .fill(app.ui_store.theme.danger.linear_multiply(0.25))
                                            .corner_radius(egui::CornerRadius::same(
                                                app.ui_store.theme.radius_sm as u8,
                                            )),
                                        )
                                        .clicked()
                                    {
                                        members_to_remove.push(idx);
                                    }
                                },
                            );
                        });
                        ui.add_space(app.ui_store.theme.space_4);

                        ui.label(
                            egui::RichText::new("Name")
                                .size(app.ui_store.theme.text_xs)
                                .color(app.ui_store.theme.text_dim),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut member.name).hint_text("Agent name"),
                        );
                        ui.label(
                            egui::RichText::new("Agent Type")
                                .size(app.ui_store.theme.text_xs)
                                .color(app.ui_store.theme.text_dim),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut member.agent_type)
                                .hint_text("e.g. coder, explore"),
                        );
                        ui.label(
                            egui::RichText::new("Description")
                                .size(app.ui_store.theme.text_xs)
                                .color(app.ui_store.theme.text_dim),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut member.description)
                                .hint_text("Role description"),
                        );
                    });
                ui.add_space(app.ui_store.theme.space_8);
            }

            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("+ Add Member")
                            .size(app.ui_store.theme.text_sm)
                            .color(app.ui_store.theme.accent),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(1.0_f32, app.ui_store.theme.accent))
                    .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                    .min_size(egui::vec2(0.0, 28.0)),
                )
                .clicked()
            {
                app.team_store.create_members.push(TeamMember {
                    name: String::new(),
                    description: String::new(),
                    agent_type: String::new(),
                });
            }

            ui.add_space(app.ui_store.theme.space_12);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_create = !app.team_store.create_name.trim().is_empty();
                    let create_btn = ui.add_sized(
                        egui::vec2(80.0, 32.0),
                        egui::Button::new(
                            egui::RichText::new("Create")
                                .size(app.ui_store.theme.text_base)
                                .color(app.ui_store.theme.text),
                        )
                        .fill(if can_create {
                            app.ui_store.theme.accent
                        } else {
                            app.ui_store.theme.bg_elevated
                        }),
                    );
                    if create_btn.clicked() && can_create {
                        created = true;
                    }
                    if ui
                        .add_sized(
                            egui::vec2(80.0, 32.0),
                            egui::Button::new(
                                egui::RichText::new("Cancel")
                                    .size(app.ui_store.theme.text_base)
                                    .color(app.ui_store.theme.text),
                            )
                            .fill(app.ui_store.theme.border),
                        )
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });
        });

    // Apply deferred mutations
    if !members_to_remove.is_empty() {
        // Remove in reverse order to keep indices valid
        members_to_remove.sort_by(|a, b| b.cmp(a));
        for idx in members_to_remove {
            if idx < app.team_store.create_members.len() {
                app.team_store.create_members.remove(idx);
            }
        }
    }

    if created {
        let team = crate::stores::Team {
            name: app.team_store.create_name.trim().to_string(),
            goal: app.team_store.create_goal.trim().to_string(),
            members: app.team_store.create_members.clone(),
            max_concurrency: app.team_store.create_max_concurrency,
            timeout_secs: app.team_store.create_timeout_secs,
        };
        app.team_store.teams.push(team.clone());

        let tool = clarity_core::tools::team::TeamCreateTool::new();
        let args = serde_json::json!({
            "team_name": team.name,
            "goal": team.goal,
            "members": team.members.iter().map(|m| serde_json::json!({
                "name": m.name,
                "description": m.description,
                "agent_type": "default",
            })).collect::<Vec<_>>(),
            "max_concurrency": team.max_concurrency,
            "timeout_secs": team.timeout_secs,
        });
        app.runtime.spawn(async move {
            let ctx = clarity_core::tools::ToolContext::new();
            match tool.execute(args, ctx).await {
                Ok(_) => tracing::info!("Team created: {}", team.name),
                Err(e) => tracing::warn!("Failed to create team: {}", e),
            }
        });

        app.team_store.create_name.clear();
        app.team_store.create_goal.clear();
        app.team_store.create_members.clear();
        app.team_store.create_max_concurrency = 4;
        app.team_store.create_timeout_secs = 300;
        app.team_store.create_modal_open = false;
    } else if close_requested {
        app.team_store.create_modal_open = false;
    }
}
