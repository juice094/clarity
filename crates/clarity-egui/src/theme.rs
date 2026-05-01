use eframe::egui;

// ============================================================================
// Design Token System — Phase A Foundation
// ============================================================================
// Accent: Warm copper (#c98a5e) — chosen for low blue-light content and
// long-session visual comfort. Backgrounds use warm slate (dark) / warm
// off-white (light) instead of pure black/white to reduce pupil strain.
// ============================================================================

/// Complete design token set for a single theme variant.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Theme {
    // --- Backgrounds & Surfaces ---
    pub bg: egui::Color32,
    pub bg_accent: egui::Color32,
    pub bg_elevated: egui::Color32,
    pub bg_hover: egui::Color32,
    pub surface: egui::Color32,
    pub surface_strong: egui::Color32,

    // --- Text ---
    pub text: egui::Color32,
    pub text_strong: egui::Color32,
    pub text_muted: egui::Color32,
    pub text_dim: egui::Color32,

    // --- Accents ---
    pub accent: egui::Color32,
    pub accent_hover: egui::Color32,
    pub accent_subtle: egui::Color32,

    // --- Chat ---
    pub user_bubble: egui::Color32,
    pub ai_bubble: egui::Color32,
    pub chat_text: egui::Color32,
    pub error_bubble: egui::Color32,
    pub error_text: egui::Color32,

    // --- Status ---
    pub status_online: egui::Color32,
    pub status_busy: egui::Color32,
    pub status_offline: egui::Color32,
    pub ok: egui::Color32,
    pub warn: egui::Color32,
    pub danger: egui::Color32,

    // --- Borders & Inputs ---
    pub border: egui::Color32,
    pub border_strong: egui::Color32,
    pub border_hover: egui::Color32,
    pub input_bg: egui::Color32,

    // --- Focus & Effects ---
    pub focus_ring: egui::Color32,
    pub focus_glow: egui::Color32,
    pub selection: egui::Color32,

    // --- Overlay (transparency layers for modal scrims and depth hints) ---
    /// Modal backdrop / scrim (e.g. dim the content behind a settings dialog).
    pub overlay: egui::Color32,
    /// Faint white/black layer for component hover / card glow.
    pub overlay_subtle: egui::Color32,
    pub overlay_light: egui::Color32,
    pub overlay_medium: egui::Color32,
    pub overlay_strong: egui::Color32,

    // --- Fonts ---
    pub font_body: String,
    pub font_mono: String,

    // --- Typography (semantic size tokens) ---
    pub text_xs: f32,
    pub text_sm: f32,
    pub text_base: f32,
    pub text_lg: f32,
    pub text_xl: f32,
    pub text_2xl: f32,

    // --- Spacing (8 px baseline grid: 4/8/12/16/20/24/40 px) ---
    pub space_4: f32,
    pub space_8: f32,
    pub space_12: f32,
    pub space_16: f32,
    pub space_20: f32,
    pub space_24: f32,
    /// Section-level spacing (5× baseline, for large block separators / empty states).
    pub space_40: f32,

    // --- Radius (6/10/12/9999 px scale) ---
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    pub radius_full: f32,

    // --- Semantic surface (content-type backgrounds beyond chat bubbles) ---
    /// Tool call lifecycle indicator bg (distinct from chat bubbles).
    pub tool_call_bg: egui::Color32,
    /// Code block background in markdown rendering.
    pub code_block_bg: egui::Color32,
    /// Agent mood / status message background.
    pub mood_bg: egui::Color32,

    // --- Shadow (z-depth hierarchy: card → panel → modal → toast) ---
    pub shadow_card: egui::Shadow,
    pub shadow_panel: egui::Shadow,
    pub shadow_modal: egui::Shadow,
    pub shadow_toast: egui::Shadow,

    // --- Animation ---
    pub duration_fast: f32,
    pub duration_normal: f32,
    pub duration_slow: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[allow(dead_code)]
