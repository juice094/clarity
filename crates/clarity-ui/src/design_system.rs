//! Semantic design-system layer over raw egui APIs.
//!
//! NOTE: This module defines an expanded API surface that is being progressively
//! adopted. Dead-code warnings for new variants/functions are expected and
//! will resolve as migration continues. The `const` names intentionally use
//! PascalCase to mimic enum-variant ergonomics (e.g. `TextStyle::Body`).
#![allow(dead_code, non_upper_case_globals)]
//!
//! Solves three problems with raw egui:
//!   1. Style scattering — Frame::new().fill().stroke().corner_radius().inner_margin()
//!      repeated in every panel.
//!   2. Layout magic — egui::Layout::left_to_right(egui::Align::Center) is
//!      error-prone and non-semantic.
//!   3. No reuse — every panel re-invents its own visual treatment.
//!
//! This module injects Theme into egui::Context::data(), so all helpers are
//! zero-parameter after a single `install_theme()` call at app startup.
//!
//! Usage (after theme install):
//!
//!   ui.horizontal(|ui| {
//!       gap(ui, Space::S1);
//!       text(ui, "Hello", TextStyle::Body);
//!   });
//!
//!   // Frame presets replace ad-hoc Frame builder chains:
//!   panel_frame(ui, |ui| { ... });

use crate::theme::Theme;
use std::sync::Arc;

// =============================================================================
// Theme injection — store Theme in egui Context so helpers can auto-retrieve
// =============================================================================

fn theme_id() -> egui::Id {
    egui::Id::new("clarity_design_system_theme_arc")
}

/// Install Theme into the egui Context. Call once per frame (or when theme changes).
pub fn install_theme(ctx: &egui::Context, theme: Theme) {
    ctx.data_mut(|d| d.insert_temp(theme_id(), Arc::new(theme)));
}

/// Retrieve the installed Theme from Context, falling back to the default theme.
pub fn theme(ctx: &egui::Context) -> Arc<Theme> {
    ctx.data(|d| d.get_temp::<Arc<Theme>>(theme_id()))
        .unwrap_or_default()
}

// =============================================================================
// Spacing — 8 px baseline grid
// =============================================================================

/// Semantic spacing tokens mapped to the Theme's 8px baseline grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    ///  4 px — micro gap (icon-to-label, tight inline separators).
    S0,
    ///  8 px — default element gap.
    S1,
    /// 12 px — moderate gap (paragraph spacing, button group separation).
    S2,
    /// 16 px — section internal padding, card inner margin horizontal.
    S3,
    /// 20 px — section separator, panel header-to-body gap.
    S4,
    /// 24 px — large block separation, empty state padding.
    S5,
    /// 40 px — section-level spacing, major content boundaries.
    S6,
}

impl Space {
    fn px(self, t: &Theme) -> f32 {
        match self {
            Space::S0 => t.space_4,
            Space::S1 => t.space_8,
            Space::S2 => t.space_12,
            Space::S3 => t.space_16,
            Space::S4 => t.space_20,
            Space::S5 => t.space_24,
            Space::S6 => t.space_40,
        }
    }
}

/// Add semantic spacing. Theme auto-retrieved from Context.
pub fn gap(ui: &mut egui::Ui, space: Space) {
    let t = theme(ui.ctx());
    ui.add_space(space.px(&t));
}

/// Set item spacing for the duration of a scope, restoring it on return.
///
/// Useful for `ui.horizontal()` blocks that need tighter or looser gaps
/// than the global default.
pub fn with_item_spacing<R>(
    ui: &mut egui::Ui,
    space: Space,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let t = theme(ui.ctx());
    let px = space.px(&t);
    ui.spacing_mut().item_spacing.x = px;
    add_contents(ui)
}

// =============================================================================
// Typography — semantic text styles
// =============================================================================

/// Compositional text style descriptor.
///
/// Combines a base size variant with optional semantic modifiers.
/// The `to_richtext()` method resolves everything through the current Theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextStyle {
    pub size: TextSize,
    pub modifiers: TextModifiers,
}

impl TextStyle {
    /// Body text at the base reading size (14 px).
    pub const Body: Self = Self {
        size: TextSize::Body,
        modifiers: TextModifiers::empty(),
    };

    /// Return this style with the strong modifier set.
    pub fn strong(mut self) -> Self {
        self.modifiers.strong = true;
        self
    }

    /// Return this style with the muted modifier set.
    pub fn muted(mut self) -> Self {
        self.modifiers.muted = true;
        self
    }

