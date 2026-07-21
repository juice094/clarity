//! Chat renderer — egui host implementation for `clarity_apps::ChatApp`.
//!
//! P1c: `ChatApp` and `ChatStore` now live in `clarity-apps`. This file only
//! implements [`clarity_shell::ChatRenderer`] for the egui `App` and keeps the
//! chat-specific panel helpers (bot bar, message list, empty stage) here until
//! they can migrate in a later phase.
//!
//! ponytail: the concrete `App` downcast and internal panel helpers are a
//! temporary seam. Phase 2 will move the panel helpers behind a trait or into
//! `clarity-apps`, removing the `App` dependency.

use crate::App;
use crate::design_system;
use clarity_apps::ChatApp;
use clarity_shell::{AppState, ChatRenderer, ClarityAppResponse};
use std::any::Any;

impl ChatRenderer for App {
    fn render_chat(
        &mut self,
        chat: &mut dyn Any,
        ui: &mut egui::Ui,
        _egui_ctx: &egui::Context,
    ) -> ClarityAppResponse {
        let Some(chat) = chat.downcast_mut::<ChatApp>() else {
            tracing::error!("ChatRenderer called with non-ChatApp");
            return ClarityAppResponse::None;
        };

        // Pre-fetch cross-app shared data before borrowing the concrete App for
        // panel helpers.
        let theme = self.theme().clone();
        let provider = self.active_provider().to_string();
        let model = self.active_model().to_string();

        if chat.store.find_open {
            self.update_find_matches();
        }

        let app = self;

        // ponytail: P1d — `render_main_stage` already runs this renderer inside the
        // central strip's middle cell. Do NOT wrap the body in another
        // `CentralPanel`; doing so re-roots layout at the window level and can
        // overwrite or collapse the input panel and side rails.
        let mut empty_response = ClarityAppResponse::None;

        if crate::ui::debug_overlay::is_enabled(ui.ctx()) {
            crate::ui::debug_overlay::show_layout_state(ui, "chat-area");
        }
        // Hard minimum to prevent layout collapse when side panels are wide.
        ui.set_min_size(egui::vec2(360.0, 200.0));

        // S6-D: Bot bar spans the full chat column; right-rail buttons are
        // pushed to the far right. Only the message list is constrained to
        // content_max_width.
        crate::panels::bot_bar::render_bot_bar(app, ui);

        // ── Context ribbon bar ──
        if !chat.store.context_items.is_empty() || !chat.store.attachments.is_empty() {
            let theme_ctx = theme.clone();
            design_system::surface_panel(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}:",
                            crate::theme::ICON_LAYERS,
                            app.t("Context")
                        ))
                        .size(theme_ctx.text_xs)
                        .color(theme_ctx.text_dim),
                    );
                    for item in &chat.store.context_items {
                        design_system::chip(ui, &item.display, None, false);
                    }
                    for att in &chat.store.attachments {
                        design_system::chip(ui, &att.name, Some(crate::theme::ICON_FILE), false);
                    }
                });
            });
            ui.add_space(theme.space_4);
        }

        if !crate::panels::chat::is_empty_state(app) {
            // Find-in-session bar (Ctrl+F).
            if chat.store.find_open {
                app.render_find_bar(ui);
                ui.add_space(theme.space_4);
            }

            // The input bar is rendered by `render_input_panel` in a separate
            // bottom strip. The remaining central area is used for the message
            // list.
            //
            // Layout rules from the design discussion:
            //   - Short conversation: top-aligned, horizontally centered, no
            //     scrollable empty space below.
            //   - Long conversation: full-height ScrollArea with the scrollbar on
            //     the right divider, anchored to the bottom (latest messages next
            //     to the input box).
            let full_w = ui.available_width();
            let max_w = app
                .context
                .ui_store
                .content_max_width
                .min(full_w - 2.0 * theme.space_24)
                .max(120.0);
            let remaining_h = ui.available_height();
            let content_h =
                crate::panels::chat::message_list::estimate_total_height(app, max_w, &theme);
            let max_scroll = (content_h - remaining_h).max(0.0);
            if chat.store.stick_to_bottom {
                app.context.ui_store.last_scroll_offset = max_scroll;
            }

            if content_h <= remaining_h {
                // Short conversation: centered column, no scroll.
                render_message_list_centered(app, ui, max_w, &theme);
            } else {
                // Long conversation: ScrollArea owns the viewport; the virtual
                // list uses last_scroll_offset for culling. Sync the offset back
                // from egui each frame so manual wheel / drag updates stick.
                let scroll_id = ui.id().with("chat_scroll");
                let prev_offset = app.context.ui_store.last_scroll_offset;
                let output = egui::ScrollArea::vertical()
                    .id_salt(scroll_id)
                    .auto_shrink([false; 2])
                    .vertical_scroll_offset(prev_offset)
                    .show(ui, |ui| {
                        render_message_list_centered(app, ui, max_w, &theme);
                    });
                let new_offset = output.state.offset.y;
                // If the user manually scrolled up while stick-to-bottom was on,
                // release the lock. Using offset delta instead of raw wheel events
                // is more robust across touchpads, trackpoints and high-res wheels.
                if chat.store.stick_to_bottom && (prev_offset - new_offset) > 1.0 {
                    chat.store.stick_to_bottom = false;
                }
                app.context.ui_store.last_scroll_offset = new_offset;

                // Scroll-to-bottom affordance when the user has scrolled up.
                if !chat.store.stick_to_bottom {
                    let button_size = theme.button_height_md;
                    let margin = theme.space_16;
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(
                            ui.max_rect().right() - button_size - margin,
                            ui.max_rect().bottom() - button_size - margin,
                        ),
                        egui::vec2(button_size, button_size),
                    );
                    let button = egui::Button::new(
                        egui::RichText::new(crate::theme::ICON_CARET_DOWN)
                            .size(theme.text_base)
                            .color(theme.text),
                    )
                    .fill(theme.surface)
                    .corner_radius(egui::CornerRadius::same(theme.radius_full as u8))
                    .stroke(egui::Stroke::NONE);
                    let resp = ui.put(rect, button);
                    if resp.on_hover_text("Scroll to bottom").clicked() {
                        chat.store.stick_to_bottom = true;
                    }
                }
            }
        } else {
            empty_response = render_empty_stage(chat, app, ui, &provider, &model);
        }

        empty_response
    }
}

