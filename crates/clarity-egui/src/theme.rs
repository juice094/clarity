use eframe::egui;

// ============================================================================
// Lucide Icons — codepoints from `lucide-icons` crate (see ADR-010).
// `ICON_*` constants kept as `&str` for backward compatibility with 123 call
// sites. New code should prefer `lucide_icons::Icon::*` directly for type
// safety and IDE autocomplete.
// ============================================================================
pub const ICON_SEND: &str = "\u{e152}"; // Lucide: Send
pub const ICON_SETTINGS: &str = "\u{e154}"; // Lucide: Settings
pub const ICON_PLAY: &str = "\u{e13c}"; // Lucide: Play
pub const ICON_HOURGLASS: &str = "\u{e296}"; // Lucide: Hourglass
pub const ICON_CHECK: &str = "\u{e06c}"; // Lucide: Check
pub const ICON_X: &str = "\u{e1b2}"; // Lucide: X
pub const ICON_WARNING: &str = "\u{e193}"; // Lucide: AlertTriangle
pub const ICON_LIST: &str = "\u{e106}"; // Lucide: List
pub const ICON_ARROW_LEFT: &str = "\u{e048}"; // Lucide: ArrowLeft
pub const ICON_PROHIBIT: &str = "\u{e051}"; // Lucide: Ban
pub const ICON_COPY: &str = "\u{e09e}"; // Lucide: Copy
pub const ICON_EDIT: &str = "\u{e1f9}"; // Lucide: Pencil
pub const ICON_REFRESH: &str = "\u{e145}"; // Lucide: RefreshCw
pub const ICON_CHAT: &str = "\u{e117}"; // Lucide: MessageSquare
pub const ICON_BOOK: &str = "\u{e05e}"; // Lucide: Book
pub const ICON_WRENCH: &str = "\u{e1b1}"; // Lucide: Wrench
pub const ICON_CARET_DOWN: &str = "\u{e06d}"; // Lucide: ChevronDown
pub const ICON_CARET_RIGHT: &str = "\u{e06f}"; // Lucide: ChevronRight
pub const ICON_MINUS: &str = "\u{e11c}"; // Lucide: Minus
pub const ICON_SQUARE: &str = "\u{e167}"; // Lucide: Square
pub const ICON_CIRCLE: &str = "\u{e076}"; // Lucide: Circle
pub const ICON_FILE: &str = "\u{e0d9}"; // Lucide: File
pub const ICON_FILE_TEXT: &str = "\u{e0cc}"; // Lucide: FileText
pub const ICON_GLOBE: &str = "\u{e0e8}"; // Lucide: Globe
pub const ICON_TABLE: &str = "\u{e17d}"; // Lucide: Table
pub const ICON_PRESENTATION: &str = "\u{e4ae}"; // Lucide: Presentation
pub const ICON_MAXIMIZE: &str = "\u{e112}"; // Lucide: Maximize
pub const ICON_MINIMIZE: &str = "\u{e11a}"; // Lucide: Minimize

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

    // --- Layout dimensions (Pretext UI token system) ---
    pub size_titlebar: f32,
    pub size_sidebar: f32,
    pub size_workspace: f32,
    pub size_panel_right: f32,
    pub size_statusbar: f32,
    pub size_input: f32,
    pub content_min_width: f32,

    // --- Chrome dimensions (P0.5.F.1 tokenization) ---
    /// Default window inner width on first launch.
    pub window_default_w: f32,
    /// Default window inner height on first launch.
    pub window_default_h: f32,
    /// Minimum allowed window inner width.
    pub window_min_w: f32,
    /// Minimum allowed window inner height.
    pub window_min_h: f32,
    /// Edge resize zone padding (px from window border).
    pub window_edge_zone: f32,
    /// Sidebar width when collapsed.
    pub size_sidebar_collapsed: f32,
    /// Session tab strip height.
    pub size_tab_h: f32,
    /// Modal vertical offset from top of viewport (palette anchor).
    pub modal_offset_y: f32,
    /// Command palette modal width.
    pub palette_w: f32,
    /// Command palette scroll-area max height.
    pub palette_max_h: f32,
    /// Titlebar LEFT zone width (sidebar toggle when collapsed + brand).
    pub titlebar_left_w: f32,

    // --- Widget micro-dimensions (P0.5.F.2 tokenization) ---
    /// File-tree indent per depth level (full mode).
    pub size_tree_indent: f32,
    /// File-tree indent per depth level (compact mode).
    pub size_tree_indent_compact: f32,
    /// File icon size in file browser (full mode).
    pub size_file_icon: f32,
    /// File icon size in file browser (compact mode).
    pub size_file_icon_compact: f32,
    /// MCP status button width when servers are connected.
    pub size_mcp_btn_w: f32,
    /// MCP status button width when no servers are configured.
    pub size_mcp_btn_w_compact: f32,
    /// New-tab [+] button reserved width in tab strip.
    pub size_new_tab_btn_w: f32,
    /// Minimum session tab width.
    pub size_tab_min_w: f32,
    /// Maximum session tab width.
    pub size_tab_max_w: f32,
    /// Tab close button width.
    pub size_close_btn_w: f32,
    /// Active tab accent underline height.
    pub size_accent_line_h: f32,

    // --- Responsive breakpoints ---
    pub breakpoint_compact: f32,
    pub breakpoint_medium: f32,
    pub breakpoint_wide: f32,
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
    ///
    /// Kimi-style dark theme — mimics Kimi Desktop v3.0.15 dark palette.
    pub fn dark() -> Self {
        Self {
            // Backgrounds: pure dark (Kimi Bg-Primary #121212)
            bg: hex("#121212"),
            bg_accent: hex("#1a1a1a"),
            bg_elevated: hex("#1f1f1f"),
            bg_hover: hex("#2a2a2a"),
            surface: hex("#1f1f1f"),
            surface_strong: hex("#2a2a2a"),
            glass: rgba(255, 255, 255, 0.04),
            glass_strong: rgba(255, 255, 255, 0.08),

            // Text: Kimi Labels-Primary #d6d6d6 hierarchy
            text: hex("#d6d6d6"),
            text_strong: hex("#ffffff"),
            text_muted: hex("#999999"),
            text_dim: hex("#666666"),

            // Accent: Kimi KMBlue #1a88ff
            accent: hex("#1a88ff"),
            accent_hover: hex("#4a9eff"),
            accent_subtle: rgba(26, 136, 255, 0.12),

            // Chat bubbles: Kimi user bubble is dark gray, not blue
            user_bubble: hex("#2a2a2a"),
            ai_bubble: hex("#1a1a1a"),
            chat_text: hex("#d6d6d6"),
            error_bubble: rgba(239, 91, 91, 0.50),
            error_text: hex("#EF8A8A"),

            // Status: semantic palette
            status_online: hex("#6BCB8A"),
            status_busy: hex("#D4A050"),
            status_offline: hex("#EF6B6B"),
            ok: hex("#6BCB8A"),
            warn: hex("#D4A050"),
            danger: hex("#EF6B6B"),

            // Borders: Kimi uses very subtle separators
            border: rgba(255, 255, 255, 0.06),
            border_strong: rgba(255, 255, 255, 0.10),
            border_hover: rgba(255, 255, 255, 0.16),
            input_bg: hex("#1f1f1f"),

            // Focus: Kimi blue glow
            focus_ring: rgba(26, 136, 255, 0.50),
            focus_glow: rgba(26, 136, 255, 0.15),
            selection: rgba(26, 136, 255, 0.25),

            // Overlay: scrim + depth layers
            overlay: hex_alpha("#000000", 0.60),
            overlay_subtle: rgba(255, 255, 255, 0.02),
            overlay_light: rgba(255, 255, 255, 0.04),
            overlay_medium: rgba(255, 255, 255, 0.08),
            overlay_strong: rgba(255, 255, 255, 0.14),

            // Fonts
            font_body: "Inter".into(),
            font_mono: "JetBrains Mono".into(),

            font_scale: 1.0,

            // Typography tokens — Kimi scale (larger than prior compact scale)
            text_xs: 10.0,
            text_sm: 12.0,
            text_base: 14.0,
            text_md: 15.0,
            text_lg: 18.0,
            text_xl: 22.0,
            text_2xl: 36.0,

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
            tool_call_bg: rgba(26, 136, 255, 0.08),
            code_block_bg: hex("#0d0d0d"),
            mood_bg: rgba(26, 136, 255, 0.05),

            // Shadow: Kimi-style softer shadows (electron can do better blur)
            shadow_card: egui::Shadow {
                offset: [0, 2],
                blur: 12,
                spread: 0,
                color: rgba(0, 0, 0, 0.40),
            },
            shadow_panel: egui::Shadow {
                offset: [0, 4],
                blur: 20,
                spread: 0,
                color: rgba(0, 0, 0, 0.50),
            },
            shadow_modal: egui::Shadow {
                offset: [0, 8],
                blur: 32,
                spread: 0,
                color: rgba(0, 0, 0, 0.60),
            },
            shadow_toast: egui::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: rgba(0, 0, 0, 0.50),
            },

            // Animation
            duration_fast: 0.10,
            duration_normal: 0.18,
            duration_slow: 0.30,

            // Layout dimensions
            size_titlebar: 36.0,
            size_sidebar: 200.0,
            size_workspace: 280.0,
            size_panel_right: 240.0,
            size_statusbar: 24.0,
            size_input: 88.0,
            content_min_width: 480.0,

            // Chrome dimensions (P0.5.F.1)
            window_default_w: 1280.0,
            window_default_h: 800.0,
            window_min_w: 900.0,
            window_min_h: 600.0,
            window_edge_zone: 10.0,
            size_sidebar_collapsed: 36.0,
            size_tab_h: 28.0,
            modal_offset_y: 40.0,
            palette_w: 520.0,
            palette_max_h: 320.0,
            titlebar_left_w: 130.0,

            // Widget micro-dimensions (P0.5.F.2)
            size_tree_indent: 16.0,
            size_tree_indent_compact: 4.0,
            size_file_icon: 14.0,
            size_file_icon_compact: 10.0,
            size_mcp_btn_w: 36.0,
            size_mcp_btn_w_compact: 20.0,
            size_new_tab_btn_w: 28.0,
            size_tab_min_w: 48.0,
            size_tab_max_w: 180.0,
            size_close_btn_w: 18.0,
            size_accent_line_h: 1.0,

            // Responsive breakpoints
            breakpoint_compact: 768.0,
            breakpoint_medium: 1100.0,
            breakpoint_wide: 1400.0,
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

            // Accent: Kimi KMBlue
            accent: hex("#1a88ff"),
            accent_hover: hex("#4a9eff"),
            accent_subtle: rgba(26, 136, 255, 0.12),

            // Chat bubbles
            user_bubble: rgba(45, 80, 160, 0.30),
            ai_bubble: rgba(255, 255, 255, 0.04),
            chat_text: hex("#E8EAEF"),
            error_bubble: rgba(239, 91, 91, 0.50),
            error_text: hex("#EF8A8A"),

            // Status
            status_online: hex("#6BCB8A"),
            status_busy: hex("#D4A050"),
            status_offline: hex("#EF6B6B"),
            ok: hex("#6BCB8A"),
            warn: hex("#D4A050"),
            danger: hex("#EF6B6B"),

            // Borders: slate-blue tint for subtle boundary visibility on pure-black backgrounds
            border: rgba(110, 135, 175, 0.04),
            border_strong: rgba(110, 135, 175, 0.08),
            border_hover: rgba(110, 135, 175, 0.14),
            input_bg: rgba(28, 28, 40, 0.80),

            // Focus: blue glow
            focus_ring: rgba(91, 141, 239, 0.60),
            focus_glow: rgba(91, 141, 239, 0.20),
            // Selection: slate-grey background for low-intrusion highlighting
            // (matches border colour family at higher opacity; replaces prior bright-blue)
            selection: rgba(55, 58, 75, 0.72),

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

            // Typography tokens — Kimi scale (larger than prior compact scale)
            text_xs: 10.0,
            text_sm: 12.0,
            text_base: 14.0,
            text_md: 15.0,
            text_lg: 18.0,
            text_xl: 22.0,
            text_2xl: 36.0,

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

            // Layout dimensions (shared across dark/oled)
            size_titlebar: 36.0,
            size_sidebar: 200.0,
            size_workspace: 280.0,
            size_panel_right: 240.0,
            size_statusbar: 24.0,
            size_input: 88.0,
            content_min_width: 480.0,

            // Chrome dimensions (P0.5.F.1, shared)
            window_default_w: 1280.0,
            window_default_h: 800.0,
            window_min_w: 900.0,
            window_min_h: 600.0,
            window_edge_zone: 10.0,
            size_sidebar_collapsed: 36.0,
            size_tab_h: 28.0,
            modal_offset_y: 40.0,
            palette_w: 520.0,
            palette_max_h: 320.0,
            titlebar_left_w: 130.0,

            // Widget micro-dimensions (P0.5.F.2, shared)
            size_tree_indent: 16.0,
            size_tree_indent_compact: 4.0,
            size_file_icon: 14.0,
            size_file_icon_compact: 10.0,
            size_mcp_btn_w: 36.0,
            size_mcp_btn_w_compact: 20.0,
            size_new_tab_btn_w: 28.0,
            size_tab_min_w: 48.0,
            size_tab_max_w: 180.0,
            size_close_btn_w: 18.0,
            size_accent_line_h: 1.0,

            // Responsive breakpoints (shared)
            breakpoint_compact: 768.0,
            breakpoint_medium: 1100.0,
            breakpoint_wide: 1400.0,
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
            error_bubble: rgba(239, 91, 91, 0.50),
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

            // Typography tokens — Kimi scale (larger than prior compact scale)
            text_xs: 10.0,
            text_sm: 12.0,
            text_base: 14.0,
            text_md: 15.0,
            text_lg: 18.0,
            text_xl: 22.0,
            text_2xl: 36.0,

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

            // Layout dimensions (shared)
            size_titlebar: 36.0,
            size_sidebar: 200.0,
            size_workspace: 280.0,
            size_panel_right: 240.0,
            size_statusbar: 24.0,
            size_input: 88.0,
            content_min_width: 480.0,

            // Chrome dimensions (P0.5.F.1, shared)
            window_default_w: 1280.0,
            window_default_h: 800.0,
            window_min_w: 900.0,
            window_min_h: 600.0,
            window_edge_zone: 10.0,
            size_sidebar_collapsed: 36.0,
            size_tab_h: 28.0,
            modal_offset_y: 40.0,
            palette_w: 520.0,
            palette_max_h: 320.0,
            titlebar_left_w: 130.0,

            // Widget micro-dimensions (P0.5.F.2, shared)
            size_tree_indent: 16.0,
            size_tree_indent_compact: 4.0,
            size_file_icon: 14.0,
            size_file_icon_compact: 10.0,
            size_mcp_btn_w: 36.0,
            size_mcp_btn_w_compact: 20.0,
            size_new_tab_btn_w: 28.0,
            size_tab_min_w: 48.0,
            size_tab_max_w: 180.0,
            size_close_btn_w: 18.0,
            size_accent_line_h: 1.0,

            // Responsive breakpoints (shared)
            breakpoint_compact: 768.0,
            breakpoint_medium: 1100.0,
            breakpoint_wide: 1400.0,
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
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, self.border);
        style.visuals.widgets.hovered.weak_bg_fill = self.bg_hover;
        style.visuals.widgets.hovered.bg_fill = self.bg_hover;
        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, self.border_hover);
        style.visuals.widgets.active.bg_fill = self.bg_hover;
        style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, self.border_strong);
        style.visuals.widgets.noninteractive.bg_fill = self.surface;
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, self.border);
        style.visuals.selection.bg_fill = self.selection;
        style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, self.text_strong);
        style.visuals.window_corner_radius = egui::CornerRadius::same(self.radius_lg as u8);
        style.visuals.window_shadow = self.shadow_panel;
        style.visuals.popup_shadow = self.shadow_panel;
        style.visuals.window_stroke = egui::Stroke::new(1.0_f32, self.border);
        style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, self.text);
        style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, self.text_strong);
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, self.text_strong);
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
            .stroke(egui::Stroke::new(1.0_f32, self.border))
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

    /// Builds a secondary button widget.
    pub fn secondary_button(&self, text: impl Into<egui::WidgetText>) -> egui::Button<'_> {
        egui::Button::new(text)
            .fill(self.surface)
            .corner_radius(egui::CornerRadius::same(self.radius_sm as u8))
    }

    /// Builds a ghost button widget.
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

    /// Bold font at the given semantic size token (Inter Medium + Noto SC).
    pub fn font_bold(&self, size: f32) -> egui::FontId {
        egui::FontId::new(size, egui::FontFamily::Name("bold".into()))
    }

    /// Italic font — falls back to proportional since no italic face is embedded.
    pub fn font_italic(&self, size: f32) -> egui::FontId {
        egui::FontId::new(size, egui::FontFamily::Proportional)
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

    /// Icon font at the given semantic size token (requires Lucide icon font registered via `lucide-icons` crate; see ADR-010).
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

/// Installs custom fonts into the egui context.
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // ------------------------------------------------------------------
    // 1. Inter — UI proportional (Regular + Medium for bold)
    // ------------------------------------------------------------------
    fonts.font_data.insert(
        "inter".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "inter-medium".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-Medium.ttf")).into(),
    );

    // ------------------------------------------------------------------
    // 2. JetBrains Mono — code/monospace
    // ------------------------------------------------------------------
    fonts.font_data.insert(
        "jetbrains-mono".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"))
            .into(),
    );

    // ------------------------------------------------------------------
    // 3. Noto Sans SC subset — CJK fallback (embedded, minimal UI coverage)
    // ------------------------------------------------------------------
    fonts.font_data.insert(
        "noto-sc".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/NotoSansSC-subset.ttf")).into(),
    );

    // ------------------------------------------------------------------
    // 3b. System CJK font — runtime fallback for full Chinese chat coverage
    // ------------------------------------------------------------------
    let system_cjk_paths: &[&str] = if cfg!(windows) {
        &[
            // Static fonts first — Variable Fonts may not be fully supported by ttf-parser
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\simsun.ttc",
            r"C:\Windows\Fonts\NotoSansSC-VF.ttf",
        ]
    } else if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
        ]
    } else {
        &[
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        ]
    };

    for path in system_cjk_paths {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                "noto-sc-system".into(),
                egui::FontData::from_owned(data).into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("noto-sc-system".into());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("noto-sc-system".into());
            fonts
                .families
                .entry(egui::FontFamily::Name("bold".into()))
                .or_default()
                .push("noto-sc-system".into());
            break;
        }
    }

    // ------------------------------------------------------------------
    // 4. Lucide — icon font (via `lucide-icons` crate; see ADR-010)
    // ------------------------------------------------------------------
    fonts.font_data.insert(
        "lucide".into(),
        egui::FontData::from_static(lucide_icons::LUCIDE_FONT_BYTES).into(),
    );

    // ------------------------------------------------------------------
    // Font stack assignments
    // ------------------------------------------------------------------
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .extend(["inter".into(), "noto-sc".into()]);

    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .extend(["jetbrains-mono".into(), "noto-sc".into()]);

    fonts
        .families
        .entry(egui::FontFamily::Name("bold".into()))
        .or_default()
        .extend(["inter-medium".into(), "noto-sc".into()]);

    fonts
        .families
        .entry(egui::FontFamily::Name("icons".into()))
        .or_default()
        .push("lucide".into());

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
