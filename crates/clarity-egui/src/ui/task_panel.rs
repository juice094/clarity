use crate::theme::Theme;
use clarity_core::background::{TaskInfo, TaskStatus};

/// Actions emitted by the task panel UI.
pub enum TaskPanelAction {
    None,
    Cancel(String),
}

/// Render the task list inside a SidePanel or Window.
/// Returns any user action (e.g. cancel request) for the caller to handle.
pub fn render_task_panel(ui: &mut egui::Ui, tasks: &[TaskInfo], theme: &Theme) -> TaskPanelAction {
    if tasks.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(theme.space_40);
            ui.label(
                egui::RichText::new("No tasks yet")
                    .size(theme.text_base)
                    .color(theme.text_dim),
            );
        });
        return TaskPanelAction::None;
    }

    let mut action = TaskPanelAction::None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for task in tasks {
            let (icon, status_color) = match task.status {
                TaskStatus::Pending => (crate::theme::ICON_HOURGLASS, theme.status_busy),
                TaskStatus::Running => (crate::theme::ICON_PLAY, theme.status_online),
                TaskStatus::Completed => (crate::theme::ICON_CHECK, theme.status_online),
                TaskStatus::Failed => (crate::theme::ICON_X, theme.danger),
                TaskStatus::Cancelled => (crate::theme::ICON_PROHIBIT, theme.text_dim),
            };

            egui::Frame::group(ui.style())
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .stroke(egui::Stroke::new(1.0, theme.border))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(icon).font(theme.font_icon(theme.text_sm)));
                        ui.label(
                            egui::RichText::new(&task.spec.name)
                                .size(theme.text_sm)
                                .strong()
                                .color(theme.text),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !task.status.is_terminal() {
                                if ui
                                    .add(
                                        egui::Button::new(egui::RichText::new("Cancel").size(theme.text_xs))
                                            .fill(theme.danger)
                                            .corner_radius(egui::CornerRadius::same(
                                                theme.radius_sm as u8,
                                            )),
                                    )
                                    .clicked()
                                {
                                    action = TaskPanelAction::Cancel(task.id.clone());
                                }
                                ui.add_space(theme.space_4);
                            }
                            ui.label(
                                egui::RichText::new(task.status.as_str())
                                    .size(theme.text_xs)
                                    .color(status_color),
                            );
                        });
                    });
                    if !task.spec.description.is_empty() {
                        ui.label(
                            egui::RichText::new(&task.spec.description)
                                .size(theme.text_sm)
                                .color(theme.text_dim),
                        );
                    }
                    ui.horizontal(|ui| {
                        let priority_label = format!("{:?}", task.spec.priority);
                        ui.label(
                            egui::RichText::new(priority_label)
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                        );
                        ui.label(
                            egui::RichText::new(format_timestamp(task.created_at))
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                        );
                    });
                });
            ui.add_space(theme.space_4);
        }
    });
    action
}

fn format_timestamp(ts: u64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::from_timestamp(ts as i64, 0).unwrap_or_else(Utc::now);
    dt.format("%Y-%m-%d %H:%M").to_string()
}
