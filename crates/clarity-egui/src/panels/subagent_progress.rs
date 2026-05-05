//! SubAgent progress panel — parallel batches + single-agent live tracking (IS-1 Sprint 30).
//!
//! Displays:
//! - Parallel batch status from Gateway polling (legacy).
//! - Single subagent live progress via channel (/coder, /explore shortcuts).

use crate::App;

/// Render subagent progress panel (embedded in Task Panel bottom or standalone sidebar)
pub fn render_subagent_progress(app: &mut App, ui: &mut egui::Ui) {
    let has_single = !app.subagent_store.running_agents.is_empty();
    let has_batch = !app.subagent_store.parallel_batches.is_empty();

    if !has_single && !has_batch {
        ui.vertical_centered(|ui| {
            ui.add_space(app.ui_store.theme.space_8);
            ui.label(
                egui::RichText::new("No subagents running")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text_dim),
            );
        });
        return;
    }

    // ── Single-agent live progress (Sprint 30) ──
    if has_single {
        ui.add_space(app.ui_store.theme.space_8);
        ui.label(
            egui::RichText::new("Live Subagents")
                .size(app.ui_store.theme.text_base)
                .strong()
                .color(app.ui_store.theme.text),
        );
        ui.add_space(app.ui_store.theme.space_4);

        let mut to_remove_single: Vec<String> = Vec::new();

        for (agent_id, agent) in app.subagent_store.running_agents.iter() {
            let is_finished = agent.status == "Completed" || agent.status == "Failed";

            egui::Frame::new()
                .fill(app.ui_store.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    // Header: type + status badge
                    ui.horizontal(|ui| {
                        let type_icon = match agent.agent_type.as_str() {
                            "coder" => crate::theme::ICON_CODE,
                            "explore" => crate::theme::ICON_SEARCH,
                            _ => crate::theme::ICON_ROBOT,
                        };
                        ui.label(
                            egui::RichText::new(type_icon)
                                .font(app.ui_store.theme.font_icon(app.ui_store.theme.text_sm)),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                agent.agent_type,
                                &agent_id[..8.min(agent_id.len())]
                            ))
                            .size(app.ui_store.theme.text_sm)
                            .strong()
                            .color(app.ui_store.theme.text),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (badge, color) = match agent.status.as_str() {
                                "Running" => ("Running", app.ui_store.theme.status_online),
                                "Completed" => ("Done", app.ui_store.theme.status_online),
                                "Failed" => ("Failed", app.ui_store.theme.danger),
                                _ => ("Pending", app.ui_store.theme.text_dim),
                            };
                            ui.label(
                                egui::RichText::new(badge)
                                    .size(app.ui_store.theme.text_xs)
                                    .color(color),
                            );
                        });
                    });

                    // Stage log (collapsible, latest 3 visible by default)
                    if !agent.stages.is_empty() {
                        ui.add_space(app.ui_store.theme.space_4);
                        let latest = agent.stages.iter().rev().take(3).rev();
                        for stage in latest {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(crate::theme::ICON_CHEVRON_RIGHT)
                                        .font(app.ui_store.theme.font_icon(app.ui_store.theme.text_xs))
                                        .color(app.ui_store.theme.accent),
                                );
                                ui.label(
                                    egui::RichText::new(stage)
                                        .size(app.ui_store.theme.text_xs)
                                        .color(app.ui_store.theme.text_dim),
                                );
                            });
                        }
                        if agent.stages.len() > 3 {
                            ui.label(
                                egui::RichText::new(format!("… and {} more", agent.stages.len() - 3))
                                    .size(app.ui_store.theme.text_xs)
                                    .color(app.ui_store.theme.text_dim),
                            );
                        }
                    }

                    // Output preview (last line only, to keep UI compact)
                    if let Some(last) = agent.output_lines.last() {
                        ui.add_space(app.ui_store.theme.space_4);
                        let truncated: String = last.chars().take(60).collect();
                        ui.label(
                            egui::RichText::new(truncated)
                                .size(app.ui_store.theme.text_xs)
                                .color(app.ui_store.theme.text_dim)
                                .italics(),
                        );
                    }

                    // Elapsed time
                    let elapsed = if let Some(completed) = agent.completed_at {
                        completed.duration_since(agent.started_at).as_secs()
                    } else {
                        agent.started_at.elapsed().as_secs()
                    };
                    ui.add_space(app.ui_store.theme.space_4);
                    ui.label(
                        egui::RichText::new(format!("{}s elapsed", elapsed))
                            .size(app.ui_store.theme.text_xs)
                            .color(app.ui_store.theme.text_dim),
                    );

                    // Mark for removal if finished and stale
                    if is_finished
                        && agent
                            .completed_at
                            .map(|t| t.elapsed() > std::time::Duration::from_secs(30))
                            .unwrap_or(false)
                    {
                        to_remove_single.push(agent_id.clone());
                    }
                });
            ui.add_space(app.ui_store.theme.space_4);
        }

        for id in to_remove_single {
            app.subagent_store.running_agents.remove(&id);
        }
    }

    // ── Parallel batch progress (legacy Gateway polling) ──
    if has_batch {
        ui.add_space(app.ui_store.theme.space_8);
        ui.label(
            egui::RichText::new("Parallel Batches")
                .size(app.ui_store.theme.text_base)
                .strong()
                .color(app.ui_store.theme.text),
        );
        ui.add_space(app.ui_store.theme.space_4);

        let mut to_remove: Vec<usize> = Vec::new();

        for (idx, batch) in app.subagent_store.parallel_batches.iter().enumerate() {
            let is_finished = batch.status != "Running";

            egui::Frame::new()
                .fill(app.ui_store.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    // Header: batch_id + overall status
                    ui.horizontal(|ui| {
                        let icon = match batch.status.as_str() {
                            "Running" => crate::theme::ICON_HOURGLASS,
                            "Completed" => crate::theme::ICON_CHECK,
                            "Failed" => crate::theme::ICON_X,
                            _ => crate::theme::ICON_QUESTION,
                        };
                        ui.label(
                            egui::RichText::new(icon)
                                .font(app.ui_store.theme.font_icon(app.ui_store.theme.text_sm)),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "Batch {}",
                                &batch.batch_id[..8.min(batch.batch_id.len())]
                            ))
                            .size(app.ui_store.theme.text_sm)
                            .strong()
                            .color(app.ui_store.theme.text),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(&batch.status)
                                    .size(app.ui_store.theme.text_xs)
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
                    let (_pb_id, pb_resp) =
                        ui.allocate_exact_size(egui::vec2(pb_width, pb_height), egui::Sense::hover());
                    let pb_rect = pb_resp.rect;
                    ui.painter().rect_filled(
                        pb_rect,
                        egui::CornerRadius::same(3),
                        app.ui_store.theme.bg_elevated,
                    );
                    if progress > 0.0 {
                        let fill_w = pb_rect.width() * progress;
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(pb_rect.min, egui::vec2(fill_w, pb_height)),
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
                        .size(app.ui_store.theme.text_xs)
                        .color(app.ui_store.theme.text_dim),
                    );

                    // Agent status list
                    ui.add_space(app.ui_store.theme.space_4);
                    for agent in &batch.agent_statuses {
                        let (icon, color) = match agent.status.as_str() {
                            "Running" => (crate::theme::ICON_PLAY, app.ui_store.theme.status_online),
                            "Completed" => (crate::theme::ICON_CHECK, app.ui_store.theme.status_online),
                            "Failed" => (crate::theme::ICON_X, app.ui_store.theme.danger),
                            _ => (crate::theme::ICON_HOURGLASS, app.ui_store.theme.text_dim),
                        };
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(icon)
                                    .font(app.ui_store.theme.font_icon(app.ui_store.theme.text_xs)),
                            );
                            ui.label(
                                egui::RichText::new(&agent.agent_id)
                                    .size(app.ui_store.theme.text_xs)
                                    .color(color),
                            );
                            if let Some(ref summary) = agent.summary {
                                let truncated: String = summary.chars().take(40).collect();
                                ui.label(
                                    egui::RichText::new(truncated)
                                        .size(app.ui_store.theme.text_xs)
                                        .color(app.ui_store.theme.text_dim),
                                );
                            }
                        });
                    }

                    // Remove completed/failed batches after showing them
                    if is_finished && batch.last_poll.elapsed() > std::time::Duration::from_secs(30) {
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
}
