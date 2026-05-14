//! Clarity icon system — code-drawn brand icons (zero external assets).
//!
//! Follows Kimi-style design grammar:
//!   - 1.5px stroke, round caps/joins
//!   - 20×20px viewport for role icons
//!   - 16×16px viewport for file/operation icons
//!   - Color: theme-controlled (no hard-coded emoji)

use egui::{Color32, Painter, Pos2, Rect, Stroke};

/// Paint a status dot (6px for sidebar, 4px for tray badge).
#[allow(dead_code)]
pub fn paint_status_dot(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    painter.circle_filled(center, radius, color);
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
