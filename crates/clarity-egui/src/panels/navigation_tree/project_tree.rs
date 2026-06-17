//! Project tree section in the left navigation tree.

use crate::App;
use std::collections::BTreeMap;

/// Lightweight summary of a session used for tree rendering.
#[derive(Clone)]
struct SessionSummary {
    id: String,
    title: String,
    category: String,
    project_id: Option<String>,
    archived: bool,
}

impl SessionSummary {
    fn from_session(session: &crate::ui::types::Session) -> Self {
        Self {
            id: session.id.clone(),
            title: session.title.clone(),
            category: session.category.clone(),
            project_id: session.project_id.clone(),
            archived: session.archived,
        }
    }
}

/// Render the project tree.
pub fn render_project_tree(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    render_section_header(ui, &theme, app.t("Projects"));

    // Snapshot session metadata locally so the rest of the render pass does not
    // need to borrow `app.session_store`.
    let summaries: Vec<SessionSummary> = app
        .session_store
        .sessions
        .iter()
        .map(SessionSummary::from_session)
        .collect();

    // Group active (non-archived) sessions by project_id.
    let mut projects: BTreeMap<String, Vec<SessionSummary>> = BTreeMap::new();
    let mut archived_sessions: Vec<SessionSummary> = Vec::new();
    for summary in summaries {
        if summary.archived {
            archived_sessions.push(summary);
        } else if let Some(ref pid) = summary.project_id {
            projects.entry(pid.clone()).or_default().push(summary);
        }
    }

    let archived_projects_empty = app.project_store.archived_projects.is_empty();
    if projects.is_empty() && archived_sessions.is_empty() && archived_projects_empty {
        ui.label(
            egui::RichText::new(app.t("No projects yet"))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        return;
    }

    // Project metadata lookup.
    let project_meta: BTreeMap<String, (String, bool)> = app
        .project_store
        .projects
        .iter()
        .map(|p| (p.id.clone(), (p.name.clone(), p.has_workspace)))
        .collect();

    let mut selected_project: Option<String> = None;
    let mut unarchive_session_ids: Vec<String> = Vec::new();
    let mut unarchive_project_ids: Vec<String> = Vec::new();

    for (project_id, sessions) in projects {
        let (name, has_workspace) = project_meta
            .get(&project_id)
            .cloned()
            .unwrap_or_else(|| (project_id.clone(), true));
        let is_selected = app.project_store.selected_project_id.as_deref() == Some(&project_id);
        let icon = if has_workspace {
            crate::theme::ICON_FOLDER_OPEN
        } else {
            crate::theme::ICON_GLOBE
        };
        let resp = ui.selectable_label(
            is_selected,
            egui::RichText::new(format!("{} {}", icon, name)).size(theme.text_sm),
        );
        if resp.clicked() {
            selected_project = Some(project_id.clone());
        }

        // Sessions belonging to this project.
        ui.horizontal(|ui| {
            ui.add_space(theme.space_16);
            ui.vertical(|ui| {
                for session in sessions {
                    render_session_row(app, ui, &session, &theme);
                }
            });
        });
    }

    // Archived sessions / projects.
    if !archived_sessions.is_empty() || !archived_projects_empty {
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new(app.t("Archived"))
                .size(theme.text_xs)
                .color(theme.text_dim)
                .strong(),
        );
        for session in archived_sessions {
            let resp = ui.selectable_label(
                false,
                egui::RichText::new(format!(
                    "{} {}",
                    crate::theme::ICON_FOLDER_OPEN,
                    truncate_session_title(&session.title)
                ))
                .size(theme.text_sm)
                .color(theme.text_dim),
            );
            if resp.clicked() {
                unarchive_session_ids.push(session.id);
            }
        }
        for project in &app.project_store.archived_projects {
            let resp = ui.selectable_label(
                false,
                egui::RichText::new(format!(
                    "{} {}",
                    crate::theme::ICON_FOLDER_OPEN,
                    project.name
                ))
                .size(theme.text_sm)
                .color(theme.text_dim),
            );
            if resp.clicked() {
                unarchive_project_ids.push(project.id.clone());
            }
        }
    }

    // Apply deferred mutations after all borrows are released.
    if let Some(pid) = selected_project {
        app.project_store.selected_project_id = Some(pid);
    }
    for id in unarchive_session_ids {
        if let Some(s) = app.session_store.sessions.iter_mut().find(|s| s.id == id) {
            s.archived = false;
        }
    }
    for id in unarchive_project_ids {
        app.project_store.unarchive(&id);
    }
}

fn render_session_row(
    app: &mut App,
    ui: &mut egui::Ui,
    session: &SessionSummary,
    theme: &crate::theme::Theme,
) {
    let is_active = session.id == app.session_store.active_session_id;
    let icon = match session.category.as_str() {
        "emotion" => crate::theme::ICON_CHAT,
        "knowledge" => crate::theme::ICON_BOOK,
        _ => crate::theme::ICON_WRENCH,
    };
    let resp = ui.selectable_label(
        is_active,
        egui::RichText::new(format!(
            "{} {}",
            icon,
            truncate_session_title(&session.title)
        ))
        .size(theme.text_sm),
    );
    if resp.clicked() && !is_active {
        switch_to_session(app, session.id.clone());
    }
}

fn switch_to_session(app: &mut App, session_id: String) {
    app.save_current_session();
    let old_id = app.session_store.active_session_id.clone();
    if !app.chat_store.input.trim().is_empty() {
        app.session_store
            .drafts
            .insert(old_id, app.chat_store.input.clone());
    } else {
        app.session_store.drafts.remove(&old_id);
    }
    app.session_store.active_session_id = session_id.clone();
    let new_session = app
        .session_store
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .cloned();
    app.chat_store.input = app
        .session_store
        .drafts
        .remove(&session_id)
        .unwrap_or_default();
    if let Some(s) = new_session {
        app.chat_store.tool_calls = crate::stores::rebuild_tool_calls(&s.messages);
    }
}

fn truncate_session_title(title: &str) -> String {
    if title.chars().count() > 24 {
        format!("{}...", title.chars().take(22).collect::<String>())
    } else {
        title.to_string()
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
