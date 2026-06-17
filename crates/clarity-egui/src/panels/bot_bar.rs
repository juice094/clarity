//! Bot bar at the top of the central chat stage.
//!
//! Shows the active persona avatar + name on the left and up to three
//! context-dependent Lucide buttons on the right that open the IDE-style right
//! rail.

use crate::App;
use crate::ui::types::Session;
use clarity_core::ui::{RightRailContext, RightRailPanel};

/// A button descriptor for the Bot bar right side.
struct BotBarButton {
    icon: &'static str,
    tooltip: &'static str,
    panel: RightRailPanel,
    context: RightRailContext,
}

/// Compute the active session context from session metadata.
///
/// This is a temporary heuristic used until `Session` carries an explicit
/// `SessionContext` field (Phase 7).
fn session_context(session: Option<&Session>) -> crate::ui::types::SessionContext {
    if let Some(s) = session {
        // Prefer the explicit context once it has been set; fall back to the
        // legacy category/title heuristic for sessions loaded before Phase 7.
        if s.context != crate::ui::types::SessionContext::Chat {
            return s.context.clone();
        }
        let marker = format!("{} {}", s.category, s.title).to_lowercase();
        if marker.contains("claw") {
            return crate::ui::types::SessionContext::Claw {
                device_id: String::new(),
            };
        }
        if marker.contains("project") || marker.contains("workspace") {
            return crate::ui::types::SessionContext::Project {
                project_id: String::new(),
                has_workspace: true,
            };
        }
    }
    crate::ui::types::SessionContext::Chat
}

/// Return the right-side buttons for the given session context.
fn bot_bar_buttons(ctx: &crate::ui::types::SessionContext) -> Vec<BotBarButton> {
    match ctx {
        crate::ui::types::SessionContext::Chat => vec![
            BotBarButton {
                icon: crate::theme::ICON_SHARE,
                tooltip: "Share",
                panel: RightRailPanel::Share,
                context: RightRailContext::Session,
            },
            BotBarButton {
                icon: crate::theme::ICON_TERMINAL,
                tooltip: "Console",
                panel: RightRailPanel::Console,
                context: RightRailContext::Session,
            },
            BotBarButton {
                icon: crate::theme::ICON_FOLDER_OPEN,
                tooltip: "Files",
                panel: RightRailPanel::Files,
                context: RightRailContext::Session,
            },
        ],
        crate::ui::types::SessionContext::Project { has_workspace, .. } => {
            if *has_workspace {
                vec![
                    BotBarButton {
                        icon: crate::theme::ICON_FOLDER_OPEN,
                        tooltip: "Files",
                        panel: RightRailPanel::Files,
                        context: RightRailContext::Project,
                    },
                    BotBarButton {
                        icon: crate::theme::ICON_TERMINAL,
                        tooltip: "Console",
                        panel: RightRailPanel::Console,
                        context: RightRailContext::Project,
                    },
                    BotBarButton {
                        icon: crate::theme::ICON_BOOK_OPEN,
                        tooltip: "Knowledge",
                        panel: RightRailPanel::KnowledgeBase,
                        context: RightRailContext::Project,
                    },
                ]
            } else {
                vec![
                    BotBarButton {
                        icon: crate::theme::ICON_GLOBE,
                        tooltip: "Web",
                        panel: RightRailPanel::Files,
                        context: RightRailContext::Project,
                    },
                    BotBarButton {
                        icon: crate::theme::ICON_CPU,
                        tooltip: "Local",
                        panel: RightRailPanel::Console,
                        context: RightRailContext::Project,
                    },
                    BotBarButton {
                        icon: crate::theme::ICON_BOOK_OPEN,
                        tooltip: "Knowledge",
                        panel: RightRailPanel::KnowledgeBase,
                        context: RightRailContext::Project,
                    },
                ]
            }
        }
        crate::ui::types::SessionContext::Claw { .. } => vec![
            BotBarButton {
                icon: crate::theme::ICON_MONITOR,
                tooltip: "Remote",
                panel: RightRailPanel::ClawSettings,
                context: RightRailContext::Claw,
            },
            BotBarButton {
                icon: crate::theme::ICON_FILE_CODE,
                tooltip: "Files",
                panel: RightRailPanel::Files,
                context: RightRailContext::Claw,
            },
            BotBarButton {
                icon: crate::theme::ICON_LAYERS,
                tooltip: "Templates",
                panel: RightRailPanel::Templates,
                context: RightRailContext::Claw,
            },
        ],
    }
}

