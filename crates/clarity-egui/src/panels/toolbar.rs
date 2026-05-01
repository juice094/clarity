use crate::App;

pub fn render_toolbar(app: &mut App, ctx: &egui::Context) {
    if !app.toolbar_open {
        return;
    }
    // Mutually exclusive with the legacy task panel to avoid two right-side panels.
    if app.task_panel_open {
        app.task_panel_open = false;
    }
    egui::SidePanel::right("toolbar")
        .default_width(240.0)
        .min_width(180.0)
        .max_width(360.0)
        .resizable(true)
        .frame(
            egui::Frame::new()
                .fill(app.theme.bg_accent)
                .inner_margin(egui::Margin::same(4)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Tools")
                        .size(16.0)
                        .strong()
                        .color(app.theme.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("×").clicked() {
                        app.toolbar_open = false;
                    }
                });
            });
            ui.add_space(8.0);

            // ── Status overview ──
            let active_tasks = app.tasks.iter().filter(|t| !t.status.is_terminal()).count();
            let status_color = if active_tasks > 0 {
                app.theme.status_busy
            } else {
                app.theme.status_online
            };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("●").size(10.0).color(status_color));
                ui.label(
                    egui::RichText::new(format!("Active tasks: {}", active_tasks))
                        .size(12.0)
                        .color(app.theme.text_muted),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("●").size(10.0).color(app.theme.status_online));
                ui.label(
                    egui::RichText::new(format!("Category: {}", app.active_category))
                        .size(12.0)
                        .color(app.theme.text_muted),
                );
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // ── Task list (reuses existing task_panel renderer) ──
            let action = crate::ui::task_panel::render_task_panel(ui, &app.tasks, &app.theme);
            ui.add_space(8.0);
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("+ Create Task")
                            .size(13.0)
                            .color(app.theme.text),
                    )
                    .fill(app.theme.accent)
                    .min_size(egui::vec2(ui.available_width(), 36.0)),
                )
                .clicked()
            {
                app.task_create_modal_open = true;
            }
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
        });
}
