use crate::App;

pub mod header;
pub mod input;
pub mod message_list;
pub mod plan;

pub use self::header::render_header;
pub use self::input::render_input;
pub use self::message_list::render_message_list;
pub use self::plan::render_plan;

/// Render input bar fixed to bottom (TopBottomPanel).
/// Must be called BEFORE CentralPanel so egui reserves space correctly.
pub fn render_input_panel(app: &mut App, ctx: &egui::Context) {
    let theme = &app.ui_store.theme;
    let max_w = app.ui_store.content_max_width;
    egui::TopBottomPanel::bottom("input_panel")
        .frame(
            egui::Frame::new()
                .fill(theme.bg)
                .inner_margin(egui::Margin::symmetric(
                    theme.space_20 as i8,
                    theme.space_12 as i8,
                )),
        )
        .show(ctx, |ui| {
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
                |ui| {
                    render_input(app, ui);
                },
            );
        });
}

pub fn render_chat_area(app: &mut App, ctx: &egui::Context) {
    let max_w = app.ui_store.content_max_width;
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
            // Header uses the full CentralPanel width (not constrained by content_max_width).
            // This gives the tab bar maximum breathing room; overflow is handled by
            // ScrollArea::horizontal inside render_header.
            render_header(app, ui);

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
                |ui| {
                    // Scrollable content area (messages + plan)
                    egui::ScrollArea::vertical()
                        .scroll_bar_visibility(
                            egui::containers::scroll_area::ScrollBarVisibility::AlwaysHidden,
                        )
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            render_message_list(app, ui);
                            render_plan(app, ui);
                        });
                },
            );
        });
}
