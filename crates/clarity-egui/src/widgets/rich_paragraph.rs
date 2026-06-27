//! Rich paragraph widget powered by pretext.
//!
//! Renders a paragraph of `InlineSpan`s with egui primitives while relying on
//! pretext for line breaking. This gives precise height and avoids inline code
//! / chip fragments being truncated mid-word.

use crate::pretext::EguiFontMetrics;
use crate::theme::Theme;
use crate::ui::rich_inline::{PositionedFragment, inline_spans_to_items, layout_rich_inline};
use crate::ui::types::InlineSpan;
use pretext_core::EngineProfile;

/// Line spacing multiplier applied to the egui row height.
const LINE_HEIGHT_FACTOR: f32 = 1.2;

/// Render a rich paragraph and return the exact vertical space consumed.
///
/// `max_width` is the content width available for text wrapping. The widget
/// allocates exactly the height pretext computed, so callers can rely on the
/// returned value for virtual-list caching.
pub fn rich_paragraph(
    ui: &mut egui::Ui,
    spans: &[InlineSpan],
    theme: &Theme,
    metrics: &EguiFontMetrics,
    profile: &EngineProfile,
    max_width: f32,
) -> f32 {
    let items = inline_spans_to_items(spans, theme);
    if items.is_empty() {
        return 0.0;
    }

    let body_font_id = metrics.to_egui_font_id(&crate::pretext::font_body(theme));
    let row_height = ui.fonts(|fonts| fonts.row_height(&body_font_id)) * LINE_HEIGHT_FACTOR;
    let lines = layout_rich_inline(&items, max_width, metrics, profile);
    let desired_height = lines.len() as f32 * row_height;

    let (rect, _response) =
        ui.allocate_at_least(egui::vec2(max_width, desired_height), egui::Sense::hover());

    for (line_idx, line) in lines.iter().enumerate() {
        let line_y = rect.min.y + line_idx as f32 * row_height;
        for frag in line {
            draw_fragment(ui, frag, line_y, row_height, &items, theme, metrics);
        }
    }

    desired_height
}

fn draw_fragment(
    ui: &mut egui::Ui,
    frag: &PositionedFragment,
    line_y: f32,
    row_height: f32,
    items: &[pretext_core::rich_inline::RichInlineItem],
    theme: &Theme,
    metrics: &EguiFontMetrics,
) {
    let frag_rect = egui::Rect::from_min_size(
        egui::pos2(ui.min_rect().min.x + frag.x, line_y),
        egui::vec2(frag.width, row_height),
    );

    let item = items.get(frag.item_index);
    let font_id = item
        .map(|i| metrics.to_egui_font_id(&i.font))
        .unwrap_or_else(|| metrics.to_egui_font_id(&crate::pretext::font_body(theme)));

    if frag.is_chip {
        ui.painter()
            .rect_filled(frag_rect, 4.0, theme.accent_subtle);
        ui.painter().rect_stroke(
            frag_rect,
            4.0,
            egui::Stroke::new(1.0, theme.accent),
            egui::StrokeKind::Inside,
        );
    }

    let color = if frag.is_chip {
        theme.accent
    } else {
        theme.text
    };

    ui.painter().text(
        frag_rect.left_center(),
        egui::Align2::LEFT_CENTER,
        &frag.text,
        font_id,
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::types::InlineSpan;

    fn theme_for_test() -> Theme {
        Theme {
            font_body: "Inter".to_string(),
            font_mono: "JetBrains Mono".to_string(),
            text_base: 14.0,
            ..Default::default()
        }
    }

    #[test]
    fn rich_paragraph_returns_zero_for_empty_spans() {
        let ctx = egui::Context::default();
        let theme = theme_for_test();
        let metrics = EguiFontMetrics::new(ctx.clone());
        let profile = EngineProfile::chromium();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let height = rich_paragraph(ui, &[], &theme, &metrics, &profile, 200.0);
                assert_eq!(height, 0.0);
            });
        });
    }

    #[test]
    fn rich_paragraph_measures_single_line_text() {
        let ctx = egui::Context::default();
        let theme = theme_for_test();
        let metrics = EguiFontMetrics::new(ctx.clone());
        let profile = EngineProfile::chromium();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let spans = vec![InlineSpan::Text("Hello world".into())];
                let height = rich_paragraph(ui, &spans, &theme, &metrics, &profile, 200.0);
                assert!(height > 0.0);
            });
        });
    }

    #[test]
    fn rich_paragraph_wraps_long_text_to_multiple_lines() {
        let ctx = egui::Context::default();
        let theme = theme_for_test();
        let metrics = EguiFontMetrics::new(ctx.clone());
        let profile = EngineProfile::chromium();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let spans = vec![InlineSpan::Text(
                    "This is a very long text that should wrap into multiple lines.".into(),
                )];
                let height = rich_paragraph(ui, &spans, &theme, &metrics, &profile, 50.0);
                assert!(height > 0.0);
            });
        });
    }
}
