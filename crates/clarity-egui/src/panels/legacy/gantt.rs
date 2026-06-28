//! Plan Gantt chart panel — simplified vertical step timeline.
//!
//! Sprint 39: Window-style popup showing plan step status as horizontal bars.

use crate::App;
use crate::ui::types::PlanStepStatus;

/// Render the Plan Timeline Gantt chart as a resizable window popup.
pub fn render_gantt_panel(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();

    let screen = ctx.screen_rect();
    let default_w = 600.0_f32.min(screen.width() * 0.85);
    let default_h = 400.0_f32.min(screen.height() * 0.8);
    egui::Window::new("Plan Timeline")
        .default_size([default_w, default_h])
        .max_size([screen.width() * 0.95, screen.height() * 0.9])
        .resizable(true)
        .collapsible(false)
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(theme.surface)
                .stroke(egui::Stroke::new(1.0_f32, theme.border))
                .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
                .inner_margin(egui::Margin::same(16)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new("Plan Timeline")
                        .size(theme.text_lg)
                        .strong()
                        .color(theme.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            egui::RichText::new(crate::theme::ICON_X)
                                .font(theme.font_icon(theme.text_sm))
                                .color(theme.text_dim),
                        )
                        .clicked()
                    {
                        app.view_state.main = clarity_core::ui::AppView::Chat;
                    }
                });
            });
            crate::design_system::gap(ui, crate::design_system::Space::S2);

            let has_plan =
                app.chat_store.pending_plan.is_some() || app.chat_store.plan_tracker.is_some();

            if !has_plan {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.label(
                        egui::RichText::new("No active plan. Start a plan with /plan.")
                            .size(theme.text_sm)
                            .color(theme.text_dim),
                    );
                });
                return;
            }

            // Determine title and steps to display
            let title = if let Some(ref tracker) = app.chat_store.plan_tracker {
                tracker.title.clone()
            } else if let Some(ref plan) = app.chat_store.pending_plan {
                plan.title.clone()
            } else {
                String::new()
            };

            if !title.is_empty() {
                ui.label(
                    egui::RichText::new(&title)
                        .size(theme.text_sm)
                        .strong()
                        .color(theme.text),
                );
                crate::design_system::gap(ui, crate::design_system::Space::S1);
            }

            // Use tracker steps if available; otherwise fall back to pending plan steps.
            // Note: pending_plan steps don't have live status, so we treat them all as Pending.
            let steps: Vec<GanttStep> = if let Some(ref tracker) = app.chat_store.plan_tracker {
                tracker
                    .steps
                    .iter()
                    .map(|s| GanttStep {
                        id: s.id.clone(),
                        description: s.description.clone(),
                        status: s.status,
                    })
                    .collect()
            } else if let Some(ref plan) = app.chat_store.pending_plan {
                plan.steps
                    .iter()
                    .map(|s| GanttStep {
                        id: s.id.clone(),
                        description: s.description.clone(),
                        status: PlanStepStatus::Pending,
                    })
                    .collect()
            } else {
                vec![]
            };

            if steps.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.label(
                        egui::RichText::new("No active plan. Start a plan with /plan.")
                            .size(theme.text_sm)
                            .color(theme.text_dim),
                    );
                });
                return;
            }

            // ── Timeline header ──
            let avail = ui.available_width();
            let label_col_w = 160.0_f32.min(avail * 0.35);
            let bar_col_w = avail - label_col_w - 80.0; // reserve space for status label
            let bar_height = 24.0;
            let row_gap = 8.0;

            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(label_col_w, 0.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.label(
                            egui::RichText::new("Step")
                                .size(theme.text_xs)
                                .strong()
                                .color(theme.text_muted),
                        );
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(bar_col_w, 0.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.label(
                            egui::RichText::new("Timeline")
                                .size(theme.text_xs)
                                .strong()
                                .color(theme.text_muted),
                        );
                    },
                );
            });
            crate::design_system::gap(ui, crate::design_system::Space::S0);
            ui.separator();
            crate::design_system::gap(ui, crate::design_system::Space::S0);

            // ── Scrollable Gantt rows ──
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let mut prev_bar_center: Option<egui::Pos2> = None;

                    for (idx, step) in steps.iter().enumerate() {
                        let _row_y = ui.cursor().min.y;

                        // Layout row manually to capture bar position for connector lines
                        let row_rect = ui.available_rect_before_wrap();
                        let row_rect = egui::Rect::from_min_size(
                            row_rect.min,
                            egui::vec2(row_rect.width(), bar_height),
                        );

                        // Label column
                        let label_rect = egui::Rect::from_min_size(
                            row_rect.min,
                            egui::vec2(label_col_w, bar_height),
                        );
                        let step_num = idx + 1;
                        let desc = if step.description.chars().count() > 30 {
                            let truncated: String = step.description.chars().take(27).collect();
                            format!("{}...", truncated)
                        } else {
                            step.description.clone()
                        };
                        ui.painter().text(
                            egui::pos2(label_rect.min.x, label_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("Step {}: {}", step_num, desc),
                            theme.font(theme.text_sm),
                            theme.text,
                        );

                        // Bar column
                        let bar_x = row_rect.min.x + label_col_w;
                        let bar_w = 200.0_f32.min(bar_col_w);
                        let bar_rect = egui::Rect::from_min_size(
                            egui::pos2(bar_x, row_rect.min.y),
                            egui::vec2(bar_w, bar_height),
                        );
                        let color = step_color(step.status, &theme);
                        let radius = egui::CornerRadius::same(theme.radius_sm as u8);
                        ui.painter().rect_filled(bar_rect, radius, color);

                        // Status label
                        let status_x = bar_x + bar_w + 8.0;
                        let (status_text, status_color) = step_status_text(step.status, &theme);
                        ui.painter().text(
                            egui::pos2(status_x, row_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            status_text,
                            theme.font(theme.text_xs),
                            status_color,
                        );

                        // Connection line to next step (simple sequential dependency)
                        let bar_center =
                            egui::pos2(bar_rect.min.x + bar_w * 0.5, row_rect.center().y);
                        if let Some(prev) = prev_bar_center {
                            let from =
                                egui::pos2(prev.x, prev.y + bar_height * 0.5 + row_gap * 0.3);
                            let to = egui::pos2(bar_center.x, bar_center.y - bar_height * 0.5);
                            ui.painter()
                                .line_segment([from, to], egui::Stroke::new(1.5, theme.border));
                        }
                        prev_bar_center = Some(bar_center);

                        ui.allocate_rect(row_rect, egui::Sense::hover());
                        ui.add_space(row_gap);
                    }
                });
        });
}

struct GanttStep {
    #[allow(dead_code)]
    id: String,
    description: String,
    status: PlanStepStatus,
}

fn step_color(status: PlanStepStatus, theme: &crate::theme::Theme) -> egui::Color32 {
    match status {
        PlanStepStatus::Pending => theme.text_dim,
        PlanStepStatus::Running => theme.status_busy,
        PlanStepStatus::Success => theme.status_online,
        PlanStepStatus::Failed => theme.status_offline,
        PlanStepStatus::Skipped => theme.bg_hover,
    }
}

fn step_status_text(
    status: PlanStepStatus,
    theme: &crate::theme::Theme,
) -> (&'static str, egui::Color32) {
    match status {
        PlanStepStatus::Pending => ("Pending", theme.text_dim),
        PlanStepStatus::Running => ("Running", theme.status_busy),
        PlanStepStatus::Success => ("Completed", theme.status_online),
        PlanStepStatus::Failed => ("Failed", theme.status_offline),
        PlanStepStatus::Skipped => ("Skipped", theme.text_muted),
    }
}
