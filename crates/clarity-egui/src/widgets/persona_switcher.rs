//! Persona switcher pill — S8 P3B.1.
//!
//! A compact titlebar control that surfaces the currently active Clarity
//! persona (Kin / Analyst / Programmer / …) and lets the user swap it
//! via a popup dropdown.
//!
//! ## Visual model
//!
//! ```text
//! ┌───────────────────┐
//! │ K  Kin        ▾ │   ← pill button, accent-tinted when popup open
//! └───────────────────┘
//!         ▼ click expands
//! ┌─────────────────────────────────────────┐
//! │ K  Kin                              │
//! │    Default reasoner — balanced …       │   ← active (highlighted)
//! ├─────────────────────────────────────────┤
//! │ A  Analyst                              │
//! │    Data persona — tables, SQL, …       │
//! ├─────────────────────────────────────────┤
//! │ P  Programmer                           │
//! │    Code persona — generation, …        │
//! └─────────────────────────────────────────┘
//! ```
//!
//! The widget is *visual-only*; it returns the user's intent as a
//! [`PersonaSwitcherResponse`] and the caller is responsible for mutating
//! the active id and triggering persistence.
//!
//! ## Design notes
//!
//! - **Icon fallback**: we deliberately do NOT consume the descriptor's
//!   `icon` field yet. The first letter of `display_name` + the per-persona
//!   `accent` color is sufficient identity. Lucide-name → font glyph
//!   mapping is left for a future Sprint.
//! - **Popup**: rendered via [`egui::Area`] anchored below the pill so the
//!   titlebar's clipping rect does not truncate it.
//! - **No internal state**: open/close state lives in `UiStore`
//!   (`persona_switcher_open`) so it survives across recompositions and
//!   can be programmatically toggled by future keyboard shortcuts
//!   (e.g. `Ctrl+P`).

use crate::theme::Theme;
use clarity_core::endpoint::{EndpointDescriptor, EndpointRegistry};
use egui::{Color32, Sense, Stroke, StrokeKind};

/// Outcome of one frame of the persona switcher.
pub struct PersonaSwitcherResponse {
    /// User clicked the pill — caller should toggle the open/close state.
    pub toggle_clicked: bool,
    /// User selected a new persona id (only set when actually changed).
    pub selected: Option<String>,
    /// Popup should close (click-outside, escape, or selection).
    pub close_requested: bool,
}

const PILL_HEIGHT: f32 = 26.0;
const PILL_MIN_WIDTH: f32 = 96.0;
const PILL_MAX_WIDTH: f32 = 160.0;
const POPUP_WIDTH: f32 = 280.0;
const ROW_HEIGHT: f32 = 44.0;

/// Render the persona pill and (if `is_open`) the popup below it.
///
/// `active_id` should match exactly one descriptor in `registry`; if not,
/// the widget falls back to rendering the first registered entry.
pub fn persona_switcher(
    ui: &mut egui::Ui,
    theme: &Theme,
    registry: &EndpointRegistry,
    active_id: &str,
    is_open: bool,
) -> PersonaSwitcherResponse {
    let mut response = PersonaSwitcherResponse {
        toggle_clicked: false,
        selected: None,
        close_requested: false,
    };

    let active = match registry.get(active_id).or_else(|| registry.iter().next()) {
        Some(d) => d,
        None => return response, // empty registry — nothing to render
    };

    // ── Pill ─────────────────────────────────────────────────────────────
    let pill_width = available_pill_width(ui);
    let pill_rect = render_pill(ui, theme, active, pill_width, is_open);
    let pill_resp = ui.interact(
        pill_rect,
        ui.id().with(("persona_switcher_pill", active_id)),
        Sense::click(),
    );
    if pill_resp.clicked() {
        response.toggle_clicked = true;
    }

    // ── Popup ────────────────────────────────────────────────────────────
    if is_open {
        let area_id = ui.id().with("persona_switcher_popup");
        let anchor = pill_rect.left_bottom() + egui::vec2(0.0, theme.space_4);
        let area = egui::Area::new(area_id)
            .order(egui::Order::Foreground)
            .fixed_pos(anchor)
            .interactable(true);
        let area_resp = area.show(ui.ctx(), |ui| {
            render_popup(ui, theme, registry, active_id, &mut response);
        });

        // Click outside (and not on the pill) → request close.
        let ptr_pressed = ui.input(|i| i.pointer.any_pressed());
        let popup_hovered = area_resp.response.hovered();
        let pill_hovered = pill_resp.hovered();
        if ptr_pressed && !popup_hovered && !pill_hovered {
            response.close_requested = true;
        }

        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            response.close_requested = true;
        }
    }

    response
}

