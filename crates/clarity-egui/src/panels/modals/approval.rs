use crate::App;
use clarity_ui::design_system::{Space, TextStyle, code_frame, gap, text};
use clarity_ui::theme::ICON_WARNING;
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::modal::Modal;

/// Renders the approval modal UI using the Clarity Design Protocol.
///
/// The modal shell itself (scrim + frame + centering) is owned by
/// `clarity_ui::widgets::modal`; this function only renders the content.
pub fn render_approval_modal(app: &mut App, ctx: &egui::Context) {
    // Refresh pending approvals each frame from the shared runtime.
    app.context.ui_store.pending_approvals = app
        .context
        .state
        .mode_aware_approval_runtime
        .inner()
        .list_pending();

    // Kimi style renders approval as a dock inside the input panel; skip modal.
    if app.context.ui_store.kimi_conversation_style {
        return;
    }

    let request = match app.context.ui_store.pending_approvals.first() {
        Some(r) => r.clone(),
        None => return,
    };

    let ui_tx = app.context.ui_tx.clone();
    let theme = &app.context.ui_store.theme;

    // Keyboard shortcuts (checked every frame while modal is open).
    let mut keyboard_approval: Option<clarity_core::approval::ApprovalResponse> = None;
    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        // Shift+Enter = Approve for Session, plain Enter = Approve
        if ctx.input(|i| i.modifiers.shift) {
            keyboard_approval = Some(clarity_core::approval::ApprovalResponse::ApproveForSession);
        } else {
            keyboard_approval = Some(clarity_core::approval::ApprovalResponse::Approve);
        }
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        keyboard_approval = Some(clarity_core::approval::ApprovalResponse::Reject);
    }
    if let Some(response) = keyboard_approval {
        let req_id = request.id.clone();
        let _ = ui_tx.send(crate::UiEvent::ResolveApproval { req_id, response });
        return;
    }

    // Filter out internal underscore-prefixed keys (_risk_level, _sensitive_file_warning, etc.)
    let display_args =
        serde_json::from_str::<serde_json::Value>(&request.tool_call.function.arguments)
            .map(|mut v| {
                if let serde_json::Value::Object(ref mut map) = v {
                    map.retain(|k, _| !k.starts_with('_'));
                }
                v.to_string()
            })
            .unwrap_or_else(|_| request.tool_call.function.arguments.clone());

    Modal::new("approval")
        .width(420.0)
        .max_height(600.0)
        .show(ctx, |ui| {
            text(ui, "Tool Call Approval", TextStyle::Title);
            gap(ui, Space::S2);

            // Tool name
            ui.horizontal(|ui| {
                text(ui, "Tool:", TextStyle::CaptionStrong);
                text(ui, &request.tool_call.function.name, TextStyle::Accent);
            });
            gap(ui, Space::S1);

            // Arguments (monospace JSON block)
            text(ui, "Arguments:", TextStyle::CaptionStrong);
            code_frame(ui, |ui| {
                text(ui, display_args, TextStyle::Mono);
            });

            // Diff preview using the unified diff viewer widget.
            if let Some(ref patch) = request.diff_preview {
                gap(ui, Space::S1);
                text(ui, "Preview:", TextStyle::CaptionStrong);
                let hunks = clarity_core::diff::parse_unified_diff(patch);
                let cfg = crate::widgets::diff_viewer::approval_diff_config();
                let _diff_resp =
                    crate::widgets::diff_viewer::render_diff_view(ui, &hunks, theme, &cfg);
            }

            // Risk / sensitivity description
            if let Some(ref desc) = request.description {
                gap(ui, Space::S1);
                ui.horizontal(|ui| {
                    // ponytail: icon+text Button not available yet; keep icon as raw label.
                    ui.label(
                        egui::RichText::new(ICON_WARNING).font(theme.font_icon(theme.text_sm)),
                    );
                    ui.label(
                        egui::RichText::new(desc)
                            .color(theme.danger)
                            .size(theme.text_sm),
                    );
                });
            }

            gap(ui, Space::S2);

            // Action buttons
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = theme.space_8;

                let approve_label = if let Some(ref patch) = request.diff_preview {
                    let file_count = patch.lines().filter(|l| l.starts_with("--- ")).count();
                    if file_count > 0 {
                        format!(
                            "Approve ({} file{}) (Enter)",
                            file_count,
                            if file_count > 1 { "s" } else { "" },
                        )
                    } else {
                        "Approve (Enter)".to_string()
                    }
                } else {
                    "Approve (Enter)".to_string()
                };

                if ui
                    .add(Button::new(&approve_label).primary().small().width(100.0))
                    .clicked()
                {
                    let req_id = request.id.clone();
                    let _ = ui_tx.send(crate::UiEvent::ResolveApproval {
                        req_id,
                        response: clarity_core::approval::ApprovalResponse::Approve,
                    });
                }

                if ui
                    .add(
                        Button::new("Approve for Session (Shift+Enter)")
                            .ghost()
                            .small()
                            .width(180.0),
                    )
                    .clicked()
                {
                    let req_id = request.id.clone();
                    let _ = ui_tx.send(crate::UiEvent::ResolveApproval {
                        req_id,
                        response: clarity_core::approval::ApprovalResponse::ApproveForSession,
                    });
                }

                if ui
                    .add(Button::new("Reject (Esc)").danger().small().width(80.0))
                    .clicked()
                {
                    let req_id = request.id.clone();
                    let _ = ui_tx.send(crate::UiEvent::ResolveApproval {
                        req_id,
                        response: clarity_core::approval::ApprovalResponse::Reject,
                    });
                }
            });
        });
}
