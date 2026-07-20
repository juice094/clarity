//! Snapshot History Modal — workspace snapshot browsing, diff preview, and restore.
//!
//! Sprint 39: Non-intrusive snapshot UI. Triggered from chat bubble hint or
//! titlebar icon. Does not consume permanent panel space.
//!
//! Migrated to the Clarity Design Protocol v1.0.

use crate::App;
use clarity_ui::design_system::{Elevation, Space, TextStyle, code_frame, gap, text};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::icon_button::icon_button;
use clarity_ui::widgets::modal::Modal;
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(3);

/// Renders the snapshot modal UI.
pub fn render_snapshot_modal(app: &mut App, ctx: &egui::Context) {
    if app.current_modal() != Some(&clarity_core::ui::ModalType::Snapshot) {
        return;
    }

    let theme = app.context.ui_store.theme.clone();

    // Refresh snapshot list lazily
    if app.context.snapshot_store.last_refresh.elapsed() > REFRESH_INTERVAL {
        app.context.snapshot_store.last_refresh = Instant::now();
        app.context.snapshot_store.snapshots = app.context.state.agent.snapshot_list();
    }

    // ESC closes modal
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.close_modal();
        app.context.snapshot_store.confirm_restore_id = None;
        app.context.snapshot_store.selected_id = None;
        return;
    }

    Modal::new("snapshot")
        .width(420.0)
        .max_height(600.0)
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                text(ui, "Workspace Snapshots", TextStyle::Title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(
                        ui,
                        crate::theme::ICON_X,
                        theme.text_base,
                        egui::Color32::TRANSPARENT,
                        egui::CornerRadius::same(theme.radius_sm as u8),
                        &theme,
                    )
                    .clicked()
                    {
                        app.close_modal();
                        app.context.snapshot_store.confirm_restore_id = None;
                        app.context.snapshot_store.selected_id = None;
                    }
                });
            });
            gap(ui, Space::S2);

            let snapshots = app.context.snapshot_store.snapshots.clone();
            if snapshots.is_empty() {
                ui.vertical_centered(|ui| {
                    gap(ui, Space::S5);
                    text(ui, "No snapshots available", TextStyle::Small);
                    text(
                        ui,
                        "Snapshots are created automatically before/after each agent turn \
                         when the working directory is a Git repository.",
                        TextStyle::Small,
                    );
                    gap(ui, Space::S5);
                });
            } else {
                // ponytail: ScrollArea is not yet wrapped in clarity-ui.
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for info in snapshots.iter().rev() {
                            render_snapshot_row(app, ui, info, &theme);
                        }
                    });
            }

            // Footer hint
            gap(ui, Space::S1);
            ui.horizontal(|ui| {
                text(
                    ui,
                    "? Restoring creates a backup of current state first",
                    TextStyle::Small,
                );
            });
        });
}

