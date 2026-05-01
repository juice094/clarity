use crate::App;

pub fn render_task_panel(app: &mut App, ctx: &egui::Context) {
    if !app.task_store.task_panel_open {
        return;
    }
    egui::SidePanel::right("task_panel")
        .exact_width(280.0)
        .resizable(false)
        .frame(egui::Frame::side_top_panel(&ctx.style()).fill(app.ui_store.theme.bg_accent))
        .show(ctx, |ui| {
            ui.add_space(app.ui_store.theme.space_12);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Tasks")
                        .size(16.0)
                        .strong()
                        .color(app.ui_store.theme.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("×").clicked() {
                        app.task_store.task_panel_open = false;
                    }
                });
            });

            // ---- Regular task list ----
            let action = crate::ui::task_panel::render_task_panel(ui, &app.task_store.tasks, &app.ui_store.theme);
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

            // ---- Create task button ----
            ui.add_space(app.ui_store.theme.space_8);
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("+ Create Task")
                            .size(13.0)
                            .color(app.ui_store.theme.text),
                    )
                    .fill(app.ui_store.theme.accent)
                    .min_size(egui::vec2(ui.available_width(), 36.0)),
                )
                .clicked()
            {
                app.task_store.task_create_modal_open = true;
            }

            // ---- SubAgent parallel progress ----
            ui.add_space(app.ui_store.theme.space_12);
            egui::Frame::group(ui.style())
                .fill(app.ui_store.theme.bg)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    crate::panels::subagent_progress::render_subagent_progress(app, ui);
                });
        });
}
