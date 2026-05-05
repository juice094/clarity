//! SubAgent progress panel — parallel batches + single-agent live tracking.
//!
//! Displays:
//! - Parallel batch status from Gateway polling.
//! - Single subagent live progress via channel.

use crate::App;

/// Render subagent progress content (parallel batches + single agents).
pub fn render_subagent_progress(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    let has_single = !app.subagent_store.running_agents.is_empty();
    let has_batch = !app.subagent_store.parallel_batches.is_empty();

    if !has_single && !has_batch {
        ui.vertical_centered(|ui| {
            ui.add_space(theme.space_8);
            ui.label(
                egui::RichText::new("No subagents running")
                    .size(theme.text_sm)
                    .color(theme.text_dim),
            );
        });
        return;
    }

    // ── Parallel batch progress ──
    if has_batch {
        ui.label(
            egui::RichText::new("Parallel Batches")
                .size(theme.text_sm)
                .strong()
                .color(theme.text),
        );
        ui.add_space(theme.space_4);

        let mut to_remove: Vec<usize> = Vec::new();

        for (idx, batch) in app.subagent_store.parallel_batches.iter().enumerate() {
            let is_finished = batch.status != "Running";
            let progress = if batch.total > 0 {
                batch.completed as f32 / batch.total as f32
            } else {
                0.0
            };

            ui.horizontal(|ui| {
                // Progress bar
                let pb_text = format!("{}/{} completed", batch.completed, batch.total);
                let mut pb = egui::ProgressBar::new(progress).text(pb_text);

                let fill_color = if batch.status == "Completed" {
                    theme.status_online
                } else if batch.failed > 0 {
                    theme.danger
                } else {
                    theme.accent
                };
                pb = pb.fill(fill_color);

                ui.add(pb);

                // Completed checkmark
                if batch.status == "Completed" {
                    ui.label(
                        egui::RichText::new(crate::theme::ICON_CHECK)
                            .font(theme.font_icon(theme.text_sm))
                            .color(theme.status_online),
                    );
                }
            });

            if batch.failed > 0 {
                ui.label(
                    egui::RichText::new(format!("{} failed", batch.failed))
                        .size(theme.text_xs)
                        .color(theme.danger),
                );
            }

            ui.add_space(theme.space_4);

            // Remove stale finished batches
            if is_finished && batch.last_poll.elapsed() > std::time::Duration::from_secs(30) {
                to_remove.push(idx);
            }
        }

        for idx in to_remove.into_iter().rev() {
            app.subagent_store.parallel_batches.remove(idx);
        }

        ui.add_space(theme.space_8);
    }

    // ── Single-agent live progress ──
    if has_single {
        ui.label(
            egui::RichText::new("Live Subagents")
                .size(theme.text_sm)
                .strong()
                .color(theme.text),
        );
        ui.add_space(theme.space_4);

        let mut to_remove_single: Vec<String> = Vec::new();

        for (agent_id, agent) in app.subagent_store.running_agents.iter() {
            let is_finished = agent.status == "Completed" || agent.status == "Failed";

            ui.horizontal(|ui| {
                // Agent type + truncated ID
                ui.label(
                    egui::RichText::new(format!(
                        "{} {}",
                        agent.agent_type,
                        &agent_id[..8.min(agent_id.len())]
                    ))
                    .size(theme.text_sm)
                    .strong()
                    .color(theme.text),
                );

                // Latest stage or status
                if let Some(stage) = agent.stages.last() {
                    ui.label(
                        egui::RichText::new(format!("· {}", stage))
                            .size(theme.text_xs)
                            .color(theme.text_muted),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(&agent.status)
                            .size(theme.text_xs)
                            .color(theme.text_muted),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(completed) = agent.completed_at {
                        let elapsed = completed.duration_since(agent.started_at).as_secs();
                        ui.label(
                            egui::RichText::new(format!("{}s", elapsed))
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("Running")
                                .size(theme.text_xs)
                                .color(theme.status_busy),
                        );
                    }
                });
            });

            ui.add_space(theme.space_4);

            // Mark for removal if finished and stale
            if is_finished
                && agent
                    .completed_at
                    .map(|t| t.elapsed() > std::time::Duration::from_secs(30))
                    .unwrap_or(false)
            {
                to_remove_single.push(agent_id.clone());
            }
        }

        for id in to_remove_single {
            app.subagent_store.running_agents.remove(&id);
        }
    }
}