impl Theme {
    /// Dark theme — deep navy-black + copper accent.
    ///
    /// Design rationale:
    /// - Backgrounds use deep blue-gray (not pure black) to reduce pupil strain
    ///   while maintaining an crisp, technical feel.
    /// - Warm copper accent provides contrast against the cool background.
    /// - Text is slightly warm to balance the cool bg.
    pub fn dark() -> Self {
        Self {
            // Backgrounds: deep blue-gray — cool, technical, depth
            bg: hex("#12141e"),
            bg_accent: hex("#181a26"),
            bg_elevated: hex("#1e2030"),
            bg_hover: hex("#282a3a"),
            surface: hex("#181a26"),
            surface_strong: hex("#1e2030"),

            // Text: warm off-white balances the cool bg
            text: hex("#e4e3e8"),
            text_strong: hex("#f0eff4"),
            text_muted: hex("#9493a0"),
            text_dim: hex("#6c6b78"),

            // Accent: Warm copper — contrast against cool bg
            accent: hex("#c98a5e"),
            accent_hover: hex("#d4a07a"),
            accent_subtle: hex_alpha("#c98a5e", 0.12),

            // Chat bubbles
            user_bubble: hex("#c98a5e"),
            ai_bubble: hex("#1e2030"),
            chat_text: hex("#dad9e0"),
            error_bubble: hex_alpha("#c97060", 0.15),
            error_text: hex("#f0eff4"),

            // Status: warm-muted
            status_online: hex("#6bb87a"),
            status_busy: hex("#d4a050"),
            status_offline: hex("#c97060"),
            ok: hex("#6bb87a"),
            warn: hex("#d4a050"),
            danger: hex("#c97060"),

            // Borders: cool-tinted
            border: hex("#2a2c3e"),
            border_strong: hex("#3a3c4e"),
            border_hover: hex("#4a4c5e"),
            input_bg: hex("#1e2030"),

            // Focus: accent-matched
            focus_ring: hex_alpha("#c98a5e", 0.25),
            focus_glow: hex_alpha("#c98a5e", 0.15),
            selection: hex_alpha("#c98a5e", 0.20),

            // Overlay: scrim + depth layers (white-on-black in dark theme)
            overlay: hex_alpha("#000000", 0.50),
            overlay_subtle: hex_alpha("#ffffff", 0.03),
            overlay_light: hex_alpha("#ffffff", 0.06),
            overlay_medium: hex_alpha("#ffffff", 0.10),
            overlay_strong: hex_alpha("#ffffff", 0.18),

            // Fonts
            font_body: "Inter".into(),
            font_mono: "JetBrains Mono".into(),

            // Typography tokens
            text_xs: 10.0,
            text_sm: 12.0,
            text_base: 14.0,
            text_lg: 16.0,
            text_xl: 20.0,
            text_2xl: 26.0,

            // Spacing: 8px baseline grid
            space_4: 4.0,
            space_8: 8.0,
            space_12: 12.0,
            space_16: 16.0,
            space_20: 20.0,
            space_24: 24.0,
            space_40: 40.0,

            // Radius
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_full: 9999.0,

            // Semantic surfaces
            tool_call_bg: hex_alpha("#c98a5e", 0.08),
            code_block_bg: hex("#191b27"),
            mood_bg: hex_alpha("#d4a07a", 0.06),

            // Shadow: z-depth hierarchy
            shadow_card: egui::Shadow {
                offset: [0, 1],
                blur: 3,
                spread: 0,
                color: hex_alpha("#000000", 0.15),
            },
            shadow_panel: egui::Shadow {
                offset: [0, 2],
                blur: 8,
                spread: 0,
                color: hex_alpha("#000000", 0.20),
            },
            shadow_modal: egui::Shadow {
                offset: [0, 8],
                blur: 24,
                spread: 0,
                color: hex_alpha("#000000", 0.25),
            },
            shadow_toast: egui::Shadow {
                offset: [0, 4],
                blur: 12,
                spread: 0,
                color: hex_alpha("#000000", 0.20),
            },

            // Animation
            duration_fast: 0.10,
            duration_normal: 0.18,
            duration_slow: 0.30,
        }
    }

