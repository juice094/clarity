//! Subagents panel — parallel batch + single-agent progress for the right IDE rail.
//!
//! Surfaces data already flowing through `SubAgentStore`:
//! - `parallel_batches` from Gateway `SubAgentBatch` events.
//! - `running_agents` from per-agent stage / output / status / progress / complete events.
//!
//! Completed agents stay in `running_agents` (with `completed_at` set) for the
//! app session, so they are rendered as a capped "recently completed" list
//! instead of disappearing.

use crate::App;
use crate::design_system::{self, BadgeVariant, Space, TextStyle};
use crate::ui::types::{SingleSubagentProgress, SubAgentProgress};

/// Maximum number of finished agents kept in the "recently completed" list.
const RECENT_COMPLETED_LIMIT: usize = 5;

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
    let (running, completed) = partition_agents(agents);

    egui::ScrollArea::vertical()
        .id_salt("subagents_panel")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            if batches.is_empty() && running.is_empty() && completed.is_empty() {
                render_empty_state(app, ui, &theme);
                return;
            }

            // ── Parallel batches ──
            if !batches.is_empty() {
                design_system::text(ui, app.t("Parallel Batches"), TextStyle::CaptionStrong);
                design_system::gap(ui, Space::S1);
                for batch in &batches {
                    render_batch_card(app, ui, batch, &theme);
                    design_system::gap(ui, Space::S2);
                }
                design_system::gap(ui, Space::S3);
            }

            // ── Running agents ──
            if !running.is_empty() {
                design_system::text(ui, app.t("Running Subagents"), TextStyle::CaptionStrong);
                design_system::gap(ui, Space::S1);
                for (agent_id, agent) in &running {
                    render_agent_card(app, ui, agent_id, agent, &theme);
                    design_system::gap(ui, Space::S2);
                }
            }

            // ── Recently completed ──
            if !completed.is_empty() {
                design_system::gap(ui, Space::S3);
                design_system::text(ui, app.t("Recently Completed"), TextStyle::CaptionStrong);
                design_system::gap(ui, Space::S1);
                for (agent_id, agent) in &completed {
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

fn render_batch_card(
    app: &App,
    ui: &mut egui::Ui,
    batch: &SubAgentProgress,
    theme: &crate::theme::Theme,
) {
    let total = batch.total.max(1);
    let done = batch.completed + batch.failed;
    let ratio = (done as f32) / (total as f32);
    let status_badge = status_badge_for(&batch.status);
    let status_label = status_label(app, &batch.status);

    design_system::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                design_system::text(
                    ui,
                    format!("{} {}", app.t("Batch"), truncate_id(&batch.batch_id, 8)),
                    TextStyle::Body,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    design_system::badge(ui, status_label, status_badge);
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
                            format!("{}: {}", app.t("Failed"), batch.failed),
                            TextStyle::Small,
                        );
                    });
                }
            });
        });
    });
}

fn render_agent_card(
    app: &App,
    ui: &mut egui::Ui,
    agent_id: &str,
    agent: &SingleSubagentProgress,
    theme: &crate::theme::Theme,
) {
    let status_badge = status_badge_for(&agent.status);
    let status_label = status_label(app, &agent.status);
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
                    design_system::badge(ui, status_label, status_badge);
                });
            });

            if let Some(done_at) = agent.completed_at {
                design_system::gap(ui, Space::S0);
                design_system::text(
                    ui,
                    format!("{}s", done_at.duration_since(agent.started_at).as_secs()),
                    TextStyle::Small,
                );
            }

            if agent.max_steps > 0 {
                design_system::gap(ui, Space::S1);
                progress_bar(ui, step_ratio, theme);
                design_system::gap(ui, Space::S0);
                design_system::text(
                    ui,
                    format!("{} {} / {}", app.t("Step"), agent.steps, agent.max_steps),
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

/// Map a backend status string to its i18n key.
///
/// Returns `None` for unknown states; the caller renders the raw status
/// untranslated.
// ponytail: extend the match when the backend gains new states.
fn status_i18n_key(status: &str) -> Option<&'static str> {
    match status.to_ascii_lowercase().as_str() {
        "completed" | "done" | "success" => Some("Completed"),
        "failed" | "error" => Some("Failed"),
        "running" => Some("Running"),
        "pending" => Some("Pending"),
        "cancelled" => Some("Cancelled"),
        _ => None,
    }
}

/// Translated badge label for a backend status string.
fn status_label<'a>(app: &App, status: &'a str) -> &'a str {
    match status_i18n_key(status) {
        Some(key) => app.t(key),
        None => status,
    }
}

