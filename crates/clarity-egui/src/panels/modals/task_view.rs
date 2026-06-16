//! Task result view modal — displays the output of a completed background task.

use crate::App;

/// Renders the task view modal UI.
pub fn render_task_view_modal(app: &mut App, ctx: &egui::Context) {
    if app.view_state.modal != Some(clarity_core::ui::ModalType::TaskView) {
        return;
    }

    let mut close_requested = false;
    let theme = &app.ui_store.theme;

    egui::Window::new("Task Result")
        .collapsible(false)
        .resizable(true)
        .min_width(480.0)
        .min_height(240.0)
        .max_width(800.0)
        .max_height(600.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_md as u8)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(480.0);

            if let Some(ref result) = app.task_store.viewing_task_result {
                // Header: status + elapsed + steps
                ui.horizontal(|ui| {
                    let (status_icon, status_color) = match result.status {
                        clarity_core::background::TaskStatus::Completed => {
                            (crate::theme::ICON_CHECK, theme.status_online)
                        }
                        clarity_core::background::TaskStatus::Failed => {
                            (crate::theme::ICON_X, theme.danger)
                        }
                        clarity_core::background::TaskStatus::Cancelled => {
                            (crate::theme::ICON_PROHIBIT, theme.text_dim)
                        }
                        _ => (crate::theme::ICON_HOURGLASS, theme.status_busy),
                    };
                    ui.label(
                        egui::RichText::new(status_icon).font(theme.font_icon(theme.text_base)),
                    );
                    ui.label(
                        egui::RichText::new(format!("{:?}", result.status))
                            .size(theme.text_base)
                            .strong()
                            .color(status_color),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if result.elapsed_ms > 0 {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{:.1}s · {} steps",
                                    result.elapsed_ms as f64 / 1000.0,
                                    result.steps
                                ))
                                .size(theme.text_sm)
                                .color(theme.text_dim),
                            );
                        }
                    });
                });
                ui.add_space(theme.space_8);

                // Output text
                egui::Frame::new()
                    .fill(theme.bg)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(400.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&result.output)
                                            .size(theme.text_sm)
                                            .color(theme.text)
                                            .monospace(),
                                    )
                                    .wrap(),
                                );
                            });
                    });
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(theme.space_40);
                    ui.label(
                        egui::RichText::new("Loading result...")
                            .size(theme.text_base)
                            .color(theme.text_dim),
                    );
                    ui.add_space(theme.space_8);
                    ui.spinner();
                });
            }

            ui.add_space(theme.space_12);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            egui::vec2(80.0, 32.0),
                            egui::Button::new(
                                egui::RichText::new("Close")
                                    .size(theme.text_base)
                                    .color(theme.text),
                            )
                            .fill(theme.border),
                        )
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        app.view_state.close_modal();
        app.task_store.viewing_task_id = None;
        app.task_store.viewing_task_result = None;
    }
}
