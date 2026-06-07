use crate::App;

/// Kimi-style rounded composer input.
///
/// Visual spec:
/// - Rounded card container (16px radius) with subtle border
/// - TextEdit inside with no internal frame
/// - Bottom toolbar: [+] [Agent] ............ [Send]
/// - Attachments shown as chips above the card
pub fn render_tui_input(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme.clone();

    // ── Empty state: minimal bar (no card frame, no toolbar) ──
    let has_active_session = !app.session_store.active_session_id.is_empty();
    let is_empty_state = !has_active_session
        || app
            .session_store
            .sessions
            .iter()
            .find(|s| s.id == app.session_store.active_session_id)
            .map_or(true, |s| s.messages.is_empty() && !app.chat_store.is_loading);

    if is_empty_state {
        // Empty-state input: subtle rounded bar with border so it doesn't blend into bg
        let bar_frame = egui::Frame::new()
            .fill(theme.input_bg)
            .corner_radius(egui::CornerRadius::same(12))
            .stroke(egui::Stroke::new(1.0, theme.border))
            .inner_margin(egui::Margin::symmetric(12, 8));
        bar_frame.show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let hint = composer_hint(app);
            let prev_input = app.chat_store.input.clone();
            let text_edit = egui::TextEdit::multiline(&mut app.chat_store.input)
                .hint_text(hint)
                .margin(egui::vec2(0.0, 0.0))
                .frame(false);
            let response = ui.add_sized(
                egui::vec2(ui.available_width(), 28.0),
                text_edit,
            );
            if app.ui_store.focus_input_requested {
                response.request_focus();
                app.ui_store.focus_input_requested = false;
            }
            if app.chat_store.input != prev_input {
                app.ui_store.last_input_modified = std::time::Instant::now();
            }
            handle_tui_keys(app, ui, &response, &prev_input);
        });
        return;
    }

    // ── Attachment chips (above the composer) ──
    if !app.chat_store.attachments.is_empty() || app.chat_store.last_snapshot.is_some() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if let Some(ref snap) = app.chat_store.last_snapshot {
                let chip = egui::Frame::new()
                    .fill(theme.bg_hover)
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("📸").size(theme.text_xs));
                            ui.label(
                                egui::RichText::new(format!("Snapshot #{}", snap.id))
                                    .size(theme.text_xs)
                                    .color(theme.text_dim),
                            );
                        });
                    });
                if chip.response.clicked() {
                    app.chat_store.last_snapshot = None;
                }
            }
            for att in &app.chat_store.attachments {
                let chip = egui::Frame::new()
                    .fill(theme.bg_hover)
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("📎").size(theme.text_xs));
                            ui.label(
                                egui::RichText::new(&att.name)
                                    .size(theme.text_xs)
                                    .color(theme.text_muted),
                            );
                        });
                    });
                if chip.response.clicked() {
                    // TODO: remove specific attachment
                }
            }
        });
        ui.add_space(theme.space_8);
    }

    // ── Composer card ──
    let composer_frame = egui::Frame::new()
        .fill(theme.input_bg)
        .corner_radius(egui::CornerRadius::same(16))
        .stroke(egui::Stroke::new(1.0, theme.border))
        .inner_margin(egui::Margin::symmetric(16, 12));

    composer_frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());

        // Text input (frameless inside the card)
        let hint = composer_hint(app);
        let prev_input = app.chat_store.input.clone();
        let line_count = app.chat_store.input.matches('\n').count() + 1;
        let input_height = (line_count as f32 * 22.0 + 12.0).clamp(28.0, 120.0);

        let text_edit = egui::TextEdit::multiline(&mut app.chat_store.input)
            .desired_rows(line_count.max(1))
            .hint_text(hint)
            .margin(egui::vec2(0.0, 2.0))
            .frame(false);
        let response = ui.add_sized(
            egui::vec2(ui.available_width(), input_height),
            text_edit,
        );

        if app.ui_store.focus_input_requested {
            response.request_focus();
            app.ui_store.focus_input_requested = false;
        }
        if app.chat_store.input != prev_input {
            app.ui_store.last_input_modified = std::time::Instant::now();
        }
        handle_tui_keys(app, ui, &response, &prev_input);

        ui.add_space(theme.space_8);

        // ── Bottom toolbar ──
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            // Left: add attachment / agent buttons
            if crate::widgets::icon_button_toolbar(
                ui,
                crate::theme::ICON_LIST,
                theme.text_sm,
                theme,
            )
            .on_hover_text("Add attachment")
            .clicked()
            {
                if let Some(paths) = rfd::FileDialog::new().pick_files() {
                    for path in paths {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        app.chat_store.attachments.push(crate::ui::types::Attachment {
                            path,
                            name,
                        });
                    }
                }
            }

            let agent_btn = egui::Button::new(
                egui::RichText::new("Agent")
                    .size(theme.text_sm)
                    .color(theme.text_muted),
            )
            .fill(theme.bg_hover)
            .corner_radius(egui::CornerRadius::same(8));
            if ui.add(agent_btn).on_hover_text("Agent mode").clicked() {
                // TODO: toggle agent mode
            }

            // Right: model selector + send button
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;

                // Send button (or stop spinner when loading)
                if app.chat_store.is_loading {
                    let stop_btn = egui::Button::new(
                        egui::RichText::new(crate::theme::ICON_X)
                            .font(theme.font_icon(theme.text_base))
                            .color(theme.danger),
                    )
                    .fill(theme.bg_hover)
                    .corner_radius(egui::CornerRadius::same(10));
                    if ui.add(stop_btn).on_hover_text("Stop generation").clicked() {
                        app.stop();
                    }
                } else {
                    let send_btn = egui::Button::new(
                        egui::RichText::new(crate::theme::ICON_SEND)
                            .font(theme.font_icon(theme.text_base))
                            .color(if app.chat_store.input.trim().is_empty() {
                                theme.text_dim
                            } else {
                                theme.accent
                            }),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(egui::CornerRadius::same(10));
                    if ui.add(send_btn).on_hover_text("Send (Ctrl+Enter)").clicked() {
                        if !app.chat_store.input.trim().is_empty() && !app.chat_store.is_loading {
                            app.chat_store.stick_to_bottom = true;
                            app.send();
                        }
                    }
                }

                // Model selector pill
                let model_name = app.settings_store.settings_edit.model.trim();
                if !model_name.is_empty() {
                    let model_btn = egui::Button::new(
                        egui::RichText::new(format!("{} ↓", model_name))
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(egui::CornerRadius::same(8));
                    if ui.add(model_btn).on_hover_text("Switch model").clicked() {
                        app.settings_store.settings_open = true;
                    }
                }
            });
        });
    });
}

