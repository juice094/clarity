use eframe::egui;

// ============================================================================
// Phosphor Icons — Regular (unicode mapped from embedded Phosphor.ttf)
// ============================================================================
pub const ICON_SEND: &str = "\u{E394}";
pub const ICON_SETTINGS: &str = "\u{E270}";
pub const ICON_PLAY: &str = "\u{E3D0}";
pub const ICON_STOP: &str = "\u{E46C}";
pub const ICON_HOURGLASS: &str = "\u{E2B2}";
pub const ICON_CHECK: &str = "\u{E182}";
pub const ICON_X: &str = "\u{E4F6}";
pub const ICON_WARNING: &str = "\u{E4E0}";
pub const ICON_PAPERCLIP: &str = "\u{E39A}";
pub const ICON_LIST: &str = "\u{E2F0}";
pub const ICON_ARROW_LEFT: &str = "\u{E058}";
pub const ICON_PROHIBIT: &str = "\u{E3DE}";
pub const ICON_QUESTION: &str = "\u{E3E8}";
pub const ICON_CODE: &str = "\u{E1B4}";
pub const ICON_SEARCH: &str = "\u{E38C}";
pub const ICON_ROBOT: &str = "\u{E3B8}";
pub const ICON_CHEVRON_RIGHT: &str = "\u{E1A0}";
pub const ICON_COPY: &str = "\u{E1CA}";
pub const ICON_EDIT: &str = "\u{E3AE}";
pub const ICON_REFRESH: &str = "\u{E19E}";

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
    pub glass: egui::Color32,
    pub glass_strong: egui::Color32,

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
    pub text_md: f32,
    pub text_lg: f32,
    pub text_xl: f32,
    pub text_2xl: f32,

    // --- Font scale ---
    pub font_scale: f32,

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
    pub radius_xl: f32,
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
    /// Dark theme — Glassmorphism (dark glass) with ice-blue accent.
    ///
    /// Design rationale:
    /// - Deep black base (#050507) provides OLED-friendly contrast.
    /// - Semi-transparent surfaces with subtle white borders simulate glass
    ///   layers without backdrop-blur (not supported by egui).
    /// - Ice-blue accent (#5B8DEF) harmonises with the cool glass aesthetic.
    /// - Larger corner radii (12/20 px) reinforce the modern glass feel.
    pub fn dark() -> Self {
        Self {
            // Backgrounds: midnight slate (not pure black) + translucent glass layers
            // #12121a provides "ambient light" for depth perception while staying OLED-friendly
            bg: hex("#12121a"),
            bg_accent: rgba(28, 28, 38, 0.55),
            bg_elevated: rgba(42, 42, 56, 0.85),
            bg_hover: rgba(55, 55, 72, 0.75),
            surface: rgba(38, 38, 52, 0.60),
            surface_strong: rgba(48, 48, 64, 0.72),
            glass: rgba(255, 255, 255, 0.06),
            glass_strong: rgba(255, 255, 255, 0.12),

            // Text: high contrast on dark glass
            text: hex("#E8EAEF"),
            text_strong: hex("#FFFFFF"),
            text_muted: rgba(200, 205, 220, 0.72),
            text_dim: rgba(200, 205, 220, 0.50),

            // Accent: ice blue
            accent: hex("#5B8DEF"),
            accent_hover: hex("#7DA8F2"),
            accent_subtle: rgba(91, 141, 239, 0.14),

            // Chat bubbles: translucent glass
            user_bubble: rgba(45, 80, 160, 0.32),
            ai_bubble: rgba(255, 255, 255, 0.06),
            chat_text: hex("#E8EAEF"),
            error_bubble: rgba(239, 91, 91, 0.30),
            error_text: hex("#EF8A8A"),

            // Status: semantic palette
            status_online: hex("#6BCB8A"),
            status_busy: hex("#D4A050"),
            status_offline: hex("#EF6B6B"),
            ok: hex("#6BCB8A"),
            warn: hex("#D4A050"),
            danger: hex("#EF6B6B"),

            // Borders: semi-transparent white — increased opacity for boundary visibility
            border: rgba(255, 255, 255, 0.08),
            border_strong: rgba(255, 255, 255, 0.14),
            border_hover: rgba(255, 255, 255, 0.22),
            input_bg: rgba(24, 24, 34, 0.65),

            // Focus: blue glow
            focus_ring: rgba(91, 141, 239, 0.60),
            focus_glow: rgba(91, 141, 239, 0.20),
            selection: rgba(91, 141, 239, 0.45),

            // Overlay: scrim + depth layers
            overlay: hex_alpha("#000000", 0.50),
            overlay_subtle: rgba(255, 255, 255, 0.03),
            overlay_light: rgba(255, 255, 255, 0.06),
            overlay_medium: rgba(255, 255, 255, 0.10),
            overlay_strong: rgba(255, 255, 255, 0.18),

            // Fonts
            font_body: "Inter".into(),
            font_mono: "JetBrains Mono".into(),

            font_scale: 1.0,

            // Typography tokens — compact scale for desktop density
            text_xs: 9.0,
            text_sm: 11.0,
            text_base: 12.0,
            text_md: 13.0,
            text_lg: 15.0,
            text_xl: 18.0,
            text_2xl: 24.0,

            // Spacing: 8px baseline grid
            space_4: 4.0,
            space_8: 8.0,
            space_12: 12.0,
            space_16: 16.0,
            space_20: 20.0,
            space_24: 24.0,
            space_40: 40.0,

            // Radius: modern glassmorphism scale
            radius_sm: 8.0,
            radius_md: 16.0,
            radius_lg: 28.0,
            radius_xl: 36.0,
            radius_full: 999.0,

            // Semantic surfaces
            tool_call_bg: rgba(91, 141, 239, 0.08),
            code_block_bg: rgba(0, 0, 0, 0.40),
            mood_bg: rgba(91, 141, 239, 0.06),

            // Shadow: subtle dark depth for glass layers
            shadow_card: egui::Shadow {
                offset: [0, 1],
                blur: 8,
                spread: 0,
                color: rgba(0, 0, 0, 0.31),
            },
            shadow_panel: egui::Shadow {
                offset: [0, 2],
                blur: 16,
                spread: 0,
                color: rgba(0, 0, 0, 0.39),
            },
            shadow_modal: egui::Shadow {
                offset: [0, 8],
                blur: 32,
                spread: 0,
                color: rgba(0, 0, 0, 0.47),
            },
            shadow_toast: egui::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: rgba(0, 0, 0, 0.39),
            },

            // Animation
            duration_fast: 0.10,
            duration_normal: 0.18,
            duration_slow: 0.30,
        }
    }

    /// OLED Black theme — pure black base + Glassmorphism glass layers.
    ///
    /// Design rationale:
    /// - True black (#000000) background for OLED pixel-off immersion.
    /// - Glass surfaces use the same translucent palette as dark theme,
    ///   but appear more dramatic against the pure-black void.
    pub fn oled_black() -> Self {
        Self {
            // Backgrounds: OLED pure black with glass elevations
            // NOTE: this is the "true black" variant for OLED pixel-off;
            // the default dark() theme now uses midnight slate for better depth.
            bg: hex("#000000"),
            bg_accent: rgba(20, 20, 28, 0.35),
            bg_elevated: rgba(35, 35, 48, 0.80),
            bg_hover: rgba(45, 45, 62, 0.65),
            surface: rgba(32, 32, 45, 0.45),
            surface_strong: rgba(38, 38, 52, 0.60),
            glass: rgba(255, 255, 255, 0.04),
            glass_strong: rgba(255, 255, 255, 0.10),

            // Text: high contrast
            text: hex("#E8EAEF"),
            text_strong: hex("#FFFFFF"),
            text_muted: rgba(200, 205, 220, 0.70),
            text_dim: rgba(200, 205, 220, 0.55),

            // Accent: ice blue
            accent: hex("#5B8DEF"),
            accent_hover: hex("#7DA8F2"),
            accent_subtle: rgba(91, 141, 239, 0.12),

            // Chat bubbles
            user_bubble: rgba(45, 80, 160, 0.30),
            ai_bubble: rgba(255, 255, 255, 0.04),
            chat_text: hex("#E8EAEF"),
            error_bubble: rgba(239, 91, 91, 0.28),
            error_text: hex("#EF8A8A"),

            // Status
            status_online: hex("#6BCB8A"),
            status_busy: hex("#D4A050"),
            status_offline: hex("#EF6B6B"),
            ok: hex("#6BCB8A"),
            warn: hex("#D4A050"),
            danger: hex("#EF6B6B"),

            // Borders: glass reflection edge
            border: rgba(255, 255, 255, 0.04),
            border_strong: rgba(255, 255, 255, 0.08),
            border_hover: rgba(255, 255, 255, 0.14),
            input_bg: rgba(18, 18, 26, 0.60),

            // Focus: blue glow
            focus_ring: rgba(91, 141, 239, 0.60),
            focus_glow: rgba(91, 141, 239, 0.20),
            selection: rgba(91, 141, 239, 0.45),

            // Overlay: scrim + depth layers
            overlay: hex_alpha("#000000", 0.60),
            overlay_subtle: rgba(255, 255, 255, 0.03),
            overlay_light: rgba(255, 255, 255, 0.06),
            overlay_medium: rgba(255, 255, 255, 0.10),
            overlay_strong: rgba(255, 255, 255, 0.18),

            // Fonts
            font_body: "Inter".into(),
            font_mono: "JetBrains Mono".into(),

            font_scale: 1.0,

            // Typography tokens — compact scale for desktop density
            text_xs: 9.0,
            text_sm: 11.0,
            text_base: 12.0,
            text_md: 13.0,
            text_lg: 15.0,
            text_xl: 18.0,
            text_2xl: 24.0,

            // Spacing: 8px baseline grid
            space_4: 4.0,
            space_8: 8.0,
            space_12: 12.0,
            space_16: 16.0,
            space_20: 20.0,
            space_24: 24.0,
            space_40: 40.0,

            // Radius: modern glassmorphism scale
            radius_sm: 8.0,
            radius_md: 16.0,
            radius_lg: 28.0,
            radius_xl: 36.0,
            radius_full: 999.0,

            // Semantic surfaces
            tool_call_bg: rgba(91, 141, 239, 0.08),
            code_block_bg: rgba(0, 0, 0, 0.40),
            mood_bg: rgba(91, 141, 239, 0.06),

            // Shadow: stronger against pure black
            shadow_card: egui::Shadow {
                offset: [0, 1],
                blur: 8,
                spread: 0,
                color: rgba(0, 0, 0, 0.31),
            },
            shadow_panel: egui::Shadow {
                offset: [0, 2],
                blur: 16,
                spread: 0,
                color: rgba(0, 0, 0, 0.39),
            },
            shadow_modal: egui::Shadow {
                offset: [0, 8],
                blur: 32,
                spread: 0,
                color: rgba(0, 0, 0, 0.47),
            },
            shadow_toast: egui::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: rgba(0, 0, 0, 0.39),
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
            // Backgrounds: cool off-white with warm-tinted mid-tones
            bg: hex("#f0f1f6"),
            bg_accent: hex("#e6e8f0"),
            bg_elevated: hex("#dde0ea"),
            bg_hover: hex("#d0d4e0"),
            surface: hex("#e6e8f0"),
            surface_strong: hex("#dde0ea"),
            glass: rgba(0, 0, 0, 0.04),
            glass_strong: rgba(0, 0, 0, 0.10),

            // Text: near-black for WCAG AA contrast
            text: hex("#18181b"),
            text_strong: hex("#09090b"),
            text_muted: hex("#52525b"),
            text_dim: hex("#71717a"),

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

            // Borders: warm-tinted — strengthened for boundary visibility
            border: hex("#c8cad4"),
            border_strong: hex("#a1a3b0"),
            border_hover: hex("#8a8c9a"),
            input_bg: hex("#e6e8f0"),

            // Focus: accent-matched
            focus_ring: hex_alpha("#c98a5e", 0.20),
            focus_glow: hex_alpha("#c98a5e", 0.10),
            selection: hex_alpha("#c98a5e", 0.35),

            // Overlay: scrim + depth layers (black-on-white in light theme)
            overlay: hex_alpha("#000000", 0.35),
            overlay_subtle: hex_alpha("#000000", 0.03),
            overlay_light: hex_alpha("#000000", 0.06),
            overlay_medium: hex_alpha("#000000", 0.10),
            overlay_strong: hex_alpha("#000000", 0.18),

            font_body: "Inter".into(),
            font_mono: "JetBrains Mono".into(),

            font_scale: 1.0,

            // Typography tokens — compact scale for desktop density
            text_xs: 9.0,
            text_sm: 11.0,
            text_base: 12.0,
            text_md: 13.0,
            text_lg: 15.0,
            text_xl: 18.0,
            text_2xl: 24.0,

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
            radius_xl: 36.0,
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
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, self.border);
        style.visuals.widgets.hovered.weak_bg_fill = self.bg_hover;
        style.visuals.widgets.hovered.bg_fill = self.bg_hover;
        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, self.border_hover);
        style.visuals.widgets.active.bg_fill = self.bg_hover;
        style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, self.border_strong);
        style.visuals.widgets.noninteractive.bg_fill = self.surface;
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, self.border);
        style.visuals.selection.bg_fill = self.selection;
        style.visuals.selection.stroke = egui::Stroke::new(1.0, self.text_strong);
        style.visuals.window_corner_radius = egui::CornerRadius::same(self.radius_lg as u8);
        style.visuals.window_shadow = self.shadow_panel;
        style.visuals.popup_shadow = self.shadow_panel;
        style.visuals.window_stroke = egui::Stroke::new(1.0, self.border);
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, self.text);
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, self.text_strong);
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, self.text_strong);
        // Scale default text styles so markdown/chat content follows the theme font scale.
        let scale = self.font_scale;
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(self.text_xl * scale, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(self.text_base * scale, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(self.text_sm * scale, egui::FontFamily::Monospace),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(self.text_base * scale, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(self.text_xs * scale, egui::FontFamily::Proportional),
        );
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

    // ------------------------------------------------------------------
    // Typography helpers
    // ------------------------------------------------------------------

    /// Proportional font at the given semantic size token.
    pub fn font(&self, size: f32) -> egui::FontId {
        egui::FontId::new(size, egui::FontFamily::Proportional)
    }

    /// Monospace font at the given semantic size token.
    pub fn font_mono(&self, size: f32) -> egui::FontId {
        egui::FontId::new(size, egui::FontFamily::Monospace)
    }

    /// Bold font at the given semantic size token (requires bold face registered).
    pub fn font_bold(&self, size: f32) -> egui::FontId {
        egui::FontId::new(size, egui::FontFamily::Name("bold".into()))
    }

    /// Italic font at the given semantic size token (requires italic face registered).
    pub fn font_italic(&self, size: f32) -> egui::FontId {
        egui::FontId::new(size, egui::FontFamily::Name("italic".into()))
    }

    /// Scale all typography tokens by a factor (e.g. 0.9 for compact, 1.15 for large).
    pub fn with_font_scale(mut self, scale: f32) -> Self {
        self.font_scale = scale;
        self.text_xs *= scale;
        self.text_sm *= scale;
        self.text_base *= scale;
        self.text_md *= scale;
        self.text_lg *= scale;
        self.text_xl *= scale;
        self.text_2xl *= scale;
        self
    }

    /// Icon font at the given semantic size token (requires Phosphor icon font registered).
    pub fn font_icon(&self, size: f32) -> egui::FontId {
        egui::FontId::new(size, egui::FontFamily::Name("icons".into()))
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

fn rgba(r: u8, g: u8, b: u8, a: f32) -> egui::Color32 {
    let a = (a * 255.0).clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgba_premultiplied(r, g, b, a)
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // ------------------------------------------------------------------
    // CJK fonts — cross-platform probing
    // ------------------------------------------------------------------
    let cjk_candidates: &[&str] = &[
        // Windows — prefer Light weight for softer CJK rendering
        r"C:\Windows\Fonts\msyhl.ttc",
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\msyhbd.ttc",
        // macOS
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        // Linux
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    ];

    for path in cjk_candidates {
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

    // ------------------------------------------------------------------
    // Bold / Italic faces — best-effort system font loading
    // ------------------------------------------------------------------
    let bold_candidates: &[(&str, &str)] = &[
        (r"C:\Windows\Fonts\segoeuib.ttf", "segoeuib"),
        (r"C:\Windows\Fonts\msyhbd.ttc", "msyhbd"),
        ("/System/Library/Fonts/Helvetica.ttc", "helvetica"),
        (
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "dejavu-sans-bold",
        ),
        (
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
            "liberation-sans-bold",
        ),
    ];

    for (path, key) in bold_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert((*key).into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(egui::FontFamily::Name("bold".into()))
                .or_default()
                .push((*key).into());
            tracing::info!("Loaded bold font from {}", path);
            break;
        }
    }

    let italic_candidates: &[(&str, &str)] = &[
        (r"C:\Windows\Fonts\segoeuii.ttf", "segoeuii"),
        ("/System/Library/Fonts/Helvetica.ttc", "helvetica-italic"),
        (
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Oblique.ttf",
            "dejavu-sans-oblique",
        ),
        (
            "/usr/share/fonts/truetype/liberation/LiberationSans-Italic.ttf",
            "liberation-sans-italic",
        ),
    ];

    for (path, key) in italic_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert((*key).into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(egui::FontFamily::Name("italic".into()))
                .or_default()
                .push((*key).into());
            tracing::info!("Loaded italic font from {}", path);
            break;
        }
    }

    // ------------------------------------------------------------------
    // Icon font — Phosphor Regular (embedded)
    // ------------------------------------------------------------------
    let icon_font_bytes = include_bytes!("../assets/fonts/Phosphor.ttf");
    fonts.font_data.insert(
        "phosphor".into(),
        egui::FontData::from_static(icon_font_bytes).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Name("icons".into()))
        .or_default()
        .push("phosphor".into());

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
