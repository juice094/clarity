use eframe::egui;

// ============================================================================
// Lucide Icons — codepoints from `lucide-icons` crate (see ADR-010).
// `ICON_*` constants kept as `&str` for backward compatibility with 123 call
// sites. New code should prefer `lucide_icons::Icon::*` directly for type
// safety and IDE autocomplete.
// ============================================================================
pub const ICON_SEND: &str = "\u{e152}"; // Lucide: Send
pub const ICON_SETTINGS: &str = "\u{e154}"; // Lucide: Settings
pub const ICON_HOURGLASS: &str = "\u{e296}"; // Lucide: Hourglass
pub const ICON_CHECK: &str = "\u{e06c}"; // Lucide: Check
pub const ICON_X: &str = "\u{e1b2}"; // Lucide: X
pub const ICON_WARNING: &str = "\u{e193}"; // Lucide: AlertTriangle
pub const ICON_LIST: &str = "\u{e106}"; // Lucide: List
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
pub const ICON_FILE: &str = "\u{e0d9}"; // Lucide: File
pub const ICON_GLOBE: &str = "\u{e0e8}"; // Lucide: Globe
pub const ICON_MAXIMIZE: &str = "\u{e112}"; // Lucide: Maximize
pub const ICON_MINIMIZE: &str = "\u{e11a}"; // Lucide: Minimize
pub const ICON_PLUS: &str = "\u{e13d}"; // Lucide: Plus
pub const ICON_TERMINAL: &str = "\u{e181}"; // Lucide: Terminal
pub const ICON_FOLDER_OPEN: &str = "\u{e247}"; // Lucide: FolderOpen
pub const ICON_LAYOUT_TEMPLATE: &str = "\u{e207}"; // Lucide: LayoutTemplate
pub const ICON_SHARE: &str = "\u{e155}"; // Lucide: Share
pub const ICON_CPU: &str = "\u{e0a9}"; // Lucide: Cpu
pub const ICON_FILE_CODE: &str = "\u{e0c3}"; // Lucide: FileCode
pub const ICON_LAYERS: &str = "\u{e529}"; // Lucide: Layers
pub const ICON_BOOK_OPEN: &str = "\u{e05f}"; // Lucide: BookOpen
pub const ICON_INFO: &str = "\u{e0f4}"; // Lucide: Info
pub const ICON_ARCHIVE: &str = "\u{e052}"; // Lucide: Archive
pub const ICON_LOCK: &str = "\u{e1f3}"; // Lucide: Lock

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
    /// Informational status (desaturated blue — avoids conflation with accent).
    pub info: egui::Color32,

    // --- Diff view ---
    /// Added-line background in unified diff view.
    pub diff_added_bg: egui::Color32,
    /// Added-line text color.
    pub diff_added_text: egui::Color32,
    /// Removed-line background in unified diff view.
    pub diff_removed_bg: egui::Color32,
    /// Removed-line text color.
    pub diff_removed_text: egui::Color32,

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
    pub text_3xl: f32,

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

    // --- Navigation (sidebar component-level tokens, inspired by gpui-component's
    //     per-component ThemeColor system) ---
    /// Text color on accent-filled CTA buttons (e.g. "New Task").
    pub nav_cta_text: egui::Color32,
    /// Navigation row background on hover.
    pub nav_row_hover: egui::Color32,
    /// Navigation row background when selected / active.
    pub nav_row_selected: egui::Color32,
    /// Text color on the active segment of the Work/Chat toggle.
    pub toggle_active_text: egui::Color32,

    // --- Sidebar micro-layout ---
    /// Fixed width of the icon column in the left navigation tree.
    pub size_nav_icon_rail: f32,
    /// Target height of a single navigation/bot/history row.
    pub size_nav_row_h: f32,

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
    pub size_bot_bar: f32,
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

// ============================================================================
// Palette — 16-color derivation source (Base16-inspired)
// ============================================================================
// Each theme variant defines exactly one Palette. All ~40 Theme color fields
// are derived from the palette via lighten/darken/alpha/saturate helpers,
// reducing per-preset code from ~200 lines to ~20 lines.
//
// Adding a new theme now requires defining 16 hex values + 2 font names,
// not hand-writing 87 struct fields.

