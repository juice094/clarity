use crate::App;

pub mod header;
pub mod input;
pub mod message_list;

pub use self::header::render_header;
pub use self::input::render_input;
pub use self::message_list::render_message_list;

/// Render input bar fixed to bottom (TopBottomPanel).
/// Must be called BEFORE CentralPanel so egui reserves space correctly.
pub fn render_input_panel(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();
    let max_w = app.ui_store.content_max_width;
    let kimi_style = app.ui_store.kimi_conversation_style;

    // Extract approval data before entering the closure to avoid borrow conflicts.
    let approval_data: Option<(String, String, Option<String>, bool, String)> = if kimi_style {
        app.ui_store.pending_approvals.first().map(|req| {
            (
                req.id.clone(),
                req.tool_call.function.name.clone(),
                req.description.clone(),
                req.diff_preview.is_some(),
                req.tool_call.function.arguments.clone(),
            )
        })
    } else {
        None
    };
    let ui_tx = app.ui_tx.clone();

    egui::TopBottomPanel::bottom("input_panel")
        .max_height(200.0)
        .frame(
            egui::Frame::new()
                .fill(theme.bg)
                .inner_margin(egui::Margin::same(0)),
        )
        .show(ctx, |ui| {
            with_centered_content(ui, max_w, |ui| {
                // Kimi-style approval dock rendered above the composer
                if let Some((id, title, desc, has_diff, args)) = approval_data {
                    let conv_req = crate::components::chat::conversation::ApprovalRequest {
                        id,
                        title: title.clone(),
                        detail: desc.unwrap_or_else(|| format!("{}({})", title, args)),
                        badge: if has_diff {
                            Some("Diff".to_string())
                        } else {
                            None
                        },
                    };
                    let (denied, allowed) =
                        crate::components::chat::conversation::approval_dock(ui, &theme, &conv_req);
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
                render_input(app, ui);
            });
        });
}

pub fn render_chat_area(app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(
            egui::Frame::central_panel(&ctx.style())
                .fill(app.ui_store.theme.bg)
                .inner_margin(egui::Margin::symmetric(
                    app.ui_store.theme.space_20 as i8,
                    app.ui_store.theme.space_16 as i8,
                )),
        )
        .show(ctx, |ui| {
            // Hard minimum to prevent layout collapse when side panels are wide.
            ui.set_min_size(egui::vec2(360.0, 200.0));
            // Header uses the full CentralPanel width (not constrained by content_max_width).
            render_header(app, ui);

            with_centered_content(ui, app.ui_store.content_max_width, |ui| {
                render_message_list(app, ui);
            });
        });
}

/// Constrain content to `max_w` centered inside the available area.
/// Eliminates duplicated side-pad math across chat panels.
fn with_centered_content(ui: &mut egui::Ui, max_w: f32, add_contents: impl FnOnce(&mut egui::Ui)) {
    let available = ui.available_width();
    let content_w = available.min(max_w);
    let side_pad = ((available - content_w) / 2.0).max(0.0);
    let rect = ui.available_rect_before_wrap();
    let centered_rect = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + side_pad, rect.min.y),
        egui::pos2(rect.min.x + side_pad + content_w, rect.max.y),
    );
    ui.allocate_new_ui(
        egui::UiBuilder::new()
            .max_rect(centered_rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
        add_contents,
    );
}
