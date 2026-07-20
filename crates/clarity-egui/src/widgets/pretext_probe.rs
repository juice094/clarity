//! Pretext measurement probe — calibration UI for the pretext PoC.
//!
//! Renders a debug overlay that compares pretext-predicted text widths with the
//! widths actually produced by egui's font stack. This is the fastest way to
//! validate whether pretext can safely drive egui layout decisions.

use crate::{App, theme::Theme};
use clarity_ui::design_system::{Space, TextStyle, gap, text};
use clarity_ui::widgets::overlay::{Overlay, overlay_scrim};
use pretext_core::{
    EngineProfile, Font, PrepareOptions, layout::layout_with_lines, layout::measure_natural_width,
};

const SAMPLES: &[&str] = &[
    "Hello world",
    "The quick brown fox jumps over the lazy dog.",
    "你好世界",
    "这是一段比较长的中文文本，用来测试 Pretext 的换行宽度预测。",
    "Mixed 中英混排 text with 中文插入.",
    "`inline_code` and @mention chip",
    "https://github.com/juice094/pretext-rust",
    "🦊 emoji 宽度 👨‍👩‍👧‍👦 test",
    "1234567890.1234567890",
    "Rust: fn main() { println!(\"hi\"); }",
];

const WRAP_SAMPLE: &str = "这是一段用于测试 Pretext 换行预测的中英混排文本。\
It contains English words, numbers 12345, and a URL https://example.com/path.";

/// Render the probe overlay. Call from `App::update` when
/// `app.context.ui_store.pretext_probe_open` is true.
pub fn render_pretext_probe(app: &mut App, ctx: &egui::Context) {
    let theme = app.context.ui_store.theme.clone();
    let mut close_requested = false;

    let scrim_response = overlay_scrim(ctx);

    Overlay::new("pretext_probe").width(720.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            text(ui, "Pretext Probe", TextStyle::Title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // ponytail: icon-button component does not yet expose per-icon colour.
                if ui
                    .button(
                        egui::RichText::new(crate::theme::ICON_X)
                            .font(theme.font_icon(theme.text_sm))
                            .color(theme.text_dim),
                    )
                    .clicked()
                {
                    close_requested = true;
                }
            });
        });
        gap(ui, Space::S1);

        text(
            ui,
            "Compare pretext predicted widths against egui actual widths.",
            TextStyle::Body,
        );
        // ponytail: hyperlink is not yet wrapped in clarity-ui.
        ui.hyperlink_to("PoC plan", "docs/planning/plans/pretext-poc-plan.md");
        // ponytail: Separator is not yet wrapped in clarity-ui.
        ui.separator();

        let font = pretext_font_body(&theme);
        let options = PrepareOptions::default();
        let profile = EngineProfile::chromium();
        let metrics = &app.pretext_metrics;
        let egui_font_id = egui::FontId::new(theme.text_base, egui::FontFamily::Proportional);

        // ponytail: ScrollArea is not yet wrapped in clarity-ui.
        egui::ScrollArea::both().show(ui, |ui| {
            ui.horizontal(|ui| {
                text(ui, "Sample", TextStyle::CaptionStrong);
                ui.add_space(120.0);
                text(ui, "Predicted", TextStyle::CaptionStrong);
                ui.add_space(40.0);
                text(ui, "Actual", TextStyle::CaptionStrong);
                ui.add_space(40.0);
                text(ui, "Δ", TextStyle::CaptionStrong);
            });
            ui.separator();

            for sample in SAMPLES {
                let predicted = measure_sample_width(sample, &font, metrics, &options, &profile);
                let actual = ui.fonts_mut(|fonts| {
                    fonts
                        .layout_no_wrap(
                            sample.to_string(),
                            egui_font_id.clone(),
                            egui::Color32::WHITE,
                        )
                        .rect
                        .width()
                });
                let delta = (predicted - actual).abs();
                let status_color = if delta < 2.0 {
                    theme.ok
                } else if delta < 5.0 {
                    theme.warn
                } else {
                    theme.danger
                };

                ui.horizontal(|ui| {
                    text(ui, truncate(sample, 28), TextStyle::Mono);
                    gap(ui, Space::S0);
                    text(ui, format!("{:>7.2}px", predicted), TextStyle::Body);
                    gap(ui, Space::S0);
                    text(ui, format!("{:>7.2}px", actual), TextStyle::Body);
                    gap(ui, Space::S0);
                    // ponytail: TextStyle has no semantic status-colour variants.
                    ui.label(
                        egui::RichText::new(format!("{:>6.2}px", delta))
                            .size(theme.text_sm)
                            .color(status_color),
                    );
                });
            }
        });

        ui.separator();

        // ── Wrap preview ──
        text(ui, "Wrap Preview", TextStyle::Subheading);
        let mut max_w = app.context.ui_store.pretext_probe_wrap_width;
        ui.horizontal(|ui| {
            text(ui, "Max width:", TextStyle::CaptionStrong);
            // ponytail: Slider is not yet wrapped in clarity-ui.
            ui.add(egui::Slider::new(&mut max_w, 200.0..=700.0));
        });
        app.context.ui_store.pretext_probe_wrap_width = max_w;

        let predicted = pretext_core::prepare_with_segments(WRAP_SAMPLE, &font, metrics, &options)
            .and_then(|p| layout_with_lines(&p, max_w, &profile))
            .ok();
        let row_height = ui.fonts_mut(|fonts| fonts.row_height(&egui_font_id));
        let predicted_lines = predicted.as_ref().map(|r| r.line_count).unwrap_or(0);
        let predicted_h = predicted_lines as f32 * row_height;

        let actual_response = ui
            .vertical(|ui| {
                ui.set_max_width(max_w);
                // ponytail: Label with explicit wrap is not yet wrapped in clarity-ui.
                ui.add(egui::Label::new(WRAP_SAMPLE).wrap());
            })
            .response;
        let actual_h = actual_response.rect.height();
        let actual_lines = (actual_h / row_height).round() as usize;

        ui.horizontal(|ui| {
            text(
                ui,
                format!("predicted {} lines / {:.1}px", predicted_lines, predicted_h),
                TextStyle::Body,
            );
            text(
                ui,
                format!("actual {} lines / {:.1}px", actual_lines, actual_h),
                TextStyle::Body,
            );
        });

        ui.separator();

        // ── Rich inline chip preview ──
        text(ui, "Rich Inline Chip Preview", TextStyle::Subheading);
        const CHIP_SAMPLE: &str =
            "Check `this_code` and @kimi mention, plus a @verylongmention that wraps whole.";
        let chip_items = crate::ui::rich_inline::build_rich_inline_items(
            &crate::ui::rich_inline::tokenize_inline(CHIP_SAMPLE),
            &theme,
        );
        let chip_lines =
            crate::ui::rich_inline::layout_rich_inline(&chip_items, max_w, metrics, &profile);
        let chip_row_height = ui.fonts_mut(|fonts| fonts.row_height(&egui_font_id));

        for line in chip_lines {
            let (line_rect, _) =
                ui.allocate_at_least([max_w, chip_row_height].into(), egui::Sense::hover());
            for frag in line {
                let frag_rect = egui::Rect::from_min_size(
                    line_rect.min + egui::vec2(frag.x, 0.0),
                    egui::vec2(frag.width, chip_row_height),
                );
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
                // ponytail: custom painter text is not yet wrapped in clarity-ui.
                ui.painter().text(
                    frag_rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    &frag.text,
                    egui_font_id.clone(),
                    theme.text,
                );
            }
        }

        ui.separator();
        text(
            ui,
            format!(
                "Mapped pretext font: {} {}px weight={}",
                font.family, font.size_px, font.weight.0
            ),
            TextStyle::Small,
        );
    });

    if close_requested
        || scrim_response.clicked()
        || ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        app.context.ui_store.pretext_probe_open = false;
    }
}

