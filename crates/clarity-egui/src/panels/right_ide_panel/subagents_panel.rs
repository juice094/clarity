//! Subagents panel — parallel batch + single-agent progress for the right IDE rail.
//!
//! Surfaces data already flowing through `SubAgentStore`:
//! - `parallel_batches` from Gateway `SubAgentBatch` events.
//! - `running_agents` from per-agent stage / output / status / progress / complete events.

use crate::App;
use crate::design_system::{self, BadgeVariant, Space, TextStyle};
use crate::ui::types::{SingleSubagentProgress, SubAgentProgress};

/// Render the subagents panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();
    let batches = app.subagent_store().parallel_batches.clone();
    let agents: Vec<(String, SingleSubagentProgress)> = app
        .subagent_store()
        .running_agents
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    egui::ScrollArea::vertical()
        .id_salt("subagents_panel")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            // ── Parallel batches ──
            if batches.is_empty() && agents.is_empty() {
                render_empty_state(app, ui, &theme);
                return;
            }

            if !batches.is_empty() {
                design_system::text(ui, app.t("Parallel Batches"), TextStyle::CaptionStrong);
                design_system::gap(ui, Space::S1);
                for batch in &batches {
                    render_batch_card(ui, batch, &theme);
                    design_system::gap(ui, Space::S2);
                }
                design_system::gap(ui, Space::S3);
            }

            // ── Single agents ──
            if !agents.is_empty() {
                design_system::text(ui, app.t("Running Agents"), TextStyle::CaptionStrong);
                design_system::gap(ui, Space::S1);
                for (agent_id, agent) in &agents {
                    render_agent_card(app, ui, agent_id, agent, &theme);
                    design_system::gap(ui, Space::S2);
                }
            }
        });
}

fn render_empty_state(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    ui.vertical_centered(|ui| {
        ui.add_space(theme.space_24);
        design_system::text(ui, app.t("No subagent activity"), TextStyle::Subheading);
        design_system::gap(ui, Space::S1);
        design_system::text(
            ui,
            app.t("Run /coder, /explore or a parallel batch to see progress here."),
            TextStyle::Small,
        );
    });
}

fn render_batch_card(ui: &mut egui::Ui, batch: &SubAgentProgress, theme: &crate::theme::Theme) {
    let total = batch.total.max(1);
    let done = batch.completed + batch.failed;
    let ratio = (done as f32) / (total as f32);
    let status_badge = status_badge_for(&batch.status);

    design_system::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                design_system::text(
                    ui,
                    format!("Batch {}", truncate_id(&batch.batch_id, 8)),
                    TextStyle::Body,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    design_system::badge(ui, &batch.status, status_badge);
                });
            });

            design_system::gap(ui, Space::S1);
            progress_bar(ui, ratio, theme);
            design_system::gap(ui, Space::S1);

            ui.horizontal(|ui| {
                design_system::text(ui, format!("{} / {}", done, total), TextStyle::Small);
                if batch.failed > 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        design_system::text(
                            ui,
                            format!("{} failed", batch.failed),
                            TextStyle::Small,
                        );
                    });
                }
            });
        });
    });
}

fn render_agent_card(
    app: &mut App,
    ui: &mut egui::Ui,
    agent_id: &str,
    agent: &SingleSubagentProgress,
    theme: &crate::theme::Theme,
) {
    let status_badge = status_badge_for(&agent.status);
    let max_steps = agent.max_steps.max(1);
    let step_ratio = (agent.steps as f32) / (max_steps as f32);

    design_system::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(crate::theme::ICON_BOT)
                        .font(theme.font_icon(theme.text_sm))
                        .color(theme.accent),
                );
                design_system::gap(ui, Space::S0);
                design_system::text(
                    ui,
                    format!("{} • {}", agent.agent_type, truncate_id(agent_id, 8)),
                    TextStyle::Body,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    design_system::badge(ui, &agent.status, status_badge);
                });
            });

            if agent.max_steps > 0 {
                design_system::gap(ui, Space::S1);
                progress_bar(ui, step_ratio, theme);
                design_system::gap(ui, Space::S0);
                design_system::text(
                    ui,
                    format!("Step {} / {}", agent.steps, agent.max_steps),
                    TextStyle::Small,
                );
            }

            if !agent.stages.is_empty() {
                design_system::gap(ui, Space::S1);
                let latest = agent
                    .stages
                    .iter()
                    .rev()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>();
                design_system::text(ui, app.t("Stages"), TextStyle::Small);
                for stage in latest.iter().rev() {
                    design_system::text(ui, format!("• {}", stage), TextStyle::Small);
                }
            }

            if !agent.output_lines.is_empty() {
                design_system::gap(ui, Space::S1);
                let latest = agent
                    .output_lines
                    .iter()
                    .rev()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>();
                design_system::text(ui, app.t("Latest output"), TextStyle::Small);
                for line in latest.iter().rev() {
                    design_system::text(ui, truncate_line(line, 80), TextStyle::Small);
                }
            }
        });
    });
}

fn progress_bar(ui: &mut egui::Ui, fraction: f32, theme: &crate::theme::Theme) {
    let desired_height = theme.space_4;
    let desired_width = ui.available_width();
    let (rect, _response) = ui.allocate_exact_size(
        egui::vec2(desired_width, desired_height),
        egui::Sense::hover(),
    );
    if ui.is_rect_visible(rect) {
        let radius = (theme.radius_sm * 0.5) as u8;
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(radius), theme.surface_strong);
        let fill_w = (rect.width() * fraction.clamp(0.0, 1.0)).max(1.0);
        if fill_w > 0.0 {
            let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
            ui.painter()
                .rect_filled(fill_rect, egui::CornerRadius::same(radius), theme.accent);
        }
    }
}

fn status_badge_for(status: &str) -> BadgeVariant {
    match status.to_ascii_lowercase().as_str() {
        "completed" | "done" | "success" => BadgeVariant::Ok,
        "failed" | "error" => BadgeVariant::Danger,
        "running" | "pending" => BadgeVariant::Accent,
        "cancelled" => BadgeVariant::Warn,
        _ => BadgeVariant::Neutral,
    }
}

fn truncate_id(id: &str, max_len: usize) -> String {
    if id.len() <= max_len {
        id.to_string()
    } else {
        format!("{}…", &id[..max_len])
    }
}

fn truncate_line(line: &str, max_chars: usize) -> String {
    if line.chars().count() <= max_chars {
        line.to_string()
    } else {
        format!("{}…", line.chars().take(max_chars).collect::<String>())
    }
}

// ── Panel trait implementation ──

/// Subagents panel renderer.
pub struct SubagentsPanel;

impl crate::design_system::Panel for SubagentsPanel {
    fn title(&self, app: &crate::App) -> &str {
        app.t("Subagents")
    }
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        render(app, ui);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_badge_maps_common_states() {
        assert!(matches!(status_badge_for("Completed"), BadgeVariant::Ok));
        assert!(matches!(status_badge_for("Failed"), BadgeVariant::Danger));
        assert!(matches!(status_badge_for("Running"), BadgeVariant::Accent));
        assert!(matches!(status_badge_for("Unknown"), BadgeVariant::Neutral));
    }

    #[test]
    fn truncate_id_respects_max_len() {
        assert_eq!(truncate_id("abcdef", 8), "abcdef");
        assert_eq!(truncate_id("abcdefghij", 8), "abcdefgh…");
    }

    #[test]
    fn truncate_line_respects_max_chars() {
        assert_eq!(truncate_line("short", 10), "short");
        assert!(truncate_line("a".repeat(100).as_str(), 80).ends_with('…'));
    }
}
