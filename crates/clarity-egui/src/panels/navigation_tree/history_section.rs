//! Collapsible session history section in the left navigation tree.
//!
//! Shows unprojected, non-archived sessions. Uses virtualized row rendering
//! via `egui::ScrollArea::show_rows` for performance with large session counts.

use crate::App;

/// Estimated row height for virtualized rendering.
///
/// Kept in sync with `Theme::size_nav_row_h` at runtime; the constant only
/// exists so the virtual item count can be computed before the theme is borrowed.
// LAYOUT-EXEMPT: virtualization estimate mirroring Theme::size_nav_row_h.
const EST_ROW_HEIGHT: f32 = 32.0;

/// Render the collapsible history/sessions section.
pub fn render_history_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let mut expanded = app.view_state.expansions.nav_history;

    // Collect session metadata outside the closure so it can be indexed
    // for virtualized rendering.
    let sessions: Vec<_> = app
        .session_store
        .sessions
        .iter()
        .filter(|s| !s.archived && s.project_id.is_none())
        .map(|s| SessionRow {
            id: s.id.clone(),
            title: truncate_title(&s.title),
            category: s.category.clone(),
            is_active: s.id == app.session_store.active_session_id,
        })
        .collect();

    crate::widgets::collapsible_section::collapsible_section(
        ui,
        "nav_history",
        app.t("History"),
        crate::theme::ICON_LIST,
        &mut expanded,
        &theme,
        |ui| {
            if sessions.is_empty() {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(app.t("No sessions"))
                            .size(theme.text_xs)
                            .color(theme.text_muted),
                    )
                    .selectable(false),
                );
                return;
            }

            let active_session_id = app.session_store.active_session_id.clone();
            let mut clicked_id: Option<String> = None;
            let count = sessions.len();
            // Cap visible body height so the sidebar scroll area handles overflow.
            let max_h = (count as f32 * EST_ROW_HEIGHT).min(theme.palette_max_h);

            egui::ScrollArea::vertical()
                .max_height(max_h)
                .auto_shrink([false; 2])
                .show_rows(ui, EST_ROW_HEIGHT, count, |ui, range| {
                    for session in &sessions[range] {
                        let is_active = session.is_active && session.id == active_session_id;
                        let icon = match session.category.as_str() {
                            "emotion" => crate::theme::ICON_CHAT,
                            "knowledge" => crate::theme::ICON_BOOK,
                            _ => crate::theme::ICON_WRENCH,
                        };

                        let resp = crate::widgets::interactive_row(ui, is_active, &theme, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = theme.space_8;
                                crate::widgets::nav_icon_rail(
                                    ui,
                                    &theme,
                                    icon,
                                    if is_active {
                                        theme.accent
                                    } else {
                                        theme.text_dim
                                    },
                                );
                                ui.label(
                                    egui::RichText::new(&session.title)
                                        .size(theme.text_sm)
                                        .color(if is_active {
                                            theme.text_strong
                                        } else {
                                            theme.text
                                        }),
                                );
                            });
                        });
                        if resp.response.clicked() && !session.is_active {
                            clicked_id = Some(session.id.clone());
                        }
                    }
                });

            // Deferred mutation — apply after the render pass releases app borrows.
            if let Some(id) = clicked_id {
                app.switch_to_session(id);
            }
        },
    );

    app.view_state.expansions.nav_history = expanded;
}

/// Pre-computed row data to avoid repeated string cloning during rendering.
struct SessionRow {
    id: String,
    title: String,
    category: String,
    is_active: bool,
}

fn truncate_title(title: &str) -> String {
    if title.chars().count() > 24 {
        format!("{}...", title.chars().take(22).collect::<String>())
    } else {
        title.to_string()
    }
}
