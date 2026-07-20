//! Collapsible session history section in the left navigation tree.
//!
//! Shows unprojected, non-archived sessions. Uses virtualized row rendering
//! via `egui::ScrollArea::show_rows` for performance with large session counts.

use crate::App;
use crate::design_system::{self, TextStyle};
use crate::ui::types::SessionContext;
use clarity_ui::widgets::text_input::TextInput;

/// Estimated row height for virtualized rendering.
///
/// Kept in sync with `Theme::size_nav_row_h` at runtime; the constant only
/// exists so the virtual item count can be computed before the theme is borrowed.
// LAYOUT-EXEMPT: virtualization estimate mirroring Theme::size_nav_row_h.
const EST_ROW_HEIGHT: f32 = 32.0;

/// Render the collapsible history/sessions section.
pub fn render_history_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();
    let mut expanded = app.view_state.expansions.nav_history;

    // Collect session metadata outside the closure so it can be indexed
    // for virtualized rendering.
    let sessions: Vec<_> = app
        .context
        .session_store
        .sessions
        .iter()
        .filter(|s| {
            !s.archived
                && s.project_id.is_none()
                && !matches!(s.context, SessionContext::Claw { .. })
                && (app.context.ui_store.history_search.is_empty()
                    || s.title
                        .to_lowercase()
                        .contains(&app.context.ui_store.history_search.to_lowercase()))
        })
        .map(|s| SessionRow {
            id: s.id.clone(),
            title: truncate_title(&s.title),
            category: s.category.clone(),
            context: s.context.clone(),
            is_active: s.id == app.context.session_store.active_session_id,
            diff_stats: s.diff_stats.clone(),
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
            // ── Search input ──
            if app.context.session_store.sessions.len() > 5 {
                let search_hint = app.t("Search sessions…");
                let mut query = app.context.ui_store.history_search.clone();
                let resp = ui.add(
                    TextInput::singleline(&mut query)
                        .hint_text(search_hint)
                        .width(ui.available_width()),
                );
                if resp.changed() {
                    app.context.ui_store.history_search = query;
                }
                design_system::gap(ui, design_system::Space::S0);
            }

            if sessions.is_empty() {
                let msg = if app.context.ui_store.history_search.is_empty() {
                    app.t("No sessions").to_string()
                } else {
                    app.t("No matching sessions").to_string()
                };
                design_system::text(ui, msg, TextStyle::Small);
                return;
            }

            let active_session_id = app.context.session_store.active_session_id.clone();
            let mut clicked_id: Option<String> = None;
            let mut close_ids: Vec<String> = Vec::new();
            let count = sessions.len();
            // Cap visible body height so the sidebar scroll area handles overflow.
            let max_h = (count as f32 * EST_ROW_HEIGHT).min(theme.palette_max_h);

            egui::ScrollArea::vertical()
                .max_height(max_h)
                .auto_shrink([false; 2])
                .show_rows(ui, EST_ROW_HEIGHT, count, |ui, range| {
                    for session in &sessions[range] {
                        let is_active = session.is_active && session.id == active_session_id;
                        let icon = session_icon(session);
                        let session_id = session.id.clone();
                        // Store diff stats so we can render them inside the
                        // trailing slot alongside the close button.
                        let diff_badge = session
                            .diff_stats
                            .as_ref()
                            .map(|s| format!("+{} -{}", s.lines_added, s.lines_removed));

                        let resp = crate::widgets::nav_row_with_trailing(
                            ui,
                            &theme,
                            icon,
                            &session.title,
                            is_active,
                            |ui| {
                                if let Some(ref badge) = diff_badge {
                                    design_system::text(ui, badge, TextStyle::Small);
                                }
                                // Close (archive) button — visible on hover.
                                let close_btn = crate::widgets::icon_button(
                                    ui,
                                    crate::theme::ICON_X,
                                    theme.text_xs,
                                    egui::Color32::TRANSPARENT,
                                    egui::CornerRadius::same(4),
                                    &theme,
                                );
                                if close_btn.clicked() {
                                    close_ids.push(session_id);
                                }
                            },
                        );
                        if resp.clicked() && !session.is_active {
                            clicked_id = Some(session.id.clone());
                        }
                    }
                });

            // Deferred mutation — apply after the render pass releases app borrows.
            for id in close_ids {
                app.set_session_archived(id, true);
            }
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
    context: SessionContext,
    is_active: bool,
    diff_stats: Option<crate::ui::types::DiffStats>,
}

/// Choose an icon that reflects the session *context* (Chat / Claw / Work)
/// rather than the legacy category string, so users can tell at a glance
/// which sessions route through OpenClaw and which use the local agent.
fn session_icon(session: &SessionRow) -> &'static str {
    match &session.context {
        SessionContext::Claw { .. } => crate::theme::ICON_CPU,
        SessionContext::Work { .. } => crate::theme::ICON_WRENCH,
        SessionContext::Chat => match session.category.as_str() {
            "knowledge" => crate::theme::ICON_BOOK,
            _ => crate::theme::ICON_CHAT,
        },
    }
}

fn truncate_title(title: &str) -> String {
    crate::ui::truncate::truncate(title, 24)
}
