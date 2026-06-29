//! Fixed-width left navigation tree.
//!
//! S6 Phase D→E: single fixed-width tree containing a Work/Chat mode toggle,
//! New Session/Task, Skills, Plugins, a fixed Web entry, work templates,
//! collapsible web bookmarks, project tree, Claw devices, history/sessions,
//! and a bottom-aligned user avatar.
//!
//! The visual style is intentionally flat: no card frames, no section dividers,
//! and selected rows are highlighted by a full-width neutral background.
//!
//! Design note on width: `theme.size_sidebar` must stay wide enough for the
//! widest row inside this panel (e.g. multi-button action bars, device rows,
//! chat items). If the container is narrowed without also compressing those
//! rows, egui may expose unpainted/overflow areas.

use crate::App;
use crate::design_system::{self, Space};

pub mod claw_section;
pub mod history_section;
pub mod nav_items;
pub mod project_section;
pub mod web_section;
pub mod work_templates;

/// Render the left navigation tree.
///
/// `panel_width` controls the rendered width of the panel. During expand/collapse
/// animation this is the current animated value; when the animation is done and
/// expanded, it equals `theme.size_sidebar`.
pub fn render_left_navigation_tree(app: &mut App, ctx: &egui::Context, panel_width: f32) {
    let theme = app.ui_store.theme.clone();

    // SAFE: the caller (render_left_rail) already guards `panel_width > 0.0`,
    // so we never pass exactly 0 here. egui handles narrowing panels by
    // clipping content; no additional clamp needed.
    egui::SidePanel::left("left_navigation_tree")
        .exact_width(panel_width)
        .min_width(0.0)
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

            // ── Scrollable content + bottom-pinned footer via ui.put() ──
            // Strategy: give the footer a fixed rect at the bottom of the
            // available space, then allocate the scroll area into whatever
            // remains above it. This avoids the fragile StripBuilder height
            // calculation (size_bot_bar / nav_row_h / space_12 arithmetic).
            let footer_h = theme
                .size_bot_bar
                .max(theme.size_nav_row_h + theme.space_12 * 2.0);
            let available = ui.available_rect_before_wrap();
            let scroll_h = (available.height() - footer_h).max(0.0);

            // Scroll area fills the top portion.
            let scroll_rect =
                egui::Rect::from_min_size(available.min, egui::vec2(available.width(), scroll_h));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(scroll_rect), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("left_nav_scroll")
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        render_top_actions(app, ui);
                        design_system::gap(ui, Space::S2);
                        render_sections(app, ui, &theme);
                    });
            });

            // Footer pinned to the bottom via ui.put() — placed at an exact
            // rectangle, bypassing flow layout entirely.
            let footer_rect = egui::Rect::from_min_size(
                egui::pos2(available.min.x, available.max.y - footer_h),
                egui::vec2(available.width(), footer_h),
            );
            ui.allocate_new_ui(
                egui::UiBuilder::new()
                    .max_rect(footer_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
                |ui| {
                    // Separator line at the very top edge of the footer.
                    footer_separator(ui, &theme);
                    // Push the avatar to the bottom of the footer by allocating
                    // a child Ui with bottom_up layout that fills the remaining space.
                    let remaining_h = ui.available_height();
                    if remaining_h > 0.0 {
                        let avatar_rect = egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(ui.available_width(), remaining_h),
                        );
                        ui.allocate_new_ui(
                            egui::UiBuilder::new()
                                .max_rect(avatar_rect)
                                .layout(egui::Layout::bottom_up(egui::Align::LEFT)),
                            |ui| {
                                render_user_avatar_row(app, ui, &theme);
                            },
                        );
                    }
                },
            );
        });
}

/// Top area: primary navigation actions.
fn render_top_actions(app: &mut App, ui: &mut egui::Ui) {
    nav_items::render_nav_items(app, ui);
}

/// Scrollable sections: work templates, collapsible web bookmarks, projects,
/// claw devices, and session history.
fn render_sections(app: &mut App, ui: &mut egui::Ui, _theme: &crate::theme::Theme) {
    // 1. Work templates (quick-launch rows)
    work_templates::render_work_templates(app, ui);
    design_system::gap(ui, Space::S2);

    // 2. Collapsible web bookmarks
    web_section::render_web_section(app, ui, "nav_web");
    design_system::gap(ui, Space::S2);

    // 3. Projects
    project_section::render_project_section(app, ui);
    design_system::gap(ui, Space::S2);

    // 4. Claw devices
    claw_section::render_claw_section(app, ui);
    design_system::gap(ui, Space::S2);

    // 5. History / sessions
    history_section::render_history_section(app, ui);
}

/// Subtle horizontal separator between the scrollable tree and the footer.
fn footer_separator(ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    let available = ui.available_width();
    let stroke = egui::Stroke::new(1.0, theme.border);
    let left = ui.cursor().min.x;
    let right = left + available;
    let y = ui.cursor().min.y + 0.5;
    ui.painter()
        .line_segment([egui::pos2(left, y), egui::pos2(right, y)], stroke);
    design_system::gap(ui, Space::S1);
}

