//! Layout shell — responsive three-column geometry and collapse policy.
//!
//! This module decouples the hot-path `update()` from layout arithmetic,
//! making the future single-page / three-column migration easier to reason about.

use crate::App;
use clarity_core::ui::{AppView, SidePanel};

/// Computed widths for the main screen regions.
#[derive(Debug, Clone, Copy)]
pub struct LayoutMetrics {
    /// Current sidebar width (collapsed or expanded).
    pub sidebar_w: f32,
    /// Right-side workspace panel width.
    pub workspace_w: f32,
    /// Combined width of all right-side business panels (Dashboard / Team / Task).
    pub right_panel_w: f32,
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
    let metrics = compute_metrics(app, ctx);
    if metrics.content_too_narrow(app.ui_store.theme.content_min_width) {
        // Collapse right business panels to protect the content area.
        if matches!(
            app.view_state.right,
            Some(SidePanel::Team) | Some(SidePanel::Task)
        ) {
            app.view_state.right = None;
        }
    }
    compute_metrics(app, ctx)
}

/// One-way responsive collapse: shrink when window gets narrower, never auto-restore.
fn apply_responsive_breakpoints(app: &mut App, ctx: &egui::Context) {
    let current_width = ctx.screen_rect().width();
    if let Some(last_width) = app.last_frame_width {
        if last_width >= app.ui_store.theme.breakpoint_medium
            && current_width < app.ui_store.theme.breakpoint_medium
        {
            // Dashboard is controlled by `view_state.main` (AppView), not right panel.
            // Team / Task right panels are collapsed via `view_state.right`.
            app.view_state.right = None;
        }
        if last_width >= app.ui_store.theme.breakpoint_compact
            && current_width < app.ui_store.theme.breakpoint_compact
        {
            app.ui_store.sidebar_collapsed = true;
        }
    }
    app.last_frame_width = Some(current_width);
}

/// Compute region widths from current view state.
fn compute_metrics(app: &App, ctx: &egui::Context) -> LayoutMetrics {
    let current_width = ctx.screen_rect().width();
    let sidebar_w = if app.ui_store.sidebar_collapsed {
        app.ui_store.theme.size_sidebar_collapsed
    } else {
        app.ui_store.theme.size_sidebar
    };
    let workspace_w = app.ui_store.theme.size_workspace;
    let dashboard_w = if app.view_state.main == AppView::Dashboard {
        app.ui_store.theme.size_panel_right
    } else {
        0.0
    };
    let team_w = if app.view_state.right == Some(SidePanel::Team) {
        app.ui_store.theme.size_panel_right
    } else {
        0.0
    };
    let task_w = if app.view_state.right == Some(SidePanel::Task) {
        app.ui_store.theme.size_panel_right
    } else {
        0.0
    };
    let right_panel_w = dashboard_w + team_w + task_w;
    let content_w = current_width - sidebar_w - workspace_w - right_panel_w;

    LayoutMetrics {
        sidebar_w,
        workspace_w,
        right_panel_w,
        content_w,
    }
}
