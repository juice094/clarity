//! Layout shell — responsive three-column geometry and collapse policy.
//!
//! This module decouples the hot-path `update()` from layout arithmetic,
//! making the future single-page / three-column migration easier to reason about.
//!
//! S6 (Pretext Phase A): the shell is now organised as
//!   [icon rail] [left expanded panel] [main stage] [right utility rail]
//! with one-way responsive collapse rules.

use crate::App;
use clarity_core::ui::SidePanel;

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
pub fn update_and_measure(app: &mut App, ctx: &egui::Context) -> LayoutMetrics {
    apply_responsive_breakpoints(app, ctx);

    // Protect the main stage: if content is too narrow, sacrifice right rail first,
    // then legacy right panels, then the left expanded list.
    let mut metrics = compute_metrics(app, ctx);
    if metrics.content_too_narrow(app.ui_store.theme.content_min_width) {
        app.view_state.right_rail_visible = false;
        if matches!(
            app.view_state.right,
            Some(SidePanel::Team) | Some(SidePanel::Task)
        ) {
            app.view_state.right = None;
        }
        metrics = compute_metrics(app, ctx);
        if metrics.content_too_narrow(app.ui_store.theme.content_min_width) {
            app.view_state.left_rail_expanded = false;
            metrics = compute_metrics(app, ctx);
        }
    }
    metrics
}

/// One-way responsive collapse: shrink when window gets narrower, never auto-restore.
fn apply_responsive_breakpoints(app: &mut App, ctx: &egui::Context) {
    let current_width = ctx.screen_rect().width();
    if let Some(last_width) = app.last_frame_width {
        // Below wide breakpoint: hide the right utility rail.
        if last_width >= app.ui_store.theme.breakpoint_wide
            && current_width < app.ui_store.theme.breakpoint_wide
        {
            app.view_state.right_rail_visible = false;
        }
        // Below medium breakpoint: collapse legacy right panels and left expanded list.
        if last_width >= app.ui_store.theme.breakpoint_medium
            && current_width < app.ui_store.theme.breakpoint_medium
        {
            app.view_state.right = None;
            app.view_state.left_rail_expanded = false;
        }
        // Below compact breakpoint: keep only the icon rail and main stage.
        if last_width >= app.ui_store.theme.breakpoint_compact
            && current_width < app.ui_store.theme.breakpoint_compact
        {
            app.view_state.left_rail_expanded = false;
            app.view_state.right_rail_visible = false;
        }
    }
    app.last_frame_width = Some(current_width);
}

/// Compute region widths from current view state.
fn compute_metrics(app: &App, ctx: &egui::Context) -> LayoutMetrics {
    let current_width = ctx.screen_rect().width();
    let left_rail_w = app.ui_store.theme.size_sidebar_collapsed;
    let left_panel_w = if app.view_state.left_rail_expanded {
        app.ui_store.theme.size_sidebar
    } else {
        0.0
    };
    let right_rail_w = if app.view_state.right_rail_visible {
        app.ui_store.theme.size_panel_right
    } else {
        0.0
    };
    // Dashboard as a main view still occupies the center stage; legacy right
    // panels are layered on top and measured separately so the content guard
    // can collapse them independently.
    let legacy_right_w = match app.view_state.right {
        Some(SidePanel::Team) | Some(SidePanel::Task) | Some(SidePanel::Dashboard) => {
            app.ui_store.theme.size_panel_right
        }
        _ => 0.0,
    };
    let content_w = current_width - left_rail_w - left_panel_w - right_rail_w - legacy_right_w;

    LayoutMetrics {
        left_rail_w,
        left_panel_w,
        right_rail_w,
        content_w,
    }
}