    /// OLED Black theme — pure black base for OLED screens + copper accent.
    ///
    /// Design rationale:
    /// - True black (#000000) background for OLED pixel-off immersion.
    /// - Elevated surfaces use subtle warm grays to avoid pure-black flatness.
    /// - Same copper accent as dark theme for brand consistency.
    pub fn oled_black() -> Self {
        Self {
            // Backgrounds: OLED pure black with warm gray elevations
            bg: hex("#000000"),
            bg_accent: hex("#0a0a0e"),
            bg_elevated: hex("#141418"),
            bg_hover: hex("#1e1e22"),
            surface: hex("#0a0a0e"),
            surface_strong: hex("#141418"),

            // Text: slightly warmer than dark theme for contrast against true black
            text: hex("#e8e7ec"),
            text_strong: hex("#ffffff"),
            text_muted: hex("#8e8d98"),
            text_dim: hex("#5c5b68"),

            // Accent: same warm copper
            accent: hex("#c98a5e"),
            accent_hover: hex("#d4a07a"),
            accent_subtle: hex_alpha("#c98a5e", 0.12),

            // Chat bubbles
            user_bubble: hex("#c98a5e"),
            ai_bubble: hex("#141418"),
            chat_text: hex("#d8d7dc"),
            error_bubble: hex_alpha("#c97060", 0.15),
            error_text: hex("#f0eff4"),

            // Status: same palette
            status_online: hex("#6bb87a"),
            status_busy: hex("#d4a050"),
            status_offline: hex("#c97060"),
            ok: hex("#6bb87a"),
            warn: hex("#d4a050"),
            danger: hex("#c97060"),

            // Borders: warmer tinted against black
            border: hex("#1e1e26"),
            border_strong: hex("#2e2e38"),
            border_hover: hex("#3e3e48"),
            input_bg: hex("#141418"),

            // Focus: accent-matched
            focus_ring: hex_alpha("#c98a5e", 0.25),
            focus_glow: hex_alpha("#c98a5e", 0.15),
            selection: hex_alpha("#c98a5e", 0.20),

            // Overlay: scrim + depth layers
            overlay: hex_alpha("#000000", 0.60),
            overlay_subtle: hex_alpha("#ffffff", 0.03),
            overlay_light: hex_alpha("#ffffff", 0.06),
            overlay_medium: hex_alpha("#ffffff", 0.10),
            overlay_strong: hex_alpha("#ffffff", 0.18),

            // Fonts
            font_body: "Inter".into(),
            font_mono: "JetBrains Mono".into(),

            // Typography tokens
            text_xs: 10.0,
            text_sm: 12.0,
            text_base: 14.0,
            text_lg: 16.0,
            text_xl: 20.0,
            text_2xl: 26.0,

            // Spacing: 8px baseline grid
            space_4: 4.0,
            space_8: 8.0,
            space_12: 12.0,
            space_16: 16.0,
            space_20: 20.0,
            space_24: 24.0,
            space_40: 40.0,

            // Radius
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_full: 9999.0,

            // Semantic surfaces
            tool_call_bg: hex_alpha("#c98a5e", 0.08),
            code_block_bg: hex("#0f0f14"),
            mood_bg: hex_alpha("#d4a07a", 0.06),

            // Shadow: z-depth hierarchy (stronger against pure black)
            shadow_card: egui::Shadow {
                offset: [0, 1],
                blur: 4,
                spread: 0,
                color: hex_alpha("#000000", 0.30),
            },
            shadow_panel: egui::Shadow {
                offset: [0, 2],
                blur: 10,
                spread: 0,
                color: hex_alpha("#000000", 0.35),
            },
            shadow_modal: egui::Shadow {
                offset: [0, 8],
                blur: 28,
                spread: 0,
                color: hex_alpha("#000000", 0.40),
            },
            shadow_toast: egui::Shadow {
                offset: [0, 4],
                blur: 14,
                spread: 0,
                color: hex_alpha("#000000", 0.35),
            },

            // Animation
            duration_fast: 0.10,
            duration_normal: 0.18,
            duration_slow: 0.30,
        }
    }

