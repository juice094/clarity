use crate::App;
use crate::ui::types::UiEvent;
use clarity_ui::design_system::{Space, TextStyle, gap, text};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::modal::Modal;
use clarity_ui::widgets::text_input::TextInput;

/// Renders the cron create modal UI using the Clarity Design Protocol.
///
/// The modal shell itself (scrim + frame + centering) is owned by
/// `clarity_ui::widgets::modal`; this function only renders the content.
pub fn render_cron_create_modal(app: &mut App, ctx: &egui::Context) {
    if app.current_modal() != Some(&clarity_core::ui::ModalType::CronCreate) {
        return;
    }

    let mut created = false;
    let mut close_requested = false;

    Modal::new("cron_create").width(420.0).show(ctx, |ui| {
        text(ui, "New Cron Job", TextStyle::Title);
        gap(ui, Space::S2);

        text(ui, "Name", TextStyle::CaptionStrong);
        ui.add(
            TextInput::singleline(&mut app.cron_store_mut().create_name)
                .hint_text("Task name")
                .width(ui.available_width()),
        );
        gap(ui, Space::S1);

        text(ui, "Description", TextStyle::CaptionStrong);
        ui.add(
            TextInput::singleline(&mut app.cron_store_mut().create_desc)
                .hint_text("Short description")
                .width(ui.available_width()),
        );
        gap(ui, Space::S1);

        text(ui, "Prompt", TextStyle::CaptionStrong);
        ui.add_sized(
            egui::vec2(ui.available_width(), 80.0),
            TextInput::multiline(&mut app.cron_store_mut().create_prompt)
                .hint_text("Agent prompt...")
                .min_height(80.0),
        );
        gap(ui, Space::S1);

        text(ui, "Cron Expression", TextStyle::CaptionStrong);
        ui.add(
            TextInput::singleline(&mut app.cron_store_mut().create_expr)
                .hint_text("e.g. 0 * * * *")
                .width(ui.available_width()),
        );
        gap(ui, Space::S1);

        text(ui, "Priority", TextStyle::CaptionStrong);
        // ponytail: ComboBox is not yet wrapped in clarity-ui. Once Select
        // component exists, replace this raw call.
        egui::ComboBox::from_id_salt("cron_priority")
            .selected_text(match app.cron_store_mut().create_priority {
                0 => "Background",
                1 => "Low",
                2 => "Normal",
                3 => "High",
                4 => "Critical",
                _ => "Normal",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.cron_store_mut().create_priority, 0, "Background");
                ui.selectable_value(&mut app.cron_store_mut().create_priority, 1, "Low");
                ui.selectable_value(&mut app.cron_store_mut().create_priority, 2, "Normal");
                ui.selectable_value(&mut app.cron_store_mut().create_priority, 3, "High");
                ui.selectable_value(&mut app.cron_store_mut().create_priority, 4, "Critical");
            });
        gap(ui, Space::S2);

        let can_create = !app.cron_store_mut().create_name.trim().is_empty()
            && !app.cron_store_mut().create_prompt.trim().is_empty()
            && !app.cron_store_mut().create_expr.trim().is_empty();

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
        let bg_manager = std::sync::Arc::clone(&app.context.state.bg_manager);
        let tx = app.context.ui_tx.clone();
        let spec = clarity_core::background::TaskSpec {
            name: app.cron_store_mut().create_name.trim().to_string(),
            description: app.cron_store_mut().create_desc.trim().to_string(),
            agent_type: "default".to_string(),
            prompt: app.cron_store_mut().create_prompt.trim().to_string(),
            max_iterations: Some(10),
            timeout_seconds: Some(300),
            priority: clarity_core::background::TaskPriority::from_value(
                app.cron_store_mut().create_priority,
            ),
            model_alias: None,
        };
        let expr = app.cron_store_mut().create_expr.trim().to_string();
        app.context.runtime.spawn(async move {
            match bg_manager.schedule_cron(spec, &expr).await {
                Ok(task_id) => {
                    tracing::info!("Scheduled cron task: {}", task_id);
                }
                Err(e) => {
                    tracing::warn!("Failed to schedule cron: {}", e);
                }
            }
            if let Ok(tasks) = bg_manager.list_cron_tasks().await {
                let _ = tx.send(UiEvent::CronList(tasks));
            }
        });

        app.cron_store_mut().create_name.clear();
        app.cron_store_mut().create_desc.clear();
        app.cron_store_mut().create_prompt.clear();
        app.cron_store_mut().create_expr.clear();
        app.cron_store_mut().create_priority = 2;
        app.close_modal();
    } else if close_requested {
        app.close_modal();
    }
}