fn render_snapshot_row(
    app: &mut App,
    ui: &mut egui::Ui,
    info: &clarity_core::agent::snapshot::SnapshotInfo,
    theme: &crate::theme::Theme,
) {
    let is_selected = app.context.snapshot_store.selected_id == Some(info.id);
    let is_confirming = app.context.snapshot_store.confirm_restore_id == Some(info.id);

    // Row card
    // ponytail: Using Elevation::Elevated.frame directly because card() doesn't
    // expose selected-state fill/stroke overrides.
    let mut frame = Elevation::Elevated.frame(theme);
    if is_selected {
        frame = frame
            .fill(theme.bg_hover)
            .stroke(egui::Stroke::new(1.0, theme.border_strong));
    }

    frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());

        // Top line: id + label + time
        ui.horizontal(|ui| {
            let tag_color = if info.label.starts_with("pre-turn") {
                theme.status_busy
            } else {
                theme.status_online
            };
            // ponytail: color + mono combination has no TextStyle constant.
            ui.label(
                egui::RichText::new(format!("#{}", info.id))
                    .monospace()
                    .size(theme.text_sm)
                    .color(theme.text_muted),
            );
            // ponytail: color-coded tag has no TextStyle constant.
            ui.label(
                egui::RichText::new(info.label.split('-').next().unwrap_or("snap"))
                    .size(theme.text_xs)
                    .color(tag_color),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                text(ui, format_time_ago(&info.timestamp), TextStyle::Small);
            });
        });

        // Hash (monospace, dim)
        // ponytail: dim + mono combination has no TextStyle constant.
        ui.label(
            egui::RichText::new(&info.hash[..8.min(info.hash.len())])
                .monospace()
                .size(theme.text_xs)
                .color(theme.text_dim),
        );

        gap(ui, Space::S0);

        // Action buttons
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Restore button
                let restore_btn = if is_confirming {
                    ui.add(Button::new("Restore").danger_ghost())
                } else {
                    ui.add(Button::new("Restore").ghost())
                };
                if restore_btn.clicked() {
                    if is_confirming {
                        // Execute restore
                        app.view_state.turn = clarity_core::ui::TurnState::Restoring;
                        app.context.snapshot_store.confirm_restore_id = None;
                        let agent = app.context.state.agent.clone();
                        let tx = app.context.ui_tx.clone();
                        let id = info.id;
                        app.context.runtime.spawn(async move {
                            match agent.restore_snapshot(id).await {
                                Ok(()) => {
                                    let _ = tx.send(crate::ui::types::UiEvent::SnapshotRestored {
                                        id,
                                        success: true,
                                        error: None,
                                    });
                                }
                                Err(e) => {
                                    let _ = tx.send(crate::ui::types::UiEvent::SnapshotRestored {
                                        id,
                                        success: false,
                                        error: Some(e.to_string()),
                                    });
                                }
                            }
                        });
                    } else {
                        app.context.snapshot_store.confirm_restore_id = Some(info.id);
                        app.context.snapshot_store.selected_id = Some(info.id);
                    }
                }

                // Preview / diff button
                let preview_label = if is_selected { "Hide" } else { "Preview" };
                let preview_btn = ui.add(Button::new(preview_label).ghost());
                if preview_btn.clicked() {
                    app.context.snapshot_store.selected_id =
                        if is_selected { None } else { Some(info.id) };
                }
            });
        });

        // Confirmation row
        if is_confirming {
            gap(ui, Space::S1);
            Elevation::Elevated
                .frame(theme)
                .fill(theme.danger.linear_multiply(0.08))
                .show(ui, |ui| {
                    // ponytail: danger-colored text has no TextStyle constant.
                    ui.label(
                        egui::RichText::new(format!(
                            "Roll back to snapshot #{}? Current state will be backed up first.",
                            info.id
                        ))
                        .size(theme.text_sm)
                        .color(theme.danger),
                    );
                });
        }

        // Diff preview (inline)
        if is_selected && app.context.snapshot_store.preview.is_some() {
            gap(ui, Space::S1);
            code_frame(ui, |ui| {
                ui.set_max_width(ui.available_width());
                // ponytail: ScrollArea is not yet wrapped in clarity-ui.
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .show(ui, |ui| {
                        if let Some(ref preview) = app.context.snapshot_store.preview {
                            for line in preview.lines() {
                                let color = if line.starts_with('+') {
                                    theme.ok
                                } else if line.starts_with('-') {
                                    theme.danger
                                } else {
                                    theme.text_dim
                                };
                                // ponytail: color-coded monospace diff lines have no TextStyle constant.
                                ui.label(
                                    egui::RichText::new(line)
                                        .monospace()
                                        .color(color)
                                        .size(theme.text_xs),
                                );
                            }
                        }
                    });
            });
        }
    });

    gap(ui, Space::S0);
}

/// Format an RFC3339 timestamp as a relative string ("2m ago", "1h ago").
fn format_time_ago(timestamp: &str) -> String {
    use chrono::DateTime;
    let parsed = DateTime::parse_from_rfc3339(timestamp).ok();
    let now = chrono::Utc::now();
    match parsed {
        Some(dt) => {
            let duration = now.signed_duration_since(dt.with_timezone(&chrono::Utc));
            let secs = duration.num_seconds();
            if secs < 60 {
                format!("{}s ago", secs.max(0))
            } else if secs < 3600 {
                format!("{}m ago", secs / 60)
            } else if secs < 86400 {
                format!("{}h ago", secs / 3600)
            } else {
                format!("{}d ago", secs / 86400)
            }
        }
        None => timestamp.to_string(),
    }
}
