use crate::App;

pub fn render_input(app: &mut App, ui: &mut egui::Ui) {
    // Attachment chips above input bar
    if !app.chat_store.attachments.is_empty() {
        let mut to_remove: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("Attachments:")
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text_dim),
            );
            for (i, att) in app.chat_store.attachments.iter().enumerate() {
                egui::Frame::group(ui.style())
                    .fill(app.ui_store.theme.surface)
                    .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_full as u8))
                    .stroke(egui::Stroke::new(1.0, app.ui_store.theme.border))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::theme::ICON_PAPERCLIP).font(app.ui_store.theme.font_icon(app.ui_store.theme.text_sm)));
                            ui.label(
                                egui::RichText::new(&att.name)
                                    .size(app.ui_store.theme.text_sm)
                                    .color(app.ui_store.theme.text)
                                    .monospace(),
                            );
                            if ui.add(egui::Button::new(egui::RichText::new(crate::theme::ICON_X).font(app.ui_store.theme.font_icon(app.ui_store.theme.text_xs))).small()).clicked() {
                                to_remove = Some(i);
                            }
                        });
                    });
            }
        });
        if let Some(i) = to_remove {
            app.chat_store.attachments.remove(i);
        }
        ui.separator();
    }

    // Input bar card
    egui::Frame::group(ui.style())
        .fill(app.ui_store.theme.input_bg)
        .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_lg as u8))
        .stroke(egui::Stroke::new(1.0, app.ui_store.theme.border))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let available_width = ui.available_width();
                let btn_area_width = if app.chat_store.is_loading { 100.0 } else { 52.0 };
                let input_width = available_width - btn_area_width;
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

                        // Track input modifications for IME suppression heuristic.
                        if app.chat_store.input != prev_input {
                            app.ui_store.last_input_modified = std::time::Instant::now();
                        }

                        // Enter sends; Shift+Enter inserts newline.
                        // IME safeguard: detect IMECommit event instead of
                        // relying on a fragile time threshold.
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
                        // Queue-send button (rightmost) — enabled only if input has content.
                        let can_queue =
                            !app.chat_store.input.trim().is_empty() || !app.chat_store.attachments.is_empty();
                        let queue_color = if can_queue {
                            app.ui_store.theme.accent
                        } else {
                            app.ui_store.theme.bg_elevated
                        };
                        let queue_text = if can_queue {
                            app.ui_store.theme.text
                        } else {
                            app.ui_store.theme.text_dim
                        };
                        let queue_btn = ui.add_sized(
                            egui::vec2(44.0, 44.0),
                            egui::Button::new(
                                egui::RichText::new(crate::theme::ICON_PLAY).font(app.ui_store.theme.font_icon(app.ui_store.theme.text_lg)).color(queue_text),
                            )
                            .fill(queue_color)
                            .corner_radius(
                                egui::CornerRadius::same(app.ui_store.theme.radius_full as u8),
                            ),
                        );
                        if queue_btn.clicked() && can_queue {
                            app.chat_store.stick_to_bottom = true;
                            app.send();
                        }
                        if can_queue {
                            queue_btn.on_hover_text(
                                "Steer — cancel current response and send immediately",
                            );
                        } else {
                            queue_btn.on_hover_text("Type a message to steer");
                        }

                        // Stop button (left of queue).
                        let stop_btn = ui.add_sized(
                            egui::vec2(44.0, 44.0),
                            egui::Button::new(
                                egui::RichText::new(crate::theme::ICON_STOP).font(app.ui_store.theme.font_icon(app.ui_store.theme.text_lg)).color(app.ui_store.theme.text),
                            )
                            .fill(app.ui_store.theme.danger)
                            .corner_radius(
                                egui::CornerRadius::same(app.ui_store.theme.radius_full as u8),
                            ),
                        );
                        if stop_btn.clicked() {
                            app.stop();
                        }
                        stop_btn.on_hover_text("Stop generating (Ctrl+C)");
                    } else {
                        // Send button.
                        let btn = ui.add_sized(
                            egui::vec2(44.0, 44.0),
                            egui::Button::new(
                                egui::RichText::new(crate::theme::ICON_PLAY).font(app.ui_store.theme.font_icon(app.ui_store.theme.text_lg)).color(app.ui_store.theme.text),
                            )
                            .fill(app.ui_store.theme.accent)
                            .corner_radius(
                                egui::CornerRadius::same(app.ui_store.theme.radius_full as u8),
                            ),
                        );
                        if btn.clicked() {
                            app.chat_store.stick_to_bottom = true;
                            app.send();
                        }
                        btn.on_hover_text("Send message");
                    }
                });
            });
        });
}