/// 16-color palette following Base16 conventions.
///
/// `overlay_base` is used as the base for transparency layers:
/// `Color32::WHITE` for light themes, `Color32::BLACK` for dark.
#[derive(Clone, Debug)]
struct Palette {
    // Background scale (darkest → lightest surface)
    bg0: egui::Color32,
    bg1: egui::Color32,
    bg2: egui::Color32,
    bg3: egui::Color32,
    // Text scale (primary → dim)
    fg0: egui::Color32,
    fg1: egui::Color32,
    fg2: egui::Color32,
    fg3: egui::Color32,
    // Accent
    accent: egui::Color32,
    accent_hover: egui::Color32,
    // Semantic colours
    red: egui::Color32,
    green: egui::Color32,
    yellow: egui::Color32,
    blue: egui::Color32,
    // Overlay base: WHITE for light themes, BLACK for dark.
    overlay_base: egui::Color32,
}

impl Palette {
    /// Derive the full set of ~40 Theme colour fields from this palette.
    fn into_theme_colors(self) -> ThemeColorFields {
        let p = &self;
        ThemeColorFields {
            // Backgrounds — direct palette mapping
            bg: p.bg0,
            bg_accent: p.bg1,
            bg_elevated: p.bg1,
            bg_hover: p.bg2,
            surface: p.bg1,
            surface_strong: p.bg2,
            // Glass: use the opposite of overlay_base so transparent
            // "glass" layers feel correct (white pass-through on dark bg,
            // dark pass-through on light bg).
            glass: alpha(invert(p.overlay_base), 0.04),
            glass_strong: alpha(invert(p.overlay_base), 0.08),

            // Text hierarchy
            text: p.fg0,
            text_strong: p.fg0, // fg0 already the brightest/primary
            text_muted: p.fg1,
            text_dim: p.fg2,

            // Accent
            accent: p.accent,
            accent_hover: p.accent_hover,
            accent_subtle: alpha(p.accent, 0.12),

            // Chat bubbles
            user_bubble: p.bg2,
            ai_bubble: p.bg1,
            chat_text: p.fg0,
            error_bubble: alpha(p.red, 0.18),
            error_text: lighten(p.red, 0.2),

            // Status
            status_online: p.green,
            status_busy: p.yellow,
            status_offline: p.red,
            ok: p.green,
            warn: p.yellow,
            danger: p.red,
            info: p.blue,

            // Diff
            diff_added_bg: alpha(p.green, 0.12),
            diff_added_text: p.green,
            diff_removed_bg: alpha(p.red, 0.12),
            diff_removed_text: lighten(p.red, 0.2),

            // Borders
            border: alpha(p.overlay_base, 0.06),
            border_strong: alpha(p.overlay_base, 0.10),
            border_hover: alpha(p.overlay_base, 0.16),
            input_bg: p.bg1,

            // Focus
            focus_ring: alpha(p.accent, 0.50),
            focus_glow: alpha(p.accent, 0.15),
            selection: alpha(p.accent, 0.25),

            // Overlays
            overlay: hex_alpha("#000000", 0.60),
            overlay_subtle: alpha(p.overlay_base, 0.02),
            overlay_light: alpha(p.overlay_base, 0.04),
            overlay_medium: alpha(p.overlay_base, 0.08),
            overlay_strong: alpha(p.overlay_base, 0.14),

            // Navigation
            nav_cta_text: p.fg0,
            nav_row_hover: alpha(p.overlay_base, 0.04),
            nav_row_selected: alpha(p.overlay_base, 0.08),
            toggle_active_text: p.fg0,

            // Semantic surfaces
            tool_call_bg: alpha(p.accent, 0.08),
            code_block_bg: p.bg0, // darkest surface for code blocks
            mood_bg: alpha(p.accent, 0.05),
        }
    }
}

