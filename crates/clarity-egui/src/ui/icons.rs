//! Clarity icon system — code-drawn brand icons (zero external assets).
//!
//! Follows Kimi-style design grammar:
//!   - 1.5px stroke, round caps/joins
//!   - 20×20px viewport for role icons
//!   - 16×16px viewport for file/operation icons
//!   - Color: theme-controlled (no hard-coded emoji)

use egui::{Align2, Color32, Painter, Pos2, Rect, Stroke, Vec2};

/// Paint a status dot (6px for sidebar, 4px for tray badge).
#[allow(dead_code)]
pub fn paint_status_dot(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    painter.circle_filled(center, radius, color);
}

/// Paint a generic file icon (rounded rect with folded corner).
/// Viewport: 16×16, stroke 1.5px.
pub fn paint_file(painter: &Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let fold = 4.0;
    // Main body: rounded rect minus top-right corner
    let body = [
        rect.left_top(),
        rect.left_bottom(),
        rect.right_bottom(),
        Pos2::new(rect.max.x, rect.min.y + fold),
        Pos2::new(rect.max.x - fold, rect.min.y),
    ];
    for i in 0..body.len() {
        painter.line_segment([body[i], body[(i + 1) % body.len()]], stroke);
    }
    // Fold line
    let fold_start = Pos2::new(rect.max.x - fold, rect.min.y);
    let fold_end = Pos2::new(rect.max.x, rect.min.y + fold);
    painter.line_segment([fold_start, fold_end], stroke);
}

/// Paint a folder icon (rounded rect with tab).
/// Viewport: 16×16, stroke 1.5px.
#[allow(dead_code)]
pub fn paint_folder(painter: &Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let tab_w = 6.0;
    let _tab_h = 3.0;
    let body = [
        Pos2::new(rect.min.x + tab_w, rect.min.y),
        rect.left_top(),
        rect.left_bottom(),
        rect.right_bottom(),
        rect.right_top(),
        Pos2::new(rect.min.x + tab_w + 2.0, rect.min.y),
    ];
    for i in 0..body.len() - 1 {
        painter.line_segment([body[i], body[i + 1]], stroke);
    }
}

/// Paint a file-type badge overlay in the bottom-right of a file icon.
/// `badge` is a single char (e.g. 'M' for Markdown, '≡' for config).
pub fn paint_file_badge(
    painter: &Painter,
    rect: Rect,
    badge: &str,
    color: Color32,
    font_size: f32,
) {
    let br = rect.right_bottom();
    let badge_center = br - Vec2::new(4.0, 4.0);
    painter.circle_filled(badge_center, 4.0, color.linear_multiply(0.15));
    painter.text(
        badge_center,
        Align2::CENTER_CENTER,
        badge,
        egui::FontId::new(font_size, egui::FontFamily::Proportional),
        color,
    );
}

/// Paint a globe/web icon (circle + equator + two arcs).
pub fn paint_globe(painter: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let r = 5.0;
    painter.circle_stroke(center, r, stroke);
    painter.line_segment([center + Vec2::new(-r, 0.0), center + Vec2::new(r, 0.0)], stroke);
    let arc_y = r * 0.55;
    painter.line_segment([center + Vec2::new(-r * 0.75, -arc_y), center + Vec2::new(r * 0.75, -arc_y)], stroke);
    painter.line_segment([center + Vec2::new(-r * 0.75, arc_y), center + Vec2::new(r * 0.75, arc_y)], stroke);
}
