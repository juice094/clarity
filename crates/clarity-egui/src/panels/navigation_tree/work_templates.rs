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
    let theme = app.ui_store.theme.clone();
    let templates: Vec<crate::settings::WorkTemplate> =
        app.settings_store.settings_edit.work_templates.clone();

    // Template action rows — one click = new session from template.
    for template in &templates {
        let resp = crate::widgets::interactive_row(ui, false, &theme, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.space_8;
                crate::widgets::nav_icon_rail(
                    ui,
                    &theme,
                    crate::theme::ICON_LAYOUT_TEMPLATE,
                    theme.text_dim,
                );
                ui.label(
                    egui::RichText::new(&template.name)
                        .size(theme.text_sm)
                        .color(theme.text),
                );
            });
        });
        if resp.response.clicked() {
            app.new_session_with_prompt(&template.prompt);
        }
    }

    // "Manage templates" entry — subtle link to the management modal.
    let manage_resp = crate::widgets::interactive_row(ui, false, &theme, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.space_8;
            crate::widgets::nav_icon_rail(ui, &theme, crate::theme::ICON_PLUS, theme.text_muted);
            ui.label(
                egui::RichText::new(app.t("Manage templates"))
                    .size(theme.text_sm)
                    .color(theme.text_muted),
            );
        });
    });
    if manage_resp.response.clicked() {
        app.view_state
            .open_modal(clarity_core::ui::ModalType::ManageWorkTemplates);
    }
}