/// Render the Bot bar.
pub fn render_bot_bar(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let active_session = app.session_store.active_session().cloned();
    let ctx = session_context(active_session.as_ref());
    let buttons = bot_bar_buttons(&ctx);
    let bot_name: String = app
        .settings_store
        .settings_edit
        .active_persona_id
        .clone()
        .unwrap_or_else(|| "Clarity".to_string());
    let initial = bot_name.chars().next().unwrap_or('C').to_string();
    // Translate tooltips before entering the closure so `app` is not borrowed twice.
    let tooltips: Vec<&'static str> = buttons.iter().map(|btn| app.t(btn.tooltip)).collect();

    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.space_8;

                // Avatar + name. Avatar is rendered small to keep the bar compact
                // (Kimi-style) while still identifying the active persona.
                crate::widgets::avatar::avatar_sized(
                    ui,
                    &initial,
                    &theme,
                    theme.text_sm,
                    Some(theme.accent.linear_multiply(0.25)),
                    theme.accent,
                );
                ui.label(
                    egui::RichText::new(&bot_name)
                        .size(theme.text_sm)
                        .strong()
                        .color(theme.text_strong),
                );

                // Right-side rail buttons.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = theme.space_4;
                    for (btn, tooltip) in buttons.iter().zip(tooltips.iter()) {
                        let is_active = app.view_state.right_rail_visible
                            && app.view_state.right_rail_panel == btn.panel
                            && app.view_state.right_rail_context == btn.context;
                        let fill = if is_active {
                            theme.accent.linear_multiply(0.2)
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        if crate::widgets::icon_button(
                            ui,
                            btn.icon,
                            theme.text_md,
                            fill,
                            egui::CornerRadius::same(theme.radius_sm as u8),
                            &theme,
                        )
                        .on_hover_text(*tooltip)
                        .clicked()
                        {
                            if is_active {
                                app.view_state.collapse_right_rail();
                            } else {
                                app.view_state.set_right_rail_context(btn.context);
                                app.view_state.set_right_rail_panel(btn.panel);
                            }
                        }
                    }
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::types::Session;

    fn make_session(category: &str, title: &str) -> Session {
        Session {
            id: "s-1".into(),
            title: title.into(),
            category: category.into(),
            project_id: None,
            context: crate::ui::types::SessionContext::Chat,
            lifecycle: crate::ui::types::SessionLifecycle::Temporary,
            archived: false,
            messages: vec![],
            updated_at: 0,
            turn_heights: vec![],
        }
    }

    #[test]
    fn plain_session_is_chat() {
        let s = make_session("engineering", "general");
        assert_eq!(
            session_context(Some(&s)),
            crate::ui::types::SessionContext::Chat
        );
    }

    #[test]
    fn claw_session_is_claw() {
        let s = make_session("claw", "remote");
        assert!(matches!(
            session_context(Some(&s)),
            crate::ui::types::SessionContext::Claw { .. }
        ));
    }

    #[test]
    fn project_session_is_project() {
        let s = make_session("project", "ui refactor");
        assert!(matches!(
            session_context(Some(&s)),
            crate::ui::types::SessionContext::Project { .. }
        ));
    }

    #[test]
    fn chat_has_three_buttons() {
        let buttons = bot_bar_buttons(&crate::ui::types::SessionContext::Chat);
        assert_eq!(buttons.len(), 3);
    }
}
