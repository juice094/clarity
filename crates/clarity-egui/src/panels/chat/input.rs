use crate::App;
use crate::components::input;

pub fn render_input(app: &mut App, ui: &mut egui::Ui) {
    // Attachment chips above input bar
    if let Some(i) = input::attachment_chips(ui, &app.chat_store.attachments, &app.ui_store.theme) {
        app.chat_store.attachments.remove(i);
        ui.separator();
    }

    // Input bar card — full width, safe margin, no overflow
    ui.add_space(4.0);
    egui::Frame::group(ui.style())
        .fill(app.ui_store.theme.input_bg)
        .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_lg as u8))
        .stroke(egui::Stroke::new(1.0, app.ui_store.theme.border))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let available_width = ui.available_width();
                let btn_area_width = if app.chat_store.is_loading { 100.0 } else { 52.0 };
                let input_width = (available_width - btn_area_width - 8.0).max(80.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(input_width, 44.0),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        let hint = if app.chat_store.pending_send.is_some() {
                            "Steer message queued — will send when current response stops..."
                        } else if !app.chat_store.attachments.is_empty() {
                            "Type a message (files attached)..."
                        } else {
                            "Type a message..."
                        };
                        let prev_input = app.chat_store.input.clone();
                        let line_count = app.chat_store.input.matches('\n').count() + 1;
                        let input_height =
                            (line_count as f32 * 20.0 + 24.0).clamp(44.0, 120.0);
                        let text_edit = egui::TextEdit::multiline(&mut app.chat_store.input)
                            .desired_rows(line_count.max(1))
                            .hint_text(hint)
                            .margin(egui::vec2(8.0, 8.0));
                        ui.add_sized(egui::vec2(input_width, input_height), text_edit);

                        if app.chat_store.input != prev_input {
                            app.ui_store.last_input_modified = std::time::Instant::now();
                        }

                        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let ime_commit = ui.input(|i| {
                            i.events.iter().any(|e| matches!(e, egui::Event::Ime(egui::ImeEvent::Commit(_))))
                        });
                        if enter_pressed && !ui.input(|i| i.modifiers.shift) && !ime_commit {
                            while app.chat_store.input.ends_with('\n') {
                                app.chat_store.input.pop();
                            }
                            if app.chat_store.input == prev_input
                                && !app.chat_store.input.trim().is_empty()
                            {
                                app.chat_store.stick_to_bottom = true;
                                app.send();
                            }
                        }
                    },
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if app.chat_store.is_loading {
                        let can_queue =
                            !app.chat_store.input.trim().is_empty() || !app.chat_store.attachments.is_empty();
                        if input::queue_button(ui, can_queue, &app.ui_store.theme).clicked() && can_queue {
                            app.chat_store.stick_to_bottom = true;
                            app.send();
                        }
                        if input::stop_button(ui, &app.ui_store.theme).clicked() {
                            app.stop();
                        }
                    } else {
                        if input::send_button(ui, &app.ui_store.theme).clicked() {
                            app.chat_store.stick_to_bottom = true;
                            app.send();
                        }
                    }
                });
            });
        });
}