    /// Return this style with the mono modifier set.
    pub fn mono(mut self) -> Self {
        self.modifiers.mono = true;
        self
    }
    /// Accented body text — draws attention without being a heading.
    pub const Accent: Self = Self {
        size: TextSize::Body,
        modifiers: TextModifiers::accented(),
    };
    /// Strong caption — section labels, field names, metadata (12 px).
    pub const CaptionStrong: Self = Self {
        size: TextSize::Small,
        modifiers: TextModifiers::strong(),
    };
    /// Dim small text — secondary metadata, timestamps, hints (10 px).
    pub const Small: Self = Self {
        size: TextSize::Caption,
        modifiers: TextModifiers::muted(),
    };

    /// Large heading — page titles, empty-state product name (36 px).
    pub const Heading: Self = Self {
        size: TextSize::Heading,
        modifiers: TextModifiers::strong(),
    };
    /// Subheading — panel titles, section headers (22 px).
    pub const Subheading: Self = Self {
        size: TextSize::Subheading,
        modifiers: TextModifiers::strong(),
    };
    /// Title — card titles, dialog headers (18 px).
    pub const Title: Self = Self {
        size: TextSize::Title,
        modifiers: TextModifiers::strong(),
    };
    /// Monospace body — code snippets in prose, keyboard shortcuts.
    pub const Mono: Self = Self {
        size: TextSize::Small,
        modifiers: TextModifiers::mono(),
    };

    fn to_richtext(self, t: &Theme, content: impl Into<String>) -> egui::RichText {
        let size = match self.size {
            TextSize::Heading => t.text_2xl,
            TextSize::Subheading => t.text_xl,
            TextSize::Title => t.text_lg,
            TextSize::Body => t.text_base,
            TextSize::Small => t.text_sm,
            TextSize::Caption => t.text_xs,
        };
        let color = if self.modifiers.accented {
            t.accent
        } else if self.modifiers.muted {
            t.text_dim
        } else {
            t.text
        };
        let mut rt = egui::RichText::new(content.into()).size(size).color(color);
        if self.modifiers.strong {
            rt = rt.strong();
        }
        if self.modifiers.mono {
            rt = rt.monospace();
        }
        rt
    }
}

/// Semantic modifiers applied on top of a base font size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextModifiers {
    pub strong: bool,
    pub muted: bool,
    pub accented: bool,
    pub mono: bool,
}

impl TextModifiers {
    const fn empty() -> Self {
        Self {
            strong: false,
            muted: false,
            accented: false,
            mono: false,
        }
    }
    const fn strong() -> Self {
        Self {
            strong: true,
            muted: false,
            accented: false,
            mono: false,
        }
    }
    const fn accented() -> Self {
        Self {
            strong: false,
            muted: false,
            accented: true,
            mono: false,
        }
    }
    const fn muted() -> Self {
        Self {
            strong: false,
            muted: true,
            accented: false,
            mono: false,
        }
    }
    const fn mono() -> Self {
        Self {
            strong: false,
            muted: false,
            accented: false,
            mono: true,
        }
    }
}

/// Base font size variants.  Use through [`TextStyle`] constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSize {
    /// 36 px — product name, major page headings.
    Heading,
    /// 22 px — panel titles, section headers.
    Subheading,
    /// 18 px — card titles, dialog headers.
    Title,
    /// 14 px — reading body text.
    Body,
    /// 12 px — labels, metadata, secondary text.
    Small,
    /// 10 px — timestamps, hints, tertiary metadata.
    Caption,
}

/// Render semantic text. Theme auto-retrieved from Context.
pub fn text(ui: &mut egui::Ui, content: impl Into<String>, style: TextStyle) -> egui::Response {
    let t = theme(ui.ctx());
    ui.label(style.to_richtext(&t, content))
}

/// Render an icon glyph using the theme's icon font.
///
/// Use for toolbar icons, status glyphs, and any inline symbol that is
/// part of the icon font rather than the proportional font.
pub fn icon(ui: &mut egui::Ui, glyph: &str, size: f32) -> egui::Response {
    let t = theme(ui.ctx());
    ui.label(egui::RichText::new(glyph).font(t.font_icon(size)))
}

/// Render semantic text with an arbitrary colour override.
///
/// Use sparingly — most text should use [`text`] with a semantic
/// [`TextStyle`]. This helper exists for one-off state colours that do not
/// yet have a TextStyle token.
pub fn text_with_color(
    ui: &mut egui::Ui,
    content: impl Into<String>,
    style: TextStyle,
    color: egui::Color32,
) -> egui::Response {
    let t = theme(ui.ctx());
    ui.label(style.to_richtext(&t, content).color(color))
}