fn available_pill_width(ui: &egui::Ui) -> f32 {
    let avail = ui.available_width();
    avail.clamp(PILL_MIN_WIDTH, PILL_MAX_WIDTH)
}

/// Paint the pill button (background + accent dot + label + chevron) and
/// return its absolute rect so the caller can register interaction.
fn render_pill(
    ui: &mut egui::Ui,
    theme: &Theme,
    active: &EndpointDescriptor,
    pill_width: f32,
    is_open: bool,
) -> egui::Rect {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(pill_width, PILL_HEIGHT), Sense::hover());
    let hovered = ui.rect_contains_pointer(rect);

    let accent = parse_accent(active.accent.as_deref()).unwrap_or(theme.accent);
    let bg = if is_open {
        theme.bg_elevated
    } else if hovered {
        theme.bg_hover
    } else {
        theme.surface
    };
    let stroke_color = if is_open { accent } else { theme.border };

    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme.radius_md.round() as u8),
        bg,
        Stroke::new(1.0, stroke_color),
        StrokeKind::Inside,
    );

    // Accent dot: first letter of display_name on a colored circle.
    let letter = first_letter(&active.display_name);
    let dot_radius = (PILL_HEIGHT - 8.0) * 0.5;
    let dot_center = egui::pos2(rect.left() + theme.space_4 + dot_radius, rect.center().y);
    ui.painter().circle_filled(dot_center, dot_radius, accent);
    ui.painter().text(
        dot_center,
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::proportional(theme.text_xs),
        theme.bg,
    );

    // Display name — truncated if pill is narrow.
    let label_start_x = dot_center.x + dot_radius + theme.space_4;
    let chevron_w = theme.text_sm + theme.space_4;
    let label_w = (rect.right() - label_start_x - chevron_w - theme.space_4).max(0.0);
    if label_w > 8.0 {
        let label_rect = egui::Rect::from_min_size(
            egui::pos2(label_start_x, rect.top()),
            egui::vec2(label_w, PILL_HEIGHT),
        );
        ui.painter().text(
            label_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            truncate(&active.display_name, label_w, theme.text_sm),
            egui::FontId::proportional(theme.text_sm),
            theme.text_strong,
        );
    }

    // Chevron (static glyph — open/close state shown via stroke color).
    let chevron_pos = egui::pos2(rect.right() - theme.space_4, rect.center().y);
    ui.painter().text(
        chevron_pos,
        egui::Align2::RIGHT_CENTER,
        crate::theme::ICON_CARET_DOWN,
        theme.font_icon(theme.text_xs),
        theme.text_muted,
    );

    rect
}

/// Draw the popup body containing all persona rows.
fn render_popup(
    ui: &mut egui::Ui,
    theme: &Theme,
    registry: &EndpointRegistry,
    active_id: &str,
    out: &mut PersonaSwitcherResponse,
) {
    egui::Frame::popup(ui.style())
        .fill(theme.bg_elevated)
        .stroke(Stroke::new(1.0, theme.border))
        .corner_radius(egui::CornerRadius::same(theme.radius_md.round() as u8))
        .inner_margin(egui::Margin::same(theme.space_4 as i8))
        .show(ui, |ui| {
            ui.set_min_width(POPUP_WIDTH);
            for (idx, descriptor) in registry.iter().enumerate() {
                if idx > 0 {
                    ui.add_space(theme.space_4 * 0.5);
                }
                if render_row(ui, theme, descriptor, descriptor.id.as_str() == active_id) {
                    out.selected = Some(descriptor.id.as_str().to_string());
                    out.close_requested = true;
                }
            }
        });
}

