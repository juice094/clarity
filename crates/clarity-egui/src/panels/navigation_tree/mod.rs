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
                .inner_margin(egui::Margin::symmetric(8, 12))
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
                .max(theme.space_12 + 28.0 + theme.space_12);
            StripBuilder::new(ui)
                .size(Size::remainder())
                .size(Size::exact(footer_height))
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("left_nav_scroll")
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                // 1. Work/Chat toggle
                                work_chat_toggle::render_work_chat_toggle(app, ui);
                                ui.add_space(theme.space_12);

                                // 2. Navigation items (New Task, Skills, Plugins)
                                nav_items::render_nav_items(app, ui);
                                ui.add_space(theme.space_12);

                                // 3. Web bookmarks (chat context)
                                web_section::render_web_section(
                                    app,
                                    ui,
                                    "nav_web_chat",
                                    web_section::WebSectionContext::Chat,
                                );
                                ui.add_space(theme.space_8);

                                // 4. Work templates
                                work_templates::render_work_templates(app, ui);
                                ui.add_space(theme.space_12);

                                // 5. Web bookmarks (work context)
                                web_section::render_web_section(
                                    app,
                                    ui,
                                    "nav_web_work",
                                    web_section::WebSectionContext::Work,
                                );
                                ui.add_space(theme.space_8);

                                // 6. Projects
                                project_section::render_project_section(app, ui);
                                ui.add_space(theme.space_8);

                                // 7. Claw devices
                                claw_section::render_claw_section(app, ui);
                                ui.add_space(theme.space_8);

                                // 8. History / sessions
                                history_section::render_history_section(app, ui);
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