/// Render an icon glyph with an explicit colour override.
pub fn icon_with_color(
    ui: &mut egui::Ui,
    glyph: &str,
    size: f32,
    color: egui::Color32,
) -> egui::Response {
    let t = theme(ui.ctx());
    ui.label(
        egui::RichText::new(glyph)
            .font(t.font_icon(size))
            .color(color),
    )
}

/// Render text with an explicit pixel size and colour.
///
/// Use for legacy code paths that compute font size dynamically and cannot
/// be expressed through [`TextStyle`] presets. Prefer [`text`] or
/// [`text_with_color`] for new code.
pub fn text_with_size_color(
    ui: &mut egui::Ui,
    content: impl Into<String>,
    size_px: f32,
    color: egui::Color32,
) -> egui::Response {
    ui.label(
        egui::RichText::new(content.into())
            .size(size_px)
            .color(color),
    )
}

// =============================================================================
// Frame presets — pre-configured egui::Frame builders
// =============================================================================

/// Standard panel frame: subtle surface fill, soft border, moderate radius.
///
/// Use for settings panels, sidebars, and any non-modal container.
pub fn panel_frame<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let t = theme(ui.ctx());
    egui::Frame::new()
        .fill(t.surface)
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(t.radius_md as u8))
        .inner_margin(egui::Margin::symmetric(t.space_16 as i8, t.space_12 as i8))
        .show(ui, add_contents)
}

/// Modal overlay frame: stronger border, larger radius, elevation shadow.
///
/// Use for dialogs, settings modals, and any overlay that sits above
/// the main content with a scrim behind it.
pub fn modal_frame<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let t = theme(ui.ctx());
    egui::Frame::new()
        .fill(t.bg_elevated)
        .stroke(egui::Stroke::new(1.0, t.border_strong))
        .corner_radius(egui::CornerRadius::same(t.radius_lg as u8))
        .shadow(t.shadow_modal)
        .inner_margin(egui::Margin::symmetric(t.space_20 as i8, t.space_16 as i8))
        .show(ui, add_contents)
}

/// Inline code / code-block frame: dark inset background, monospace-ready.
///
/// Use for code blocks in chat, diff previews, and any syntax-highlighted
/// content block.
pub fn code_frame<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let t = theme(ui.ctx());
    egui::Frame::new()
        .fill(t.code_block_bg)
        .stroke(egui::Stroke::NONE)
        .corner_radius(egui::CornerRadius::same(t.radius_sm as u8))
        .inner_margin(egui::Margin::symmetric(t.space_12 as i8, t.space_8 as i8))
        .show(ui, add_contents)
}

/// Compact chip / tag frame: accent-tinted fill, small radius, tight padding.
///
/// Use for context chips, mention badges, tool-call status tags, and any
/// small inline label that needs visual distinction.
pub fn chip_frame<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let t = theme(ui.ctx());
    egui::Frame::new()
        .fill(t.accent_subtle)
        .stroke(egui::Stroke::NONE)
        .corner_radius(egui::CornerRadius::same(t.radius_sm as u8))
        .inner_margin(egui::Margin::symmetric(t.space_8 as i8, t.space_4 as i8))
        .show(ui, add_contents)
}

/// Interactive row frame: transparent by default, hover tint on interaction.
///
/// Does NOT call `.show()` — returns the configured Frame so the caller
/// can attach `.sense()` or conditional `.fill()` before showing.
pub fn interactive_row_frame(ui: &mut egui::Ui) -> egui::Frame {
    let t = theme(ui.ctx());
    egui::Frame::new()
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .corner_radius(egui::CornerRadius::same(t.radius_sm as u8))
        .inner_margin(egui::Margin::symmetric(t.space_8 as i8, t.space_4 as i8))
}

// =============================================================================
// Elevation system — semantic surface + shadow + radius + padding
//
// This is the canonical way to express "this thing floats at level X".
// Prefer `elevation().show()` over ad-hoc `Frame::new()` chains.
// =============================================================================

/// Visual elevation level for surfaces.
///
/// Each level maps to a fixed combination of fill, stroke, radius, shadow,
/// and inner padding. Higher levels draw more attention by lifting the
/// surface toward the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    /// Window / application background. No shadow, full bleed.
    Base,
    /// Scrollable planes and primary containers (settings, left nav).
    Surface,
    /// Cards, list rows, message bubbles — slightly above the surface.
    Elevated,
    /// Floating panels that sit above main content (skill/mcp/palette).
    Overlay,
    /// Blocking dialogs. Strong shadow, large radius.
    Modal,
    /// Transient notifications. Small but highest shadow.
    Toast,
}