/// Collected colour fields — destructured into Theme by `from_palette`.
struct ThemeColorFields {
    bg: egui::Color32,
    bg_accent: egui::Color32,
    bg_elevated: egui::Color32,
    bg_hover: egui::Color32,
    surface: egui::Color32,
    surface_strong: egui::Color32,
    glass: egui::Color32,
    glass_strong: egui::Color32,
    text: egui::Color32,
    text_strong: egui::Color32,
    text_muted: egui::Color32,
    text_dim: egui::Color32,
    accent: egui::Color32,
    accent_hover: egui::Color32,
    accent_subtle: egui::Color32,
    user_bubble: egui::Color32,
    ai_bubble: egui::Color32,
    chat_text: egui::Color32,
    error_bubble: egui::Color32,
    error_text: egui::Color32,
    status_online: egui::Color32,
    status_busy: egui::Color32,
    status_offline: egui::Color32,
    ok: egui::Color32,
    warn: egui::Color32,
    danger: egui::Color32,
    info: egui::Color32,
    diff_added_bg: egui::Color32,
    diff_added_text: egui::Color32,
    diff_removed_bg: egui::Color32,
    diff_removed_text: egui::Color32,
    border: egui::Color32,
    border_strong: egui::Color32,
    border_hover: egui::Color32,
    input_bg: egui::Color32,
    focus_ring: egui::Color32,
    focus_glow: egui::Color32,
    selection: egui::Color32,
    overlay: egui::Color32,
    overlay_subtle: egui::Color32,
    overlay_light: egui::Color32,
    overlay_medium: egui::Color32,
    overlay_strong: egui::Color32,
    nav_cta_text: egui::Color32,
    nav_row_hover: egui::Color32,
    nav_row_selected: egui::Color32,
    toggle_active_text: egui::Color32,
    tool_call_bg: egui::Color32,
    code_block_bg: egui::Color32,
    mood_bg: egui::Color32,
}

// Shared invariant tokens — identical across all presets.
fn shared_tokens() -> SharedTokens {
    SharedTokens {
        font_scale: 1.0,
        text_xs: 10.0,
        text_sm: 12.0,
        text_base: 14.0,
        text_md: 15.0,
        text_lg: 18.0,
        text_xl: 22.0,
        text_2xl: 36.0,
        text_3xl: 42.0,
        space_4: 4.0,
        space_8: 8.0,
        space_12: 12.0,
        space_16: 16.0,
        space_20: 20.0,
        space_24: 24.0,
        space_40: 40.0,
        radius_sm: 8.0,
        radius_md: 16.0,
        radius_lg: 28.0,
        radius_xl: 36.0,
        radius_full: 999.0,
        size_nav_icon_rail: 24.0,
        size_nav_row_h: 32.0,
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
        duration_fast: 0.10,
        duration_normal: 0.18,
        duration_slow: 0.30,
        size_titlebar: 32.0,
        size_sidebar: 14.0 * 15.0,
        size_workspace: 280.0,
        size_panel_right: 240.0,
        size_statusbar: 24.0,
        size_input: 88.0,
        size_bot_bar: 44.0,
        content_min_width: 480.0,
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
        breakpoint_compact: 768.0,
        breakpoint_medium: 1100.0,
        breakpoint_wide: 1400.0,
    }
}

struct SharedTokens {
    font_scale: f32,
    text_xs: f32,
    text_sm: f32,
    text_base: f32,
    text_md: f32,
    text_lg: f32,
    text_xl: f32,
    text_2xl: f32,
    text_3xl: f32,
    space_4: f32,
    space_8: f32,
    space_12: f32,
    space_16: f32,
    space_20: f32,
    space_24: f32,
    space_40: f32,
    radius_sm: f32,
    radius_md: f32,
    radius_lg: f32,
    radius_xl: f32,
    radius_full: f32,
    size_nav_icon_rail: f32,
    size_nav_row_h: f32,
    shadow_card: egui::Shadow,
    shadow_panel: egui::Shadow,
    shadow_modal: egui::Shadow,
    shadow_toast: egui::Shadow,
    duration_fast: f32,
    duration_normal: f32,
    duration_slow: f32,
    size_titlebar: f32,
    size_sidebar: f32,
    size_workspace: f32,
    size_panel_right: f32,
    size_statusbar: f32,
    size_input: f32,
    size_bot_bar: f32,
    content_min_width: f32,
    window_default_w: f32,
    window_default_h: f32,
    window_min_w: f32,
    window_min_h: f32,
    window_edge_zone: f32,
    size_sidebar_collapsed: f32,
    size_tab_h: f32,
    modal_offset_y: f32,
    palette_w: f32,
    palette_max_h: f32,
    titlebar_left_w: f32,
    size_tree_indent: f32,
    size_tree_indent_compact: f32,
    size_file_icon: f32,
    size_file_icon_compact: f32,
    size_mcp_btn_w: f32,
    size_mcp_btn_w_compact: f32,
    size_new_tab_btn_w: f32,
    size_tab_min_w: f32,
    size_tab_max_w: f32,
    size_close_btn_w: f32,
    size_accent_line_h: f32,
    breakpoint_compact: f32,
    breakpoint_medium: f32,
    breakpoint_wide: f32,
}

