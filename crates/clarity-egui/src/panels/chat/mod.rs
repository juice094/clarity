use crate::App;

pub mod input;
pub mod message_list;

pub use self::input::render_input;
pub use self::message_list::render_message_list;

/// Returns true when the central stage should show the centered empty-state
/// composer instead of the bottom input bar + message list.
pub(crate) fn is_empty_state(app: &crate::App) -> bool {
    let active = app
        .session_store
        .sessions
        .iter()
        .find(|s| s.id == app.session_store.active_session_id);
    active.is_none_or(|s| {
        s.messages.is_empty() && app.view_state.turn != clarity_core::ui::TurnState::Loading
    })
}

/// Render the message list inside a content column of width `max_w` that is
/// horizontally centered within the parent area.
fn render_message_list_centered(app: &mut App, ui: &mut egui::Ui, max_w: f32) {
    let active_id = app.session_store.active_session_id.clone();
    ui.push_id(&active_id, |ui| {
        let inner_rect = ui.available_rect_before_wrap();
        let inner_w = inner_rect.width();
        let x_offset = (inner_w - max_w).max(0.0) / 2.0;
        let content_rect = egui::Rect::from_min_size(
            egui::pos2(inner_rect.min.x + x_offset, inner_rect.min.y),
            egui::vec2(max_w.min(inner_w), inner_rect.height()),
        );
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
            |ui| {
                if crate::ui::debug_overlay::is_enabled(ui.ctx()) {
                    crate::ui::debug_overlay::show_layout_state(ui, "chat-content");
                }
                render_message_list(app, ui);
            },
        );
    });
}

/// Render the bottom input/composer panel.
///
/// The composer is declared as a `TopBottomPanel::bottom` so it takes its
/// natural height and leaves the remaining area to the scrollable conversation
/// in `render_chat_area`. The composer is always rendered so that its top edge
/// lines up with the bottom boundary of the chat area, both in empty and active
/// sessions.
pub fn render_input_panel(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();
    let kimi_style = app.ui_store.kimi_conversation_style;
    let ui_tx = app.ui_tx.clone();

    egui::TopBottomPanel::bottom("chat_input_panel")
        .frame(
            egui::Frame::new()
                // The main-stage background already provides the surface; keep
                // this panel transparent so the rounded bottom corners show
                // through consistently.
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(
                    theme.space_24 as i8,
                    theme.space_16 as i8,
                ))
                .outer_margin(egui::Margin::ZERO),
        )
        .show_separator_line(false)
        .show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                let full_w = ui.available_width();
                let max_w = app
                    .ui_store
                    .content_max_width
                    .min(full_w - 2.0 * theme.space_24)
                    .max(120.0);
                ui.set_max_width(max_w);

                if kimi_style {
                    if let Some(req) = app.ui_store.pending_approvals.first() {
                        let conv_req = crate::components::chat::conversation::ApprovalRequest {
                            id: req.id.clone(),
                            title: req.tool_call.function.name.clone(),
                            detail: req.description.clone().unwrap_or_else(|| {
                                format!(
                                    "{}({})",
                                    req.tool_call.function.name, req.tool_call.function.arguments
                                )
                            }),
                            badge: if req.diff_preview.is_some() {
                                Some("Diff".to_string())
                            } else {
                                None
                            },
                        };
                        let (denied, allowed) =
                            crate::components::chat::conversation::approval_dock(
                                ui,
                                &app.ui_store.theme,
                                &conv_req,
                            );
                        if let Some(req_id) = denied {
                            let _ = ui_tx.send(crate::ui::types::UiEvent::ResolveApproval {
                                req_id,
                                response: clarity_core::approval::ApprovalResponse::Reject,
                            });
                        }
                        if let Some(req_id) = allowed {
                            let _ = ui_tx.send(crate::ui::types::UiEvent::ResolveApproval {
                                req_id,
                                response: clarity_core::approval::ApprovalResponse::Approve,
                            });
                        }
                    }
                }

                render_input(app, ui);
            });
        });
}