/// One persona row in the popup. Returns true when clicked.
fn render_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    descriptor: &EndpointDescriptor,
    is_active: bool,
) -> bool {
    let (row_rect, row_resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), Sense::click());

    let hovered = row_resp.hovered();
    let bg = if is_active {
        theme.accent_subtle
    } else if hovered {
        theme.bg_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(
        row_rect,
        egui::CornerRadius::same(theme.radius_sm.round() as u8),
        bg,
    );

    let accent = parse_accent(descriptor.accent.as_deref()).unwrap_or(theme.accent);
    let letter = first_letter(&descriptor.display_name);
    let dot_radius = 12.0;
    let dot_center = egui::pos2(
        row_rect.left() + theme.space_4 + dot_radius,
        row_rect.center().y,
    );
    ui.painter().circle_filled(dot_center, dot_radius, accent);
    ui.painter().text(
        dot_center,
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::proportional(theme.text_md),
        theme.bg,
    );

    // Title + description column.
    let pad_y = theme.space_4 * 0.5;
    let text_x = dot_center.x + dot_radius + theme.space_4;
    ui.painter().text(
        egui::pos2(text_x, row_rect.top() + pad_y),
        egui::Align2::LEFT_TOP,
        descriptor.display_name.as_str(),
        egui::FontId::proportional(theme.text_md),
        theme.text_strong,
    );
    let desc_color = if is_active {
        theme.text
    } else {
        theme.text_muted
    };
    let desc_y = row_rect.top() + pad_y + theme.text_md + pad_y;
    let desc_max_w = (row_rect.right() - text_x - theme.space_4).max(0.0);
    ui.painter().text(
        egui::pos2(text_x, desc_y),
        egui::Align2::LEFT_TOP,
        truncate(&descriptor.description, desc_max_w, theme.text_xs),
        egui::FontId::proportional(theme.text_xs),
        desc_color,
    );

    // Capability count badge (top-right corner).
    let cap_label = format!("{} cap", descriptor.capabilities.len());
    ui.painter().text(
        egui::pos2(row_rect.right() - theme.space_4, row_rect.top() + pad_y),
        egui::Align2::RIGHT_TOP,
        cap_label,
        egui::FontId::proportional(theme.text_xs),
        theme.text_muted,
    );

    // Focus ring for keyboard navigation.
    if row_resp.has_focus() {
        ui.painter().rect_stroke(
            row_rect,
            egui::CornerRadius::same(theme.radius_sm.round() as u8),
            Stroke::new(2.0, theme.focus_ring),
            StrokeKind::Inside,
        );
    }

    row_resp.clicked()
}

/// Extract the first uppercase letter of the display name.
fn first_letter(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// Parse a hex `"#RRGGBB"` string into a `Color32`.
fn parse_accent(hex: Option<&str>) -> Option<Color32> {
    let s = hex?.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}

/// Truncate text to fit within `max_width` at the given font size by
/// estimating an average glyph width of `0.55 * font_size`.
fn truncate(text: &str, max_width: f32, font_size: f32) -> String {
    if max_width <= 0.0 {
        return String::new();
    }
    let approx_glyph_w = (font_size * 0.55).max(1.0);
    let max_chars = (max_width / approx_glyph_w).floor() as usize;
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_letter_uppercases_lowercase_input() {
        assert_eq!(first_letter("kin"), "K");
        assert_eq!(first_letter("analyst"), "A");
        assert_eq!(first_letter(""), "?");
    }

    #[test]
    fn parse_accent_accepts_six_digit_hex_with_hash() {
        let c = parse_accent(Some("#5B8DEF")).expect("valid hex");
        assert_eq!(c, Color32::from_rgb(0x5B, 0x8D, 0xEF));
    }

    #[test]
    fn parse_accent_rejects_malformed_input() {
        assert!(parse_accent(Some("5B8DEF")).is_none()); // missing #
        assert!(parse_accent(Some("#5B8D")).is_none()); // too short
        assert!(parse_accent(Some("#GGGGGG")).is_none()); // non-hex
        assert!(parse_accent(None).is_none());
    }

    #[test]
    fn truncate_returns_input_when_it_fits() {
        let s = truncate("Kin", 200.0, 14.0);
        assert_eq!(s, "Kin");
    }

    #[test]
    fn truncate_adds_ellipsis_when_too_long() {
        let s = truncate(
            "This is a very long description that does not fit",
            60.0,
            14.0,
        );
        assert!(s.ends_with('…'));
    }

    #[test]
    fn truncate_returns_empty_for_zero_width() {
        assert_eq!(truncate("anything", 0.0, 14.0), "");
    }
}
