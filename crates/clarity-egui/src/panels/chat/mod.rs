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
    let max_w = app.ui_store.content_max_width;
    egui::CentralPanel::default()
        .frame(
            egui::Frame::central_panel(&ctx.style())
                .fill(app.ui_store.theme.bg)
                .inner_margin(egui::Margin::symmetric(app.ui_store.theme.space_20 as i8, app.ui_store.theme.space_16 as i8)),
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
                    render_header(app, ui);

                    // File preview (rendered in main chat area when selected from workspace panel)
                    if let Some((ref name, ref content)) = app.ui_store.preview_file {
                        let preview_name = name.clone();
                        let preview_content = content.clone();
                        let theme = &app.ui_store.theme;
                        ui.add_space(theme.space_12);
                        egui::Frame::group(ui.style())
                            .fill(theme.surface)
                            .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
                            .stroke(egui::Stroke::new(1.0, theme.border))
                            .inner_margin(egui::Margin::symmetric(16, 12))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(crate::theme::ICON_PAPERCLIP)
                                            .font(theme.font_icon(theme.text_sm)),
                                    );
                                    ui.label(
                                        egui::RichText::new(&preview_name)
                                            .size(theme.text_sm)
                                            .color(theme.text)
                                            .monospace(),
                                    );
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new(egui::RichText::new(crate::theme::ICON_X).font(theme.font_icon(theme.text_xs))).small()).clicked() {
                                            app.ui_store.preview_file = None;
                                        }
                                    });
                                });
                                ui.add_space(theme.space_8);
                                let mut text = if preview_content.chars().count() > 4000 {
                                    let truncated: String = preview_content.chars().take(4000).collect();
                                    format!("{}…\n\n[Preview truncated: {} total characters]", truncated, preview_content.len())
                                } else {
                                    preview_content
                                };
                                egui::ScrollArea::vertical()
                                    .id_salt("preview_scroll_main")
                                    .max_height(400.0)
                                    .show(ui, |ui| {
                                        ui.add_sized(
                                            egui::vec2(ui.available_width(), 400.0),
                                            egui::TextEdit::multiline(&mut text)
                                                .desired_rows(20)
                                                .font(egui::TextStyle::Monospace)
                                                .text_color(theme.text_dim)
                                                .margin(egui::vec2(8.0, 6.0)),
                                        );
                                    });
                            });
                        ui.add_space(theme.space_12);
                        ui.separator();
                        ui.add_space(theme.space_8);
                    }

                    render_message_list(app, ui);
                    render_plan(app, ui);
                    render_input(app, ui);
                },
            );
        });
}
