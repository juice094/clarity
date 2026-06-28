//! File preview floating overlay — renders outside panel layout constraints.
//!
//! Triggered by `ui_store.preview_item` (set from workspace panel clicks).
//! Uses `egui::Area` with `Order::Foreground` to escape CentralPanel width limits.

use crate::App;
use crate::ui::types::PreviewItem;

/// Renders the file preview overlay UI.
#[allow(dead_code)]
pub fn render_file_preview_overlay(app: &mut App, ctx: &egui::Context) {
    if app.ui_store.preview_item.is_none() {
        return;
    }

    let screen = ctx.screen_rect();
    let theme = app.ui_store.theme.clone();

    // ── Fullscreen scrim (visual) ──
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        screen,
        egui::CornerRadius::ZERO,
        egui::Color32::from_black_alpha(80),
    );

    // ── Fullscreen click blocker ──
    let mut close_requested = false;
    egui::Area::new("file_preview_scrim".into())
        .interactable(true)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            if ui
                .allocate_response(screen.size(), egui::Sense::click())
                .clicked()
            {
                close_requested = true;
            }
        });

    // ── Fullscreen toggle state ──
    let fullscreen_id = egui::Id::new("file_preview_fullscreen");
    let mut is_fullscreen = ctx.data(|d| d.get_temp::<bool>(fullscreen_id).unwrap_or(false));

    let preview_width = if is_fullscreen {
        screen.width() * 0.95
    } else {
        800.0_f32.min(screen.width() * 0.85)
    };
    let preview_height = if is_fullscreen {
        screen.height() * 0.90
    } else {
        600.0_f32.min(screen.height() * 0.80)
    };

    // ── Extract preview data ──
    let Some(preview) = app.ui_store.preview_item.as_ref() else {
        return;
    };
    let (title, content, is_web, url) = match preview {
        PreviewItem::File { name, content, .. } => (name.clone(), content.clone(), false, None),
        PreviewItem::WebPage {
            title,
            url,
            content,
        } => (title.clone(), content.clone(), true, Some(url.clone())),
    };

    // ── Floating card (centered) ──
    egui::Area::new("file_preview_card".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(preview_width);
            ui.set_max_width(preview_width);
            ui.set_max_height(preview_height);

            // Outer card: rounded corners + shadow, no stroke
            egui::Frame::new()
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
                .shadow(theme.shadow_modal)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::ZERO)
                .show(ui, |ui| {
                    ui.set_min_width(preview_width);
                    ui.set_max_width(preview_width);
                    ui.set_max_height(preview_height);

                    ui.vertical(|ui| {
                        // ── Title bar ──
                        let top_radius = egui::CornerRadius {
                            nw: theme.radius_md as u8,
                            ne: theme.radius_md as u8,
                            sw: 0,
                            se: 0,
                        };
                        egui::Frame::new()
                            .fill(theme.surface_strong)
                            .corner_radius(top_radius)
                            .stroke(egui::Stroke::NONE)
                            .inner_margin(egui::Margin::symmetric(16, 10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let icon = if is_web {
                                        crate::theme::ICON_GLOBE
                                    } else {
                                        crate::theme::ICON_FILE
                                    };
                                    ui.label(
                                        egui::RichText::new(icon)
                                            .font(theme.font_icon(theme.text_sm)),
                                    );
                                    ui.label(
                                        egui::RichText::new(&title)
                                            .size(theme.text_sm)
                                            .color(theme.text)
                                            .monospace(),
                                    );

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            // Close
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new(crate::theme::ICON_X)
                                                            .font(theme.font_icon(theme.text_base)),
                                                    )
                                                    .fill(egui::Color32::TRANSPARENT)
                                                    .corner_radius(egui::CornerRadius::same(
                                                        theme.radius_sm as u8,
                                                    )),
                                                )
                                                .clicked()
                                            {
                                                close_requested = true;
                                            }

                                            // Fullscreen toggle
                                            let fs_icon = if is_fullscreen {
                                                crate::theme::ICON_MINIMIZE
                                            } else {
                                                crate::theme::ICON_MAXIMIZE
                                            };
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new(fs_icon)
                                                            .font(theme.font_icon(theme.text_base)),
                                                    )
                                                    .fill(egui::Color32::TRANSPARENT)
                                                    .corner_radius(egui::CornerRadius::same(
                                                        theme.radius_sm as u8,
                                                    )),
                                                )
                                                .clicked()
                                            {
                                                is_fullscreen = !is_fullscreen;
                                            }
                                        },
                                    );
                                });
                            });

                        // ── URL label for web pages ──
                        if let Some(ref url) = url {
                            crate::design_system::gap(ui, crate::design_system::Space::S1);
                            ui.horizontal(|ui| {
                                crate::design_system::gap(ui, crate::design_system::Space::S3);
                                ui.label(
                                    egui::RichText::new(url)
                                        .size(theme.text_xs)
                                        .color(theme.text_muted),
                                );
                            });
                        }

                        // ── Content area ──
                        crate::design_system::gap(ui, crate::design_system::Space::S2);
                        ui.horizontal(|ui| {
                            crate::design_system::gap(ui, crate::design_system::Space::S3);
                            ui.vertical(|ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt("preview_scroll_overlay")
                                    .show(ui, |ui| {
                                        let parsed = crate::ui::markdown::parse_markdown(&content);
                                        crate::ui::markdown::render_blocks(
                                            ui,
                                            &parsed,
                                            &theme,
                                            theme.chat_text,
                                        );
                                    });
                            });
                        });
                        crate::design_system::gap(ui, crate::design_system::Space::S2);
                    });
                });
        });

    if close_requested {
        app.ui_store.preview_item = None;
    }
    ctx.data_mut(|d| d.insert_temp(fullscreen_id, is_fullscreen));
}
