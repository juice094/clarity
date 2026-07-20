use crate::App;
use crate::services::gateway_task_client::GatewayTaskClient;
use crate::ui::types::UiEvent;
use clarity_ui::design_system::{Space, TextStyle, gap, text};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::modal::Modal;
use clarity_ui::widgets::text_input::TextInput;

/// Renders the task create modal UI using the Clarity Design Protocol.
///
/// The modal shell itself (scrim + frame + centering) is owned by
/// `clarity_ui::widgets::modal`; this function only renders the content.
pub fn render_task_create_modal(app: &mut App, ctx: &egui::Context) {
    if app.current_modal() != Some(&clarity_core::ui::ModalType::TaskCreate) {
        return;
    }

    let mut created = false;
    let mut close_requested = false;

    Modal::new("task_create").width(420.0).show(ctx, |ui| {
        text(ui, "New Background Task", TextStyle::Title);
        gap(ui, Space::S2);

        text(ui, "Name", TextStyle::CaptionStrong);
        ui.add(
            TextInput::singleline(&mut app.task_store_mut().task_create_name)
                .hint_text("Task name")
                .width(ui.available_width()),
        );
        gap(ui, Space::S1);

        text(ui, "Description", TextStyle::CaptionStrong);
        ui.add(
            TextInput::singleline(&mut app.task_store_mut().task_create_desc)
                .hint_text("Short description")
                .width(ui.available_width()),
        );
        gap(ui, Space::S1);

        text(ui, "Prompt", TextStyle::CaptionStrong);
        ui.add_sized(
            egui::vec2(ui.available_width(), 80.0),
            TextInput::multiline(&mut app.task_store_mut().task_create_prompt)
                .hint_text("Agent prompt...")
                .min_height(80.0),
        );
        gap(ui, Space::S1);

        text(ui, "Priority", TextStyle::CaptionStrong);
        // ponytail: ComboBox is not yet wrapped in clarity-ui. Once Select
        // component exists, replace this raw call.
        egui::ComboBox::from_id_salt("task_priority")
            .selected_text(match app.task_store_mut().task_create_priority {
                0 => "Background",
                1 => "Low",
                2 => "Normal",
                3 => "High",
                4 => "Critical",
                _ => "Normal",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut app.task_store_mut().task_create_priority,
                    0,
                    "Background",
                );
                ui.selectable_value(&mut app.task_store_mut().task_create_priority, 1, "Low");
                ui.selectable_value(&mut app.task_store_mut().task_create_priority, 2, "Normal");
                ui.selectable_value(&mut app.task_store_mut().task_create_priority, 3, "High");
                ui.selectable_value(
                    &mut app.task_store_mut().task_create_priority,
                    4,
                    "Critical",
                );
            });
        gap(ui, Space::S2);

        let can_create = !app.task_store_mut().task_create_name.trim().is_empty()
            && !app.task_store_mut().task_create_prompt.trim().is_empty();

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

    if created {
        let name = app.task_store_mut().task_create_name.trim().to_string();
        let prompt = app.task_store_mut().task_create_prompt.trim().to_string();
        let gateway_client = GatewayTaskClient::new();
        let local_store = app.context.state.task_store.clone();
        let tx = app.context.ui_tx.clone();
        let session_id = app.context.session_store.active_session_id.clone();

        app.context.runtime.spawn(async move {
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
                        let _ = tx.send(UiEvent::Error {
                            session_id: session_id.clone(),
                            message: format!("Task create failed: {}", e),
                        });
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

        app.task_store_mut().task_create_name.clear();
        app.task_store_mut().task_create_desc.clear();
        app.task_store_mut().task_create_prompt.clear();
        app.task_store_mut().task_create_priority = 2;
        app.close_modal();
    } else if close_requested {
        app.close_modal();
    }
}
