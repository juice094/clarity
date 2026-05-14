use crate::App;

pub fn render_tui_input(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme.clone();

    ui.horizontal(|ui| {
        let available = ui.available_width();
        let y = ui.cursor().min.y;
        ui.painter().hline(
            ui.min_rect().min.x..=ui.min_rect().min.x + available,
            y,
            egui::Stroke::new(1.0, theme.border),
        );
        ui.allocate_space(egui::vec2(available, 2.0));
    });
    ui.add_space(2.0);

    if let Some(ref snap) = app.chat_store.last_snapshot {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("📸").size(theme.text_xs));
            ui.label(egui::RichText::new(format!("Snapshot #{}", snap.id)).size(theme.text_xs).color(theme.text_dim));
        });
        ui.add_space(2.0);
    }

    if !app.chat_store.attachments.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (i, att) in app.chat_store.attachments.iter().enumerate() {
                if i > 0 { ui.add_space(4.0); }
                ui.label(egui::RichText::new(format!("📎 {}", att.name)).size(theme.text_xs).color(theme.text_muted).underline());
            }
        });
        ui.add_space(2.0);
    }

    let mut text_response: Option<egui::Response> = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        // Shell context prompt: "cwd branch" in muted small text, then ❯ icon
        let mut prompt_width = 0.0;
        if !app.ui_store.shell_prompt.is_empty() {
            let ctx_label = ui.label(
                egui::RichText::new(&app.ui_store.shell_prompt)
                    .size(theme.text_sm)
                    .color(theme.text_dim),
            );
            prompt_width += ctx_label.rect.width() + ui.spacing().item_spacing.x;
        }
        let prompt = ui.label(egui::RichText::new("❯").font(theme.font_icon(theme.text_base)).color(theme.accent));
        prompt_width += prompt.rect.width();

        if app.chat_store.is_loading {
            let spinner = ui.label(egui::RichText::new("◐").font(theme.font_icon(theme.text_sm)).color(theme.status_busy));
            if spinner.clicked() { app.stop(); }
        }

        let available_width = ui.available_width();
        let input_width = (available_width - prompt_width - 8.0).max(80.0);
        ui.allocate_ui_with_layout(
            egui::vec2(input_width, 44.0),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                let hint = tui_hint(app);
                let prev_input = app.chat_store.input.clone();
                let line_count = app.chat_store.input.matches('\n').count() + 1;
                let input_height = (line_count as f32 * 20.0 + 16.0).clamp(36.0, 120.0);
                let text_edit = egui::TextEdit::multiline(&mut app.chat_store.input)
                    .desired_rows(line_count.max(1))
                    .hint_text(hint)
                    .margin(egui::vec2(0.0, 4.0))
                    .frame(false);
                let response = ui.add_sized(egui::vec2(input_width, input_height), text_edit);
                text_response = Some(response.clone());

                if app.ui_store.focus_input_requested {
                    response.request_focus();
                    app.ui_store.focus_input_requested = false;
                }
                if app.chat_store.input != prev_input {
                    app.ui_store.last_input_modified = std::time::Instant::now();
                }
                handle_tui_keys(app, ui, &response, &prev_input);
            },
        );
    });

    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let available = ui.available_width();
        let y = ui.cursor().min.y;
        ui.painter().hline(ui.min_rect().min.x..=ui.min_rect().min.x + available, y, egui::Stroke::new(1.0, theme.border));
        ui.allocate_space(egui::vec2(available, 2.0));
    });

    if let Some(response) = text_response {
        let hint_text = micro_hint(app, &response);
        if !hint_text.is_empty() {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(hint_text).size(theme.text_xs).color(theme.text_dim));
            });
        }
    }
}

fn tui_hint(app: &App) -> String {
    if app.chat_store.stopping { app.t("Stopping current turn...").to_string() }
    else if app.chat_store.pending_send.is_some() { app.t("Message queued, will send after current response...").to_string() }
    else if !app.chat_store.attachments.is_empty() { app.t("Type a message (files attached)...").to_string() }
    else { app.t("Type a message...").to_string() }
}

fn micro_hint(app: &App, response: &egui::Response) -> String {
    if app.chat_store.is_loading { return "Ctrl+C Stop".to_string(); }
    if !response.has_focus() && app.chat_store.input.is_empty() { return "Ctrl+K focus · ? help".to_string(); }
    if response.has_focus() && app.chat_store.input.is_empty() { return "Ctrl+Enter Send · Shift+↑ History · /coder · !cmd".to_string(); }
    if response.has_focus() && !app.chat_store.input.is_empty() { return "Ctrl+Enter Send".to_string(); }
    String::new()
}

fn handle_tui_keys(app: &mut App, ui: &egui::Ui, response: &egui::Response, prev_input: &str) {
    if !response.has_focus() { return; }
    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
    let shift = ui.input(|i| i.modifiers.shift);
    let ime_commit = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(egui::ImeEvent::Commit(_)))));

    if enter_pressed && !shift && !ime_commit {
        while app.chat_store.input.ends_with('\n') { app.chat_store.input.pop(); }
        if app.chat_store.input == *prev_input && !app.chat_store.input.trim().is_empty() && !app.chat_store.is_loading {
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

    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !shift { recall_history(app, -1); }
    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && !shift { recall_history(app, 1); }
}

fn push_input_history(app: &mut App) {
    let text = app.chat_store.input.trim().to_string();
    if text.is_empty() { return; }
    if app.chat_store.input_history.last() != Some(&text) {
        app.chat_store.input_history.push(text);
        if app.chat_store.input_history.len() > 30 { app.chat_store.input_history.remove(0); }
    }
    app.chat_store.input_history_idx = None;
}

fn recall_history(app: &mut App, delta: isize) {
    let hist = &app.chat_store.input_history;
    if hist.is_empty() { return; }
    let max_idx = hist.len().saturating_sub(1);
    let new_idx = match app.chat_store.input_history_idx {
        None => { if delta < 0 { Some(max_idx) } else { None } }
        Some(idx) => {
            let new_i = if delta < 0 { idx.saturating_sub((-delta) as usize) } else { (idx + delta as usize).min(max_idx) };
            if new_i == idx && delta > 0 { None } else { Some(new_i) }
        }
    };
    app.chat_store.input_history_idx = new_idx;
    app.chat_store.input = new_idx.map_or(String::new(), |i| hist[i].clone());
}