/// Renders the chat area UI.
pub fn render_chat_area(app: &mut App, ctx: &egui::Context) {
    // Ensure the central stage covers the full remaining area. If the available
    // width reported by egui is fractional, rounding can leave a 1 px gap next
    // to a side panel on HiDPI or Windows fractional scaling; drawing the
    // central panel with an explicit outer margin of zero overpaints it.
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                // Unified background is drawn by render_main_stage_border; keep
                // this panel transparent so the chat stage reads as one surface.
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::ZERO)
                .outer_margin(egui::Margin::ZERO),
        )
        .show(ctx, |ui| {
            if crate::ui::debug_overlay::is_enabled(ui.ctx()) {
                crate::ui::debug_overlay::show_layout_state(ui, "chat-area");
            }
            // Hard minimum to prevent layout collapse when side panels are wide.
            ui.set_min_size(egui::vec2(360.0, 200.0));

            // S6-D: Bot bar spans the full chat column; right-rail buttons are
            // pushed to the far right. Only the message list is constrained to
            // content_max_width.
            crate::panels::bot_bar::render_bot_bar(app, ui);

            if !is_empty_state(app) {
                // The input bar is rendered by `render_input_panel` in a separate
                // bottom panel. The remaining central area is used for the message
                // list.
                //
                // Layout rules from the design discussion:
                //   - Short conversation: top-aligned, horizontally centered, no
                //     scrollable empty space below.
                //   - Long conversation: full-height ScrollArea with the scrollbar on
                //     the right divider, anchored to the bottom (latest messages next
                //     to the input box).
                let theme = app.ui_store.theme.clone();
                let full_w = ui.available_width();
                let max_w = app
                    .ui_store
                    .content_max_width
                    .min(full_w - 2.0 * theme.space_24)
                    .max(120.0);
                let remaining_rect = ui.available_rect_before_wrap();
                let content_h = message_list::estimate_total_height(app, max_w);
                let long_conversation = content_h > 0.0 && content_h > remaining_rect.height();

                if long_conversation {
                    let max_scroll = (content_h - remaining_rect.height()).max(0.0);
                    if app.chat_store.stick_to_bottom {
                        app.ui_store.last_scroll_offset = max_scroll;
                    }
                    let active_id = app.session_store.active_session_id.clone();
                    let output = egui::ScrollArea::vertical()
                        .id_salt(format!("chat_scroll_{}", active_id))
                        .auto_shrink([false; 2])
                        .scroll_bar_visibility(
                            egui::containers::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                        )
                        .scroll_offset(egui::vec2(0.0, app.ui_store.last_scroll_offset))
                        .show(ui, |ui| {
                            render_message_list_centered(app, ui, max_w);
                        });
                    app.ui_store.last_scroll_offset = output.state.offset.y;
                } else {
                    // Short conversation: top-aligned inside a horizontally centered
                    // content column, no scrollable empty space below.
                    app.ui_store.last_scroll_offset = 0.0;
                    render_message_list_centered(app, ui, max_w);
                }
            } else {
                render_empty_stage(app, ui);
            }
        });
}

/// Centered empty state: logo, prompt suggestions, and model info.
///
/// The composer is now rendered by `render_input_panel` at the bottom of the
/// window so its top edge always aligns with the bottom of the chat area.
fn render_empty_stage(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let full_w = ui.available_width();
    let content_w = app
        .ui_store
        .content_max_width
        .min(full_w - 2.0 * theme.space_24)
        .clamp(360.0, 600.0);

    ui.vertical_centered(|ui| {
        ui.set_max_width(content_w);
        let full_h = ui.available_height();
        // Center block vertically with slight upward shift.
        let block_h = 280.0;
        ui.add_space(((full_h - block_h) / 2.0 - 32.0).max(24.0));

        // ── Logo area ──
        ui.label(
            egui::RichText::new("Clarity")
                .size(42.0)
                .strong()
                .color(theme.text_strong),
        );
        ui.add_space(theme.space_8);

        ui.label(
            egui::RichText::new(app.t("Your personal AI coding assistant"))
                .size(theme.text_sm)
                .color(theme.text_muted),
        );
        ui.add_space(theme.space_24);

        // ── Quick-start suggestion chips ──
        let suggestions: &[(
            /* icon */ &str,
            /* label */ &str,
            /* prompt */ &str,
        )] = &[
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
                crate::theme::ICON_CHECK,
                "Code review",
                "Review my code for bugs and improvements.",
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
            (
                crate::theme::ICON_TERMINAL,
                "Run command",
                "Analyze the output of this terminal command.",
            ),
        ];

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(theme.space_8, theme.space_8);
            for (icon, label, prompt) in suggestions {
                let chip = egui::Frame::new()
                    .fill(theme.surface)
                    .stroke(egui::Stroke::new(1.0, theme.border))
                    .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
                    .inner_margin(egui::Margin::symmetric(
                        theme.space_12 as i8,
                        theme.space_8 as i8,
                    ))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(*icon)
                                    .font(theme.font_icon(theme.text_sm))
                                    .color(theme.accent),
                            );
                            ui.add_space(theme.space_8);
                            ui.label(
                                egui::RichText::new(app.t(label))
                                    .size(theme.text_sm)
                                    .color(theme.text),
                            );
                        });
                    });
                if chip.response.clicked() {
                    app.chat_store.input = prompt.to_string();
                    app.ui_store.focus_input_requested = true;
                }
                if chip.response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        });

        ui.add_space(theme.space_20);

        // ── Active provider / model info line ──
        let provider_name = app
            .settings_store
            .settings_edit
            .active_persona_id
            .clone()
            .unwrap_or_else(|| "local".to_string());
        ui.label(
            egui::RichText::new(format!(
                "{} {} · {} {}",
                crate::theme::ICON_CPU,
                app.t("Provider:"),
                provider_name,
                if app.state.llm_binding.lock().is_some() {
                    "\u{2713}"
                } else {
                    "\u{26A0}"
                },
            ))
            .size(theme.text_xs)
            .color(theme.text_dim),
        );

        // ── Keyboard shortcuts hint ──
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new(format!(
                "{} Ctrl+Shift+P  {} Ctrl+B",
                app.t("Commands"),
                app.t("Sidebar"),
            ))
            .size(theme.text_xs)
            .color(theme.text_muted),
        );
    });
}
