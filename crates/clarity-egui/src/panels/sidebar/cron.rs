use crate::design_system::{self, Space, TextStyle};
use crate::App;
use crate::widgets::icon_button_toolbar;
use clarity_core::background::cron::CronTask;
use clarity_ui::widgets::button::Button;

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
    let theme = &app.context.ui_store.theme;
    let task_count = app.cron_store().tasks.len();
    let expanded = app.view_state.expansions.cron;

    ui.horizontal(|ui| {
        design_system::text(ui, "Cron Jobs", TextStyle::CaptionStrong);
        if task_count > 0 {
            ui.label(
                egui::RichText::new(format!("({})", task_count))
                    .size(theme.text_sm)
                    .color(theme.text_muted),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // ponytail: keep raw button to preserve accent text colour; Button has no
            // accent-text variant.
            if ui
                .button(
                    egui::RichText::new("+ New")
                        .size(theme.text_xs)
                        .color(theme.accent),
                )
                .clicked()
            {
                app.open_modal(clarity_core::ui::ModalType::CronCreate);
            }
            design_system::gap(ui, Space::S0);

            let arrow = if expanded {
                crate::theme::ICON_CARET_DOWN
            } else {
                crate::theme::ICON_CARET_RIGHT
            };
            if icon_button_toolbar(ui, arrow, theme.text_sm, theme)
                .clicked()
            {
                app.view_state.expansions.cron = !expanded;
            }
        });
    });

    if expanded {
        design_system::gap(ui, Space::S1);
        let action = render_cron_task_list(ui, &app.cron_store().tasks, theme);
        match action {
            CronSectionAction::Delete(task_id) => {
                let bg_manager = std::sync::Arc::clone(&app.context.state.bg_manager);
                let tx = app.context.ui_tx.clone();
                app.context.runtime.spawn(async move {
                    if let Err(e) = bg_manager.cancel_cron(&task_id).await {
                        tracing::warn!("Failed to cancel cron: {}", e);
                    }
                    if let Ok(tasks) = bg_manager.list_cron_tasks().await {
                        let _ = tx.send(crate::ui::types::UiEvent::CronList(tasks));
                    }
                });
            }
            CronSectionAction::ToggleEnabled(task_id, enabled) => {
                let bg_manager = std::sync::Arc::clone(&app.context.state.bg_manager);
                let tx = app.context.ui_tx.clone();
                app.context.runtime.spawn(async move {
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
            design_system::gap(ui, Space::S4);
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

                design_system::card(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    // Row 1: name + delete button
                    ui.horizontal(|ui| {
                        design_system::text(ui, &task.task_spec.name, TextStyle::CaptionStrong);
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .add(Button::new("Delete").danger().small())
                                    .clicked()
                                {
                                    action = CronSectionAction::Delete(task.task_id.clone());
                                }
                                design_system::gap(ui, Space::S0);

                                // Enable/disable toggle
                                let enabled = task.enabled;
                                let toggle_label = if enabled { "Disable" } else { "Enable" };
                                let toggle_btn = if enabled {
                                    Button::new(toggle_label).secondary().small()
                                } else {
                                    Button::new(toggle_label).primary().small()
                                };
                                if ui.add(toggle_btn).clicked() {
                                    action = CronSectionAction::ToggleEnabled(
                                        task.task_id.clone(),
                                        !enabled,
                                    );
                                }
                                design_system::gap(ui, Space::S0);

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
                    design_system::text(ui, format!("Next: {next_run_str}"), TextStyle::Small);

                    // Description (if any)
                    if !task.task_spec.description.is_empty() {
                        design_system::text(
                            ui,
                            &task.task_spec.description,
                            TextStyle::Small,
                        );
                    }
                });
                design_system::gap(ui, Space::S0);
            }
        });

    action
}
