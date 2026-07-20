//! Pretext integration — text measurement backend for `pretext_core`.
//!
//! This module provides an egui-native [`FontMetrics`] implementation so that
//! pretext's line-breaking decisions are based on the exact same font stack
//! that egui uses to render text. This avoids the font-resolution mismatches
//! that would come from using `pretext-fontdb` with a separate system-font
//! query.
//!
//! Current scope (PoC):
//! - `EguiFontMetrics` maps `pretext_core::Font` to `egui::FontId` and measures
//!   widths via `egui::Fonts::layout_no_wrap`.
//! - Helper `font_for_style` builds common `pretext_core::Font` descriptors from
//!   the current `Theme` tokens.

use pretext_core::{Font, FontMetrics};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A `pretext_core::FontMetrics` backend backed by egui's own font atlas.
///
/// Holding an `egui::Context` lets us call into `egui::Fonts` during the
/// `prepare()` phase. Because the measurement uses the same font stack that
/// will later draw the text, width predictions should match rendered output
/// as closely as egui's own layout engine allows.
///
/// `Font` → `egui::FontId` mappings are cached because the mapping involves
/// string normalization (lowercase family lookup, weight threshold) and is
/// called for every span during pretext preparation/layout.
#[derive(Debug, Clone)]
pub struct EguiFontMetrics {
    ctx: egui::Context,
    font_id_cache: Arc<Mutex<HashMap<Font, egui::FontId>>>,
}

impl EguiFontMetrics {
    /// Create a new backend bound to the given egui context.
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            ctx,
            font_id_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Calibrated line height for the theme's body font.
    ///
    /// Queries font metrics from the same egui context used for text
    /// measurement, ensuring height estimates match rendered output.
    #[allow(dead_code)] // gated behind cfg(feature = "line-mode")
    pub fn line_height(&self, theme: &crate::theme::Theme) -> f32 {
        let font_id = theme.font(theme.text_base);
        self.ctx.fonts_mut(|f| f.row_height(&font_id))
    }

    pub(crate) fn to_egui_font_id(&self, font: &Font) -> egui::FontId {
        // Fast path: cache hit avoids repeated lowercase/contains checks.
        if let Ok(cache) = self.font_id_cache.lock() {
            if let Some(id) = cache.get(font) {
                return id.clone();
            }
        }

        let size = font.size_px;
        let family_lower = font.family.to_lowercase();
        let is_monospace = family_lower.contains("mono")
            || family_lower.contains("code")
            || family_lower.contains("jetbrains");

        let id = if is_monospace {
            egui::FontId::new(size, egui::FontFamily::Monospace)
        } else if font.weight.0 >= 500 {
            egui::FontId::new(size, egui::FontFamily::Name("bold".into()))
        } else {
            egui::FontId::new(size, egui::FontFamily::Proportional)
        };

        if let Ok(mut cache) = self.font_id_cache.lock() {
            cache.insert(font.clone(), id.clone());
        }
        id
    }
}

impl FontMetrics for EguiFontMetrics {
    fn measure(&self, text: &str, font: &Font) -> f32 {
        let font_id = self.to_egui_font_id(font);
        self.ctx.fonts_mut(|fonts| {
            // Color is irrelevant for width measurement; white is a safe neutral.
            // ponytail: `layout_no_wrap` takes an owned `String`; the allocation
            // is unavoidable at this API boundary. Higher-level widgets cache
            // line layouts so this is only called during cold-path estimation.
            fonts
                .layout_no_wrap(text.to_string(), font_id, egui::Color32::WHITE)
                .rect
                .width()
        })
    }

    fn supports_char(&self, c: char, font: &Font) -> bool {
        let font_id = self.to_egui_font_id(font);
        self.ctx
            .fonts_mut(|fonts| fonts.glyph_width(&font_id, c) > 0.0)
    }
}

impl EguiFontMetrics {
    /// Row height for the given pretext font descriptor, as reported by egui.
    pub(crate) fn row_height(&self, font: &Font) -> f32 {
        let font_id = self.to_egui_font_id(font);
        self.ctx.fonts_mut(|fonts| fonts.row_height(&font_id))
    }
}

/// Build a `pretext_core::Font` descriptor for the UI body style.
pub fn font_body(theme: &crate::theme::Theme) -> Font {
    Font::new(format!("{}px {}", theme.text_base, theme.font_body))
}

/// Build a `pretext_core::Font` descriptor for code/monospace text.
pub fn font_code(theme: &crate::theme::Theme) -> Font {
    Font::new(format!("{}px JetBrains Mono", theme.text_sm))
}

/// Build a `pretext_core::Font` descriptor for bold body text.
pub fn font_bold(theme: &crate::theme::Theme) -> Font {
    Font::new(format!("bold {}px {}", theme.text_base, theme.font_body))
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests run inside the egui binary test harness where a Context is
    // available; they verify the mapping logic rather than full layout.

    #[test]
    fn monospace_family_maps_to_egui_monospace() {
        let ctx = egui::Context::default();
        let metrics = EguiFontMetrics::new(ctx);
        let font = Font::new("14px JetBrains Mono");
        let id = metrics.to_egui_font_id(&font);
        assert_eq!(id.family, egui::FontFamily::Monospace);
        assert_eq!(id.size, 14.0);
    }

    #[test]
    fn bold_weight_maps_to_bold_family() {
        let ctx = egui::Context::default();
        let metrics = EguiFontMetrics::new(ctx);
        let font = Font::new("bold 16px Inter");
        let id = metrics.to_egui_font_id(&font);
        assert_eq!(id.family, egui::FontFamily::Name("bold".into()));
        assert_eq!(id.size, 16.0);
    }

    #[test]
    fn normal_weight_maps_to_proportional() {
        let ctx = egui::Context::default();
        let metrics = EguiFontMetrics::new(ctx);
        let font = Font::new("14px Inter");
        let id = metrics.to_egui_font_id(&font);
        assert_eq!(id.family, egui::FontFamily::Proportional);
        assert_eq!(id.size, 14.0);
    }
}