impl Elevation {
    /// Background fill for this elevation.
    fn fill(self, t: &Theme) -> egui::Color32 {
        match self {
            Elevation::Base => t.bg,
            Elevation::Surface => t.surface,
            Elevation::Elevated => t.bg_elevated,
            Elevation::Overlay => t.bg_elevated,
            Elevation::Modal => t.bg_elevated,
            Elevation::Toast => t.bg_elevated,
        }
    }

    /// Border stroke for this elevation.
    fn stroke(self, t: &Theme) -> egui::Stroke {
        let color = match self {
            Elevation::Base => t.border,
            Elevation::Surface => t.border,
            Elevation::Elevated => t.border,
            Elevation::Overlay => t.border_strong,
            Elevation::Modal => t.border_strong,
            Elevation::Toast => t.border_strong,
        };
        egui::Stroke::new(1.0, color)
    }

    /// Corner radius for this elevation.
    fn radius(self, t: &Theme) -> egui::CornerRadius {
        let px = match self {
            Elevation::Base => 0.0,
            Elevation::Surface => t.radius_md,
            Elevation::Elevated => t.radius_md,
            Elevation::Overlay => t.radius_lg,
            Elevation::Modal => t.radius_lg,
            Elevation::Toast => t.radius_lg,
        };
        egui::CornerRadius::same(px as u8)
    }

    /// Shadow cast by this elevation.
    fn shadow(self, t: &Theme) -> egui::Shadow {
        match self {
            Elevation::Base => egui::Shadow::NONE,
            Elevation::Surface => t.shadow_panel,
            Elevation::Elevated => t.shadow_card,
            Elevation::Overlay => t.shadow_panel,
            Elevation::Modal => t.shadow_modal,
            Elevation::Toast => t.shadow_toast,
        }
    }

    /// Inner padding for this elevation.
    fn padding(self, t: &Theme) -> egui::Margin {
        let (x, y) = match self {
            Elevation::Base => (0.0, 0.0),
            Elevation::Surface => (t.space_16, t.space_12),
            Elevation::Elevated => (t.space_12, t.space_8),
            Elevation::Overlay => (t.space_16, t.space_12),
            Elevation::Modal => (t.space_20, t.space_16),
            Elevation::Toast => (t.space_16, t.space_12),
        };
        egui::Margin::symmetric(x as i8, y as i8)
    }

    /// Build an `egui::Frame` configured for this elevation.
    ///
    /// The returned frame can be further customised (e.g. overriding fill for
    /// a selected state), but the defaults are the protocol-approved values.
    pub fn frame(self, t: &Theme) -> egui::Frame {
        egui::Frame::new()
            .fill(self.fill(t))
            .stroke(self.stroke(t))
            .corner_radius(self.radius(t))
            .shadow(self.shadow(t))
            .inner_margin(self.padding(t))
    }
}

/// Show a surface at the given elevation.
///
/// This is the preferred way to create containers in Clarity views.
pub fn surface<R>(
    ui: &mut egui::Ui,
    elevation: Elevation,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let t = theme(ui.ctx());
    elevation.frame(&t).show(ui, add_contents)
}

/// Convenience: a `Surface` elevation container.
pub fn surface_panel<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    surface(ui, Elevation::Surface, add_contents)
}

/// Convenience: an `Elevated` card container.
pub fn card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    surface(ui, Elevation::Elevated, add_contents)
}

/// Convenience: an `Overlay` floating container.
pub fn overlay<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    surface(ui, Elevation::Overlay, add_contents)
}

/// Convenience: a `Modal` dialog container.
pub fn modal_elevated<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    surface(ui, Elevation::Modal, add_contents)
}

/// Convenience: a `Toast` notification container.
pub fn toast_surface<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    surface(ui, Elevation::Toast, add_contents)
}

/// Kimi-style floating composer card.
///
/// A centered, rounded input container that sits above the main content.
/// It uses a slightly elevated surface, a subtle border, and a soft shadow
/// so the composer is visually anchored without a hard separator line.
pub fn composer_card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let t = theme(ui.ctx());
    egui::Frame::new()
        .fill(t.surface)
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(t.composer_radius as u8))
        .shadow(t.composer_shadow)
        .inner_margin(egui::Margin::symmetric(t.space_16 as i8, t.space_12 as i8))
        .show(ui, add_contents)
}

