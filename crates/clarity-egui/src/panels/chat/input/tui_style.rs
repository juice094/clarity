use crate::App;

/// Kimi-style rounded composer input.
///
/// Visual spec:
/// - Rounded card container (16px radius) with subtle border
/// - TextEdit inside with no internal frame
/// - Bottom toolbar: [+] `Agent` ............ [Send]
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
            .is_none_or(|s| {
                s.messages.is_empty() && app.view_state.turn != clarity_core::ui::TurnState::Loading
            });

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
            let input_id = egui::Id::new(format!(
                "composer_input_{}",
                app.session_store.active_session_id
            ));
            let text_edit = egui::TextEdit::multiline(&mut app.chat_store.input)
                .id(input_id)
                .hint_text(hint)
                .margin(egui::vec2(0.0, 0.0))
                .frame(false);
            let response = ui.add_sized(egui::vec2(ui.available_width(), 28.0), text_edit);
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

    // Outer padding around the floating composer card.
    ui.add_space(theme.space_8);

    // ── Context items (from # quick-add) ──
    if !app.chat_store.context_items.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let mut remove_idx: Option<usize> = None;
            for (i, item) in app.chat_store.context_items.iter().enumerate() {
                let chip = egui::Frame::new()
                    .fill(theme.accent_subtle)
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("#").size(theme.text_xs).color(theme.accent),
                            );
                            ui.label(
                                egui::RichText::new(&item.display)
                                    .size(theme.text_xs)
                                    .color(theme.text),
                            );
                        });
                    });
                if chip.response.clicked() {
                    remove_idx = Some(i);
                }
            }
            if let Some(i) = remove_idx {
                app.chat_store.context_items.remove(i);
            }
        });
        ui.add_space(theme.space_4);
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
                    // Tracked: github.com/juice094/clarity/issues — remove specific attachment
                }
            }
        });
        ui.add_space(theme.space_8);
    }

    // ── Composer card ──
    // Compact card hugging the top of the bottom panel. Top padding is removed
    // so the text area sits flush with the old separator line.
    let composer_frame = egui::Frame::new()
        .fill(theme.input_bg)
        .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
        .stroke(egui::Stroke::new(1.0, theme.border))
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 6,
            bottom: 10,
        });

    composer_frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());

        // Text input (frameless inside the card)
        let hint = composer_hint(app);
        let prev_input = app.chat_store.input.clone();
        let line_count = app.chat_store.input.matches('\n').count() + 1;
        // Compact single-line baseline; expands smoothly with Shift+Enter.
        let input_height = (line_count as f32 * 20.0 + 10.0).clamp(24.0, 100.0);

        let input_id = egui::Id::new(format!(
            "composer_input_{}",
            app.session_store.active_session_id
        ));
        let text_edit = egui::TextEdit::multiline(&mut app.chat_store.input)
            .id(input_id)
            .desired_rows(line_count.max(1))
            .hint_text(hint)
            .margin(egui::vec2(0.0, 1.0))
            .frame(false);
        let response = ui.add_sized(egui::vec2(ui.available_width(), input_height), text_edit);

        if app.ui_store.focus_input_requested {
            response.request_focus();
            app.ui_store.focus_input_requested = false;
        }
        if app.chat_store.input != prev_input {
            app.ui_store.last_input_modified = std::time::Instant::now();
        }
        handle_tui_keys(app, ui, &response, &prev_input);

        ui.add_space(theme.space_4);

        // ── Bottom toolbar ──
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

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
                        app.chat_store
                            .attachments
                            .push(crate::ui::types::Attachment { path, name });
                    }
                }
            }

            let agent_btn = egui::Button::new(
                egui::RichText::new("Agent")
                    .size(theme.text_xs)
                    .color(theme.text_muted),
            )
            .fill(theme.bg_hover)
            .corner_radius(egui::CornerRadius::same(6));
            if ui.add(agent_btn).on_hover_text("Agent mode").clicked() {
                // Tracked: github.com/juice094/clarity/issues — toggle agent mode
            }

            // Right: model selector + send button
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                // Send button (or stop spinner when loading)
                if app.view_state.turn == clarity_core::ui::TurnState::Loading {
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
                    if ui
                        .add(send_btn)
                        .on_hover_text("Send (Ctrl+Enter)")
                        .clicked()
                        && !app.chat_store.input.trim().is_empty()
                        && app.view_state.turn != clarity_core::ui::TurnState::Loading
                    {
                        app.chat_store.stick_to_bottom = true;
                        app.send();
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
                        app.view_state.main = clarity_core::ui::AppView::Settings;
                    }
                }
            });
        });
    });
}

fn composer_hint(app: &App) -> String {
    if app.view_state.turn == clarity_core::ui::TurnState::Stopping {
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
            && app.view_state.turn != clarity_core::ui::TurnState::Loading
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
