//! Fixed-width icon column helpers for the left navigation tree.

use crate::theme::Theme;

/// Render an icon centered inside the fixed-width navigation icon rail.
///
/// This guarantees that all sidebar rows (nav items, collapsible headers,
/// bot rows, history entries) share the same left icon column and that text
/// labels start on the same vertical grid line.
pub fn nav_icon_rail(ui: &mut egui::Ui, theme: &Theme, icon: &str, color: egui::Color32) {
    let rail_w = theme.size_nav_icon_rail;
    let row_h = theme.size_nav_row_h;
    ui.allocate_ui_with_layout(
        egui::vec2(rail_w, row_h),
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
        |ui| {
            ui.label(egui::RichText::new(icon).size(theme.text_base).color(color));
        },
    );
}

/// Render a status dot centered inside the navigation icon rail.
pub fn nav_status_dot(ui: &mut egui::Ui, theme: &Theme, color: egui::Color32) {
    let rail_w = theme.size_nav_icon_rail;
    let row_h = theme.size_nav_row_h;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(rail_w, row_h), egui::Sense::hover());
    let dot_radius = theme.space_4;
    ui.painter().circle_filled(rect.center(), dot_radius, color);
}