fn composer_hint(app: &App) -> String {
    if app.chat_store.stopping {
        app.t("Stopping current turn...").to_string()
    } else if app.chat_store.pending_send.is_some() {
        app.t("Message queued, will send after current response...")
            .to_string()
    } else if !app.chat_store.attachments.is_empty() {
        app.t("Type a message (files attached)...").to_string()
    } else {
        app.t("Type a message...").to_string()
    }
}

fn handle_tui_keys(app: &mut App, ui: &egui::Ui, response: &egui::Response, prev_input: &str) {
    if !response.has_focus() {
        return;
    }
    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
    let shift = ui.input(|i| i.modifiers.shift);
    let ime_commit = ui.input(|i| {
        i.events
            .iter()
            .any(|e| matches!(e, egui::Event::Ime(egui::ImeEvent::Commit(_))))
    });

    if enter_pressed && !shift && !ime_commit {
        while app.chat_store.input.ends_with('\n') {
            app.chat_store.input.pop();
        }
        if app.chat_store.input == *prev_input
            && !app.chat_store.input.trim().is_empty()
            && !app.chat_store.is_loading
        {
            let trimmed = app.chat_store.input.trim().to_string();
            if let Some(cmd) = trimmed.strip_prefix('!') {
                let cmd = cmd.trim().to_string();
                if !cmd.is_empty() {
                    app.chat_store.input.clear();
                    app.execute_shell_direct(cmd);
                    return;
                }
            }
            push_input_history(app);
            app.chat_store.stick_to_bottom = true;
            app.send();
        }
    }

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !app.chat_store.input.is_empty() {
        app.chat_store.input.clear();
        app.chat_store.input_history_idx = None;
    }

    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !shift {
        recall_history(app, -1);
    }
    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && !shift {
        recall_history(app, 1);
    }
}

fn push_input_history(app: &mut App) {
    let text = app.chat_store.input.trim().to_string();
    if text.is_empty() {
        return;
    }
    if app.chat_store.input_history.last() != Some(&text) {
        app.chat_store.input_history.push(text);
        if app.chat_store.input_history.len() > 30 {
            app.chat_store.input_history.remove(0);
        }
    }
    app.chat_store.input_history_idx = None;
}

fn recall_history(app: &mut App, delta: isize) {
    let hist = &app.chat_store.input_history;
    if hist.is_empty() {
        return;
    }
    let max_idx = hist.len().saturating_sub(1);
    let new_idx = match app.chat_store.input_history_idx {
        None => {
            if delta < 0 {
                Some(max_idx)
            } else {
                None
            }
        }
        Some(idx) => {
            let new_i = if delta < 0 {
                idx.saturating_sub((-delta) as usize)
            } else {
                (idx + delta as usize).min(max_idx)
            };
            if new_i == idx && delta > 0 {
                None
            } else {
                Some(new_i)
            }
        }
    };
    app.chat_store.input_history_idx = new_idx;
    app.chat_store.input = new_idx.map_or(String::new(), |i| hist[i].clone());
}
