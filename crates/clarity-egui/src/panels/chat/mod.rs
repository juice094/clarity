use crate::App;
use crate::ui::types::ToastLevel;

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
                .inner_margin(egui::Margin::symmetric(theme.space_20 as i8, theme.space_12 as i8)),
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
                .inner_margin(egui::Margin::symmetric(app.ui_store.theme.space_20 as i8, app.ui_store.theme.space_16 as i8)),
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
                    // Scrollable content area (messages + preview + plan)
                    egui::ScrollArea::vertical()
                        .scroll_bar_visibility(egui::containers::scroll_area::ScrollBarVisibility::AlwaysHidden)
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            // File preview (rendered in main chat area when selected from workspace panel)
                            if app.ui_store.preview_item.is_some() {
                                let (preview_name, preview_content, preview_url, file_path) =
                                    match app.ui_store.preview_item.as_ref().unwrap() {
                                        crate::ui::types::PreviewItem::File { name, content, path } => {
                                            (name.clone(), content.clone(), None, Some(path.clone()))
                                        }
                                        crate::ui::types::PreviewItem::WebPage { title, url, content } => {
                                            (title.clone(), content.clone(), Some(url.clone()), None)
                                        }
                                    };
                                let theme = app.ui_store.theme.clone();
                                ui.add_space(theme.space_12);
                                let edit_id = ui.id().with(&preview_name).with("preview_edit");
                                let mut text = ui.data(|d| {
                                    d.get_temp::<String>(edit_id)
                                        .unwrap_or(preview_content.clone())
                                });
                                let is_dirty = text != preview_content;
                                egui::Frame::group(ui.style())
                                    .fill(theme.surface)
                                    .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
                                    .stroke(egui::Stroke::NONE)
                                    .inner_margin(egui::Margin::symmetric(16, 12))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let icon = if preview_url.is_some() { "🌐" } else { crate::theme::ICON_PAPERCLIP };
                                            ui.label(
                                                egui::RichText::new(icon)
                                                    .font(theme.font_icon(theme.text_sm)),
                                            );
                                            ui.label(
                                                egui::RichText::new(&preview_name)
                                                    .size(theme.text_sm)
                                                    .color(theme.text)
                                                    .monospace(),
                                            );
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                // Close
                                                if ui.add(egui::Button::new(egui::RichText::new(crate::theme::ICON_X).font(theme.font_icon(theme.text_xs))).small()).clicked() {
                                                    app.ui_store.preview_item = None;
                                                }
                                                // Save (files only)
                                                if let Some(ref path) = file_path {
                                                    let save_btn = if is_dirty {
                                                        theme.primary_button("Save")
                                                    } else {
                                                        theme.ghost_button("Save")
                                                    };
                                                    if ui.add(save_btn).clicked() && is_dirty {
                                                        match std::fs::write(path, &text) {
                                                            Ok(()) => {
                                                                if let Some(crate::ui::types::PreviewItem::File { content, .. }) = app.ui_store.preview_item.as_mut() {
                                                                    *content = text.clone();
                                                                }
                                                                app.push_toast(format!("Saved: {}", preview_name), ToastLevel::Info);
                                                            }
                                                            Err(e) => {
                                                                app.push_toast(format!("Save failed: {}", e), ToastLevel::Error);
                                                            }
                                                        }
                                                    }
                                                }
                                            });
                                        });
                                        if let Some(ref url) = preview_url {
                                            ui.label(
                                                egui::RichText::new(url)
                                                    .size(theme.text_xs)
                                                    .color(theme.text_muted),
                                            );
                                        }
                                        ui.add_space(theme.space_8);
                                        egui::ScrollArea::vertical()
                                            .id_salt("preview_scroll_main")
                                            .max_height(400.0)
                                            .show(ui, |ui| {
                                                let te = egui::TextEdit::multiline(&mut text)
                                                    .desired_rows(20)
                                                    .font(egui::TextStyle::Monospace)
                                                    .text_color(theme.text_dim)
                                                    .margin(egui::vec2(8.0, 6.0));
                                                let resp = ui.add_sized(
                                                    egui::vec2(ui.available_width(), 400.0),
                                                    te,
                                                );
                                                if resp.changed() {
                                                    ui.data_mut(|d| d.insert_temp(edit_id, text.clone()));
                                                }
                                            });
                                    });
                                ui.add_space(theme.space_12);
                                ui.separator();
                                ui.add_space(theme.space_8);
                            }

                            render_message_list(app, ui);
                            render_plan(app, ui);
                        });
                },
            );
        });
}
