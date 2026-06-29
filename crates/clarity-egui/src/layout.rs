//! Layout shell — responsive three-column geometry and collapse policy.
//!
//! This module decouples the hot-path `update()` from layout arithmetic,
//! making the future single-page / three-column migration easier to reason about.
//!
//! S6 (Pretext Phase A): the shell is now organised as
//!   [icon rail] [left expanded panel] [main stage] [right utility rail]
//! with one-way responsive collapse rules.

use crate::App;

/// Computed widths for the main screen regions.
///
/// The public fields are exposed for callers that need geometry; some are not
/// yet read by the current layout shell but are part of the stable API.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct LayoutMetrics {
    /// Width of the icon-only left rail.
    pub left_rail_w: f32,
    /// Width of the expanded list next to the icon rail (0 when collapsed).
    pub left_panel_w: f32,
    /// Width of the right utility rail (0 when collapsed).
    pub right_rail_w: f32,
    /// Remaining width available for the main content area.
    pub content_w: f32,
}

impl LayoutMetrics {
    /// Returns true if the content area is narrower than the configured minimum.
    pub fn content_too_narrow(&self, content_min_width: f32) -> bool {
        self.content_w < content_min_width
    }
}

/// Update responsive state and return computed geometry.
///
/// Should be called once per frame, before rendering panels. It applies
/// one-way collapse rules when the window shrinks below breakpoints and
/// ensures the chat/content area never drops below the configured minimum.
///
/// Design note: the left navigation rail is intentionally **never** auto-
/// collapsed by window size; it only toggles manually via the titlebar button
/// or `Ctrl+B`. The right rail and legacy right panels are still sacrificed
/// first when space is tight.
pub fn update_and_measure(app: &mut App, ctx: &egui::Context) -> LayoutMetrics {
    apply_responsive_breakpoints(app, ctx);

    // Protect the main stage: if content is too narrow, sacrifice right rail
    // first, then legacy right panels. The left rail is never auto-collapsed.
    let mut metrics = compute_metrics(app, ctx);
    if metrics.content_too_narrow(app.ui_store.theme.content_min_width) {
        app.view_state.right_rail_visible = false;
        metrics = compute_metrics(app, ctx);
    }
    metrics
}

/// One-way responsive collapse: shrink when window gets narrower, never auto-restore.
///
/// The left rail is intentionally excluded from all breakpoint-driven collapse;
/// it is only controlled by the user.
fn apply_responsive_breakpoints(app: &mut App, ctx: &egui::Context) {
    let current_width = ctx.screen_rect().width();
    if let Some(last_width) = app.last_frame_width {
        // Below wide breakpoint: hide the right utility rail.
        if last_width >= app.ui_store.theme.breakpoint_wide
            && current_width < app.ui_store.theme.breakpoint_wide
        {
            app.view_state.right_rail_visible = false;
        }
        // Below medium breakpoint: collapse legacy right panels only.
        if last_width >= app.ui_store.theme.breakpoint_medium
            && current_width < app.ui_store.theme.breakpoint_medium
        {
            app.view_state.right = None;
        }
        // Below compact breakpoint: hide the right utility rail only.
        if last_width >= app.ui_store.theme.breakpoint_compact
            && current_width < app.ui_store.theme.breakpoint_compact
        {
            app.view_state.right_rail_visible = false;
        }
    }
    app.last_frame_width = Some(current_width);
}

/// Compute region widths from current view state.
fn compute_metrics(app: &App, ctx: &egui::Context) -> LayoutMetrics {
    compute_metrics_raw(
        ctx.screen_rect().width(),
        app.ui_store.theme.size_sidebar,
        app.ui_store.theme.size_panel_right,
        app.view_state.left_rail_expanded,
        app.view_state.right_rail_visible,
    )
}

/// Pure geometry calculation — usable from tests without an egui context.
fn compute_metrics_raw(
    current_width: f32,
    size_sidebar: f32,
    size_panel_right: f32,
    left_rail_expanded: bool,
    right_rail_visible: bool,
) -> LayoutMetrics {
    // S6 Phase D: the left chrome is a single fixed-width navigation tree.
    let left_rail_w = if left_rail_expanded {
        size_sidebar
    } else {
        0.0
    };
    let left_panel_w = 0.0;
    let right_rail_w = if right_rail_visible {
        size_panel_right
    } else {
        0.0
    };
    let content_w = current_width - left_rail_w - left_panel_w - right_rail_w;

    LayoutMetrics {
        left_rail_w,
        left_panel_w,
        right_rail_w,
        content_w,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_rail_stays_expanded_even_when_narrow() {
        let m = compute_metrics_raw(
            800.0, // window width
            144.0, // sidebar
            240.0, // right rail
            true,  // left expanded
            true,  // right visible
        );
        assert_eq!(m.left_rail_w, 144.0);
        // Content is tight but left rail is preserved.
        assert!(m.content_w < 500.0);
    }

    #[test]
    fn content_area_increases_when_left_collapsed_manually() {
        let m = compute_metrics_raw(
            800.0, 144.0, 240.0, false, // left collapsed
            true,
        );
        assert_eq!(m.left_rail_w, 0.0);
        assert!(m.content_w > 500.0);
    }

    #[test]
    fn right_rail_consumes_width() {
        let m = compute_metrics_raw(1200.0, 144.0, 240.0, true, true);
        assert_eq!(m.left_rail_w, 144.0);
        assert_eq!(m.right_rail_w, 240.0);
        assert_eq!(m.content_w, 1200.0 - 144.0 - 240.0);
    }

    #[test]
    fn content_too_narrow_detects_underflow() {
        let m = compute_metrics_raw(400.0, 144.0, 240.0, true, true);
        assert!(m.content_too_narrow(480.0));
    }
}
