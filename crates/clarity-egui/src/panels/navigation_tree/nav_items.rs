//! Primary navigation items: New Session row, Skills, and Plugins links.
//!
//! These rows use the shared [`nav_row`] helper so they share the same flat
//! hover / selected background and icon-rail grid as the rest of the sidebar.

use crate::App;

/// Render the "New Session" row and Skills/Plugins navigation links.
pub fn render_nav_items(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let is_loading = matches!(app.view_state.turn, clarity_core::ui::TurnState::Loading);

    // ── New Session ──
    // Shown as a flat row with a trailing shortcut, matching the reference UI.
    let new_session_resp = ui.add_enabled_ui(!is_loading, |ui| {
        crate::widgets::nav_row_with_trailing(
            ui,
            &theme,
            crate::theme::ICON_PLUS,
            app.t("New Session"),
            false,
            |ui| {
                ui.label(
                    egui::RichText::new(app.t("New Session Shortcut"))
                        .size(theme.text_xs)
                        .color(theme.text_muted),
                );
            },
        )
    });
    let mut new_session_response = new_session_resp.inner;
    if !is_loading {
        new_session_response = new_session_response.on_hover_text(app.t("New session (Ctrl+N)"));
    }
    if new_session_response.clicked() {
        app.new_session();
    }

    ui.add_space(theme.space_4);

    // ── Skills ──
    let is_skills_open = matches!(
        app.view_state.modal,
        Some(clarity_core::ui::ModalType::Skill)
    );
    let skills_resp = crate::widgets::nav_row(
        ui,
        &theme,
        crate::theme::ICON_BOOK,
        app.t("Skills"),
        is_skills_open,
    );
    if skills_resp.clicked() {
        app.view_state
            .open_modal(clarity_core::ui::ModalType::Skill);
    }

    // ── Plugins ──
    let is_plugins_open = matches!(app.view_state.modal, Some(clarity_core::ui::ModalType::Mcp));
    let plugins_resp = crate::widgets::nav_row(
        ui,
        &theme,
        crate::theme::ICON_LAYERS,
        app.t("Plugins"),
        is_plugins_open,
    );
    if plugins_resp.clicked() {
        app.view_state.open_modal(clarity_core::ui::ModalType::Mcp);
    }
}
