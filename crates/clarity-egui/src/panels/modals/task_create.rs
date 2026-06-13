use crate::services::gateway_task_client::GatewayTaskClient;
use crate::ui::types::UiEvent;
use crate::App;

pub fn render_task_create_modal(app: &mut App, ctx: &egui::Context) {
    if !app.task_store.task_create_modal_open {
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
                .fill(app.ui_store.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.set_max_width(480.0);
            ui.heading(egui::RichText::new("New Background Task").color(app.ui_store.theme.text));
            ui.add_space(app.ui_store.theme.space_12);
            ui.label(
                egui::RichText::new("Name")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.task_store.task_create_name)
                    .hint_text("Task name"),
            );
            ui.add_space(app.ui_store.theme.space_8);
            ui.label(
                egui::RichText::new("Description")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.task_store.task_create_desc)
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
                egui::TextEdit::multiline(&mut app.task_store.task_create_prompt)
                    .hint_text("Agent prompt..."),
            );
            ui.add_space(app.ui_store.theme.space_8);
            ui.label(
                egui::RichText::new("Priority")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text)
                    .strong(),
            );
            egui::ComboBox::from_id_salt("task_priority")
                .selected_text(match app.task_store.task_create_priority {
                    0 => "Background",
                    1 => "Low",
                    2 => "Normal",
                    3 => "High",
                    4 => "Critical",
                    _ => "Normal",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.task_store.task_create_priority, 0, "Background");
                    ui.selectable_value(&mut app.task_store.task_create_priority, 1, "Low");
                    ui.selectable_value(&mut app.task_store.task_create_priority, 2, "Normal");
                    ui.selectable_value(&mut app.task_store.task_create_priority, 3, "High");
                    ui.selectable_value(&mut app.task_store.task_create_priority, 4, "Critical");
                });
            ui.add_space(app.ui_store.theme.space_12);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_create = !app.task_store.task_create_name.trim().is_empty()
                        && !app.task_store.task_create_prompt.trim().is_empty();
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
        let name = app.task_store.task_create_name.trim().to_string();
        let prompt = app.task_store.task_create_prompt.trim().to_string();
        let gateway_client = GatewayTaskClient::new();
        let local_store = app.state.task_store.clone();
        let tx = app.ui_tx.clone();

        app.runtime.spawn(async move {
            // Try Gateway first
            match gateway_client.create_task(&name, &prompt, Some(10)).await {
                Ok(task_id) => {
                    tracing::info!("Created task via Gateway: {}", task_id);
                }
                Err(e) => {
                    tracing::debug!(
                        "Gateway create_task failed ({}), falling back to local store",
                        e
                    );
                    let spec = clarity_core::background::TaskSpec {
                        name: name.clone(),
                        description: String::new(),
                        agent_type: "default".to_string(),
                        prompt: prompt.clone(),
                        max_iterations: Some(10),
                        timeout_seconds: Some(300),
                        priority: clarity_core::background::TaskPriority::Normal,
                        model_alias: None,
                    };
                    let task_id = format!("task-{}", uuid::Uuid::new_v4());
                    if let Err(e) = local_store.create(&task_id, spec).await {
                        tracing::warn!("Failed to create task {} locally: {}", task_id, e);
                        let _ = tx.send(UiEvent::Error(format!("Task create failed: {}", e)));
                        return;
                    }
                    tracing::info!("Created task locally: {}", task_id);
                }
            }
            // Refresh task list regardless of which path succeeded
            match local_store.list_all().await {
                Ok(tasks) => {
                    let _ = tx.send(UiEvent::TaskList(tasks));
                }
                Err(e) => {
                    tracing::warn!("Failed to list tasks after create: {}", e);
                }
            }
        });

        app.task_store.task_create_name.clear();
        app.task_store.task_create_desc.clear();
        app.task_store.task_create_prompt.clear();
        app.task_store.task_create_priority = 2;
        app.task_store.task_create_modal_open = false;
    } else if close_requested {
        app.task_store.task_create_modal_open = false;
    }
}
