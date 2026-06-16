//! Task Board — full-screen main view for background task management.
//!
//! Sprint 40 MVP: list view with status badges. Kanban columns deferred.

use crate::App;
use clarity_core::background::TaskStatus;

/// Render the full-screen Task Board as the active main view.
pub fn render_task_board(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(theme.bg)
                .inner_margin(egui::Margin::same(16)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Task Board")
                        .size(theme.text_2xl)
                        .strong()
                        .color(theme.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            egui::RichText::new("+ New Task")
                                .size(theme.text_sm)
                                .color(theme.text),
                        )
                        .clicked()
                    {
                        app.view_state
                            .open_modal(clarity_core::ui::ModalType::TaskCreate);
                    }
                });
            });
            ui.add_space(theme.space_16);

            if app.task_store.tasks.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(120.0);
                    ui.label(
                        egui::RichText::new("No background tasks yet.")
                            .size(theme.text_lg)
                            .color(theme.text_dim),
                    );
                    ui.add_space(theme.space_8);
                    ui.label(
                        egui::RichText::new("Create a task from the sidebar or press + New Task.")
                            .size(theme.text_sm)
                            .color(theme.text_muted),
                    );
                });
                return;
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for task in &app.task_store.tasks {
                        task_card(ui, &theme, task);
                        ui.add_space(theme.space_8);
                    }
                });
        });
}

fn task_card(
    ui: &mut egui::Ui,
    theme: &crate::theme::Theme,
    task: &clarity_core::background::TaskInfo,
) {
    let (status_label, status_color) = match task.status {
        TaskStatus::Pending => ("Pending", theme.status_busy),
        TaskStatus::Running => ("Running", theme.status_busy),
        TaskStatus::Completed => ("Completed", theme.status_online),
        TaskStatus::Failed => ("Failed", theme.status_offline),
        TaskStatus::Cancelled => ("Cancelled", theme.text_dim),
    };

    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .stroke(egui::Stroke::new(1.0_f32, theme.border))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&task.spec.name)
                        .size(theme.text_base)
                        .strong()
                        .color(theme.text),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    status_badge(ui, theme, status_label, status_color);
                });
            });

            if !task.spec.description.is_empty() {
                ui.add_space(theme.space_4);
                ui.label(
                    egui::RichText::new(&task.spec.description)
                        .size(theme.text_sm)
                        .color(theme.text_muted),
                );
            }

            ui.add_space(theme.space_4);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Priority: {}", task.spec.priority.value()))
                        .size(theme.text_xs)
                        .color(theme.text_dim),
                );
                if task.created_at > 0 {
                    let dt = chrono::DateTime::from_timestamp(task.created_at as i64, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| format!("{}", task.created_at));
                    ui.label(
                        egui::RichText::new(format!("Created: {}", dt))
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    );
                }
            });
        });
}

fn status_badge(ui: &mut egui::Ui, theme: &crate::theme::Theme, label: &str, color: egui::Color32) {
    let padding = egui::vec2(8.0, 4.0);
    let text = egui::RichText::new(label)
        .size(theme.text_xs)
        .strong()
        .color(theme.bg);
    let galley = ui.painter().layout(
        text.text().to_string(),
        theme.font(theme.text_xs),
        theme.bg,
        ui.available_width(),
    );
    let size = egui::vec2(
        galley.size().x + padding.x * 2.0,
        galley.size().y + padding.y * 2.0,
    );
    let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());

    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(theme.radius_sm as u8), color);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        theme.font(theme.text_xs),
        theme.bg,
    );
}