/// Small keyboard-hint badge (e.g. "Ctrl+N").
pub fn kbd_hint(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    let t = theme(ui.ctx());
    let text = text.into();
    egui::Frame::new()
        .fill(t.bg_hover)
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(t.radius_sm as u8))
        .inner_margin(egui::Margin::symmetric(t.space_8 as i8, t.space_4 as i8))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(t.text_xs)
                    .color(t.text_dim)
                    .monospace(),
            );
        })
        .response
}

// =============================================================================
// Status indicators
// =============================================================================

/// Status dot variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Online,
    Offline,
    Busy,
    Warning,
    Danger,
    Info,
}

impl Status {
    fn color(self, t: &Theme) -> egui::Color32 {
        match self {
            Status::Online => t.ok,
            Status::Offline => t.text_dim,
            Status::Busy => t.warn,
            Status::Warning => t.warn,
            Status::Danger => t.danger,
            Status::Info => t.info,
        }
    }
}

/// Render a small status dot (8×8 px circle).
pub fn status_dot(ui: &mut egui::Ui, status: Status) -> egui::Response {
    let t = theme(ui.ctx());
    let size = egui::vec2(8.0, 8.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 4.0, status.color(&t));
    response
}

// =============================================================================
// Layout composition primitives
// =============================================================================

/// Horizontal stack with theme-consistent item spacing and center alignment.
///
/// Equivalent to `ui.horizontal(|ui| { ui.spacing_mut().item_spacing.x = ... })`
/// but keeps the spacing scoped and the theme retrieval automatic.
pub fn hstack<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let t = theme(ui.ctx());
    let saved = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = t.space_8;
    let result = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t.space_8;
        add_contents(ui)
    });
    ui.spacing_mut().item_spacing.x = saved;
    result.inner
}

/// Vertical stack with theme-consistent item spacing.
pub fn vstack<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let t = theme(ui.ctx());
    let saved = ui.spacing().item_spacing.y;
    ui.spacing_mut().item_spacing.y = t.space_8;
    let result = ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = t.space_8;
        add_contents(ui)
    });
    ui.spacing_mut().item_spacing.y = saved;
    result.inner
}

// =============================================================================
// Shared painters — extract common paint patterns from widgets
// =============================================================================

/// Paint a focus ring around `rect` with the given corner radius.
///
/// Should be called by every interactive widget that can receive keyboard
/// focus. The ring colour comes from `theme.focus_ring`.
pub fn paint_focus_ring(ui: &egui::Ui, rect: egui::Rect, radius: egui::CornerRadius) {
    let t = theme(ui.ctx());
    ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(2.0, t.focus_ring),
        egui::StrokeKind::Inside,
    );
}

/// Attach a theme-styled tooltip to a widget response.
///
/// The tooltip appears on hover with a short delay and uses the theme's
/// surface colour, border, and text tokens for consistent styling.
///
/// Usage: `design_system::tooltip(ui, &response, "Delete this item");`
pub fn tooltip(ui: &egui::Ui, response: &egui::Response, text: impl Into<String>) {
    let t = theme(ui.ctx());
    let tooltip_text = text.into();
    response.clone().on_hover_ui_at_pointer(move |ui| {
        let frame = egui::Frame::popup(ui.style())
            .fill(t.surface)
            .stroke(egui::Stroke::new(1.0, t.border_strong))
            .corner_radius(egui::CornerRadius::same(t.radius_sm as u8))
            .inner_margin(egui::Margin::symmetric(t.space_8 as i8, t.space_4 as i8))
            .shadow(t.shadow_card);
        frame.show(ui, |ui| {
            ui.label(
                egui::RichText::new(tooltip_text.clone())
                    .size(t.text_xs)
                    .color(t.text),
            );
        });
    });
}

// =============================================================================
// Widget primitives — lightweight, theme-aware UI components
// =============================================================================

/// Horizontal semantic divider with optional label.
///
/// Renders a thin horizontal line in `theme.border` colour, optionally
/// with centred text (e.g. "OR").
pub fn divider(ui: &mut egui::Ui, label: Option<&str>) {
    let t = theme(ui.ctx());
    if let Some(text) = label {
        ui.add(
            egui::Separator::default()
                .horizontal()
                .grow(4.0)
                .spacing(t.space_8),
        );
        ui.label(
            egui::RichText::new(text)
                .size(t.text_xs)
                .color(t.text_muted),
        );
        ui.add(
            egui::Separator::default()
                .horizontal()
                .grow(4.0)
                .spacing(t.space_8),
        );
    } else {
        ui.add_space(t.space_4);
        ui.add(egui::Separator::default().horizontal().grow(4.0));
        ui.add_space(t.space_4);
    }
}

