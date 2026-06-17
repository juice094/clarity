//! Unprojected chat sessions in the left navigation tree.
//!
//! S6 Phase D: until every session is bound to a project, unprojected sessions
//! are shown in a flat "Chats" group at the bottom of the tree.

use crate::App;

/// Render the unprojected chat list.
pub fn render_unprojected_chats(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    render_section_header(ui, &theme, app.t("Chats"));

    let sessions: Vec<_> = app
        .session_store
        .sessions
        .iter()
        .filter(|s| !s.archived && s.project_id.is_none())
        .cloned()
        .collect();

    if sessions.is_empty() {
        ui.label(
            egui::RichText::new(app.t("No sessions"))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        return;
    }

    for session in sessions {
        let is_active = session.id == app.session_store.active_session_id;
        let label = if session.title.chars().count() > 24 {
            let truncated: String = session.title.chars().take(22).collect();
            format!("{}...", truncated)
        } else {
            session.title.clone()
        };
        let icon = match session.category.as_str() {
            "emotion" => crate::theme::ICON_CHAT,
            "knowledge" => crate::theme::ICON_BOOK,
            _ => crate::theme::ICON_WRENCH,
        };

        let resp = ui.selectable_label(
            is_active,
            egui::RichText::new(format!("{} {}", icon, label)).size(theme.text_sm),
        );
        if resp.clicked() && !is_active {
            app.save_current_session();
            let old_id = app.session_store.active_session_id.clone();
            if !app.chat_store.input.trim().is_empty() {
                app.session_store
                    .drafts
                    .insert(old_id, app.chat_store.input.clone());
            } else {
                app.session_store.drafts.remove(&old_id);
            }
            app.session_store.active_session_id = session.id.clone();
            app.chat_store.input = app
                .session_store
                .drafts
                .remove(&session.id)
                .unwrap_or_default();
            app.chat_store.tool_calls = crate::stores::rebuild_tool_calls(&session.messages);
        }
    }
}

fn render_section_header(ui: &mut egui::Ui, theme: &crate::theme::Theme, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(theme.text_xs)
            .color(theme.text_dim)
            .strong(),
    );
    ui.add_space(theme.space_4);
}
