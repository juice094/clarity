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

/// Compute the active session context from explicit session metadata.
fn session_context(session: Option<&Session>) -> crate::ui::types::SessionContext {
    session
        .map(|s| s.context.clone())
        .unwrap_or(crate::ui::types::SessionContext::Chat)
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
        crate::ui::types::SessionContext::Work { has_workspace, .. } => {
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
                icon: crate::theme::ICON_SETTINGS,
                tooltip: "Settings",
                panel: RightRailPanel::ClawSettings,
                context: RightRailContext::Claw,
            },
            BotBarButton {
                icon: crate::theme::ICON_FOLDER_OPEN,
                tooltip: "Workspace",
                panel: RightRailPanel::ClawWorkspace,
                context: RightRailContext::Claw,
            },
            BotBarButton {
                icon: crate::theme::ICON_TERMINAL,
                tooltip: "Terminal",
                panel: RightRailPanel::ClawTerminal,
                context: RightRailContext::Claw,
            },
            BotBarButton {
                icon: crate::theme::ICON_GLOBE,
                tooltip: "WebBridge",
                panel: RightRailPanel::ClawWebBridge,
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

            // Token usage progress bar.
            let tu = &app.chat_store.token_usage;
            if tu.total_tokens > 0 {
                let ratio = (tu.total_tokens as f32) / (tu.context_limit as f32);
                let pct = (ratio * 100.0).min(100.0);
                let bar_color = if pct > 95.0 {
                    theme.danger
                } else if pct > 80.0 {
                    theme.warn
                } else {
                    theme.accent
                };
                let bar_w = ui.available_width();
                let bar_h = 3.0;
                let (rect, _resp) =
                    ui.allocate_exact_size(egui::vec2(bar_w, bar_h), egui::Sense::hover());
                // Background track.
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(1), theme.bg_hover);
                // Filled portion.
                let fill_w = bar_w * ratio.min(1.0);
                if fill_w > 0.0 {
                    let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, bar_h));
                    ui.painter()
                        .rect_filled(fill_rect, egui::CornerRadius::same(1), bar_color);
                }
                // Tooltip on hover.
                if rect.contains(ui.input(|i| i.pointer.hover_pos()).unwrap_or_default()) {
                    egui::show_tooltip_at_pointer(ui.ctx(), egui::Id::new("token_bar_tooltip"), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} / {} tokens ({:.1}%)",
                                format_num(tu.total_tokens),
                                format_num(tu.context_limit),
                                pct,
                            ))
                            .size(theme.text_xs)
                            .color(theme.text),
                        );
                    });
                }
            }
            });
        });
}

/// Format a u64 as a human-readable number (e.g. 128000 → "128K").
fn format_num(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{}K", n / 1_000)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::types::Session;
    use std::collections::HashMap;

    fn make_session(context: crate::ui::types::SessionContext, title: &str) -> Session {
        Session {
            id: "s-1".into(),
            title: title.into(),
            category: "chat".into(),
            project_id: None,
            context,
            lifecycle: crate::ui::types::SessionLifecycle::Temporary,
            archived: false,
            messages: vec![],
            updated_at: 0,
            turn_heights: vec![],
            provider_state: HashMap::new(),
            in_flight: false,
            diff_stats: None,
        }
    }

    #[test]
    fn plain_session_is_chat() {
        let s = make_session(crate::ui::types::SessionContext::Chat, "general");
        assert_eq!(
            session_context(Some(&s)),
            crate::ui::types::SessionContext::Chat
        );
    }

    #[test]
    fn claw_session_is_claw() {
        let s = make_session(
            crate::ui::types::SessionContext::Claw {
                role: "operator".into(),
                session_key: "agent:main:main".into(),
                affinity: crate::ui::types::DeviceAffinity::Specific(String::new()),
            },
            "remote",
        );
        assert!(matches!(
            session_context(Some(&s)),
            crate::ui::types::SessionContext::Claw { .. }
        ));
    }

    #[test]
    fn project_session_is_work() {
        let s = make_session(
            crate::ui::types::SessionContext::Work {
                workspace_id: Some(String::new()),
                has_workspace: true,
            },
            "ui refactor",
        );
        assert!(matches!(
            session_context(Some(&s)),
            crate::ui::types::SessionContext::Work { .. }
        ));
    }

    #[test]
    fn chat_has_three_buttons() {
        let buttons = bot_bar_buttons(&crate::ui::types::SessionContext::Chat);
        assert_eq!(buttons.len(), 3);
    }
}
