use crate::App;

/// Renders the approval modal UI.
pub fn render_approval_modal(app: &mut App, ctx: &egui::Context) {
    // Refresh pending approvals each frame from the shared runtime.
    app.ui_store.pending_approvals = app.state.mode_aware_approval_runtime.inner().list_pending();

    // Kimi style renders approval as a dock inside the input panel; skip modal.
    if app.ui_store.kimi_conversation_style {
        return;
    }

    let request = match app.ui_store.pending_approvals.first() {
        Some(r) => r.clone(),
        None => return,
    };

    let screen = ctx.screen_rect();

    // Full-screen click blocker: consumes all mouse events so they don't
    // pass through to the main UI underneath the modal.
    let blocker_id = egui::Id::new("approval_blocker");
    egui::Area::new(blocker_id)
        .order(egui::Order::Background)
        .interactable(true)
        .show(ctx, |ui| {
            let response = ui.allocate_response(screen.size(), egui::Sense::click());
            // Darken the background
            ui.painter_at(response.rect).rect_filled(
                response.rect,
                0.0,
                app.ui_store.theme.overlay,
            );
        });

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
        let _ = app
            .ui_tx
            .send(crate::UiEvent::ResolveApproval { req_id, response });
        return;
    }

    egui::Window::new("🔒 Tool Approval Required")
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::group(&ctx.style())
                .fill(app.ui_store.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8))
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::same(20)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.set_max_width(600.0);

            ui.heading(egui::RichText::new("Tool Call Approval").color(app.ui_store.theme.text));
            ui.add_space(app.ui_store.theme.space_12);

            // Tool name
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Tool:")
                        .strong()
                        .color(app.ui_store.theme.text),
                );
                ui.label(
                    egui::RichText::new(&request.tool_call.function.name)
                        .color(app.ui_store.theme.accent),
                );
            });

            ui.add_space(app.ui_store.theme.space_8);

            // Arguments (monospace JSON block)
            ui.label(
                egui::RichText::new("Arguments:")
                    .strong()
                    .color(app.ui_store.theme.text),
            );
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
            egui::Frame::new()
                .fill(app.ui_store.theme.code_block_bg)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_max_width(560.0);
                    ui.monospace(
                        egui::RichText::new(display_args)
                            .color(app.ui_store.theme.text)
                            .size(app.ui_store.theme.text_sm),
                    );
                });

            // Diff preview using the unified diff viewer widget.
            if let Some(ref patch) = request.diff_preview {
                ui.add_space(app.ui_store.theme.space_8);
                ui.label(
                    egui::RichText::new("Preview:")
                        .strong()
                        .color(app.ui_store.theme.text),
                );
                let hunks = clarity_core::diff::parse_unified_diff(patch);
                let cfg = crate::widgets::diff_viewer::approval_diff_config();
                let _diff_resp = crate::widgets::diff_viewer::render_diff_view(
                    ui,
                    &hunks,
                    &app.ui_store.theme,
                    &cfg,
                );
            }

            // Risk / sensitivity description
            if let Some(ref desc) = request.description {
                ui.add_space(app.ui_store.theme.space_8);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(crate::theme::ICON_WARNING)
                            .font(app.ui_store.theme.font_icon(14.0)),
                    );
                    ui.label(
                        egui::RichText::new(desc)
                            .color(app.ui_store.theme.danger)
                            .size(app.ui_store.theme.text_sm),
                    );
                });
            }

            ui.add_space(app.ui_store.theme.space_16);

            // Action buttons
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Reject
                    if ui
                        .button(
                            egui::RichText::new(format!("{} Reject (Esc)", crate::theme::ICON_X))
                                .font(app.ui_store.theme.font_icon(app.ui_store.theme.text_sm))
                                .color(app.ui_store.theme.danger),
                        )
                        .clicked()
                    {
                        let req_id = request.id.clone();
                        let _ = app.ui_tx.send(crate::UiEvent::ResolveApproval {
                            req_id,
                            response: clarity_core::approval::ApprovalResponse::Reject,
                        });
                    }

                    // Approve for Session
                    if ui
                        .button(
                            egui::RichText::new(format!(
                                "{} Approve for Session (Shift+Enter)",
                                crate::theme::ICON_CHECK
                            ))
                            .font(app.ui_store.theme.font_icon(app.ui_store.theme.text_sm))
                            .color(app.ui_store.theme.accent),
                        )
                        .clicked()
                    {
                        let req_id = request.id.clone();
                        let _ = app.ui_tx.send(crate::UiEvent::ResolveApproval {
                            req_id,
                            response: clarity_core::approval::ApprovalResponse::ApproveForSession,
                        });
                    }

                    // Approve (with optional file count)
                    let approve_label = if let Some(ref patch) = request.diff_preview {
                        let file_count = patch.lines().filter(|l| l.starts_with("--- ")).count();
                        if file_count > 0 {
                            format!(
                                "{} Approve ({} file{}) (Enter)",
                                crate::theme::ICON_CHECK,
                                file_count,
                                if file_count > 1 { "s" } else { "" },
                            )
                        } else {
                            format!("{} Approve (Enter)", crate::theme::ICON_CHECK)
                        }
                    } else {
                        format!("{} Approve (Enter)", crate::theme::ICON_CHECK)
                    };
                    if ui
                        .button(
                            egui::RichText::new(approve_label)
                                .font(app.ui_store.theme.font_icon(app.ui_store.theme.text_sm))
                                .color(app.ui_store.theme.ok),
                        )
                        .clicked()
                    {
                        let req_id = request.id.clone();
                        let _ = app.ui_tx.send(crate::UiEvent::ResolveApproval {
                            req_id,
                            response: clarity_core::approval::ApprovalResponse::Approve,
                        });
                    }
                });
            });
        });
}

// ── Panel trait implementation ──

pub struct ApprovalModal;

impl crate::design_system::Panel for ApprovalModal {
    fn title(&self) -> &str {
        "Approval"
    }
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        render_approval_modal(app, &ctx);
    }
}
