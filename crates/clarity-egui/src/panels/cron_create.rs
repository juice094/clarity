use crate::App;

pub fn render_cron_create_modal(app: &mut App, ctx: &egui::Context) {
    if !app.cron_store.create_modal_open {
        return;
    }
    let mut created = false;
    let mut close_requested = false;
    egui::Window::new("Create Cron Job")
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
            ui.set_max_width(480.0);
            ui.heading(egui::RichText::new("New Cron Job").color(app.ui_store.theme.text));
            ui.add_space(app.ui_store.theme.space_12);

            ui.label(
                egui::RichText::new("Name")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.cron_store.create_name).hint_text("Task name"),
            );
            ui.add_space(app.ui_store.theme.space_8);

            ui.label(
                egui::RichText::new("Description")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.cron_store.create_desc)
                    .hint_text("Short description"),
            );
            ui.add_space(app.ui_store.theme.space_8);

            ui.label(
                egui::RichText::new("Prompt")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            ui.add_sized(
                egui::vec2(ui.available_width(), 80.0),
                egui::TextEdit::multiline(&mut app.cron_store.create_prompt)
                    .hint_text("Agent prompt..."),
            );
            ui.add_space(app.ui_store.theme.space_8);

            ui.label(
                egui::RichText::new("Cron Expression")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.cron_store.create_expr)
                    .hint_text("e.g. 0 * * * *"),
            );
            ui.add_space(app.ui_store.theme.space_8);

            ui.label(
                egui::RichText::new("Priority")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            egui::ComboBox::from_id_salt("cron_priority")
                .selected_text(match app.cron_store.create_priority {
                    0 => "Background",
                    1 => "Low",
                    2 => "Normal",
                    3 => "High",
                    4 => "Critical",
                    _ => "Normal",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.cron_store.create_priority, 0, "Background");
                    ui.selectable_value(&mut app.cron_store.create_priority, 1, "Low");
                    ui.selectable_value(&mut app.cron_store.create_priority, 2, "Normal");
                    ui.selectable_value(&mut app.cron_store.create_priority, 3, "High");
                    ui.selectable_value(&mut app.cron_store.create_priority, 4, "Critical");
                });
            ui.add_space(app.ui_store.theme.space_12);

            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_create = !app.cron_store.create_name.trim().is_empty()
                        && !app.cron_store.create_prompt.trim().is_empty()
                        && !app.cron_store.create_expr.trim().is_empty();
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

    if created {
        // TODO: wire to clarity-core backend (schedule_cron)
        // For now, create a local mock entry so the UI is testable.
        let expr = app.cron_store.create_expr.trim().to_string();
        let next_run = match clarity_core::background::cron::CronSchedule::new(&expr) {
            Ok(schedule) => schedule.next_run,
            Err(_) => chrono::Utc::now() + chrono::Duration::hours(1),
        };
        let mock_task = clarity_core::background::cron::CronTask {
            task_spec: clarity_core::background::TaskSpec {
                name: app.cron_store.create_name.trim().to_string(),
                description: app.cron_store.create_desc.trim().to_string(),
                agent_type: "default".to_string(),
                prompt: app.cron_store.create_prompt.trim().to_string(),
                max_iterations: Some(10),
                timeout_seconds: Some(300),
                priority: clarity_core::background::TaskPriority::from_value(
                    app.cron_store.create_priority,
                ),
                model_alias: None,
            },
            schedule: clarity_core::background::cron::CronSchedule { expr, next_run },
            task_id: format!("cron-{}", uuid::Uuid::new_v4()),
            enabled: true,
        };
        app.cron_store.tasks.push(mock_task);

        app.cron_store.create_name.clear();
        app.cron_store.create_desc.clear();
        app.cron_store.create_prompt.clear();
        app.cron_store.create_expr.clear();
        app.cron_store.create_priority = 2;
        app.cron_store.create_modal_open = false;
    } else if close_requested {
        app.cron_store.create_modal_open = false;
    }
}