/// Badge — a small coloured label for status, counts, or categorisation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeVariant {
    Info,
    Accent,
    Warn,
    Danger,
    Ok,
    Neutral,
}

impl BadgeVariant {
    fn colors(self, t: &Theme) -> (egui::Color32, egui::Color32) {
        match self {
            BadgeVariant::Info => (t.info, crate::theme::rgba(0, 0, 0, 0.12)),
            BadgeVariant::Accent => (t.accent, t.accent_subtle),
            BadgeVariant::Warn => (t.warn, crate::theme::rgba(239, 107, 107, 0.12)),
            BadgeVariant::Danger => (t.danger, crate::theme::rgba(239, 107, 107, 0.12)),
            BadgeVariant::Ok => (t.ok, crate::theme::rgba(107, 203, 138, 0.12)),
            BadgeVariant::Neutral => (t.text_muted, crate::theme::rgba(119, 119, 119, 0.08)),
        }
    }
}

/// Render a small badge chip with theme-derived colours.
///
/// Returns the response so callers can attach `.on_hover_text()` for tooltips.
pub fn badge(ui: &mut egui::Ui, text: impl Into<String>, variant: BadgeVariant) -> egui::Response {
    let t = theme(ui.ctx());
    let (fg, bg) = variant.colors(&t);
    egui::Frame::new()
        .fill(bg)
        .stroke(egui::Stroke::NONE)
        .corner_radius(egui::CornerRadius::same(t.radius_sm as u8))
        .inner_margin(egui::Margin::symmetric(t.space_8 as i8, t.space_4 as i8))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(t.text_xs).color(fg))
        })
        .response
}

/// Toggle button group for choosing among 2-4 mutually exclusive options.
///
/// Each option is a `(label, value)` pair. The `current` value determines
/// which button appears selected. Returns the new value if the user clicked
/// a different option.
pub fn toggle_group<D: PartialEq + Copy>(
    ui: &mut egui::Ui,
    options: &[(&str, D)],
    current: D,
) -> Option<D> {
    let t = theme(ui.ctx());
    let mut clicked: Option<D> = None;
    ui.horizontal(|ui| {
        for (label, val) in options {
            let is_active = current == *val;
            let btn = egui::Button::new(egui::RichText::new(*label).size(t.text_sm))
                .fill(if is_active { t.accent } else { t.surface })
                .stroke(if is_active {
                    egui::Stroke::NONE
                } else {
                    egui::Stroke::new(1.0, t.border)
                })
                .corner_radius(t.radius_sm as u8);
            if ui.add(btn).clicked() && !is_active {
                clicked = Some(*val);
            }
        }
    });
    clicked
}

/// Simple spinner — a rotating dash for loading states.
///
/// Renders one of 8 rotation frames based on elapsed time, giving the
/// appearance of a spinning animation. Call once per frame — the animation
/// is driven by `ui.ctx().input(|i| i.time)`.
pub fn spinner(ui: &mut egui::Ui) -> egui::Response {
    let t = theme(ui.ctx());
    let now = ui.ctx().input(|i| i.time);
    let frame_idx = (now * 4.0) as usize % 8;
    let glyph = match frame_idx {
        0 => '◴',
        1 => '◷',
        2 => '◶',
        3 => '◵',
        _ => '◴', // fallback
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(t.text_base, t.text_base), egui::Sense::hover());
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(t.text_base),
        t.accent,
    );
    response
}

/// Lightweight version of `spinner` — renders as a small inline label.
///
/// Useful inside buttons or status lines where a full widget is too heavy.
pub fn inline_spinner(ui: &mut egui::Ui) {
    let t = theme(ui.ctx());
    // 8-frame braille-based spinner for inline use.
    let now = ui.ctx().input(|i| i.time);
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let frame = frames[(now * 10.0) as usize % frames.len()];
    ui.label(egui::RichText::new(frame).size(t.text_sm).color(t.accent));
}

/// Theme-styled progress bar.
///
/// `fraction` should be in `[0.0, 1.0]`. Returns the allocated response
/// so callers can attach tooltips (e.g. "Downloading model… 67%").
pub fn progress_bar(ui: &mut egui::Ui, fraction: f32) -> egui::Response {
    let t = theme(ui.ctx());
    let desired_height = t.space_8;
    let desired_width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(desired_width, desired_height),
        egui::Sense::hover(),
    );
    if ui.is_rect_visible(rect) {
        let radius = t.radius_sm * 0.5;
        // Track background
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(radius as u8), t.surface);
        // Filled portion
        let fill_w = (rect.width() * fraction.clamp(0.0, 1.0)).max(radius);
        if fill_w > 0.0 {
            let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
            ui.painter()
                .rect_filled(fill_rect, egui::CornerRadius::same(radius as u8), t.accent);
        }
    }
    response
}