    /// Light theme — cool off-white with copper accent.
    pub fn light() -> Self {
        Self {
            // Backgrounds: cool off-white
            bg: hex("#f0f1f6"),
            bg_accent: hex("#e8eaf0"),
            bg_elevated: hex("#e0e2ea"),
            bg_hover: hex("#d6d8e0"),
            surface: hex("#e8eaf0"),
            surface_strong: hex("#e0e2ea"),

            // Text: cool dark
            text: hex("#1e1d24"),
            text_strong: hex("#121118"),
            text_muted: hex("#6c6a76"),
            text_dim: hex("#9a98a4"),

            // Accent: same warm copper as dark theme
            accent: hex("#c98a5e"),
            accent_hover: hex("#b87a4e"),
            accent_subtle: hex_alpha("#c98a5e", 0.08),

            // Chat bubbles
            user_bubble: hex("#c98a5e"),
            ai_bubble: hex("#e8eaf0"),
            chat_text: hex("#1e1d24"),
            error_bubble: hex_alpha("#c97060", 0.10),
            error_text: hex("#1e1d24"),

            // Status: same palette as dark, works on light bg
            status_online: hex("#6bb87a"),
            status_busy: hex("#d4a050"),
            status_offline: hex("#c97060"),
            ok: hex("#6bb87a"),
            warn: hex("#d4a050"),
            danger: hex("#c97060"),

            // Borders: warm-tinted
            border: hex("#d0d2da"),
            border_strong: hex("#b8bac6"),
            border_hover: hex("#a0a3b2"),
            input_bg: hex("#e8eaf0"),

            // Focus: accent-matched
            focus_ring: hex_alpha("#c98a5e", 0.20),
            focus_glow: hex_alpha("#c98a5e", 0.10),
            selection: hex_alpha("#c98a5e", 0.20),

            // Overlay: scrim + depth layers (black-on-white in light theme)
            overlay: hex_alpha("#000000", 0.35),
            overlay_subtle: hex_alpha("#000000", 0.03),
            overlay_light: hex_alpha("#000000", 0.06),
            overlay_medium: hex_alpha("#000000", 0.10),
            overlay_strong: hex_alpha("#000000", 0.18),

            font_body: "Inter".into(),
            font_mono: "JetBrains Mono".into(),

            // Typography tokens
            text_xs: 10.0,
            text_sm: 12.0,
            text_base: 14.0,
            text_lg: 16.0,
            text_xl: 20.0,
            text_2xl: 26.0,

            space_4: 4.0,
            space_8: 8.0,
            space_12: 12.0,
            space_16: 16.0,
            space_20: 20.0,
            space_24: 24.0,
            space_40: 40.0,

            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_full: 9999.0,

            // Semantic surfaces
            tool_call_bg: hex_alpha("#c98a5e", 0.06),
            code_block_bg: hex("#e0e2ea"),
            mood_bg: hex_alpha("#c98a5e", 0.04),

            // Shadow: z-depth hierarchy
            shadow_card: egui::Shadow {
                offset: [0, 1],
                blur: 3,
                spread: 0,
                color: hex_alpha("#000000", 0.08),
            },
            shadow_panel: egui::Shadow {
                offset: [0, 2],
                blur: 8,
                spread: 0,
                color: hex_alpha("#000000", 0.10),
            },
            shadow_modal: egui::Shadow {
                offset: [0, 8],
                blur: 24,
                spread: 0,
                color: hex_alpha("#000000", 0.15),
            },
            shadow_toast: egui::Shadow {
                offset: [0, 4],
                blur: 12,
                spread: 0,
                color: hex_alpha("#000000", 0.12),
            },

            duration_fast: 0.10,
            duration_normal: 0.18,
            duration_slow: 0.30,
        }
    }