impl Theme {
    /// Build a Theme from a palette + font family names.
    ///
    /// All color fields are derived from the palette; layout/typography/spacing
    /// tokens use the shared invariant defaults.
    fn from_palette(palette: Palette, font_body: &str, font_mono: &str) -> Self {
        let c = palette.into_theme_colors();
        let t = shared_tokens();
        Self {
            bg: c.bg,
            bg_accent: c.bg_accent,
            bg_elevated: c.bg_elevated,
            bg_hover: c.bg_hover,
            surface: c.surface,
            surface_strong: c.surface_strong,
            glass: c.glass,
            glass_strong: c.glass_strong,
            text: c.text,
            text_strong: c.text_strong,
            text_muted: c.text_muted,
            text_dim: c.text_dim,
            accent: c.accent,
            accent_hover: c.accent_hover,
            accent_subtle: c.accent_subtle,
            user_bubble: c.user_bubble,
            ai_bubble: c.ai_bubble,
            chat_text: c.chat_text,
            error_bubble: c.error_bubble,
            error_text: c.error_text,
            status_online: c.status_online,
            status_busy: c.status_busy,
            status_offline: c.status_offline,
            ok: c.ok,
            warn: c.warn,
            danger: c.danger,
            info: c.info,
            diff_added_bg: c.diff_added_bg,
            diff_added_text: c.diff_added_text,
            diff_removed_bg: c.diff_removed_bg,
            diff_removed_text: c.diff_removed_text,
            border: c.border,
            border_strong: c.border_strong,
            border_hover: c.border_hover,
            input_bg: c.input_bg,
            focus_ring: c.focus_ring,
            focus_glow: c.focus_glow,
            selection: c.selection,
            overlay: c.overlay,
            overlay_subtle: c.overlay_subtle,
            overlay_light: c.overlay_light,
            overlay_medium: c.overlay_medium,
            overlay_strong: c.overlay_strong,
            nav_cta_text: c.nav_cta_text,
            nav_row_hover: c.nav_row_hover,
            nav_row_selected: c.nav_row_selected,
            toggle_active_text: c.toggle_active_text,
            tool_call_bg: c.tool_call_bg,
            code_block_bg: c.code_block_bg,
            mood_bg: c.mood_bg,
            font_body: font_body.into(),
            font_mono: font_mono.into(),
            font_scale: t.font_scale,
            text_xs: t.text_xs,
            text_sm: t.text_sm,
            text_base: t.text_base,
            text_md: t.text_md,
            text_lg: t.text_lg,
            text_xl: t.text_xl,
            text_2xl: t.text_2xl,
            text_3xl: t.text_3xl,
            space_4: t.space_4,
            space_8: t.space_8,
            space_12: t.space_12,
            space_16: t.space_16,
            space_20: t.space_20,
            space_24: t.space_24,
            space_40: t.space_40,
            radius_sm: t.radius_sm,
            radius_md: t.radius_md,
            radius_lg: t.radius_lg,
            radius_xl: t.radius_xl,
            radius_full: t.radius_full,
            size_nav_icon_rail: t.size_nav_icon_rail,
            size_nav_row_h: t.size_nav_row_h,
            shadow_card: t.shadow_card,
            shadow_panel: t.shadow_panel,
            shadow_modal: t.shadow_modal,
            shadow_toast: t.shadow_toast,
            duration_fast: t.duration_fast,
            duration_normal: t.duration_normal,
            duration_slow: t.duration_slow,
            size_titlebar: t.size_titlebar,
            size_sidebar: t.size_sidebar,
            size_workspace: t.size_workspace,
            size_panel_right: t.size_panel_right,
            size_statusbar: t.size_statusbar,
            size_input: t.size_input,
            size_bot_bar: t.size_bot_bar,
            content_min_width: t.content_min_width,
            window_default_w: t.window_default_w,
            window_default_h: t.window_default_h,
            window_min_w: t.window_min_w,
            window_min_h: t.window_min_h,
            window_edge_zone: t.window_edge_zone,
            size_sidebar_collapsed: t.size_sidebar_collapsed,
            size_tab_h: t.size_tab_h,
            modal_offset_y: t.modal_offset_y,
            palette_w: t.palette_w,
            palette_max_h: t.palette_max_h,
            titlebar_left_w: t.titlebar_left_w,
            size_tree_indent: t.size_tree_indent,
            size_tree_indent_compact: t.size_tree_indent_compact,
            size_file_icon: t.size_file_icon,
            size_file_icon_compact: t.size_file_icon_compact,
            size_mcp_btn_w: t.size_mcp_btn_w,
            size_mcp_btn_w_compact: t.size_mcp_btn_w_compact,
            size_new_tab_btn_w: t.size_new_tab_btn_w,
            size_tab_min_w: t.size_tab_min_w,
            size_tab_max_w: t.size_tab_max_w,
            size_close_btn_w: t.size_close_btn_w,
            size_accent_line_h: t.size_accent_line_h,
            breakpoint_compact: t.breakpoint_compact,
            breakpoint_medium: t.breakpoint_medium,
            breakpoint_wide: t.breakpoint_wide,
        }
    }
}

