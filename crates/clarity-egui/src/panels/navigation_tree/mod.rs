//! Fixed-width left navigation tree.
//!
//! S6 Phase D: this replaces the old 36px icon rail + collapsible sidebar with
//! a single fixed-width tree that contains quick actions, Claw devices, project
//! tree, unprojected chats, and a bottom-aligned user avatar.
//!
//! Design note on width: `theme.size_sidebar` must stay wide enough for the
//! widest row inside this panel (e.g. multi-button action bars, device rows,
//! chat items). If the container is narrowed without also compressing those
//! rows, egui may expose unpainted/overflow areas. Future narrowing should be
//! paired with a responsive internal layout: icon-only secondary actions,
//! wrapping chips, or an overflow menu.

use crate::App;
use egui_extras::{Size, StripBuilder};

pub mod claw_section;
pub mod project_tree;
pub mod quick_actions;
pub mod unprojected_chats;

/// Render the left navigation tree.
pub fn render_left_navigation_tree(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();

    egui::SidePanel::left("left_navigation_tree")
        .exact_width(theme.size_sidebar)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                // Use the same background as the unified base painter; this keeps
                // the left rail visually continuous with the rest of the chrome.
                .fill(theme.bg)
                .stroke(egui::Stroke::NONE)
                .shadow(egui::Shadow::NONE)
                .inner_margin(egui::Margin::symmetric(8, 12))
                // Align the left rail vertically with the right rail / inner surface.
                .outer_margin(egui::Margin::symmetric(0, theme.space_4 as i8)),
        )
        .show(ctx, |ui| {
            if crate::ui::debug_overlay::is_enabled(ctx) {
                crate::ui::debug_overlay::show_layout_state(ui, "left-nav-tree");
            }

            // Use a vertical strip so the avatar stays pinned to the bottom.
            StripBuilder::new(ui)
                .size(Size::remainder())
                .size(Size::exact(theme.size_bot_bar + theme.space_16))
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("left_nav_scroll")
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                quick_actions::render_quick_actions(app, ui);
                                ui.add_space(theme.space_16);
                                claw_section::render_claw_section(app, ui);
                                ui.add_space(theme.space_16);
                                project_tree::render_project_tree(app, ui);
                                ui.add_space(theme.space_16);
                                unprojected_chats::render_unprojected_chats(app, ui);
                            });
                    });

                    strip.cell(|ui| {
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                            render_user_avatar(app, ui);
                        });
                    });
                });
        });
}

fn render_user_avatar(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    let model = app.settings_store.settings_edit.model.trim();
    let subtitle = if model.is_empty() { None } else { Some(model) };
    let _ = crate::widgets::user_avatar_row(ui, "User", subtitle, theme);
}
