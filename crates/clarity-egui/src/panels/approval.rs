use crate::App;
use clarity_core::approval::ApprovalRuntime;

pub fn render_approval_modal(app: &mut App, ctx: &egui::Context) {
    // Refresh pending approvals each frame from the shared runtime.
    app.pending_approvals = app.state.approval_runtime.list_pending();

    let request = match app.pending_approvals.first() {
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
                app.theme.overlay,
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
        let rt = app.state.approval_runtime.clone();
        let _ = app
            .runtime
            .block_on(async move { rt.resolve(&req_id, response).await });
        return;
    }

    egui::Window::new("🔒 Tool Approval Required")
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::group(&ctx.style())
                .fill(app.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.theme.radius_md as u8))
                .inner_margin(egui::Margin::same(20)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.set_max_width(600.0);

            ui.heading(egui::RichText::new("Tool Call Approval").color(app.theme.text));
            ui.add_space(app.theme.space_12);

            // Tool name
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tool:").strong().color(app.theme.text));
                ui.label(
                    egui::RichText::new(&request.tool_call.function.name).color(app.theme.accent),
                );
            });

            ui.add_space(app.theme.space_8);

            // Arguments (monospace JSON block)
            ui.label(
                egui::RichText::new("Arguments:")
                    .strong()
                    .color(app.theme.text),
            );
            // Filter out internal underscore-prefixed keys (_risk_level, _sensitive_file_warning, etc.)
            let display_args = serde_json::from_str::<serde_json::Value>(&request.tool_call.function.arguments)
                .map(|mut v| {
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.retain(|k, _| !k.starts_with('_'));
                    }
                    v.to_string()
                })
                .unwrap_or_else(|_| request.tool_call.function.arguments.clone());
            egui::Frame::new()
                .fill(app.theme.bg_accent)
                .corner_radius(egui::CornerRadius::same(app.theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_max_width(560.0);
                    ui.monospace(
                        egui::RichText::new(display_args)
                            .color(app.theme.text)
                            .size(12.0),
                    );
                });

            // Diff preview (for file_edit etc.)
            if let Some(ref patch) = request.diff_preview {
                ui.add_space(app.theme.space_8);
                ui.label(
                    egui::RichText::new("Preview:")
                        .strong()
                        .color(app.theme.text),
                );
                egui::Frame::new()
                    .fill(app.theme.bg_accent)
                    .corner_radius(egui::CornerRadius::same(app.theme.radius_sm as u8))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_max_width(560.0);
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                let hunks = clarity_core::diff::parse_unified_diff(patch);
                                let lines = clarity_core::diff::flatten_hunks(&hunks);
                                for (tag, text) in lines {
                                    let color = match tag {
                                        "header" => app.theme.accent,
                                        "-" => app.theme.danger,
                                        "+" => app.theme.ok,
                                        _ => app.theme.text,
                                    };
                                    ui.monospace(egui::RichText::new(text).color(color).size(11.0));
                                }
                            });
                    });
            }

            // Risk / sensitivity description
            if let Some(ref desc) = request.description {
                ui.add_space(app.theme.space_8);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("⚠️ ").size(14.0));
                    ui.label(egui::RichText::new(desc).color(app.theme.danger).size(13.0));
                });
            }

            ui.add_space(app.theme.space_16);

            // Action buttons
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Reject
                    if ui
                        .button(egui::RichText::new("❌ Reject (Esc)").color(app.theme.danger))
                        .clicked()
                    {
                        let req_id = request.id.clone();
                        let rt = app.state.approval_runtime.clone();
                        let _ = app.runtime.block_on(async move {
                            rt.resolve(&req_id, clarity_core::approval::ApprovalResponse::Reject)
                                .await
                        });
                    }

                    // Approve for Session
                    if ui
                        .button(
                            egui::RichText::new("✅ Approve for Session (Shift+Enter)")
                                .color(app.theme.accent),
                        )
                        .clicked()
                    {
                        let req_id = request.id.clone();
                        let rt = app.state.approval_runtime.clone();
                        let _ = app.runtime.block_on(async move {
                            rt.resolve(
                                &req_id,
                                clarity_core::approval::ApprovalResponse::ApproveForSession,
                            )
                            .await
                        });
                    }

                    // Approve
                    if ui
                        .button(egui::RichText::new("✅ Approve (Enter)").color(app.theme.ok))
                        .clicked()
                    {
                        let req_id = request.id.clone();
                        let rt = app.state.approval_runtime.clone();
                        let _ = app.runtime.block_on(async move {
                            rt.resolve(&req_id, clarity_core::approval::ApprovalResponse::Approve)
                                .await
                        });
                    }
                });
            });
        });
}
