//! Primary navigation items: New Chat and Plugins.
//!
//! These rows use the shared [`nav_row`] helper so they share the same flat
//! hover / selected background and icon-rail grid as the rest of the sidebar.

use crate::App;
use crate::design_system::{self, TextStyle};

/// Render the top-of-tree actions: New Chat and Plugins.
pub fn render_nav_items(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();
    let is_loading = matches!(app.view_state.turn, clarity_core::ui::TurnState::Loading);

    // ── New Chat ──
    let new_session_resp = ui.add_enabled_ui(!is_loading, |ui| {
        crate::widgets::nav_row_with_trailing(
            ui,
            &theme,
            crate::theme::ICON_PLUS,
            app.t("New Chat"),
            false,
            |ui| {
                design_system::text(ui, "Ctrl+N", TextStyle::Small);
            },
        )
    });
    let mut new_session_response = new_session_resp.inner;
    if !is_loading {
        new_session_response = new_session_response.on_hover_text(app.t("New chat (Ctrl+N)"));
    }
    if new_session_response.clicked() {
        app.new_session();
    }

    crate::design_system::gap(ui, crate::design_system::Space::S0);

    // ── Plugins ──
    // Unified entry for skills, MCP tools, web tabs, and built-in actions.
    // The full plugin picker is invoked from here and from the composer `/` menu.
    let is_plugins_open = app.context.ui_store.plugin_picker_state.open;
    let plugins_resp = crate::widgets::nav_row(
        ui,
        &theme,
        crate::theme::ICON_LAYERS,
        app.t("Plugins"),
        is_plugins_open,
    );
    if plugins_resp.clicked() {
        app.navigate(clarity_core::ui::AppView::Chat.into());
        app.context.ui_store.plugin_picker_state.open = true;
        app.context.ui_store.plugin_picker_state.filter.clear();
        app.context.ui_store.focus_target = Some(crate::stores::FocusTarget::ChatInput);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_nav_items_does_not_panic() {
        let egui_ctx = egui::Context::default();
        let mut app = crate::apps::test_app(&egui_ctx);
        let _ = egui_ctx.run_ui(egui::RawInput::default(), |egui_ctx| {
            egui::Area::new("nav_items_test".into()).show(egui_ctx, |ui| {
                render_nav_items(&mut app, ui);
            });
        });
    }
}
