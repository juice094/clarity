use crate::App;

pub mod input;
pub mod message_list;

pub use self::input::render_input;
pub use self::message_list::render_message_list;

/// Returns true when the central stage should show the centered empty-state
/// composer instead of the bottom input bar + message list.
pub(crate) fn is_empty_state(app: &crate::App) -> bool {
    let active = app
        .context
        .session_store
        .sessions
        .iter()
        .find(|s| s.id == app.context.session_store.active_session_id);
    active.is_none_or(|s| {
        s.messages.is_empty() && app.view_state.turn != clarity_core::ui::TurnState::Loading
    })
}

/// Render the bottom input/composer panel.
///
/// The composer is declared as a `TopBottomPanel::bottom` so it takes its
/// natural height and leaves the remaining area to the scrollable conversation
/// in `ChatApp::render`. The composer is always rendered so that its top edge
/// lines up with the bottom boundary of the chat area, both in empty and active
/// sessions.
pub fn render_input_panel(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();
    let kimi_style = app.context.ui_store.kimi_conversation_style;
    let ui_tx = app.context.ui_tx.clone();

    // ponytail: P5 — do not nest another Panel::bottom inside chrome_input.
    // The chrome already allocates the bottom strip; we only need a frame +
    // contents so the input hugs the status bar without fighting over space.
    let input_frame = clarity_ui::design_system::Elevation::Base
        .frame(&theme)
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .inner_margin(egui::Margin::symmetric(
            theme.space_24 as i8,
            theme.space_8 as i8,
        ))
        .outer_margin(egui::Margin::ZERO);
    input_frame.show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            let full_w = ui.available_width();
            let max_w = app
                .context
                .ui_store
                .content_max_width
                .min(full_w - 2.0 * theme.space_24)
                .max(120.0);
            ui.set_max_width(max_w);

            if kimi_style {
                if let Some(req) = app.context.ui_store.pending_approvals.first() {
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
                    let (denied, allowed) = crate::components::chat::conversation::approval_dock(
                        ui,
                        &app.context.ui_store.theme,
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
