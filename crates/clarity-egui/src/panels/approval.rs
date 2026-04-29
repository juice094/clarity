use crate::App;
use clarity_core::approval::ApprovalRuntime;

pub fn render_approval_modal(app: &mut App, ctx: &egui::Context) {
    // Refresh pending approvals each frame from the shared runtime.
    app.pending_approvals = app.state.approval_runtime.list_pending();

    let request = match app.pending_approvals.first() {
        Some(r) => r.clone(),
        None => return,
    };

    // Dim the background to focus attention on the modal.
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("approval_dimmer"),
    ));
    painter.rect_filled(screen, 0.0, egui::Color32::from_black_alpha(120));

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
            ui.set_max_width(520.0);

            ui.heading(egui::RichText::new("Tool Call Approval").color(app.theme.text));
            ui.add_space(12.0);

            // Tool name
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tool:").strong().color(app.theme.text));
                ui.label(
                    egui::RichText::new(&request.tool_call.function.name).color(app.theme.accent),
                );
            });

            ui.add_space(8.0);

            // Arguments (monospace JSON block)
            ui.label(
                egui::RichText::new("Arguments:")
                    .strong()
                    .color(app.theme.text),
            );
            egui::Frame::new()
                .fill(app.theme.bg_accent)
                .corner_radius(egui::CornerRadius::same(app.theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_max_width(480.0);
                    ui.monospace(
                        egui::RichText::new(&request.tool_call.function.arguments)
                            .color(app.theme.text)
                            .size(12.0),
                    );
                });

            // Risk / sensitivity description
            if let Some(ref desc) = request.description {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("⚠️ ").size(14.0));
                    ui.label(egui::RichText::new(desc).color(app.theme.danger).size(13.0));
                });
            }

            ui.add_space(16.0);

            // Action buttons
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Reject
                    if ui
                        .button(egui::RichText::new("❌ Reject").color(app.theme.danger))
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
                            egui::RichText::new("✅ Approve for Session").color(app.theme.accent),
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
                        .button(egui::RichText::new("✅ Approve").color(app.theme.ok))
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