/// Bottom user avatar row showing the active user and current model.
fn render_user_avatar_row(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    let model = app.settings_store.settings_edit.model.trim();
    let subtitle = if model.is_empty() { None } else { Some(model) };
    let _ = crate::widgets::user_avatar_row(ui, "User", subtitle, theme);
}

// ============================================================================
// Layout tests — verify footer and scroll rect geometry
// ============================================================================
#[cfg(test)]
mod tests {
    /// Pure helper: given an available rect and footer height, compute the
    /// scroll and footer sub-rects the same way `render_left_navigation_tree` does.
    fn compute_footer_geometry(available: egui::Rect, footer_h: f32) -> (egui::Rect, egui::Rect) {
        let scroll_h = (available.height() - footer_h).max(0.0);
        let scroll_rect =
            egui::Rect::from_min_size(available.min, egui::vec2(available.width(), scroll_h));
        let footer_rect = egui::Rect::from_min_size(
            egui::pos2(available.min.x, available.max.y - footer_h),
            egui::vec2(available.width(), footer_h),
        );
        (scroll_rect, footer_rect)
    }

    #[test]
    fn footer_rect_sits_at_bottom_of_available_space() {
        let available = egui::Rect::from_min_max(egui::pos2(8.0, 12.0), egui::pos2(202.0, 712.0));
        let footer_h = 56.0;
        let (_scroll, footer) = compute_footer_geometry(available, footer_h);

        assert_eq!(footer.bottom(), available.bottom());
        assert_eq!(footer.top(), available.bottom() - footer_h);
        assert_eq!(footer.width(), available.width());
    }

    #[test]
    fn scroll_rect_fills_space_above_footer() {
        let available = egui::Rect::from_min_max(egui::pos2(8.0, 12.0), egui::pos2(202.0, 712.0));
        let footer_h = 56.0;
        let (scroll, _) = compute_footer_geometry(available, footer_h);

        assert_eq!(scroll.top(), available.top());
        assert_eq!(scroll.bottom(), available.bottom() - footer_h);
    }

    #[test]
    fn footer_h_is_zero_when_content_too_short() {
        let available = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 30.0));
        let footer_h = 56.0;
        let (scroll, _) = compute_footer_geometry(available, footer_h);

        assert_eq!(scroll.height(), 0.0);
        assert!(scroll.top() <= scroll.bottom());
    }

    /// Verify that `ui.allocate_new_ui` with `Layout::top_down` places the
    /// first widget at the *top* of its max_rect, and `Layout::bottom_up`
    /// places the first widget at the *bottom* of its max_rect.
    #[test]
    fn top_down_cursor_starts_at_rect_top_bottom_up_at_rect_bottom() {
        let footer_rect =
            egui::Rect::from_min_max(egui::pos2(8.0, 656.0), egui::pos2(202.0, 712.0));

        let ctx = egui::Context::default();
        let mut separator_y = 0.0_f32;
        let mut avatar_widget_rect = egui::Rect::NOTHING;

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_new_ui(
                    egui::UiBuilder::new()
                        .max_rect(footer_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                    |ui| {
                        // First widget in top_down: cursor at rect top.
                        separator_y = ui.cursor().min.y;
                        ui.add_space(4.0);

                        let remaining_h = ui.available_height();
                        assert!(remaining_h > 10.0, "remaining_h={} too small", remaining_h);

                        let avatar_area = egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(ui.available_width(), remaining_h),
                        );
                        ui.allocate_new_ui(
                            egui::UiBuilder::new()
                                .max_rect(avatar_area)
                                .layout(egui::Layout::bottom_up(egui::Align::LEFT)),
                            |ui| {
                                // First widget in bottom_up: placed at rect bottom.
                                let r = ui.allocate_response(
                                    egui::vec2(100.0, 28.0),
                                    egui::Sense::hover(),
                                );
                                avatar_widget_rect = r.rect;
                            },
                        );
                    },
                );
            });
        });

        // Separator cursor.y = footer_rect.top()
        assert!(
            (separator_y - footer_rect.top()).abs() < 1.0,
            "separator y={:.1} should be at footer top {:.1}",
            separator_y,
            footer_rect.top()
        );

        // Avatar widget bottom edge = footer_rect.bottom()
        assert!(
            (avatar_widget_rect.bottom() - footer_rect.bottom()).abs() < 1.0,
            "avatar bottom={:.1} should be at footer bottom {:.1}",
            avatar_widget_rect.bottom(),
            footer_rect.bottom()
        );

        // Avatar is fully contained inside footer.
        assert!(
            footer_rect.contains_rect(avatar_widget_rect),
            "avatar {:?} must be inside footer {:?}",
            avatar_widget_rect,
            footer_rect
        );
    }
}