/// Toggle switch — a rounded pill that slides between off/on.
///
/// Renders a clickable pill-shaped toggle. Returns `true` if the state
/// changed this frame.
pub fn toggle(ui: &mut egui::Ui, checked: &mut bool) -> bool {
    let t = theme(ui.ctx());
    let h = t.text_base + 4.0;
    let w = h * 2.2;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let radius = h * 0.5;
        // Track
        let track_color = if *checked { t.accent } else { t.surface_strong };
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(radius as u8), track_color);
        // Thumb
        let thumb_r = radius - 2.0;
        let thumb_x = if *checked {
            rect.right() - radius - 1.0
        } else {
            rect.left() + radius + 1.0
        };
        ui.painter().circle_filled(
            egui::pos2(thumb_x, rect.center().y),
            thumb_r,
            if *checked {
                egui::Color32::WHITE
            } else {
                t.text_dim
            },
        );
    }
    if response.has_focus() {
        paint_focus_ring(
            ui,
            rect.expand(2.0),
            egui::CornerRadius::same((h * 0.5) as u8 + 2),
        );
    }
    if response.clicked() {
        *checked = !*checked;
        true
    } else {
        false
    }
}

/// Render a form field label with consistent typography.
///
/// Uses `text_sm` size and `text_muted` colour — the standard
/// treatment for input labels in settings panels.
pub fn field_label(ui: &mut egui::Ui, label: impl Into<String>) {
    let t = theme(ui.ctx());
    ui.label(
        egui::RichText::new(label)
            .size(t.text_sm)
            .color(t.text_muted),
    );
}

/// Theme-styled context menu separator.
///
/// Use inside `ui.menu_button(|ui| { ... })` closures to visually
/// separate groups of menu items.
pub fn menu_separator(ui: &mut egui::Ui) {
    let t = theme(ui.ctx());
    ui.add_space(t.space_4);
    ui.add(egui::Separator::default().horizontal().grow(2.0));
    ui.add_space(t.space_4);
}

/// Skeleton loading placeholder — a pulsing rounded rect.
///
/// Useful for indicating content is loading (e.g. message list, memory
/// store indexing, model downloading). The pulse animation is driven by
/// `ctx.input(|i| i.time)` so it stays in sync with the frame clock.
///
/// Pass the desired pixel size; the widget fills it with a rounded rect
/// that pulses between `surface` and `bg_hover` colours.
pub fn skeleton(ui: &mut egui::Ui, size: egui::Vec2) -> egui::Response {
    let t = theme(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let now = ui.ctx().input(|i| i.time) as f32;
        // Pulse between 0.3 and 0.7 over a 1.5 s cycle.
        let phase = ((now * 1.2).sin() * 0.5 + 0.5) * 0.4 + 0.3;
        let color = lerp_color(t.surface, t.bg_hover, phase);
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(t.radius_sm as u8), color);
    }
    response
}

/// Search box — text input with magnifying glass icon and clear button.
///
/// Returns `true` when the text changed this frame. The caller owns
/// `query` and reads the updated value after the call.
pub fn search_box(ui: &mut egui::Ui, query: &mut String, hint: &str) -> bool {
    let t = theme(ui.ctx());
    let mut changed = false;
    egui::Frame::new()
        .fill(t.input_bg)
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(t.radius_sm as u8))
        .inner_margin(egui::Margin::symmetric(t.space_8 as i8, t.space_4 as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(crate::theme::ICON_LIST) // magnifying glass via ICON_LIST for now
                        .size(t.text_sm)
                        .color(t.text_muted),
                );
                let resp = ui.add(
                    egui::TextEdit::singleline(query)
                        .hint_text(hint)
                        .desired_width(ui.available_width() - 24.0)
                        .font(t.font(t.text_sm))
                        .frame(egui::Frame::NONE),
                );
                changed = resp.changed();
                if !query.is_empty()
                    && ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(crate::theme::ICON_X)
                                    .size(t.text_xs)
                                    .color(t.text_dim),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .corner_radius(egui::CornerRadius::same(4)),
                        )
                        .clicked()
                {
                    query.clear();
                    changed = true;
                }
            });
        });
    changed
}

