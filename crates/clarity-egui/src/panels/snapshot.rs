//! Snapshot History Modal — workspace snapshot browsing, diff preview, and restore.
//!
//! Sprint 39: Non-intrusive snapshot UI. Triggered from chat bubble hint or
//! titlebar icon. Does not consume permanent panel space.

use crate::App;
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(3);

pub fn render_snapshot_modal(app: &mut App, ctx: &egui::Context) {
    if !app.snapshot_store.modal_open {
        return;
    }

    let theme = app.ui_store.theme.clone();

    // Refresh snapshot list lazily
    if app.snapshot_store.last_refresh.elapsed() > REFRESH_INTERVAL {
        app.snapshot_store.last_refresh = Instant::now();
        app.snapshot_store.snapshots = app.state.agent.snapshot_list();
    }

    // Full-screen click blocker
    let screen = ctx.screen_rect();
    let blocker_id = egui::Id::new("snapshot_blocker");
    egui::Area::new(blocker_id)
        .order(egui::Order::Background)
        .interactable(true)
        .show(ctx, |ui| {
            let response = ui.allocate_response(screen.size(), egui::Sense::click());
            ui.painter_at(response.rect)
                .rect_filled(response.rect, 0.0, theme.overlay);
        });

    // ESC closes modal
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.snapshot_store.modal_open = false;
        app.snapshot_store.confirm_restore_id = None;
        app.snapshot_store.selected_id = None;
        return;
    }

    egui::Window::new("📸 Snapshot History")
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::group(&ctx.style())
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::same(20)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.set_max_width(520.0);

            // Header
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Workspace Snapshots")
                        .size(theme.text_lg)
                        .strong()
                        .color(theme.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(crate::theme::ICON_X)
                                    .font(theme.font_icon(theme.text_base)),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                        )
                        .clicked()
                    {
                        app.snapshot_store.modal_open = false;
                        app.snapshot_store.confirm_restore_id = None;
                        app.snapshot_store.selected_id = None;
                    }
                });
            });
            ui.add_space(theme.space_12);

            let snapshots = app.snapshot_store.snapshots.clone();
            if snapshots.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(theme.space_24);
                    ui.label(
                        egui::RichText::new("No snapshots available")
                            .size(theme.text_sm)
                            .color(theme.text_dim),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Snapshots are created automatically before/after each agent turn \
                             when the working directory is a Git repository.",
                        )
                        .size(theme.text_xs)
                        .color(theme.text_dim),
                    );
                    ui.add_space(theme.space_24);
                });
            } else {
                // Snapshot list
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for info in snapshots.iter().rev() {
                            render_snapshot_row(app, ui, info, &theme);
                        }
                    });
            }

            // Footer hint
            ui.add_space(theme.space_8);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("? Restoring creates a backup of current state first")
                        .size(theme.text_xs)
                        .color(theme.text_dim),
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
    let is_selected = app.snapshot_store.selected_id == Some(info.id);
    let is_confirming = app.snapshot_store.confirm_restore_id == Some(info.id);

    // Row card
    let frame = egui::Frame::new()
        .fill(if is_selected {
            theme.bg_hover
        } else {
            egui::Color32::TRANSPARENT
        })
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .stroke(egui::Stroke::new(
            1.0_f32,
            if is_selected {
                theme.border_strong
            } else {
                egui::Color32::TRANSPARENT
            },
        ))
        .inner_margin(egui::Margin::symmetric(10, 8));

    frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());

        // Top line: id + label + time
        ui.horizontal(|ui| {
            let tag_color = if info.label.starts_with("pre-turn") {
                theme.status_busy
            } else {
                theme.status_online
            };
            ui.label(
                egui::RichText::new(format!("#{}", info.id))
                    .font(theme.font_mono(theme.text_sm))
                    .color(theme.text_muted),
            );
            ui.label(
                egui::RichText::new(info.label.split('-').next().unwrap_or("snap"))
                    .size(theme.text_xs)
                    .color(tag_color),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format_time_ago(&info.timestamp))
                        .size(theme.text_xs)
                        .color(theme.text_dim),
                );
            });
        });

        // Hash (monospace, dim)
        ui.label(
            egui::RichText::new(&info.hash[..8.min(info.hash.len())])
                .font(theme.font_mono(theme.text_xs))
                .color(theme.text_dim),
        );

        ui.add_space(theme.space_4);

        // Action buttons
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Restore button
                let restore_btn = ui.add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} Restore", crate::theme::ICON_REFRESH))
                            .font(theme.font_icon(theme.text_sm))
                            .color(if is_confirming {
                                theme.danger
                            } else {
                                theme.text_dim
                            }),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                );
                if restore_btn.clicked() {
                    if is_confirming {
                        // Execute restore
                        app.snapshot_store.restoring = true;
                        app.snapshot_store.confirm_restore_id = None;
                        let agent = app.state.agent.clone();
                        let tx = app.ui_tx.clone();
                        let id = info.id;
                        app.runtime.spawn(async move {
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
                        app.snapshot_store.confirm_restore_id = Some(info.id);
                        app.snapshot_store.selected_id = Some(info.id);
                    }
                }

                // Preview / diff button
                let preview_btn = ui.add(
                    egui::Button::new(
                        egui::RichText::new(if is_selected {
                            "👁 Hide"
                        } else {
                            "👁 Preview"
                        })
                        .size(theme.text_xs)
                        .color(theme.accent),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                );
                if preview_btn.clicked() {
                    app.snapshot_store.selected_id = if is_selected { None } else { Some(info.id) };
                }
            });
        });

        // Confirmation row
        if is_confirming {
            ui.add_space(theme.space_8);
            egui::Frame::new()
                .fill(theme.danger.linear_multiply(0.08))
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "⚠️ Roll back to snapshot #{}? Current state will be backed up first.",
                            info.id
                        ))
                        .size(theme.text_sm)
                        .color(theme.danger),
                    );
                });
        }

        // Diff preview (inline)
        if is_selected && app.snapshot_store.preview.is_some() {
            ui.add_space(theme.space_8);
            egui::Frame::new()
                .fill(theme.code_block_bg)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .show(ui, |ui| {
                            if let Some(ref preview) = app.snapshot_store.preview {
                                for line in preview.lines() {
                                    let color = if line.starts_with('+') {
                                        theme.ok
                                    } else if line.starts_with('-') {
                                        theme.danger
                                    } else {
                                        theme.text_dim
                                    };
                                    ui.monospace(egui::RichText::new(line).color(color).size(11.0));
                                }
                            }
                        });
                });
        }
    });

    ui.add_space(theme.space_4);
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
