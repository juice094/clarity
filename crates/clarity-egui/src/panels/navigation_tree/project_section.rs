//! Collapsible project tree section in the left navigation tree.

use crate::App;
use std::collections::BTreeMap;

/// Lightweight snapshot of session metadata for tree rendering.
#[derive(Clone)]
struct SessionSummary {
    id: String,
    title: String,
    category: String,
    context: crate::ui::types::SessionContext,
    project_id: Option<String>,
    archived: bool,
}

impl SessionSummary {
    fn from_session(session: &crate::ui::types::Session) -> Self {
        Self {
            id: session.id.clone(),
            title: session.title.clone(),
            category: session.category.clone(),
            context: session.context.clone(),
            project_id: session.project_id.clone(),
            archived: session.archived,
        }
    }
}

/// Render the collapsible project tree section.
pub fn render_project_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let mut expanded = app.view_state.expansions.nav_projects;

    crate::widgets::collapsible_section::collapsible_section(
        ui,
        "nav_projects",
        app.t("Projects"),
        crate::theme::ICON_FOLDER_OPEN,
        &mut expanded,
        &theme,
        |ui| {
            render_project_body(app, ui, &theme);
        },
    );

    app.view_state.expansions.nav_projects = expanded;
}

fn render_project_body(app: &mut App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    use crate::ui::types::SessionContext;

    let summaries: Vec<SessionSummary> = app
        .session_store
        .sessions
        .iter()
        .filter(|s| !matches!(s.context, SessionContext::Claw { .. }))
        .map(SessionSummary::from_session)
        .collect();

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
                .size(theme.text_xs)
                .color(theme.text_muted),
        );
        return;
    }

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
        let resp = crate::widgets::nav_row(ui, theme, icon, &name, is_selected);
        if resp.clicked() {
            selected_project = Some(project_id.clone());
        }

        ui.horizontal(|ui| {
            ui.add_space(theme.space_16);
            ui.vertical(|ui| {
                for session in sessions {
                    render_session_row(app, ui, &session, theme);
                }
            });
        });
    }

    // Archived entries.
    if !archived_sessions.is_empty() || !archived_projects_empty {
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new(app.t("Archived"))
                .size(theme.text_xs)
                .color(theme.text_dim)
                .strong(),
        );
        for session in archived_sessions {
            let title = truncate_session_title(&session.title);
            let resp = crate::widgets::interactive_row(ui, false, theme, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme.space_8;
                    crate::widgets::nav_icon_rail(
                        ui,
                        theme,
                        crate::theme::ICON_FOLDER_OPEN,
                        theme.text_dim,
                    );
                    ui.label(
                        egui::RichText::new(title)
                            .size(theme.text_sm)
                            .color(theme.text_dim),
                    );
                });
            });
            if resp.response.clicked() {
                unarchive_session_ids.push(session.id);
            }
        }
        for project in &app.project_store.archived_projects {
            let name = project.name.clone();
            let resp = crate::widgets::interactive_row(ui, false, theme, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme.space_8;
                    crate::widgets::nav_icon_rail(
                        ui,
                        theme,
                        crate::theme::ICON_FOLDER_OPEN,
                        theme.text_dim,
                    );
                    ui.label(
                        egui::RichText::new(name)
                            .size(theme.text_sm)
                            .color(theme.text_dim),
                    );
                });
            });
            if resp.response.clicked() {
                unarchive_project_ids.push(project.id.clone());
            }
        }
    }

    // Deferred mutations.
    if let Some(pid) = selected_project {
        app.project_store.selected_project_id = Some(pid);
    }
    for id in unarchive_session_ids {
        app.set_session_archived(id, false);
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
    use crate::ui::types::SessionContext;
    let icon = match &session.context {
        SessionContext::Claw { .. } => crate::theme::ICON_CPU,
        SessionContext::Work { .. } => crate::theme::ICON_WRENCH,
        SessionContext::Chat => match session.category.as_str() {
            "knowledge" => crate::theme::ICON_BOOK,
            _ => crate::theme::ICON_CHAT,
        },
    };
    let title = truncate_session_title(&session.title);
    let resp = crate::widgets::nav_row(ui, theme, icon, &title, is_active);
    if resp.clicked() && !is_active {
        app.switch_to_session(session.id.clone());
    }
}

fn truncate_session_title(title: &str) -> String {
    if title.chars().count() > 24 {
        format!("{}...", title.chars().take(22).collect::<String>())
    } else {
        title.to_string()
    }
}
