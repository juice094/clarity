//! Collapsible web bookmarks section in the left navigation tree.

use crate::App;

/// Render a collapsible web bookmarks section.
pub fn render_web_section(app: &mut App, ui: &mut egui::Ui, section_stable_id: &str) {
    let theme = app.ui_store.theme.clone();
    let links = app.settings_store.settings_edit.web_links.clone();
    let mut expanded = app.view_state.expansions.nav_web;

    crate::widgets::collapsible_section::collapsible_section(
        ui,
        section_stable_id,
        app.t("Web"),
        crate::theme::ICON_GLOBE,
        &mut expanded,
        &theme,
        |ui| {
            if links.is_empty() {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(app.t("No bookmarks"))
                            .size(theme.text_xs)
                            .color(theme.text_muted),
                    )
                    .selectable(false),
                );
            } else {
                for link in &links {
                    let resp = crate::widgets::nav_row(
                        ui,
                        &theme,
                        crate::theme::ICON_GLOBE,
                        &link.name,
                        false,
                    );
                    if resp.clicked() {
                        app.open_web_link(&link.url);
                    }
                }
            }

            crate::design_system::gap(ui, crate::design_system::Space::S0);
            let add_resp =
                crate::widgets::nav_row(ui, &theme, crate::theme::ICON_PLUS, app.t("Add"), false);
            if add_resp.on_hover_text(app.t("Manage bookmarks")).clicked() {
                app.view_state
                    .open_modal(clarity_core::ui::ModalType::ManageWebLinks);
            }
        },
    );

    app.view_state.expansions.nav_web = expanded;
}
