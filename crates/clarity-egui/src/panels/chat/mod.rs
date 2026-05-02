use crate::App;

pub mod header;
pub mod input;
pub mod message_list;
pub mod plan;

pub use self::header::render_header;
pub use self::input::render_input;
pub use self::message_list::render_message_list;
pub use self::plan::render_plan;

pub fn render_chat_area(app: &mut App, ctx: &egui::Context) {
    let theme = &app.ui_store.theme;
    let max_w = app.ui_store.content_max_width;
    egui::CentralPanel::default()
        .frame(
            egui::Frame::central_panel(&ctx.style())
                .fill(theme.bg)
                .inner_margin(egui::Margin::symmetric(theme.space_20 as i8, theme.space_16 as i8)),
        )
        .show(ctx, |ui| {
            ui.set_max_width(ui.available_width().min(max_w));
            render_header(app, ui);
            render_message_list(app, ui);
            render_plan(app, ui);
            render_input(app, ui);
        });
}
