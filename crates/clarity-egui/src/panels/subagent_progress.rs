//! SubAgent 并行执行进度面板
//!
//! 显示每个并行批处理中各个子代理的实时状态（Pending/Running/Completed/Failed）。
//! 数据通过轮询 Gateway `/v1/parallel/:batch_id/status` 获取。

use crate::App;

/// 渲染子代理进度面板（嵌入在 Task Panel 底部或独立侧栏）
pub fn render_subagent_progress(app: &mut App, ui: &mut egui::Ui) {
    if app.subagent_store.parallel_batches.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(app.ui_store.theme.space_8);
            ui.label(
                egui::RichText::new("No parallel batches running")
                    .size(11.0)
                    .color(app.ui_store.theme.text_dim),
            );
        });
        return;
    }

    ui.add_space(app.ui_store.theme.space_8);
    ui.label(
        egui::RichText::new("Parallel Batches")
            .size(13.0)
            .strong()
            .color(app.ui_store.theme.text),
    );
    ui.add_space(app.ui_store.theme.space_4);

    let mut to_remove: Vec<usize> = Vec::new();

    for (idx, batch) in app.subagent_store.parallel_batches.iter().enumerate() {
        let is_finished = batch.status != "Running";

        egui::Frame::group(ui.style())
            .fill(app.ui_store.theme.surface)
            .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
            .stroke(egui::Stroke::new(1.0, app.ui_store.theme.border))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // Header: batch_id + overall status
                ui.horizontal(|ui| {
                    let icon = match batch.status.as_str() {
                        "Running" => "⏳",
                        "Completed" => "✅",
                        "Failed" => "❌",
                        _ => "❓",
                    };
                        ui.label(egui::RichText::new(icon).size(12.0));
                        ui.label(
                            egui::RichText::new(format!(
                                "Batch {}",
                                &batch.batch_id[..8.min(batch.batch_id.len())]
                            ))
                            .size(12.0)
                            .strong()
                            .color(app.ui_store.theme.text),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(&batch.status)
                                    .size(10.0)
                                    .color(match batch.status.as_str() {
                                        "Running" => app.ui_store.theme.status_online,
                                        "Completed" => app.ui_store.theme.status_online,
                                        "Failed" => app.ui_store.theme.danger,
                                        _ => app.ui_store.theme.text_dim,
                                    }),
                            );
                        });
                    });

                    // Progress bar
                    ui.add_space(app.ui_store.theme.space_4);
                    let progress = if batch.total > 0 {
                        (batch.completed + batch.failed) as f32 / batch.total as f32
                    } else {
                        0.0
                    };
                    let pb_width = ui.available_width();
                    let pb_height = 6.0;
                    let (_pb_id, pb_resp) = ui.allocate_exact_size(
                        egui::vec2(pb_width, pb_height),
                        egui::Sense::hover(),
                    );
                    let pb_rect = pb_resp.rect;
                    ui.painter().rect_filled(
                        pb_rect,
                        egui::CornerRadius::same(3),
                        app.ui_store.theme.bg_elevated,
                    );
                    if progress > 0.0 {
                        let fill_w = pb_rect.width() * progress;
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                pb_rect.min,
                                egui::vec2(fill_w, pb_height),
                            ),
                            egui::CornerRadius::same(3),
                            if batch.status == "Failed" {
                                app.ui_store.theme.danger
                            } else {
                                app.ui_store.theme.accent
                            },
                        );
                    }

                    ui.add_space(app.ui_store.theme.space_4);
                    ui.label(
                        egui::RichText::new(format!(
                            "{}/{} completed · {} failed · {}ms",
                            batch.completed, batch.total, batch.failed, batch.elapsed_ms
                        ))
                        .size(10.0)
                        .color(app.ui_store.theme.text_dim),
                    );

                    // Agent status list
                    ui.add_space(app.ui_store.theme.space_4);
                    for agent in &batch.agent_statuses {
                        let (icon, color) = match agent.status.as_str() {
                            "Running" => ("▶", app.ui_store.theme.status_online),
                            "Completed" => ("✅", app.ui_store.theme.status_online),
                            "Failed" => ("❌", app.ui_store.theme.danger),
                            _ => ("⏳", app.ui_store.theme.text_dim),
                        };
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(icon).size(10.0));
                            ui.label(
                                egui::RichText::new(&agent.agent_id)
                                    .size(10.0)
                                    .color(color),
                            );
                            if let Some(ref summary) = agent.summary {
                                let truncated: String = summary.chars().take(40).collect();
                                ui.label(
                                    egui::RichText::new(truncated)
                                        .size(9.0)
                                        .color(app.ui_store.theme.text_dim),
                                );
                            }
                        });
                    }

                    // Remove completed/failed batches after showing them
                    if is_finished && batch.last_poll.elapsed() > std::time::Duration::from_secs(30)
                    {
                        to_remove.push(idx);
                    }
                });
            ui.add_space(app.ui_store.theme.space_4);
        }

        // Remove stale entries (reverse order to preserve indices)
        for idx in to_remove.into_iter().rev() {
            app.subagent_store.parallel_batches.remove(idx);
        }
}
