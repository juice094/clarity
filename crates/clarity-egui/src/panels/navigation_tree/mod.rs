//! Fixed-width left navigation tree.
//!
//! S6 Phase D→E: single fixed-width tree containing work/chat toggle,
//! navigation items, collapsible web bookmarks, work templates, Claw devices,
//! project tree, history/sessions, and a bottom-aligned user avatar.
//!
//! Design note on width: `theme.size_sidebar` must stay wide enough for the
//! widest row inside this panel (e.g. multi-button action bars, device rows,
//! chat items). If the container is narrowed without also compressing those
//! rows, egui may expose unpainted/overflow areas.

use crate::App;
use egui_extras::{Size, StripBuilder};

pub mod claw_section;
pub mod history_section;
pub mod nav_items;
pub mod project_section;
pub mod web_section;
pub mod work_chat_toggle;
pub mod work_templates;

/// Render the left navigation tree.
pub fn render_left_navigation_tree(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();

    egui::SidePanel::left("left_navigation_tree")
        .exact_width(theme.size_sidebar)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(theme.bg)
                .stroke(egui::Stroke::NONE)
                .shadow(egui::Shadow::NONE)
                .inner_margin(egui::Margin::symmetric(
                    theme.space_8 as i8,
                    theme.space_12 as i8,
                ))
                .outer_margin(egui::Margin::symmetric(0, theme.space_4 as i8)),
        )
        .show(ctx, |ui| {
            if crate::ui::debug_overlay::is_enabled(ctx) {
                crate::ui::debug_overlay::show_layout_state(ui, "left-nav-tree");
            }

            // Bottom cell fits the user avatar row without reserving excessive
            // height, preventing dead space or clipping at the bottom of the rail.
            let footer_height = theme
                .size_bot_bar
                .max(theme.space_12 + theme.size_nav_row_h + theme.space_12);
            StripBuilder::new(ui)
                .size(Size::remainder())
                .size(Size::exact(footer_height))
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("left_nav_scroll")
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                render_top_actions_card(app, ui, &theme);
                                ui.add_space(theme.space_16);
                                render_sections_card(app, ui, &theme);
                            });
                    });

                    strip.cell(|ui| {
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                            render_user_avatar_card(app, ui, &theme);
                        });
                    });
                });
        });
}

/// Top card: Work/Chat toggle + primary navigation actions.
fn render_top_actions_card(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .inner_margin(egui::Margin::same(theme.space_8 as i8))
        .show(ui, |ui| {
            work_chat_toggle::render_work_chat_toggle(app, ui);
            ui.add_space(theme.space_12);
            nav_items::render_nav_items(app, ui);
        });
}

/// Scrollable sections card: collapsible web/templates/projects/claw/history.
fn render_sections_card(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .inner_margin(egui::Margin::same(theme.space_8 as i8))
        .show(ui, |ui| {
            // 1. Web bookmarks (chat context)
            web_section::render_web_section(
                app,
                ui,
                "nav_web_chat",
                web_section::WebSectionContext::Chat,
            );
            section_divider(ui, theme);

            // 2. Work templates
            work_templates::render_work_templates(app, ui);
            section_divider(ui, theme);

            // 3. Web bookmarks (work context)
            web_section::render_web_section(
                app,
                ui,
                "nav_web_work",
                web_section::WebSectionContext::Work,
            );
            section_divider(ui, theme);

            // 4. Projects
            project_section::render_project_section(app, ui);
            section_divider(ui, theme);

            // 5. Claw devices
            claw_section::render_claw_section(app, ui);
            section_divider(ui, theme);

            // 6. History / sessions
            history_section::render_history_section(app, ui);
        });
}

/// Subtle horizontal divider between sidebar sections.
fn section_divider(ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    ui.add_space(theme.space_8);
    let available = ui.available_width();
    let stroke = egui::Stroke::new(1.0, theme.border);
    let left = ui.cursor().min.x;
    let right = left + available;
    let y = ui.cursor().min.y + 0.5;
    ui.painter()
        .line_segment([egui::pos2(left, y), egui::pos2(right, y)], stroke);
    ui.add_space(theme.space_8);
}

/// Bottom user avatar card showing the active user and current model.
fn render_user_avatar_card(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    let model = app.settings_store.settings_edit.model.trim();
    let subtitle = if model.is_empty() { None } else { Some(model) };

    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .inner_margin(egui::Margin::same(theme.space_8 as i8))
        .show(ui, |ui| {
            let _ = crate::widgets::user_avatar_row(ui, "User", subtitle, theme);
        });
}
