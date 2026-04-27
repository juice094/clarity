use crate::theme::Theme;
use clarity_core::background::{TaskInfo, TaskStatus};

/// Render the task list inside a SidePanel or Window.
pub fn render_task_panel(ui: &mut egui::Ui, tasks: &[TaskInfo], theme: &Theme) {
    if tasks.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                egui::RichText::new("No tasks yet")
                    .size(13.0)
                    .color(theme.text_dim),
            );
        });
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for task in tasks {
            let (icon, status_color) = match task.status {
                TaskStatus::Pending => ("⏳", theme.status_busy),
                TaskStatus::Running => ("▶", theme.status_online),
                TaskStatus::Completed => ("✅", theme.status_online),
                TaskStatus::Failed => ("❌", theme.danger),
                TaskStatus::Cancelled => ("🚫", theme.text_dim),
            };

            egui::Frame::group(ui.style())
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .stroke(egui::Stroke::new(1.0, theme.border))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(icon).size(12.0));
                        ui.label(
                            egui::RichText::new(&task.spec.name)
                                .size(12.0)
                                .strong()
                                .color(theme.text),
                        );
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(task.status.as_str())
                                        .size(10.0)
                                        .color(status_color),
                                );
                            },
                        );
                    });
                    if !task.spec.description.is_empty() {
                        ui.label(
                            egui::RichText::new(&task.spec.description)
                                .size(11.0)
                                .color(theme.text_dim),
                        );
                    }
                    ui.horizontal(|ui| {
                        let priority_label = format!("{:?}", task.spec.priority);
                        ui.label(
                            egui::RichText::new(priority_label)
                                .size(10.0)
                                .color(theme.text_dim),
                        );
                        ui.label(
                            egui::RichText::new(format_timestamp(task.created_at))
                                .size(10.0)
                                .color(theme.text_dim),
                        );
                    });
                });
            ui.add_space(theme.space_4);
        }
    });
}

fn format_timestamp(ts: u64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::from_timestamp(ts as i64, 0).unwrap_or_else(|| Utc::now());
    dt.format("%Y-%m-%d %H:%M").to_string()
}