/// Render the message list inside a content column of width `max_w` that is
/// horizontally centered within the parent area.
fn render_message_list_centered(
    app: &mut App,
    ui: &mut egui::Ui,
    max_w: f32,
    theme: &crate::theme::Theme,
) {
    let active_id = app.context.session_store.active_session_id.clone();
    ui.push_id(&active_id, |ui| {
        // P6h: center the content column horizontally while keeping it
        // top-aligned. top_down(Align::Center) places children in a centered
        // column, and set_max_width caps the column so message bubbles do not
        // stretch to the full main-stage width.
        let full_w = ui.available_width();
        let content_w = max_w.min(full_w);
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.set_max_width(content_w);
            if crate::ui::debug_overlay::is_enabled(ui.ctx()) {
                crate::ui::debug_overlay::show_layout_state(ui, "chat-content");
            }
            crate::panels::chat::render_message_list(app, ui, theme);
        });
    });
}

/// Centered empty state: logo, prompt suggestions, and model info.
///
/// The composer is rendered by `render_input_panel` at the bottom of the
/// window so its top edge always aligns with the bottom of the chat area.
#[cfg(test)]
mod tests {
    use super::*;
    use clarity_shell::ChatRenderer;

    #[test]
    fn chat_app_renders_empty_state_without_panic() {
        let egui_ctx = egui::Context::default();
        let mut app = crate::apps::test_app(&egui_ctx);
        let mut chat = ChatApp::new();

        let _output = egui_ctx.run_ui(egui::RawInput::default(), |egui_ctx| {
            egui::Area::new("chat_test".into()).show(egui_ctx, |ui| {
                let response = app.render_chat(&mut chat as &mut dyn std::any::Any, ui, egui_ctx);
                assert_eq!(response, ClarityAppResponse::None);
            });
        });
    }

    /// Regression: when the active session has messages, `render_chat` must
    /// route to the message list and actually render bubbles. We verify this
    /// by checking that `Message::cached_height` gets populated.
    #[test]
    fn chat_app_renders_message_list_for_active_session() {
        let egui_ctx = egui::Context::default();
        let mut app = crate::apps::test_app(&egui_ctx);
        let mut chat = ChatApp::new();

        let session_id = {
            let mut session =
                crate::session::new_session(0, crate::ui::types::SessionContext::Chat);
            session.id = "test-session".to_string();
            let mut msg = crate::ui::types::Message {
                role: crate::ui::types::Role::User,
                content: "Hello, world!".to_string(),
                blocks: vec![crate::ui::types::ContentBlock::Text {
                    text: "Hello, world!".to_string(),
                }],
                timestamp: std::time::Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: false,
                lines: vec![],
            };
            msg.prepare();
            session.messages.push(msg);
            let id = session.id.clone();
            app.context.session_store.sessions.push(session);
            id
        };
        app.context.session_store.active_session_id = session_id;

        let _output = egui_ctx.run_ui(egui::RawInput::default(), |egui_ctx| {
            egui::Area::new("chat_test".into()).show(egui_ctx, |ui| {
                let _response = app.render_chat(&mut chat as &mut dyn std::any::Any, ui, egui_ctx);
            });
        });

        let session = app
            .context
            .session_store
            .sessions
            .iter()
            .find(|s| s.id == "test-session")
            .expect("session exists");
        assert!(
            session.messages[0].cached_height.is_some(),
            "message list should have been rendered and cached_height set"
        );
    }
}

