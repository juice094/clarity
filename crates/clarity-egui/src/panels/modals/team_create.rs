use crate::App;
use crate::stores::TeamMember;
use clarity_core::tools::Tool;
use clarity_ui::design_system::{Space, TextStyle, gap, text};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::modal::Modal;
use clarity_ui::widgets::text_input::TextInput;

/// Renders the team create modal UI using the Clarity Design Protocol.
pub fn render_team_create_modal(app: &mut App, ctx: &egui::Context) {
    if app.current_modal() != Some(&clarity_core::ui::ModalType::TeamCreate) {
        return;
    }

    let mut created = false;
    let mut close_requested = false;
    let mut members_to_remove: Vec<usize> = Vec::new();

    Modal::new("team_create").width(420.0).show(ctx, |ui| {
        text(ui, "New Team", TextStyle::Title);
        gap(ui, Space::S2);

        text(ui, "Team Name", TextStyle::CaptionStrong);
        ui.add(
            TextInput::singleline(&mut app.team_store_mut().create_name)
                .hint_text("e.g. Code Review Squad")
                .width(ui.available_width()),
        );
        gap(ui, Space::S1);

        text(ui, "Goal", TextStyle::CaptionStrong);
        ui.add_sized(
            egui::vec2(ui.available_width(), 60.0),
            TextInput::multiline(&mut app.team_store_mut().create_goal)
                .hint_text("What this team aims to accomplish...")
                .min_height(60.0),
        );
        gap(ui, Space::S1);

        text(ui, "Max Concurrency", TextStyle::CaptionStrong);
        // ponytail: DragValue is not yet wrapped in clarity-ui.
        ui.add(
            egui::DragValue::new(&mut app.team_store_mut().create_max_concurrency)
                .speed(1)
                .range(1..=32),
        );
        gap(ui, Space::S1);

        text(ui, "Timeout (seconds)", TextStyle::CaptionStrong);
        // ponytail: DragValue is not yet wrapped in clarity-ui.
        ui.add(
            egui::DragValue::new(&mut app.team_store_mut().create_timeout_secs)
                .speed(10)
                .range(10..=3600)
                .suffix("s"),
        );
        gap(ui, Space::S2);

        text(ui, "Members", TextStyle::CaptionStrong);
        gap(ui, Space::S1);

        for (idx, member) in app.team_store_mut().create_members.iter_mut().enumerate() {
            clarity_ui::design_system::card(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    text(ui, format!("Member {}", idx + 1), TextStyle::CaptionStrong);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(Button::new("Remove").danger_ghost().small())
                            .clicked()
                        {
                            members_to_remove.push(idx);
                        }
                    });
                });
                gap(ui, Space::S1);

                text(ui, "Name", TextStyle::Small);
                ui.add(
                    TextInput::singleline(&mut member.name)
                        .hint_text("Agent name")
                        .width(ui.available_width()),
                );
                gap(ui, Space::S0);

                text(ui, "Agent Type", TextStyle::Small);
                ui.add(
                    TextInput::singleline(&mut member.agent_type)
                        .hint_text("e.g. coder, explore")
                        .width(ui.available_width()),
                );
                gap(ui, Space::S0);

                text(ui, "Description", TextStyle::Small);
                ui.add(
                    TextInput::singleline(&mut member.description)
                        .hint_text("Role description")
                        .width(ui.available_width()),
                );
            });
            gap(ui, Space::S1);
        }

        if ui.add(Button::new("+ Add Member").ghost()).clicked() {
            app.team_store_mut().create_members.push(TeamMember {
                name: String::new(),
                description: String::new(),
                agent_type: String::new(),
            });
        }

        gap(ui, Space::S2);
        let can_create = !app.team_store_mut().create_name.trim().is_empty();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = app.context.ui_store.theme.space_8;
            if ui
                .add(
                    Button::new("Create")
                        .primary()
                        .enabled(can_create)
                        .width(80.0),
                )
                .clicked()
            {
                created = true;
            }
            if ui.add(Button::new("Cancel").ghost().width(80.0)).clicked() {
                close_requested = true;
            }
        });
    });

    // Apply deferred mutations
    if !members_to_remove.is_empty() {
        // Remove in reverse order to keep indices valid
        members_to_remove.sort_by(|a, b| b.cmp(a));
        for idx in members_to_remove {
            if idx < app.team_store_mut().create_members.len() {
                app.team_store_mut().create_members.remove(idx);
            }
        }
    }

    if created {
        let team = crate::stores::Team {
            name: app.team_store_mut().create_name.trim().to_string(),
            goal: app.team_store_mut().create_goal.trim().to_string(),
            members: app.team_store_mut().create_members.clone(),
            max_concurrency: app.team_store_mut().create_max_concurrency,
            timeout_secs: app.team_store_mut().create_timeout_secs,
        };
        app.team_store_mut().teams.push(team.clone());

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
        app.context.runtime.spawn(async move {
            let ctx = clarity_core::tools::ToolContext::new();
            match tool.execute(args, ctx).await {
                Ok(_) => tracing::info!("Team created: {}", team.name),
                Err(e) => tracing::warn!("Failed to create team: {}", e),
            }
        });

        app.team_store_mut().create_name.clear();
        app.team_store_mut().create_goal.clear();
        app.team_store_mut().create_members.clear();
        app.team_store_mut().create_max_concurrency = 4;
        app.team_store_mut().create_timeout_secs = 300;
        app.close_modal();
    } else if close_requested {
        app.close_modal();
    }
}
