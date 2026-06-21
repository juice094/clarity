//! Primary navigation items: Work/Chat toggle, New Session/Task row,
//! Skills, Plugins, and a fixed Web entry.
//!
//! These rows use the shared [`nav_row`] helper so they share the same flat
//! hover / selected background and icon-rail grid as the rest of the sidebar.

use crate::App;
use clarity_core::ui::NewSessionMode;

/// Render the top-of-tree actions: Work/Chat toggle, New Session/Task,
/// Skills, Plugins, and a fixed Web entry.
pub fn render_nav_items(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let is_loading = matches!(app.view_state.turn, clarity_core::ui::TurnState::Loading);

    // ── Work / Chat mode toggle ──
    render_mode_toggle(app, ui);
    ui.add_space(theme.space_4);

    // ── New Session / New Task ──
    // Label follows the toggle: Work mode creates tasks, Chat mode creates
    // plain sessions.
    let (label, hover) = match app.view_state.new_session_mode {
        NewSessionMode::Work => (app.t("New Task"), app.t("New task (Ctrl+N)")),
        NewSessionMode::Chat => (app.t("New Session"), app.t("New session (Ctrl+N)")),
    };
    let new_session_resp = ui.add_enabled_ui(!is_loading, |ui| {
        crate::widgets::nav_row_with_trailing(
            ui,
            &theme,
            crate::theme::ICON_PLUS,
            label,
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
        new_session_response = new_session_response.on_hover_text(hover);
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

    // ── Web (fixed entry) ──
    let web_resp =
        crate::widgets::nav_row(ui, &theme, crate::theme::ICON_GLOBE, app.t("Web"), false);
    if web_resp.on_hover_text(app.t("Manage bookmarks")).clicked() {
        app.view_state
            .open_modal(clarity_core::ui::ModalType::ManageWebLinks);
    }
}

/// Render a two-segment Work/Chat toggle.
///
/// ponytail: simple text-button pair rather than a custom widget; the selected
/// segment uses a stronger surface fill so it reads as a single toggle.
fn render_mode_toggle(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let mode = app.view_state.new_session_mode;
    let available = ui.available_width();
    let gap = theme.space_4;
    let button_width = (available - gap) / 2.0;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;

        let work_resp = mode_button(
            ui,
            &theme,
            app.t("Work"),
            mode == NewSessionMode::Work,
            egui::Vec2::new(button_width, theme.size_nav_row_h),
        );
        if work_resp.clicked() {
            app.view_state.new_session_mode = NewSessionMode::Work;
        }

        let chat_resp = mode_button(
            ui,
            &theme,
            app.t("Chat"),
            mode == NewSessionMode::Chat,
            egui::Vec2::new(button_width, theme.size_nav_row_h),
        );
        if chat_resp.clicked() {
            app.view_state.new_session_mode = NewSessionMode::Chat;
        }
    });
}

/// Render one segment of the Work/Chat toggle.
fn mode_button(
    ui: &mut egui::Ui,
    theme: &crate::theme::Theme,
    label: &str,
    selected: bool,
    size: egui::Vec2,
) -> egui::Response {
    let text_color = if selected {
        theme.text_strong
    } else {
        theme.text
    };
    let bg_color = if selected {
        theme.surface_strong
    } else {
        theme.surface
    };
    let stroke_color = if selected {
        theme.border_strong
    } else {
        theme.border
    };

    let button = egui::Button::new(
        egui::RichText::new(label)
            .size(theme.text_sm)
            .color(text_color),
    )
    .min_size(size)
    .fill(bg_color)
    .stroke(egui::Stroke::new(1.0, stroke_color))
    .corner_radius(theme.radius_md);

    ui.add(button)
}