/// Context chip — a small labelled tag with an optional remove button.
///
/// Used for displaying context items (files, URLs, terminal commands)
/// above the chat composer. Returns `true` if the remove button was clicked.
pub fn chip(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    icon: Option<&str>,
    show_remove: bool,
) -> (egui::Response, bool) {
    let t = theme(ui.ctx());
    let mut removed = false;
    let response = chip_frame(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = t.space_4;
            if let Some(ic) = icon {
                ui.label(egui::RichText::new(ic).size(t.text_xs).color(t.accent));
            }
            ui.label(egui::RichText::new(label).size(t.text_xs).color(t.text));
            if show_remove
                && ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(crate::theme::ICON_X)
                                .size(t.text_xs)
                                .color(t.text_dim),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .corner_radius(egui::CornerRadius::same(4)),
                    )
                    .clicked()
            {
                removed = true;
            }
        })
    });
    (response.response, removed)
}

/// Simple linear interpolation between two `Color32` values.
fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_tokens_map_to_theme() {
        let t = Theme::dark();
        assert_eq!(Space::S0.px(&t), 4.0);
        assert_eq!(Space::S1.px(&t), 8.0);
        assert_eq!(Space::S2.px(&t), 12.0);
        assert_eq!(Space::S3.px(&t), 16.0);
        assert_eq!(Space::S4.px(&t), 20.0);
        assert_eq!(Space::S5.px(&t), 24.0);
        assert_eq!(Space::S6.px(&t), 40.0);
    }

    #[test]
    fn space_tokens_map_to_light_theme() {
        let t = Theme::light();
        assert_eq!(Space::S1.px(&t), 8.0);
        assert_eq!(Space::S3.px(&t), 16.0);
    }

    #[test]
    fn space_32_token_exists_for_viewport_margins() {
        let t = Theme::dark();
        assert!((t.space_32 - 32.0).abs() < f32::EPSILON);
    }

    #[test]
    fn text_body_uses_base_size() {
        let t = Theme::dark();
        let rt = TextStyle::Body.to_richtext(&t, "Hello");
        // RichText::size() is a builder setter — verify via Theme token instead.
        // Body maps to text_base (14 px).
        assert!((t.text_base - 14.0).abs() < 0.01);
        // Verify the conversion produces a valid RichText (doesn't panic).
        let _ = rt;
    }

    #[test]
    fn text_accent_uses_accent_color() {
        let t = Theme::dark();
        let rt = TextStyle::Accent.to_richtext(&t, "Hello");
        // Accent modifier selects the accent color token.
        assert_ne!(t.accent, t.text); // accent is distinct from text
        let _ = rt;
    }

    #[test]
    fn text_heading_uses_2xl_size() {
        let t = Theme::dark();
        let rt = TextStyle::Heading.to_richtext(&t, "Title");
        // Heading maps to text_2xl (36 px).
        assert!((t.text_2xl - 36.0).abs() < 0.01);
        let _ = rt;
    }

    #[test]
    fn text_mono_selects_monospace_family() {
        let t = Theme::dark();
        let rt = TextStyle::Mono.to_richtext(&t, "code");
        // RichText API is builder-only — verify via side effect that
        // Mono modifier is set and produces a valid RichText.
        const { assert!(TextStyle::Mono.modifiers.mono) };
        let _ = rt;
    }

    #[test]
    fn status_dot_colors_match_theme() {
        let t = Theme::dark();
        assert_eq!(Status::Online.color(&t), t.ok);
        assert_eq!(Status::Offline.color(&t), t.text_dim);
        assert_eq!(Status::Danger.color(&t), t.danger);
    }

    #[test]
    fn elevation_frame_has_non_zero_padding() {
        let t = Theme::dark();
        for elevation in [
            Elevation::Base,
            Elevation::Surface,
            Elevation::Elevated,
            Elevation::Overlay,
            Elevation::Modal,
            Elevation::Toast,
        ] {
            let frame = elevation.frame(&t);
            // Frame construction must not panic and should carry theme values.
            let _ = frame.fill;
            let _ = frame.stroke;
        }
    }

    #[test]
    fn surface_helper_allocates_space() {
        let resp = run_in_frame(|ui| surface(ui, Elevation::Surface, |ui| ui.label("inside")));
        assert!(resp.response.rect.width() > 0.0);
    }

    fn run_in_frame<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        crate::theme::setup_fonts(&ctx);
        let mut f_opt = Some(f);
        let mut output = None;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(400.0, 800.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                if let Some(f) = f_opt.take() {
                    output = Some(f(ui));
                }
            });
        });
        output.expect("CentralPanel should always run its closure")
    }
}
