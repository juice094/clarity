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
//!   ui.horizontal(|ui| {
//!       gap(ui, Space::S1);
//!       text(ui, "Hello", Text::Body);
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
pub fn theme(ctx: &egui::Context) -> Theme {
    ctx.data(|d| d.get_temp::<Theme>(theme_id()))
        .unwrap_or_default()
}

// =============================================================================
// Surface — visual layering (background, border, radius, shadow, padding)
// =============================================================================

// =============================================================================
// Text — typography semantics
// =============================================================================

/// Text variants actively used by the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Text {
    /// 14px, normal, primary color.
    Body,
    /// 14px, strong, accent color.
    Accent,
    /// 12px, strong, primary color.
    CaptionStrong,
    /// 10px, normal, dim color.
    Small,
}

impl Text {
    fn size(self, t: &Theme) -> f32 {
        match self {
            Text::Body | Text::Accent => t.text_base,
            Text::CaptionStrong => t.text_sm,
            Text::Small => t.text_xs,
        }
    }

    fn color(self, t: &Theme) -> egui::Color32 {
        match self {
            Text::Body | Text::CaptionStrong => t.text,
            Text::Small => t.text_dim,
            Text::Accent => t.accent,
        }
    }

    fn strong(self) -> bool {
        matches!(self, Text::CaptionStrong | Text::Accent)
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
    /// 8px — default element gap.
    S1,
}

impl Space {
    fn px(self, t: &Theme) -> f32 {
        match self {
            Space::S1 => t.space_8,
        }
    }
}

/// Add semantic spacing. Theme auto-retrieved from Context.
pub fn gap(ui: &mut egui::Ui, space: Space) {
    let t = theme(ui.ctx());
    ui.add_space(space.px(&t));
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
