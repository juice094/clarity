use crate::App;

pub fn render_task_panel(app: &mut App, ctx: &egui::Context) {
    if !app.task_panel_open { return; }
    egui::SidePanel::right("task_panel")
        .exact_width(280.0)
        .resizable(false)
        .frame(egui::Frame::side_top_panel(&ctx.style()).fill(app.theme.bg_accent))
        .show(ctx, |ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tasks").size(16.0).strong().color(app.theme.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("×").clicked() { app.task_panel_open = false; }
                });
            });
            ui.add_space(8.0);
            let action = crate::ui::task_panel::render_task_panel(ui, &app.tasks, &app.theme);
            ui.add_space(8.0);
            if ui.add(egui::Button::new(egui::RichText::new("+ Create Task").size(13.0).color(app.theme.text)).fill(app.theme.accent).min_size(egui::vec2(ui.available_width(), 36.0))).clicked() {
                app.task_create_modal_open = true;
            }
            if let crate::ui::task_panel::TaskPanelAction::Cancel(task_id) = action {
                let store = app.state.task_store.clone();
                app.runtime.spawn(async move {
                    if let Err(e) = store.update_status(&task_id, clarity_core::background::TaskStatus::Cancelled).await {
                        tracing::warn!("Failed to cancel task {}: {}", task_id, e);
                    }
                });
            }
        });
}