fn render_empty_stage(
    chat: &mut ChatApp,
    app: &mut App,
    ui: &mut egui::Ui,
    provider: &str,
    model: &str,
) -> ClarityAppResponse {
    let mut app_response = ClarityAppResponse::None;
    let theme = app.context.ui_store.theme.clone();
    let full_w = ui.available_width();
    let content_w = app
        .context
        .ui_store
        .content_max_width
        .min(full_w - 2.0 * theme.space_24)
        .clamp(360.0, 720.0);

    ui.vertical_centered(|ui| {
        ui.set_max_width(content_w);
        let full_h = ui.available_height();
        // Push the greeting slightly above true centre so the composer below
        // feels anchored rather than crowding the text.
        let top_space = ((full_h * 0.38) - 120.0).max(24.0);
        ui.add_space(top_space);

        // ── Greeting ──
        // Kimi-style empty state: minimal product wordmark + a single
        // human-readable prompt instead of a dense grid of chips.
        ui.label(
            egui::RichText::new(app.t("What can I help you with?"))
                .size(theme.text_2xl)
                .strong()
                .color(theme.text_strong),
        );
        crate::design_system::gap(ui, crate::design_system::Space::S2);

        ui.label(
            egui::RichText::new(
                app.t("Type / for plugins, # for context, or pick a starter below."),
            )
            .size(theme.text_sm)
            .color(theme.text_dim),
        );
        crate::design_system::gap(ui, crate::design_system::Space::S5);

        // ── Starter suggestion cards ──
        // Rendered as a horizontal row of compact cards rather than wrapped
        // chips so the empty state feels intentional and scannable.
        let starters: &[(&str, &str, &str)] = &[
            (
                crate::theme::ICON_WRENCH,
                "Fix a bug",
                "Help me debug an issue in my code.",
            ),
            (
                crate::theme::ICON_PLUS,
                "New feature",
                "Implement a new feature based on the spec.",
            ),
            (
                crate::theme::ICON_FILE_CODE,
                "Refactor",
                "Refactor this code for clarity and performance.",
            ),
            (
                crate::theme::ICON_BOOK,
                "Explain code",
                "Explain how this code works in detail.",
            ),
        ];

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.space_12;
            let card_w = ((content_w - theme.space_12 * (starters.len() as f32 - 1.0))
                / starters.len() as f32)
                .max(120.0);
            for (icon, label, prompt) in starters {
                let resp = render_starter_card(ui, &theme, card_w, icon, app.t(label));
                if resp.clicked() {
                    chat.store.input = prompt.to_string();
                    app_response = ClarityAppResponse::FocusChatInput;
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        });

        crate::design_system::gap(ui, crate::design_system::Space::S6);

        // ── Status micro-line ──
        let configured = !provider.is_empty();
        ui.label(
            egui::RichText::new(format!(
                "{} · {}",
                if configured {
                    format!("{} {}", crate::theme::ICON_CHECK, provider)
                } else {
                    format!("{} {}", crate::theme::ICON_WARNING, app.t("no provider"))
                },
                if !model.is_empty() {
                    model.to_string()
                } else {
                    app.t("no model").to_string()
                },
            ))
            .size(theme.text_xs)
            .color(if configured {
                theme.text_dim
            } else {
                theme.warn
            }),
        );
    });

    app_response
}

/// A single starter suggestion card for the empty state.
fn render_starter_card(
    ui: &mut egui::Ui,
    theme: &crate::theme::Theme,
    width: f32,
    icon: &str,
    label: &str,
) -> egui::Response {
    let desired = egui::vec2(width, 72.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if response.hovered() {
            theme.surface_strong
        } else {
            theme.surface
        };
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(theme.radius_md as u8), bg);
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(theme.radius_md as u8),
            egui::Stroke::new(1.0, theme.border),
            egui::StrokeKind::Inside,
        );

        let icon_size = theme.text_lg;
        let icon_pos = rect.left_top() + egui::vec2(theme.space_16, theme.space_16);
        ui.painter().text(
            icon_pos,
            egui::Align2::LEFT_TOP,
            icon,
            theme.font_icon(icon_size),
            theme.accent,
        );

        let label_pos =
            rect.left_bottom() - egui::vec2(-theme.space_16, theme.space_16 + theme.text_sm * 0.5);
        ui.painter().text(
            label_pos,
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(theme.text_sm),
            theme.text,
        );
    }

    response
}
