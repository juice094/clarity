//! Collapsible web bookmarks section in the left navigation tree.

use crate::App;

/// Context for selecting which web link list to display/edit.
#[derive(Clone, Copy)]
pub enum WebSectionContext {
    Chat,
    Work,
}

/// Render a collapsible web bookmarks section.
pub fn render_web_section(
    app: &mut App,
    ui: &mut egui::Ui,
    section_stable_id: &str,
    context: WebSectionContext,
) {
    let theme = app.ui_store.theme.clone();

    let links: Vec<crate::settings::WebLink> = match context {
        WebSectionContext::Chat => app.settings_store.settings_edit.web_links_chat.clone(),
        WebSectionContext::Work => app.settings_store.settings_edit.web_links_work.clone(),
    };

    let mut expanded = match context {
        WebSectionContext::Chat => app.view_state.expansions.nav_web_chat,
        WebSectionContext::Work => app.view_state.expansions.nav_web_work,
    };

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

            ui.add_space(theme.space_4);
            let add_resp =
                crate::widgets::nav_row(ui, &theme, crate::theme::ICON_PLUS, app.t("Add"), false);
            if add_resp.on_hover_text(app.t("Manage bookmarks")).clicked() {
                let modal = match context {
                    WebSectionContext::Chat => clarity_core::ui::ModalType::ManageWebLinksChat,
                    WebSectionContext::Work => clarity_core::ui::ModalType::ManageWebLinksWork,
                };
                app.view_state.open_modal(modal);
            }
        },
    );

    match context {
        WebSectionContext::Chat => app.view_state.expansions.nav_web_chat = expanded,
        WebSectionContext::Work => app.view_state.expansions.nav_web_work = expanded,
    }
}
