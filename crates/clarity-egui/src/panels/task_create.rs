use crate::ui::types::UiEvent;
use crate::App;

pub fn render_task_create_modal(app: &mut App, ctx: &egui::Context) {
    if !app.task_create_modal_open {
        return;
    }
    let mut created = false;
    let mut close_requested = false;
    egui::Window::new("Create Task")
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(app.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.theme.radius_md as u8)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.set_max_width(480.0);
            ui.heading(egui::RichText::new("New Background Task").color(app.theme.text));
            ui.add_space(app.theme.space_12);
            ui.label(
                egui::RichText::new("Name")
                    .size(12.0)
                    .color(app.theme.text)
                    .strong(),
            );
            ui.add(egui::TextEdit::singleline(&mut app.task_create_name).hint_text("Task name"));
            ui.add_space(app.theme.space_8);
            ui.label(
                egui::RichText::new("Description")
                    .size(12.0)
                    .color(app.theme.text)
                    .strong(),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.task_create_desc)
                    .hint_text("Short description"),
            );
            ui.add_space(app.theme.space_8);
            ui.label(
                egui::RichText::new("Prompt")
                    .size(12.0)
                    .color(app.theme.text)
                    .strong(),
            );
            ui.add_sized(
                egui::vec2(ui.available_width(), 80.0),
                egui::TextEdit::multiline(&mut app.task_create_prompt).hint_text("Agent prompt..."),
            );
            ui.add_space(app.theme.space_8);
            ui.label(
                egui::RichText::new("Priority")
                    .size(12.0)
                    .color(app.theme.text)
                    .strong(),
            );
            egui::ComboBox::from_id_salt("task_priority")
                .selected_text(match app.task_create_priority {
                    0 => "Background",
                    1 => "Low",
                    2 => "Normal",
                    3 => "High",
                    4 => "Critical",
                    _ => "Normal",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.task_create_priority, 0, "Background");
                    ui.selectable_value(&mut app.task_create_priority, 1, "Low");
                    ui.selectable_value(&mut app.task_create_priority, 2, "Normal");
                    ui.selectable_value(&mut app.task_create_priority, 3, "High");
                    ui.selectable_value(&mut app.task_create_priority, 4, "Critical");
                });
            ui.add_space(app.theme.space_12);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_create = !app.task_create_name.trim().is_empty()
                        && !app.task_create_prompt.trim().is_empty();
                    let create_btn = ui.add_sized(
                        egui::vec2(80.0, 32.0),
                        egui::Button::new(
                            egui::RichText::new("Create")
                                .size(13.0)
                                .color(app.theme.text),
                        )
                        .fill(if can_create {
                            app.theme.accent
                        } else {
                            app.theme.bg_elevated
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
                                    .size(13.0)
                                    .color(app.theme.text),
                            )
                            .fill(app.theme.border),
                        )
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });
        });
    if created {
        let spec = clarity_core::background::TaskSpec {
            name: app.task_create_name.trim().to_string(),
            description: app.task_create_desc.trim().to_string(),
            agent_type: "default".to_string(),
            prompt: app.task_create_prompt.trim().to_string(),
            max_iterations: Some(10),
            timeout_seconds: Some(300),
            priority: clarity_core::background::TaskPriority::from_value(app.task_create_priority),
            model_alias: None,
        };
        let task_id = format!("task-{}", uuid::Uuid::new_v4());
        let store = app.state.task_store.clone();
        let tx = app.ui_tx.clone();
        app.runtime.spawn(async move {
            if let Err(e) = store.create(&task_id, spec).await {
                tracing::warn!("Failed to create task {}: {}", task_id, e);
                let _ = tx.send(UiEvent::Error(format!("Task create failed: {}", e)));
            } else {
                tracing::info!("Created task: {}", task_id);
                let _ = tx.send(UiEvent::TaskList(
                    store.list_all().await.unwrap_or_default(),
                ));
            }
        });
        app.task_create_name.clear();
        app.task_create_desc.clear();
        app.task_create_prompt.clear();
        app.task_create_priority = 2;
        app.task_create_modal_open = false;
    } else if close_requested {
        app.task_create_modal_open = false;
    }
}