    /// Apply theme to egui context visuals.
    pub fn apply(&self, style: &mut egui::Style) {
        style.visuals.override_text_color = Some(self.text);
        style.visuals.panel_fill = self.bg_accent;
        style.visuals.window_fill = self.surface;
        style.visuals.extreme_bg_color = self.bg_accent;
        style.visuals.widgets.inactive.weak_bg_fill = self.surface;
        style.visuals.widgets.inactive.bg_fill = self.surface;
        style.visuals.widgets.hovered.weak_bg_fill = self.bg_hover;
        style.visuals.widgets.hovered.bg_fill = self.bg_hover;
        style.visuals.widgets.active.bg_fill = self.bg_hover;
        style.visuals.selection.bg_fill = self.selection;
        style.visuals.selection.stroke = egui::Stroke::new(1.0, self.text_strong);
        style.visuals.window_corner_radius = egui::CornerRadius::same(self.radius_lg as u8);
        style.visuals.window_shadow = self.shadow_panel;
        style.visuals.popup_shadow = self.shadow_panel;
        style.visuals.window_stroke = egui::Stroke::NONE;
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, self.text);
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, self.text_strong);
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, self.text_strong);
    }

    /// Create a frame for chat bubbles.
    pub fn bubble_frame(&self, is_user: bool) -> egui::Frame {
        let fill = if is_user {
            self.user_bubble
        } else {
            self.ai_bubble
        };
        egui::Frame::group(&egui::Style::default())
            .fill(fill)
            .corner_radius(egui::CornerRadius::same(self.radius_lg as u8))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(14, 10))
    }

    /// Create a frame for cards / panels.
    pub fn card_frame(&self) -> egui::Frame {
        egui::Frame::group(&egui::Style::default())
            .fill(self.surface)
            .corner_radius(egui::CornerRadius::same(self.radius_md as u8))
            .stroke(egui::Stroke::new(1.0, self.border))
            .shadow(self.shadow_card)
    }

    /// Create a frame for the sidebar.
    pub fn sidebar_frame(&self) -> egui::Frame {
        egui::Frame::side_top_panel(&egui::Style::default()).fill(self.bg_accent)
    }

    /// Create a button with theme styling.
    pub fn primary_button(&self, text: impl Into<egui::WidgetText>) -> egui::Button<'_> {
        egui::Button::new(text)
            .fill(self.accent)
            .corner_radius(egui::CornerRadius::same(self.radius_sm as u8))
    }

    pub fn secondary_button(&self, text: impl Into<egui::WidgetText>) -> egui::Button<'_> {
        egui::Button::new(text)
            .fill(self.surface)
            .corner_radius(egui::CornerRadius::same(self.radius_sm as u8))
    }

    pub fn ghost_button(&self, text: impl Into<egui::WidgetText>) -> egui::Button<'_> {
        egui::Button::new(text)
            .fill(egui::Color32::TRANSPARENT)
            .corner_radius(egui::CornerRadius::same(self.radius_sm as u8))
    }
}

// ---- Helpers ----

fn hex(s: &str) -> egui::Color32 {
    let s = s.trim_start_matches('#');
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
    egui::Color32::from_rgb(r, g, b)
}

fn hex_alpha(s: &str, alpha: f32) -> egui::Color32 {
    let base = hex(s);
    let a = (alpha * 255.0).clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgba_premultiplied(base.r(), base.g(), base.b(), a)
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates = [
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
        "C:\\Windows\\Fonts\\msyhbd.ttc",
    ];
    for path in &candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let name = std::path::Path::new(path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push(name.clone());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push(name);
            tracing::info!("Loaded CJK font from {}", path);
            break;
        }
    }
    ctx.set_fonts(fonts);
}

// ============================================================================
// Unit tests for theme construction and color helpers
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme_construction() {
        let t = Theme::dark();
        // Backgrounds should be dark (value < 0.2 in any channel heuristic)
        assert!(t.bg.r() < 60, "dark bg expected");
        assert!(t.text.r() > 200, "bright text expected");
        assert!(t.accent.g() > 80, "accent should have some green component");
    }

    #[test]
    fn test_light_theme_construction() {
        let t = Theme::light();
        assert!(t.bg.r() >= 240, "light bg expected");
        assert!(t.text.r() < 50, "dark text expected");
    }

    #[test]
    fn test_hex_parsing() {
        let c = hex("#8b5cf6");
        assert_eq!(c.r(), 139);
        assert_eq!(c.g(), 92);
        assert_eq!(c.b(), 246);
    }

    #[test]
    fn test_apply_does_not_panic() {
        let t = Theme::dark();
        let mut style = egui::Style::default();
        t.apply(&mut style);
        assert_eq!(style.visuals.override_text_color, Some(t.text));
    }
}
