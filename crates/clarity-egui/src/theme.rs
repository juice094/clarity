use eframe::egui;

// ============================================================================
// Design Token System — Phase A Foundation
// ============================================================================
// Based on OpenClaw Dashboard token architecture, adapted for egui.
// Accent: Indigo (#6366f1) per user selection (calm, low saturation, dev-friendly)
//
// Reference:
//   OpenClaw: 78 tokens, 4 themes (claw/knot/dash × dark/light/system)
//   Kimi: Naive UI variable system, 4-level grayscale
//   Tauri: GitHub-style CSS variables (15 tokens)
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

    // --- Fonts ---
    pub font_body: String,
    pub font_mono: String,

    // --- Spacing (4/8/12/16/20/24 px scale) ---
    pub space_4: f32,
    pub space_8: f32,
    pub space_12: f32,
    pub space_16: f32,
    pub space_20: f32,
    pub space_24: f32,

    // --- Radius (6/10/12/9999 px scale) ---
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    pub radius_full: f32,

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
    /// Dark theme — based on OpenClaw "claw:dark" with indigo accent.
    pub fn dark() -> Self {
        Self {
            // Backgrounds: Linear + Zed inspired deep space palette
            bg: hex("#0f0f11"),
            bg_accent: hex("#18181b"),
            bg_elevated: hex("#27272a"),
            bg_hover: hex("#3f3f46"),
            surface: hex("#18181b"),
            surface_strong: hex("#27272a"),

            // Text: crisp grayscale for OLED readability
            text: hex("#fafafa"),
            text_strong: hex("#ffffff"),
            text_muted: hex("#a1a1aa"),
            text_dim: hex("#71717a"),

            // Accent: Violet (#8b5cf6) — modern, distinctive
            accent: hex("#8b5cf6"),
            accent_hover: hex("#a78bfa"),
            accent_subtle: hex_alpha("#8b5cf6", 0.10),

            // Chat bubbles
            user_bubble: hex("#8b5cf6"),
            ai_bubble: hex("#27272a"),
            chat_text: hex("#d4d4d8"),
            error_bubble: hex_alpha("#ef4444", 0.15),
            error_text: hex("#fafafa"),

            // Status
            status_online: hex("#22c55e"),
            status_busy: hex("#f59e0b"),
            status_offline: hex("#ef4444"),
            ok: hex("#22c55e"),
            warn: hex("#f59e0b"),
            danger: hex("#ef4444"),

            // Borders
            border: hex("#3f3f46"),
            border_strong: hex("#52525b"),
            border_hover: hex("#71717a"),
            input_bg: hex("#27272a"),

            // Focus
            focus_ring: hex_alpha("#8b5cf6", 0.20),
            focus_glow: hex_alpha("#8b5cf6", 0.15),
            selection: hex_alpha("#a78bfa", 0.35),

            // Fonts
            font_body: "Inter".into(),
            font_mono: "JetBrains Mono".into(),

            // Spacing: 8px baseline grid
            space_4: 4.0,
            space_8: 8.0,
            space_12: 12.0,
            space_16: 16.0,
            space_20: 20.0,
            space_24: 24.0,

            // Radius
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_full: 9999.0,

            // Animation
            duration_fast: 0.10,
            duration_normal: 0.18,
            duration_slow: 0.30,
        }
    }

    /// Light theme — inverted surfaces, preserved accent.
    pub fn light() -> Self {
        Self {
            bg: hex("#ffffff"),
            bg_accent: hex("#f6f8fa"),
            bg_elevated: hex("#eaeef2"),
            bg_hover: hex("#e1e4e8"),
            surface: hex("#f6f8fa"),
            surface_strong: hex("#eaeef2"),

            text: hex("#1f2328"),
            text_strong: hex("#000000"),
            text_muted: hex("#656d76"),
            text_dim: hex("#8c959f"),

            accent: hex("#8b5cf6"),
            accent_hover: hex("#7c3aed"),
            accent_subtle: hex_alpha("#8b5cf6", 0.08),

            user_bubble: hex("#8b5cf6"),
            ai_bubble: hex("#f6f8fa"),
            chat_text: hex("#1f2328"),
            error_bubble: hex_alpha("#ef4444", 0.10),
            error_text: hex("#1f2328"),

            status_online: hex("#22c55e"),
            status_busy: hex("#f59e0b"),
            status_offline: hex("#ef4444"),
            ok: hex("#22c55e"),
            warn: hex("#f59e0b"),
            danger: hex("#ef4444"),

            border: hex("#d0d7de"),
            border_strong: hex("#b0b7be"),
            border_hover: hex("#9099a2"),
            input_bg: hex("#f6f8fa"),

            focus_ring: hex_alpha("#8b5cf6", 0.20),
            focus_glow: hex_alpha("#8b5cf6", 0.10),
            selection: hex_alpha("#7c3aed", 0.25),

            font_body: "Inter".into(),
            font_mono: "JetBrains Mono".into(),

            space_4: 4.0,
            space_8: 8.0,
            space_12: 12.0,
            space_16: 16.0,
            space_20: 20.0,
            space_24: 24.0,

            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_full: 9999.0,

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
        style.visuals.window_shadow = egui::Shadow::NONE;
        style.visuals.popup_shadow = egui::Shadow::NONE;
        style.visuals.window_stroke = egui::Stroke::NONE;
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, self.text);
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, self.text_strong);
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, self.text_strong);
    }

    /// Create a frame for chat bubbles.
    pub fn bubble_frame(&self, is_user: bool) -> egui::Frame {
        let fill = if is_user { self.user_bubble } else { self.ai_bubble };
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
    }

    /// Create a frame for the sidebar.
    pub fn sidebar_frame(&self) -> egui::Frame {
        egui::Frame::side_top_panel(&egui::Style::default())
            .fill(self.bg_accent)
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
