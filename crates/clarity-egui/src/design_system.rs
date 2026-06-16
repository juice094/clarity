//! Semantic design-system layer over raw egui APIs.
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
//!   row(ui, |ui| {
//!       gap(ui, Space::S2);
//!       surface(ui, Surface::Card, |ui| {
//!           text(ui, "Hello", Text::Body);
//!       });
//!   });
//!
//! Primitives that are not yet wired in the UI have been removed to keep the
//! dead-code surface minimal. They can be recovered from git history when a
//! concrete consumer is added.

use crate::theme::Theme;

// =============================================================================
// Theme injection — store Theme in egui Context so helpers can auto-retrieve
// =============================================================================

fn theme_id() -> egui::Id {
    egui::Id::new("clarity_design_system_theme")
}

/// Install Theme into the egui Context. Call once per frame (or when theme changes).
pub fn install_theme(ctx: &egui::Context, theme: Theme) {
    ctx.data_mut(|d| d.insert_temp(theme_id(), theme));
}

/// Retrieve the installed Theme from Context, falling back to the default theme.
fn theme(ctx: &egui::Context) -> Theme {
    ctx.data(|d| d.get_temp::<Theme>(theme_id()))
        .unwrap_or_default()
}

// =============================================================================
// Surface — visual layering (background, border, radius, shadow, padding)
// =============================================================================

/// Surface variants actively used by the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// Elevated card with border, radius, shadow, padding.
    Card,
    /// Compact list/summary well — small radius, small padding, no border.
    Well,
    /// Inline warning block — tinted orange/yellow bg.
    Warning,
}

impl Surface {
    fn frame(self, t: &Theme) -> egui::Frame {
        match self {
            Surface::Card => egui::Frame::new()
                .fill(t.bg)
                .stroke(egui::Stroke::new(0.5, t.border))
                .corner_radius(egui::CornerRadius::same(t.radius_lg.round() as u8))
                .shadow(t.shadow_card),
            Surface::Well => egui::Frame::new()
                .fill(t.surface)
                .corner_radius(egui::CornerRadius::same(t.radius_sm.round() as u8)),
            Surface::Warning => egui::Frame::new()
                .fill(t.warn.linear_multiply(0.15))
                .corner_radius(egui::CornerRadius::same(t.radius_sm.round() as u8)),
        }
    }

    fn padding(self, t: &Theme) -> egui::Margin {
        match self {
            Surface::Card => egui::Margin::same(t.space_16.round() as i8),
            Surface::Well => egui::Margin::same(t.space_8.round() as i8),
            Surface::Warning => {
                egui::Margin::symmetric(t.space_12.round() as i8, t.space_12.round() as i8)
            }
        }
    }
}

/// Render a semantic surface. Theme is retrieved from Context automatically.
pub fn surface<R>(
    ui: &mut egui::Ui,
    kind: Surface,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::Response, R) {
    let t = theme(ui.ctx());
    let mut frame = kind.frame(&t);
    frame = frame.inner_margin(kind.padding(&t));
    let inner = frame.show(ui, add_contents);
    (inner.response, inner.inner)
}

// =============================================================================
// Stack — layout primitives
// =============================================================================

/// Horizontal row, items left-to-right, vertically centered.
pub fn row<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| add_contents(ui)).inner
}

/// Center content in both axes.
pub fn center<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.with_layout(
        egui::Layout::top_down_justified(egui::Align::Center),
        |ui| add_contents(ui),
    )
    .inner
}

/// Push next item to the right edge.
pub fn push_right(ui: &mut egui::Ui) {
    ui.add_space(ui.available_width());
}

// =============================================================================
// Text — typography semantics
// =============================================================================

/// Text variants actively used by the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Text {
    /// 14px, normal, primary color.
    Body,
    /// 14px, strong, primary color.
    BodyStrong,
    /// 14px, normal, muted color.
    BodyMuted,
    /// 14px, strong, accent color.
    Accent,
    /// 12px, normal, dim color.
    Caption,
    /// 12px, strong, primary color.
    CaptionStrong,
    /// 10px, normal, dim color.
    Small,
}