fn measure_sample_width(
    text: &str,
    font: &Font,
    metrics: &crate::pretext::EguiFontMetrics,
    options: &PrepareOptions,
    profile: &EngineProfile,
) -> f32 {
    match pretext_core::prepare_with_segments(text, font, metrics, options) {
        Ok(prepared) => measure_natural_width(&prepared, profile),
        Err(_) => 0.0,
    }
}

fn pretext_font_body(theme: &Theme) -> Font {
    Font::new(format!("{}px {}", theme.text_base, theme.font_body))
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_short_strings() {
        assert_eq!(truncate("hi", 10), "hi");
    }

    #[test]
    fn truncate_adds_ellipsis() {
        assert_eq!(truncate("abcdefghijklmnop", 10).chars().count(), 11); // 10 + '…'
    }

    /// Manual data-collection test: run with
    /// `cargo test -p clarity-egui --bin clarity-egui pretext_probe_data -- --ignored --nocapture`
    #[test]
    #[ignore = "manual data collection"]
    fn print_probe_data() {
        let ctx = egui::Context::default();
        // Fonts are not available until the first call to Context::run.
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            crate::theme::setup_fonts(ui.ctx());
        });
        crate::theme::setup_fonts(&ctx);
        let metrics = crate::pretext::EguiFontMetrics::new(ctx.clone());
        let theme = crate::theme::Theme::default();
        let font = pretext_font_body(&theme);
        let options = PrepareOptions::default();
        let profile = EngineProfile::chromium();
        let egui_font_id = egui::FontId::new(theme.text_base, egui::FontFamily::Proportional);

        println!("\n=== Pretext width calibration ===");
        println!(
            "{:<40} {:>10} {:>10} {:>8}",
            "sample", "predicted", "actual", "delta"
        );
        for sample in SAMPLES {
            let predicted = measure_sample_width(sample, &font, &metrics, &options, &profile);
            let actual = ctx.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(
                        sample.to_string(),
                        egui_font_id.clone(),
                        egui::Color32::WHITE,
                    )
                    .rect
                    .width()
            });
            println!(
                "{:<40} {:>10.2} {:>10.2} {:>8.2}",
                truncate(sample, 38),
                predicted,
                actual,
                (predicted - actual).abs()
            );
        }

        println!("\n=== Pretext wrap calibration ===");
        println!(
            "{:<6} {:>10} {:>10} {:>10} {:>10}",
            "max_w", "pred_lines", "pred_h", "act_lines", "act_h"
        );
        for max_w in [200.0_f32, 300.0, 400.0, 500.0, 600.0, 700.0] {
            let predicted =
                pretext_core::prepare_with_segments(WRAP_SAMPLE, &font, &metrics, &options)
                    .and_then(|p| layout_with_lines(&p, max_w, &profile))
                    .ok();
            let row_height = ctx.fonts_mut(|fonts| fonts.row_height(&egui_font_id));
            let pred_lines = predicted.as_ref().map(|r| r.line_count).unwrap_or(0);
            let pred_h = pred_lines as f32 * row_height;

            let galley = ctx.fonts_mut(|fonts| {
                fonts.layout(
                    WRAP_SAMPLE.to_string(),
                    egui_font_id.clone(),
                    egui::Color32::WHITE,
                    max_w,
                )
            });
            let act_lines = galley.rows.len();
            let act_h = galley.rect.height();
            println!(
                "{:<6.0} {:>10} {:>10.2} {:>10} {:>10.2}",
                max_w, pred_lines, pred_h, act_lines, act_h
            );
        }
    }
}
