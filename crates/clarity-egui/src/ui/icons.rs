//! Clarity icon system — code-drawn brand icons (zero external assets).
//!
//! Follows Kimi-style design grammar:
//!   - 1.5px stroke, round caps/joins
//!   - 20×20px viewport for role icons
//!   - 16×16px viewport for file/operation icons
//!   - Color: theme-controlled (no hard-coded emoji)

use egui::{Align2, Color32, Painter, Pos2, Rect, Stroke, Vec2};

/// Paint the Emotion icon (two wave lines) at the given center.
/// Viewport: 20×20, stroke 1.5px.
pub fn paint_emotion(painter: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    // Upper wave
    let p1 = center + Vec2::new(-7.0, -3.0);
    let p2 = center + Vec2::new(-3.5, -6.0);
    let p3 = center + Vec2::new(0.0, -3.0);
    let p4 = center + Vec2::new(3.5, -6.0);
    let p5 = center + Vec2::new(7.0, -3.0);
    painter.line_segment([p1, p2], stroke);
    painter.line_segment([p2, p3], stroke);
    painter.line_segment([p3, p4], stroke);
    painter.line_segment([p4, p5], stroke);

    // Lower wave (offset down by 5px)
    let q1 = center + Vec2::new(-7.0, 2.0);
    let q2 = center + Vec2::new(-3.5, -1.0);
    let q3 = center + Vec2::new(0.0, 2.0);
    let q4 = center + Vec2::new(3.5, -1.0);
    let q5 = center + Vec2::new(7.0, 2.0);
    painter.line_segment([q1, q2], stroke);
    painter.line_segment([q2, q3], stroke);
    painter.line_segment([q3, q4], stroke);
    painter.line_segment([q4, q5], stroke);
}

/// Paint the Knowledge icon (two offset diamonds).
/// Viewport: 20×20, stroke 1.5px.
pub fn paint_knowledge(painter: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    // Back diamond (larger, offset down-right)
    let back = diamond_points(center + Vec2::new(2.0, 2.0), 6.0);
    painter.line_segment([back[0], back[1]], stroke);
    painter.line_segment([back[1], back[2]], stroke);
    painter.line_segment([back[2], back[3]], stroke);
    painter.line_segment([back[3], back[0]], stroke);

    // Front diamond (smaller)
    let front = diamond_points(center + Vec2::new(-1.0, -1.0), 5.0);
    painter.line_segment([front[0], front[1]], stroke);
    painter.line_segment([front[1], front[2]], stroke);
    painter.line_segment([front[2], front[3]], stroke);
    painter.line_segment([front[3], front[0]], stroke);
}

fn diamond_points(center: Pos2, half_size: f32) -> [Pos2; 4] {
    [
        center + Vec2::new(0.0, -half_size), // top
        center + Vec2::new(half_size, 0.0),  // right
        center + Vec2::new(0.0, half_size),  // bottom
        center + Vec2::new(-half_size, 0.0), // left
    ]
}

/// Paint the Engineering icon (hexagon + vertical stem).
/// Viewport: 20×20, stroke 1.5px.
pub fn paint_engineering(painter: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let r = 5.0; // hexagon circumradius
    let hex: Vec<Pos2> = (0..6)
        .map(|i| {
            let angle = (i as f32) * std::f32::consts::PI / 3.0 - std::f32::consts::PI / 2.0;
            center + Vec2::new(r * angle.cos(), r * angle.sin())
        })
        .collect();
    for i in 0..6 {
        painter.line_segment([hex[i], hex[(i + 1) % 6]], stroke);
    }
    // Vertical stem below hexagon
    let stem_top = center + Vec2::new(0.0, r);
    let stem_bottom = center + Vec2::new(0.0, r + 4.0);
    painter.line_segment([stem_top, stem_bottom], stroke);
}

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

/// Convenience: get the correct role paint function by category name.
pub fn paint_role_icon(painter: &Painter, category: &str, center: Pos2, color: Color32) {
    match category {
        "emotion" => paint_emotion(painter, center, color),
        "knowledge" => paint_knowledge(painter, center, color),
        "engineering" => paint_engineering(painter, center, color),
        _ => paint_engineering(painter, center, color),
    }
}

/// Paint a chevron pointing down.
pub fn paint_chevron_down(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let left = center + Vec2::new(-size * 0.5, -size * 0.25);
    let right = center + Vec2::new(size * 0.5, -size * 0.25);
    let bottom = center + Vec2::new(0.0, size * 0.4);
    painter.line_segment([left, bottom], stroke);
    painter.line_segment([right, bottom], stroke);
}

/// Paint a chevron pointing right.
pub fn paint_chevron_right(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let top = center + Vec2::new(-size * 0.25, -size * 0.4);
    let right = center + Vec2::new(size * 0.3, 0.0);
    let bottom = center + Vec2::new(-size * 0.25, size * 0.4);
    painter.line_segment([top, right], stroke);
    painter.line_segment([bottom, right], stroke);
}

/// Paint MCP icon (two linked circles — protocol/connection).
pub fn paint_mcp(painter: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let r = 2.5;
    let gap = 3.5;
    let left_c = center + Vec2::new(-gap, 0.0);
    let right_c = center + Vec2::new(gap, 0.0);
    painter.circle_stroke(left_c, r, stroke);
    painter.circle_stroke(right_c, r, stroke);
    painter.line_segment([left_c + Vec2::new(r, 0.0), right_c + Vec2::new(-r, 0.0)], stroke);
}

/// Paint Skills icon (four-spoke gear symbol).
pub fn paint_skills(painter: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let r = 2.0;
    painter.circle_stroke(center, r, stroke);
    let spoke = 4.0;
    painter.line_segment([center + Vec2::new(-spoke, 0.0), center + Vec2::new(-r, 0.0)], stroke);
    painter.line_segment([center + Vec2::new(spoke, 0.0), center + Vec2::new(r, 0.0)], stroke);
    painter.line_segment([center + Vec2::new(0.0, -spoke), center + Vec2::new(0.0, -r)], stroke);
    painter.line_segment([center + Vec2::new(0.0, spoke), center + Vec2::new(0.0, r)], stroke);
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