/// One agent entry: `(agent_id, progress)`.
type AgentEntry = (String, SingleSubagentProgress);

/// Split agents into still-running and recently-completed lists.
///
/// `SubAgentStore` retains completed agents for the app session, so the
/// completed half doubles as the recent-results summary: newest first, capped
/// at [`RECENT_COMPLETED_LIMIT`]. Running agents are ordered by start time.
fn partition_agents(agents: Vec<AgentEntry>) -> (Vec<AgentEntry>, Vec<AgentEntry>) {
    let (mut running, mut completed): (Vec<_>, Vec<_>) = agents
        .into_iter()
        .partition(|(_, agent)| agent.completed_at.is_none());
    running.sort_by_key(|(_, agent)| agent.started_at);
    completed.sort_by_key(|(_, agent)| std::cmp::Reverse(agent.completed_at));
    completed.truncate(RECENT_COMPLETED_LIMIT);
    (running, completed)
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
    use std::time::{Duration, Instant};

    fn make_agent(
        status: &str,
        started_at: Instant,
        completed_at: Option<Instant>,
    ) -> SingleSubagentProgress {
        SingleSubagentProgress {
            agent_type: "coder".to_string(),
            status: status.to_string(),
            stages: vec![],
            output_lines: vec![],
            started_at,
            completed_at,
            steps: 0,
            max_steps: 0,
        }
    }

    #[test]
    fn status_badge_maps_common_states() {
        assert!(matches!(status_badge_for("Completed"), BadgeVariant::Ok));
        assert!(matches!(status_badge_for("Failed"), BadgeVariant::Danger));
        assert!(matches!(status_badge_for("Running"), BadgeVariant::Accent));
        assert!(matches!(status_badge_for("Unknown"), BadgeVariant::Neutral));
    }

    #[test]
    fn status_i18n_key_maps_known_states() {
        assert_eq!(status_i18n_key("Completed"), Some("Completed"));
        assert_eq!(status_i18n_key("done"), Some("Completed"));
        assert_eq!(status_i18n_key("error"), Some("Failed"));
        assert_eq!(status_i18n_key("running"), Some("Running"));
        assert_eq!(status_i18n_key("Pending"), Some("Pending"));
        assert_eq!(status_i18n_key("cancelled"), Some("Cancelled"));
        assert_eq!(status_i18n_key("some-new-state"), None);
    }

    #[test]
    fn partition_agents_splits_running_and_completed() {
        let now = Instant::now();
        let agents = vec![
            ("running-1".to_string(), make_agent("Running", now, None)),
            (
                "done-1".to_string(),
                make_agent("Completed", now, Some(now + Duration::from_secs(1))),
            ),
        ];
        let (running, completed) = partition_agents(agents);
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].0, "running-1");
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].0, "done-1");
    }

    #[test]
    fn partition_agents_caps_completed_newest_first() {
        let now = Instant::now();
        let agents = (0..7)
            .map(|i| {
                (
                    format!("done-{i}"),
                    make_agent("Completed", now, Some(now + Duration::from_secs(i + 1))),
                )
            })
            .collect();
        let (running, completed) = partition_agents(agents);
        assert!(running.is_empty());
        assert_eq!(completed.len(), RECENT_COMPLETED_LIMIT);
        assert_eq!(completed[0].0, "done-6");
        assert_eq!(completed[4].0, "done-2");
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
