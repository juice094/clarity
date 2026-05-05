use crate::App;
use clarity_core::background::cron::CronTask;

/// Actions emitted by the cron panel UI.
pub enum CronPanelAction {
    None,
    Delete(String),
    ToggleEnabled(String, bool),
}

pub fn render_cron_panel(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::right("cron_panel")
        .default_width(320.0)
        .min_width(240.0)
        .max_width(400.0)
        .resizable(true)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(app.ui_store.theme.bg)
                .stroke(egui::Stroke::new(1.0, app.ui_store.theme.border))
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            ui.add_space(app.ui_store.theme.space_12);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Cron Jobs")
                        .size(app.ui_store.theme.text_lg)
                        .strong()
                        .color(app.ui_store.theme.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            egui::RichText::new("+ New")
                                .size(app.ui_store.theme.text_sm)
                                .color(app.ui_store.theme.text),
                        )
                        .clicked()
                    {
                        app.cron_store.create_modal_open = true;
                    }
                });
            });

            ui.add_space(app.ui_store.theme.space_12);
            let action = render_cron_task_list(ui, &app.cron_store.tasks, &app.ui_store.theme);
            match action {
                CronPanelAction::Delete(task_id) => {
                    // TODO: wire to clarity-core backend (cancel_cron)
                    app.cron_store.tasks.retain(|t| t.task_id != task_id);
                }
                CronPanelAction::ToggleEnabled(task_id, enabled) => {
                    // TODO: wire to clarity-core backend (set_enabled)
                    if let Some(task) = app.cron_store.tasks.iter_mut().find(|t| t.task_id == task_id) {
                        task.enabled = enabled;
                    }
                }
                CronPanelAction::None => {}
            }
        });
}

fn render_cron_task_list(
    ui: &mut egui::Ui,
    tasks: &[CronTask],
    theme: &crate::theme::Theme,
) -> CronPanelAction {
    if tasks.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(theme.space_40);
            ui.label(
                egui::RichText::new("No cron jobs yet")
                    .size(theme.text_base)
                    .color(theme.text_dim),
            );
        });
        return CronPanelAction::None;
    }

    let mut action = CronPanelAction::None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for task in tasks {
            let status_color = if task.enabled {
                theme.status_online
            } else {
                theme.text_dim
            };
            let status_label = if task.enabled { "Enabled" } else { "Disabled" };

            egui::Frame::new()
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    // Row 1: name + delete button
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&task.task_spec.name)
                                .size(theme.text_sm)
                                .strong()
                                .color(theme.text),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Delete").size(theme.text_xs),
                                    )
                                    .fill(theme.danger)
                                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                                )
                                .clicked()
                            {
                                action = CronPanelAction::Delete(task.task_id.clone());
                            }
                            ui.add_space(theme.space_4);

                            // Enable/disable toggle
                            let enabled = task.enabled;
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(if enabled { "Disable" } else { "Enable" })
                                            .size(theme.text_xs),
                                    )
                                    .fill(if enabled { theme.bg_elevated } else { theme.accent })
                                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                                )
                                .clicked()
                            {
                                action = CronPanelAction::ToggleEnabled(task.task_id.clone(), !enabled);
                            }
                            ui.add_space(theme.space_4);

                            ui.label(
                                egui::RichText::new(status_label)
                                    .size(theme.text_xs)
                                    .color(status_color),
                            );
                        });
                    });

                    // Cron expression
                    ui.label(
                        egui::RichText::new(format!("⏱ {}", task.schedule.expr))
                            .size(theme.text_sm)
                            .color(theme.text_muted)
                            .monospace(),
                    );

                    // Next run time
                    let next_run_str = task
                        .schedule
                        .next_run
                        .format("%Y-%m-%d %H:%M UTC")
                        .to_string();
                    ui.label(
                        egui::RichText::new(format!("Next: {next_run_str}"))
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    );

                    // Description (if any)
                    if !task.task_spec.description.is_empty() {
                        ui.label(
                            egui::RichText::new(&task.task_spec.description)
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                        );
                    }
                });
            ui.add_space(theme.space_4);
        }
    });

    action
}
