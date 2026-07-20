//! Work template quick-launch items — flat action rows in the left
//! navigation tree (no section wrapper). Each template creates a new
//! session pre-filled with the template's prompt.
//!
//! Always visible regardless of Work/Chat mode per design spec:
//! the sidebar structure is identical across contexts; mode only
//! affects session initialization defaults.

use crate::App;

/// Render work template action rows and the "Manage" entry.
pub fn render_work_templates(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();
    let templates: Vec<crate::settings::WorkTemplate> =
        app.settings_store().settings_edit.work_templates.clone();

    // Template action rows — one click = new session from template.
    for template in &templates {
        let resp = crate::widgets::nav_row(
            ui,
            &theme,
            crate::theme::ICON_LAYOUT_TEMPLATE,
            &template.name,
            false,
        );
        if resp.clicked() {
            app.new_session_with_prompt(&template.prompt);
        }
    }

    // "Manage templates" entry — subtle link to the management modal.
    let manage_resp = crate::widgets::nav_row(
        ui,
        &theme,
        crate::theme::ICON_PLUS,
        app.t("Manage templates"),
        false,
    );
    if manage_resp
        .on_hover_text(app.t("Manage templates"))
        .clicked()
    {
        app.open_modal(clarity_core::ui::ModalType::ManageWorkTemplates);
    }
}