fn is_dark(c: &egui::Color32) -> bool {
    // Perceived brightness (BT.601 luma)
    (c.r() as f32 * 0.299 + c.g() as f32 * 0.587 + c.b() as f32 * 0.114) < 128.0
}

/// Return BLACK for light inputs, WHITE for dark inputs.
fn invert(c: egui::Color32) -> egui::Color32 {
    if is_dark(&c) {
        egui::Color32::WHITE
    } else {
        egui::Color32::BLACK
    }
}

/// Build a `Color32` from a base colour with an alpha multiplier (0.0–1.0).
fn alpha(color: egui::Color32, a: f32) -> egui::Color32 {
    let a = (a * 255.0).clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), a)
}

/// Lighten a colour by blending it toward white.
fn lighten(color: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from_rgb(
        (color.r() as f32 + (255.0 - color.r() as f32) * t) as u8,
        (color.g() as f32 + (255.0 - color.g() as f32) * t) as u8,
        (color.b() as f32 + (255.0 - color.b() as f32) * t) as u8,
    )
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
        let mut t = Self::from_palette(
            Palette {
                bg0: hex("#121212"),
                bg1: hex("#1f1f1f"),
                bg2: hex("#2a2a2a"),
                bg3: hex("#333333"),
                fg0: hex("#d6d6d6"),
                fg1: hex("#999999"),
                fg2: hex("#777777"),
                fg3: hex("#555555"),
                accent: hex("#1a88ff"),
                accent_hover: hex("#4a9eff"),
                red: hex("#EF6B6B"),
                green: hex("#6BCB8A"),
                yellow: hex("#D4A050"),
                blue: hex("#89B4FA"),
                overlay_base: egui::Color32::BLACK,
            },
            "Inter",
            "JetBrains Mono",
        );
        // Bespoke overrides — values the derivation gets close but not exact.
        // Keep these minimal; prefer improving the derivation formulas instead.
        t.text_strong = hex("#ffffff"); // pure white for maximum contrast
        t.bg_accent = hex("#1a1a1a"); // subtle bg variant
        t.ai_bubble = hex("#1a1a1a"); // matches bg_accent
        t.code_block_bg = hex("#0d0d0d"); // darker-than-bg0 for inset feel
        t.error_bubble = rgba(239, 91, 91, 0.50); // stronger error tint
        t.error_text = hex("#EF8A8A"); // specific error text tone
        t.user_bubble = hex("#2a2a2a"); // matches bg2
        t
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
            text_dim: rgba(200, 205, 220, 0.62),

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
            info: hex("#89B4FA"),

            // Diff
            diff_added_bg: rgba(70, 180, 100, 0.12),
            diff_added_text: hex("#6BCB8A"),
            diff_removed_bg: rgba(239, 91, 91, 0.12),
            diff_removed_text: hex("#EF8A8A"),

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
            text_3xl: 42.0,

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

            // Navigation (component-level)
            nav_cta_text: hex("#ffffff"),
            nav_row_hover: rgba(255, 255, 255, 0.04),
            nav_row_selected: rgba(255, 255, 255, 0.08),
            toggle_active_text: hex("#ffffff"),

            // Sidebar micro-layout
            size_nav_icon_rail: 24.0,
            size_nav_row_h: 32.0,

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
            size_titlebar: 32.0,
            // S6 layout: sidebar width tracks text_base so it stays proportional
            // under font scaling (see `with_font_scale`). The effective width must
            // also accommodate the widest content row inside the left navigation
            // tree (multi-button bars, device rows, chat items).
            size_sidebar: 14.0 * 15.0,
            size_workspace: 280.0,
            size_panel_right: 240.0,
            size_statusbar: 24.0,
            size_input: 88.0,
            size_bot_bar: 44.0,
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
        let mut t = Self::from_palette(
            Palette {
                bg0: hex("#f0f1f6"),
                bg1: hex("#dde0ea"),
                bg2: hex("#d0d4e0"),
                bg3: hex("#c4c8d2"),
                fg0: hex("#18181b"),
                fg1: hex("#52525b"),
                fg2: hex("#757575"),
                fg3: hex("#999999"),
                accent: hex("#c98a5e"),
                accent_hover: hex("#b87a4e"),
                red: hex("#c97060"),
                green: hex("#6bb87a"),
                yellow: hex("#d4a050"),
                blue: hex("#2969C4"),
                overlay_base: egui::Color32::WHITE,
            },
            "Inter",
            "JetBrains Mono",
        );
        // Light theme overrides — many values need light-specific tuning.
        t.text_strong = hex("#09090b");
        t.bg_accent = hex("#e6e8f0");
        t.surface = hex("#e6e8f0");
        t.glass_strong = rgba(0, 0, 0, 0.10);
        t.user_bubble = hex("#c98a5e");
        t.ai_bubble = hex("#e8eaf0");
        t.chat_text = hex("#1e1d24");
        t.error_text = hex("#1e1d24");
        t.accent_subtle = hex_alpha("#c98a5e", 0.08);
        t.code_block_bg = rgba(0, 0, 0, 0.40);
        t.tool_call_bg = rgba(91, 141, 239, 0.08);
        t.mood_bg = rgba(91, 141, 239, 0.06);
        t.border = hex("#c8cad4");
        t.border_strong = hex("#a1a3b0");
        t.border_hover = hex("#8a8c9a");
        t.input_bg = hex("#e6e8f0");
        t.focus_ring = hex_alpha("#c98a5e", 0.20);
        t.focus_glow = hex_alpha("#c98a5e", 0.10);
        t.selection = hex_alpha("#c98a5e", 0.35);
        t.overlay = hex_alpha("#000000", 0.35);
        t.overlay_subtle = hex_alpha("#000000", 0.03);
        t.overlay_light = hex_alpha("#000000", 0.06);
        t.overlay_medium = hex_alpha("#000000", 0.10);
        t.overlay_strong = hex_alpha("#000000", 0.18);
        t.diff_added_text = hex("#2d6a3a");
        t.diff_removed_text = hex("#a04040");
        t.diff_added_bg = rgba(70, 140, 80, 0.10);
        t.diff_removed_bg = rgba(200, 70, 70, 0.10);
        t.error_bubble = rgba(239, 91, 91, 0.50);
        t
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

    /// Italic font — falls back to proportional.
    ///
    /// NOTE: No italic face is embedded. To add italic support, bundle an
    /// italic font file (e.g. Inter-Italic.ttf) and register it under
    /// `FontFamily::Name("italic")`. egui's FontTweak does not support
    /// synthetic italic in v0.31.
    pub fn font_italic(&self, size: f32) -> egui::FontId {
        egui::FontId::new(size, egui::FontFamily::Proportional)
    }

    /// Scale all typography tokens by a factor (e.g. 0.9 for compact, 1.15 for large).
    ///
    /// Layout dimensions that are derived from `text_base` are re-computed so the
    /// UI remains proportional after font scaling.
    pub fn with_font_scale(mut self, scale: f32) -> Self {
        self.font_scale = scale;
        self.text_xs *= scale;
        self.text_sm *= scale;
        self.text_base *= scale;
        self.text_md *= scale;
        self.text_lg *= scale;
        self.text_xl *= scale;
        self.text_2xl *= scale;
        // S6: sidebar width is fixed at `text_base * 15` and must follow font scale.
        self.size_sidebar = self.text_base * 15.0;
        // Sidebar micro-layout is driven by text/icon size, so it also scales.
        self.size_nav_icon_rail *= scale;
        self.size_nav_row_h *= scale;
        self
    }

    /// Default font scale for first launch — two notches smaller than the
    /// unscaled token scale (1.0), matching the visual density target.
    pub const DEFAULT_FONT_SCALE: f32 = 0.85;
    /// Single Ctrl + +/- step in the UI.
    pub const FONT_SCALE_STEP: f32 = 0.075;
    /// Lower bound for user-driven font scaling.
    pub const MIN_FONT_SCALE: f32 = 0.7;
    /// Upper bound for user-driven font scaling.
    pub const MAX_FONT_SCALE: f32 = 1.3;

    /// Icon font at the given semantic size token (requires Lucide icon font registered via `lucide-icons` crate; see ADR-010).
    pub fn font_icon(&self, size: f32) -> egui::FontId {
        egui::FontId::new(size, egui::FontFamily::Name("icons".into()))
    }

    /// Calibrated line height for the proportional body font at current scale.
    ///
    /// Queries `egui::Fonts::row_height()` so the value tracks font family,
    /// font scale, and DPI changes — no hardcoded constant.
    pub fn line_height(&self, ctx: &egui::Context) -> f32 {
        let font_id = self.font(self.text_base);
        ctx.fonts(|f| f.row_height(&font_id))
    }

    /// Calibrated line height for the monospace font at current scale.
    pub fn line_height_code(&self, ctx: &egui::Context) -> f32 {
        self.line_height_mono_at(ctx, self.text_sm)
    }

    /// Calibrated line height for the monospace font at a specific size.
    pub fn line_height_mono_at(&self, ctx: &egui::Context, size: f32) -> f32 {
        let font_id = self.font_mono(size);
        ctx.fonts(|f| f.row_height(&font_id))
    }

    // ── Third-party theme presets ─────────────────────────────────────────

    /// Catppuccin Mocha — warm pastel dark theme with lavender accent.
    /// Palette: https://github.com/catppuccin/catppuccin
    pub fn catppuccin_mocha() -> Self {
        let mut t = Self::from_palette(
            Palette {
                bg0: hex("#1E1E2E"),
                bg1: hex("#313244"),
                bg2: hex("#45475A"),
                bg3: hex("#585B70"),
                fg0: hex("#CDD6F4"),
                fg1: hex("#A6ADC8"),
                fg2: hex("#6C7086"),
                fg3: hex("#585B70"),
                accent: hex("#CBA6F7"),
                accent_hover: hex("#DDB6FA"),
                red: hex("#F38BA8"),
                green: hex("#A6E3A1"),
                yellow: hex("#F9E2AF"),
                blue: hex("#89B4FA"),
                overlay_base: egui::Color32::BLACK,
            },
            "Inter",
            "JetBrains Mono",
        );
        t.code_block_bg = hex("#11111B");
        t.warn = hex("#FAB387");
        t.user_bubble = hex("#45475A");
        t
    }

    /// Tokyo Night — deep blue-black with vibrant syntax colors.
    /// Inspired by the Tokyo Night VS Code theme.
    pub fn tokyo_night() -> Self {
        let mut t = Self::from_palette(
            Palette {
                bg0: hex("#1A1B26"),
                bg1: hex("#24283B"),
                bg2: hex("#3B4261"),
                bg3: hex("#565F89"),
                fg0: hex("#C0CAF5"),
                fg1: hex("#9AA5CE"),
                fg2: hex("#565F89"),
                fg3: hex("#414868"),
                accent: hex("#7AA2F7"),
                accent_hover: hex("#89B4FA"),
                red: hex("#F7768E"),
                green: hex("#9ECE6A"),
                yellow: hex("#E0AF68"),
                blue: hex("#7DCFFF"),
                overlay_base: egui::Color32::BLACK,
            },
            "Inter",
            "JetBrains Mono",
        );
        t.code_block_bg = hex("#16161E");
        t
    }

    /// One Dark — Atom's iconic dark theme with blue accent.
    pub fn one_dark() -> Self {
        let mut t = Self::from_palette(
            Palette {
                bg0: hex("#282C34"),
                bg1: hex("#2C313A"),
                bg2: hex("#3A3F4B"),
                bg3: hex("#4B5362"),
                fg0: hex("#ABB2BF"),
                fg1: hex("#828997"),
                fg2: hex("#5C6370"),
                fg3: hex("#4B5362"),
                accent: hex("#61AFEF"),
                accent_hover: hex("#7EC8FF"),
                red: hex("#E06C75"),
                green: hex("#98C379"),
                yellow: hex("#E5C07B"),
                blue: hex("#56B6C2"),
                overlay_base: egui::Color32::BLACK,
            },
            "Inter",
            "JetBrains Mono",
        );
        t.code_block_bg = hex("#1E2229");
        t
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

pub(crate) fn rgba(r: u8, g: u8, b: u8, a: f32) -> egui::Color32 {
    let a = (a * 255.0).clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgba_premultiplied(r, g, b, a)
}

/// Installs custom fonts into the egui context.
///
/// Embedded fonts are set synchronously so the first frame can render
/// immediately. The (potentially large) system CJK font is loaded in a
/// background thread and applied once available.
pub fn setup_fonts(ctx: &egui::Context) {
    ctx.set_fonts(build_font_definitions(false));

    // Load the system CJK fallback without blocking window creation.
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let fonts = build_font_definitions(true);
        ctx.set_fonts(fonts);
        ctx.request_repaint();
    });
}

