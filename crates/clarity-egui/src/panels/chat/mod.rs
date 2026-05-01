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
    egui::CentralPanel::default()
        .frame(egui::Frame::central_panel(&ctx.style()).fill(app.ui_store.theme.bg))
        .show(ctx, |ui| {
            ui.set_max_width(720.0);
            render_header(app, ui);
            render_message_list(app, ui);
            render_plan(app, ui);
            render_input(app, ui);
        });
}