impl Text {
    fn size(self, t: &Theme) -> f32 {
        match self {
            Text::Body | Text::BodyStrong | Text::BodyMuted | Text::Accent => t.text_base,
            Text::Caption | Text::CaptionStrong => t.text_sm,
            Text::Small => t.text_xs,
        }
    }

    fn color(self, t: &Theme) -> egui::Color32 {
        match self {
            Text::Body | Text::BodyStrong | Text::CaptionStrong => t.text,
            Text::BodyMuted => t.text_muted,
            Text::Caption | Text::Small => t.text_dim,
            Text::Accent => t.accent,
        }
    }

    fn strong(self) -> bool {
        matches!(self, Text::BodyStrong | Text::CaptionStrong | Text::Accent)
    }

    fn to_richtext(self, t: &Theme, content: impl Into<String>) -> egui::RichText {
        let mut rt = egui::RichText::new(content.into())
            .size(self.size(t))
            .color(self.color(t));
        if self.strong() {
            rt = rt.strong();
        }
        rt
    }
}

/// Render semantic text. Theme auto-retrieved from Context.
pub fn text(ui: &mut egui::Ui, content: impl Into<String>, style: Text) -> egui::Response {
    let t = theme(ui.ctx());
    ui.label(style.to_richtext(&t, content))
}

// =============================================================================
// Spacer — spacing tokens (4px baseline)
// =============================================================================

/// Space variants actively used by the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    /// 4px — tight inset, icon gaps.
    S0,
    /// 8px — default element gap.
    S1,
    /// 12px — related group separation.
    S2,
    /// 16px — card padding, section gap.
    S3,
    /// 40px — page-level spacing.
    S6,
}

impl Space {
    fn px(self, t: &Theme) -> f32 {
        match self {
            Space::S0 => t.space_4,
            Space::S1 => t.space_8,
            Space::S2 => t.space_12,
            Space::S3 => t.space_16,
            Space::S6 => t.space_40,
        }
    }
}

/// Add semantic spacing. Theme auto-retrieved from Context.
pub fn gap(ui: &mut egui::Ui, space: Space) {
    let t = theme(ui.ctx());
    ui.add_space(space.px(&t));
}

// =============================================================================
// Button primitives
// =============================================================================

/// Button style variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonStyle {
    Primary,
    Secondary,
    Danger,
    Ghost,
}

/// Semantic button. Theme auto-retrieved.
pub fn btn(ui: &mut egui::Ui, label: impl Into<String>, style: ButtonStyle) -> egui::Response {
    let t = theme(ui.ctx());
    let (bg, fg, radius) = match style {
        ButtonStyle::Primary => (t.accent, t.bg, t.radius_md),
        ButtonStyle::Secondary => (t.surface, t.text, t.radius_sm),
        ButtonStyle::Danger => (t.danger, t.bg, t.radius_md),
        ButtonStyle::Ghost => (egui::Color32::TRANSPARENT, t.text, t.radius_sm),
    };
    ui.add(
        egui::Button::new(
            egui::RichText::new(label.into())
                .size(t.text_base)
                .color(fg),
        )
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(radius.round() as u8)),
    )
}

// =============================================================================
// Scroll — semantic scroll areas
// =============================================================================

/// Scroll configurations actively used by the UI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scroll {
    /// Vertical scroll with a max height (most common for lists).
    VerticalMax(f32),
}

/// Render a semantic scroll area. Theme auto-retrieved.
pub fn scroll<R>(
    ui: &mut egui::Ui,
    kind: Scroll,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    match kind {
        Scroll::VerticalMax(max_h) => {
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .auto_shrink([false, true])
                .show(ui, add_contents)
                .inner
        }
    }
}

// =============================================================================
// Status indicators
// =============================================================================

/// Status dot colors actively used by the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Online,
    Offline,
}

impl Status {
    fn color(self, t: &Theme) -> egui::Color32 {
        match self {
            Status::Online => t.ok,
            Status::Offline => t.text_dim,
        }
    }
}

/// Render a small status dot.
pub fn status_dot(ui: &mut egui::Ui, status: Status) -> egui::Response {
    let t = theme(ui.ctx());
    let size = egui::vec2(8.0, 8.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 4.0, status.color(&t));
    response
}
