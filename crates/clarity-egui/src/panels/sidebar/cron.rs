use crate::App;
use clarity_core::background::cron::CronTask;

/// Actions emitted by the cron section UI.
#[allow(dead_code)]
pub enum CronSectionAction {
    None,
    Delete(String),
    ToggleEnabled(String, bool),
}

/// Render Cron Jobs as a collapsible section inside the left sidebar.
#[allow(dead_code)]
pub fn render_cron_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    let task_count = app.cron_store.tasks.len();
    let expanded = app.cron_store.cron_expanded;

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Cron Jobs")
                .size(theme.text_sm)
                .strong()
                .color(theme.text),
        );
        if task_count > 0 {
            ui.label(
                egui::RichText::new(format!("({})", task_count))
                    .size(theme.text_sm)
                    .color(theme.text_muted),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(
                    egui::RichText::new("+ New")
                        .size(theme.text_xs)
                        .color(theme.accent),
                )
                .clicked()
            {
                app.cron_store.create_modal_open = true;
            }
            ui.add_space(theme.space_4);

            let arrow = if expanded {
                crate::theme::ICON_CARET_DOWN
            } else {
                crate::theme::ICON_CARET_RIGHT
            };
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(arrow).font(theme.font_icon(theme.text_sm)),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                )
                .clicked()
            {
                app.cron_store.cron_expanded = !expanded;
            }
        });
    });

    if expanded {
        ui.add_space(theme.space_8);
        let action = render_cron_task_list(ui, &app.cron_store.tasks, theme);
        match action {
            CronSectionAction::Delete(task_id) => {
                let bg_manager = std::sync::Arc::clone(&app.state.bg_manager);
                let tx = app.ui_tx.clone();
                app.runtime.spawn(async move {
                    if let Err(e) = bg_manager.cancel_cron(&task_id).await {
                        tracing::warn!("Failed to cancel cron: {}", e);
                    }
                    if let Ok(tasks) = bg_manager.list_cron_tasks().await {
                        let _ = tx.send(crate::ui::types::UiEvent::CronList(tasks));
                    }
                });
            }
            CronSectionAction::ToggleEnabled(task_id, enabled) => {
                let bg_manager = std::sync::Arc::clone(&app.state.bg_manager);
                let tx = app.ui_tx.clone();
                app.runtime.spawn(async move {
                    if let Err(e) = bg_manager.set_cron_enabled(&task_id, enabled).await {
                        tracing::warn!("Failed to set cron enabled: {}", e);
                    }
                    if let Ok(tasks) = bg_manager.list_cron_tasks().await {
                        let _ = tx.send(crate::ui::types::UiEvent::CronList(tasks));
                    }
                });
            }
            CronSectionAction::None => {}
        }
    }
}

fn render_cron_task_list(
    ui: &mut egui::Ui,
    tasks: &[CronTask],
    theme: &crate::theme::Theme,
) -> CronSectionAction {
    if tasks.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(theme.space_20);
            ui.label(
                egui::RichText::new("No cron jobs yet")
                    .size(theme.text_base)
                    .color(theme.text_dim),
            );
        });
        return CronSectionAction::None;
    }

    let mut action = CronSectionAction::None;

    egui::ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
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
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new("Delete").size(theme.text_xs),
                                            )
                                            .fill(theme.danger)
                                            .corner_radius(egui::CornerRadius::same(
                                                theme.radius_sm as u8,
                                            )),
                                        )
                                        .clicked()
                                    {
                                        action = CronSectionAction::Delete(task.task_id.clone());
                                    }
                                    ui.add_space(theme.space_4);

                                    // Enable/disable toggle
                                    let enabled = task.enabled;
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new(if enabled {
                                                    "Disable"
                                                } else {
                                                    "Enable"
                                                })
                                                .size(theme.text_xs),
                                            )
                                            .fill(if enabled {
                                                theme.bg_elevated
                                            } else {
                                                theme.accent
                                            })
                                            .corner_radius(egui::CornerRadius::same(
                                                theme.radius_sm as u8,
                                            )),
                                        )
                                        .clicked()
                                    {
                                        action = CronSectionAction::ToggleEnabled(
                                            task.task_id.clone(),
                                            !enabled,
                                        );
                                    }
                                    ui.add_space(theme.space_4);

                                    ui.label(
                                        egui::RichText::new(status_label)
                                            .size(theme.text_xs)
                                            .color(status_color),
                                    );
                                },
                            );
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
