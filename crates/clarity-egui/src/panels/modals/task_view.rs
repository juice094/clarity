//! Task result view modal — displays the output of a completed background task.

use crate::App;
use clarity_ui::design_system::{Space, TextStyle, code_frame, gap, spinner, text};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::modal::Modal;

/// Renders the task view modal UI using the Clarity Design Protocol.
///
/// The modal shell itself (scrim + frame + centering) is owned by
/// `clarity_ui::widgets::modal`; this function only renders the content.
pub fn render_task_view_modal(app: &mut App, ctx: &egui::Context) {
    if app.current_modal() != Some(&clarity_core::ui::ModalType::TaskView) {
        return;
    }

    let mut close_requested = false;
    let theme = &app.context.ui_store.theme;

    Modal::new("task_view")
        .width(420.0)
        .max_height(600.0)
        .show(ctx, |ui| {
            text(ui, "Task Result", TextStyle::Title);
            gap(ui, Space::S2);

            if let Some(ref result) = app.task_store().viewing_task_result {
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
                            text(
                                ui,
                                format!(
                                    "{:.1}s · {} steps",
                                    result.elapsed_ms as f64 / 1000.0,
                                    result.steps
                                ),
                                TextStyle::Small,
                            );
                        }
                    });
                });
                gap(ui, Space::S1);

                // Output text
                code_frame(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(400.0)
                        .show(ui, |ui| {
                            text(ui, &result.output, TextStyle::Mono);
                        });
                });
            } else {
                ui.vertical_centered(|ui| {
                    gap(ui, Space::S6);
                    text(ui, "Loading result...", TextStyle::Body);
                    gap(ui, Space::S1);
                    spinner(ui);
                });
            }

            gap(ui, Space::S2);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(Button::new("Close").ghost().width(80.0)).clicked() {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        app.close_modal();
        app.task_store_mut().viewing_task_id = None;
        app.task_store_mut().viewing_task_result = None;
    }
}