/// Build the full font stack. When `include_system_cjk` is `true`, the function
/// attempts to read a system CJK font from disk; this is intentionally done in
/// a helper so it can be deferred to a background thread.
fn build_font_definitions(include_system_cjk: bool) -> egui::FontDefinitions {
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
    if include_system_cjk {
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
        .extend(["inter".into(), "noto-sc".into(), "lucide".into()]);

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

    fonts
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

    #[test]
    fn sidebar_width_is_text_base_times_fifteen() {
        let t = Theme::dark();
        assert!((t.size_sidebar - t.text_base * 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn font_scale_updates_sidebar_width() {
        let t = Theme::dark().with_font_scale(1.5);
        assert!((t.size_sidebar - t.text_base * 15.0).abs() < f32::EPSILON);
        assert!(t.size_sidebar > Theme::dark().size_sidebar);
    }

    #[test]
    fn default_font_scale_matches_layout_target() {
        // DEFAULT_FONT_SCALE is a compile-time constant; verify it is neither
        // the old 1.0 default nor below the minimum zoom bound.
        assert!((Theme::DEFAULT_FONT_SCALE - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn bot_bar_has_positive_height() {
        let t = Theme::dark();
        assert!(t.size_bot_bar > 0.0);
    }

    #[test]
    fn nav_layout_tokens_are_positive() {
        let t = Theme::dark();
        assert!(t.size_nav_icon_rail > 0.0);
        assert!(t.size_nav_row_h > 0.0);
    }

    #[test]
    fn font_scale_updates_nav_layout_tokens() {
        let base = Theme::dark();
        let scaled = Theme::dark().with_font_scale(1.5);
        assert!((scaled.size_nav_icon_rail - base.size_nav_icon_rail * 1.5).abs() < f32::EPSILON);
        assert!((scaled.size_nav_row_h - base.size_nav_row_h * 1.5).abs() < f32::EPSILON);
    }
}
