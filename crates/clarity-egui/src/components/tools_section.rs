use crate::App;

/// Render the Tools/Tasks section for the left sidebar.
pub fn render_tools_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;

    // Section header
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Tools")
                .size(theme.text_sm)
                .strong()
                .color(theme.text_muted),
        );
    });
    ui.add_space(theme.space_4);

    // Task list
    let action = crate::ui::task_panel::render_task_panel(ui, &app.task_store.tasks, theme);
    if let crate::ui::task_panel::TaskPanelAction::Cancel(task_id) = action {
        let store = app.state.task_store.clone();
        app.runtime.spawn(async move {
            if let Err(e) = store
                .update_status(&task_id, clarity_core::background::TaskStatus::Cancelled)
                .await
            {
                tracing::warn!("Failed to cancel task {}: {}", task_id, e);
            }
        });
    }

    // Create task button
    ui.add_space(theme.space_8);
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new("+ Create Task")
                    .size(theme.text_base)
                    .color(theme.accent),
            )
            .fill(theme.accent_subtle)
            .stroke(egui::Stroke::new(1.0, theme.accent))
            .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
            .min_size(egui::vec2(ui.available_width(), 36.0)),
        )
        .clicked()
    {
        app.task_store.task_create_modal_open = true;
    }

    // SubAgent parallel progress
    ui.add_space(theme.space_12);
    egui::Frame::group(ui.style())
        .fill(theme.bg)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            crate::panels::subagent_progress::render_subagent_progress(app, ui);
        });
}
