use crate::App;
use crate::design_system::{self, Space};
use crate::widgets::icon_button_toolbar;
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::text_input::TextInput;

/// Kimi-style rounded composer input.
///
/// Visual spec:
/// - Rounded floating card container with subtle border and soft shadow.
/// - TextEdit inside with no internal frame.
/// - Bottom toolbar: [+] `/` plugins  [agent mode] ............ [model] [Send].
/// - Attachments and context shown as chips above the card.
pub fn render_tui_input(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.context.ui_store.theme.clone();

    let has_active_session = !app.context.session_store.active_session_id.is_empty();
    let is_empty_state = !has_active_session
        || app
            .context
            .session_store
            .sessions
            .iter()
            .find(|s| s.id == app.context.session_store.active_session_id)
            .is_none_or(|s| {
                s.messages.is_empty() && app.view_state.turn != clarity_core::ui::TurnState::Loading
            });

    if is_empty_state {
        render_empty_input(app, ui, theme);
        return;
    }

    // ── Context items (from # quick-add) ──
    if !app.chat_store_mut().context_items.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let mut remove_idx: Option<usize> = None;
            for (i, item) in app.chat_store_mut().context_items.iter().enumerate() {
                let (response, _) = design_system::chip(ui, &item.display, Some("#"), false);
                if response.clicked() {
                    remove_idx = Some(i);
                }
            }
            if let Some(i) = remove_idx {
                app.chat_store_mut().context_items.remove(i);
            }
        });
        design_system::gap(ui, Space::S0);
    }

    // ── Attachment chips (above the composer) ──
    if !app.chat_store_mut().attachments.is_empty() || app.chat_store_mut().last_snapshot.is_some()
    {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if let Some(ref snap) = app.chat_store_mut().last_snapshot {
                let label = format!("Snapshot #{}", snap.id);
                let (response, _) = design_system::chip(ui, label, Some("📸"), false);
                if response.clicked() {
                    app.chat_store_mut().last_snapshot = None;
                }
            }
            for att in &app.chat_store_mut().attachments {
                let (response, _) = design_system::chip(ui, &att.name, Some("📎"), false);
                if response.clicked() {
                    // Tracked: remove specific attachment.
                }
            }
        });
        design_system::gap(ui, Space::S1);
    }

    // ── Composer card ──
    let plugin_picker_open = app.context.ui_store.plugin_picker_state.open;
    design_system::composer_card(ui, |ui| {
        ui.set_min_width(ui.available_width());

        // Text input (frameless inside the card)
        let hint = composer_hint(app);
        let prev_input = app.chat_store_mut().input.clone();
        let line_count = app.chat_store_mut().input.matches('\n').count() + 1;
        let input_height = (line_count as f32 * 20.0 + 10.0).clamp(24.0, 100.0);

        let input_id = egui::Id::new(format!(
            "composer_input_{}",
            app.context.session_store.active_session_id
        ));
        let response = ui.add_sized(
            egui::vec2(ui.available_width(), input_height),
            TextInput::multiline(&mut app.chat_store_mut().input)
                .transparent()
                .id(input_id)
                .desired_rows(line_count.max(1))
                .hint_text(hint),
        );

        if app.context.ui_store.focus_target.is_some() {
            response.request_focus();
            app.context.ui_store.focus_target = None;
        }
        if app.chat_store_mut().input != prev_input {
            app.context.ui_store.last_input_modified = std::time::Instant::now();
            check_context_picker_trigger(app);
            check_plugin_picker_trigger(app, &prev_input);
        }
        handle_tui_keys(app, ui, &response, &prev_input);

        design_system::gap(ui, Space::S0);

        // ── Bottom toolbar ──
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // Left: attachment / plugins / agent mode.
            if icon_button_toolbar(ui, crate::theme::ICON_PLUS, theme.text_sm, theme)
                .on_hover_text("Add attachment")
                .clicked()
            {
                if let Some(paths) = rfd::FileDialog::new().pick_files() {
                    for path in paths {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        app.chat_store_mut()
                            .attachments
                            .push(crate::ui::types::Attachment { path, name });
                    }
                }
            }

            if ui
                .add(Button::new("/").ghost().small())
                .on_hover_text("Plugins (type /)")
                .clicked()
            {
                app.context.ui_store.plugin_picker_state.open = true;
                app.context.ui_store.plugin_picker_state.filter.clear();
            }

            render_agent_mode_dropdown(app, ui, theme);

            // Right: model selector + send button.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                if app.view_state.turn == clarity_core::ui::TurnState::Loading {
                    let stop_resp = clarity_ui::widgets::icon_button::icon_button(
                        ui,
                        crate::theme::ICON_X,
                        theme.text_base,
                        theme.bg_hover,
                        egui::CornerRadius::same(10),
                        theme,
                    );
                    if stop_resp.on_hover_text("Stop generation").clicked() {
                        app.stop();
                    }
                } else {
                    let send_color = if app.chat_store_mut().input.trim().is_empty() {
                        theme.text_dim
                    } else {
                        theme.accent
                    };
                    let send_resp = clarity_ui::widgets::icon_button::icon_button_with_color(
                        ui,
                        crate::theme::ICON_SEND,
                        theme.text_base,
                        egui::Color32::TRANSPARENT,
                        send_color,
                        egui::CornerRadius::same(10),
                        theme,
                    );
                    if send_resp.on_hover_text("Send (Ctrl+Enter)").clicked()
                        && !app.chat_store_mut().input.trim().is_empty()
                        && app.view_state.turn != clarity_core::ui::TurnState::Loading
                    {
                        app.chat_store_mut().stick_to_bottom = true;
                        app.send();
                    }
                }

                let model_name = app.settings_store().settings_edit.model.trim();
                if !model_name.is_empty() {
                    let label = format!("{} ↓", model_name);
                    if ui
                        .add(Button::new(&label).ghost().small())
                        .on_hover_text("Switch model")
                        .clicked()
                    {
                        app.navigate(clarity_core::ui::AppView::Settings.into());
                    }
                }
            });
        });
    });

    // ── `/` plugin picker popup ──
    if plugin_picker_open {
        let plugins: Vec<crate::widgets::plugin_picker::PluginEntry> =
            app.current_plugins().into_iter().map(Into::into).collect();
        let mut state = app.context.ui_store.plugin_picker_state.clone();
        if let Some(selected) =
            crate::widgets::plugin_picker::render_plugin_picker(ui, &mut state, theme, &plugins)
        {
            apply_plugin_selection(app, &selected);
            state.open = false;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            state.open = false;
        }
        app.context.ui_store.plugin_picker_state = state;
    }

    // ── # Context picker popup ──
    if app.context.ui_store.context_picker_state.open {
        let ws_root = app
            .context
            .files_store
            .workspace_root
            .to_string_lossy()
            .into_owned();
        let available: Vec<crate::ui::types::ContextSource> = vec![
            crate::ui::types::ContextSource::File {
                path: ws_root.clone(),
                start_line: None,
                end_line: None,
            },
            crate::ui::types::ContextSource::Folder { path: ws_root },
            crate::ui::types::ContextSource::Terminal {
                command: String::new(),
            },
            crate::ui::types::ContextSource::Web { url: String::new() },
        ];
        let mut state = app.context.ui_store.context_picker_state.clone();
        if let Some(item) =
            crate::widgets::context_picker::render_context_picker(ui, &mut state, theme, &available)
        {
            app.chat_store_mut().context_items.push(item);
            if app.chat_store_mut().input.ends_with('#') {
                app.chat_store_mut().input.pop();
            }
            app.context.ui_store.focus_target = Some(crate::stores::FocusTarget::ChatInput);
        }
        app.context.ui_store.context_picker_state = state;
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if app.context.ui_store.context_picker_state.browsing.is_some() {
                app.context.ui_store.context_picker_state.browsing = None;
                app.context.ui_store.context_picker_state.filter.clear();
            } else {
                app.context.ui_store.context_picker_state.open = false;
            }
        }
    }
}

/// Estimate the total height the active-state composer needs inside
/// `render_input_panel`. Used by the chrome to carve out the bottom strip.
///
/// ponytail: this is a cheap analytical estimate; it mirrors the margins and
/// row heights used by `render_tui_input`. If the visual spec changes, keep
/// this in sync or replace with a measurement pass.
pub fn estimate_input_height(app: &App) -> f32 {
    let theme = &app.context.ui_store.theme;
    let line_count = app.chat_store().input.matches('\n').count() + 1;
    let text_h = (line_count as f32 * 20.0 + 10.0).clamp(24.0, 100.0);
    let toolbar_h = theme.text_base + 14.0;
    let gap_s0 = theme.space_4;
    let composer_margin = theme.space_12 * 2.0;
    let outer_margin = theme.space_8 * 2.0;
    let mut h = outer_margin + composer_margin + text_h + gap_s0 + toolbar_h;
    if !app.chat_store().context_items.is_empty() {
        h += theme.text_base + 10.0 + gap_s0;
    }
    if !app.chat_store().attachments.is_empty() || app.chat_store().last_snapshot.is_some() {
        h += theme.text_base + 10.0 + theme.space_8;
    }
    h.clamp(90.0, 180.0)
}

/// Empty-state input: a single subtle rounded bar.
fn render_empty_input(app: &mut App, ui: &mut egui::Ui, _theme: &crate::theme::Theme) {
    design_system::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let hint = composer_hint(app);
        let prev_input = app.chat_store_mut().input.clone();
        let input_id = egui::Id::new(format!(
            "composer_input_{}",
            app.context.session_store.active_session_id
        ));
        let response = ui.add_sized(
            egui::vec2(ui.available_width(), 28.0),
            TextInput::multiline(&mut app.chat_store_mut().input)
                .transparent()
                .id(input_id)
                .hint_text(hint),
        );
        if app.context.ui_store.focus_target.is_some() {
            response.request_focus();
            app.context.ui_store.focus_target = None;
        }
        if app.chat_store_mut().input != prev_input {
            app.context.ui_store.last_input_modified = std::time::Instant::now();
            check_context_picker_trigger(app);
            check_plugin_picker_trigger(app, &prev_input);
        }
        handle_tui_keys(app, ui, &response, &prev_input);
    });
}

/// Render the agent-mode dropdown in the composer toolbar.
fn render_agent_mode_dropdown(app: &mut App, ui: &mut egui::Ui, _theme: &crate::theme::Theme) {
    let mode = app.view_state.agent_mode;
    let label = agent_mode_label(mode);
    ui.menu_button(label, |ui| {
        ui.set_min_width(100.0);
        for candidate in [
            clarity_core::ui::AgentMode::Chat,
            clarity_core::ui::AgentMode::Code,
            clarity_core::ui::AgentMode::Work,
            clarity_core::ui::AgentMode::Claw,
        ] {
            if ui
                .selectable_label(mode == candidate, agent_mode_label(candidate))
                .clicked()
            {
                app.view_state.agent_mode = candidate;
                ui.close();
            }
        }
    })
    .response
    .on_hover_text("Agent mode");
}

fn agent_mode_label(mode: clarity_core::ui::AgentMode) -> &'static str {
    match mode {
        clarity_core::ui::AgentMode::Chat => "Chat",
        clarity_core::ui::AgentMode::Code => "Code",
        clarity_core::ui::AgentMode::Work => "Work",
        clarity_core::ui::AgentMode::Claw => "Claw",
    }
}

/// Apply a plugin selection to the current app state.
fn apply_plugin_selection(app: &mut App, plugin: &crate::widgets::plugin_picker::PluginEntry) {
    match plugin.id.as_str() {
        "chat" => app.view_state.agent_mode = clarity_core::ui::AgentMode::Chat,
        "code" => app.view_state.agent_mode = clarity_core::ui::AgentMode::Code,
        "work" => app.view_state.agent_mode = clarity_core::ui::AgentMode::Work,
        "claw" => app.view_state.agent_mode = clarity_core::ui::AgentMode::Claw,
        _ => {
            // ponytail: skills/MCP tools/web tabs are not yet wired to runtime
            // actions. For now, selecting one inserts its display name as a
            // reminder tag so the user sees the picker is working.
            let tag = format!("[{}]", plugin.name);
            if !app.chat_store_mut().input.contains(&tag) {
                if !app.chat_store_mut().input.is_empty()
                    && !app.chat_store_mut().input.ends_with(' ')
                {
                    app.chat_store_mut().input.push(' ');
                }
                app.chat_store_mut().input.push_str(&tag);
                app.chat_store_mut().input.push(' ');
            }
        }
    }
}

/// Open the `#` context picker when the user types `#` at the end of input.
fn check_context_picker_trigger(app: &mut App) {
    let input = app.chat_store().input.clone();
    if input.ends_with('#') && !input.ends_with("##") {
        app.context.ui_store.context_picker_state.open = true;
        app.context.ui_store.context_picker_state.cwd =
            app.context.files_store.workspace_root.clone();
    }
    if !input.contains('#') {
        app.context.ui_store.context_picker_state.open = false;
    }
}

/// Open the `/` plugin picker when the user types `/` at the start or after whitespace.
fn check_plugin_picker_trigger(app: &mut App, _prev_input: &str) {
    let input = app.chat_store().input.clone();
    if is_plugin_trigger(&input) {
        app.context.ui_store.plugin_picker_state.open = true;
        app.context.ui_store.plugin_picker_state.filter.clear();
        // Remove the trigger character from the composer; the picker has its own filter.
        if app.chat_store_mut().input.ends_with('/') {
            app.chat_store_mut().input.pop();
        }
    }
}

fn is_plugin_trigger(input: &str) -> bool {
    if input.is_empty() {
        return false;
    }
    if input == "/" {
        return true;
    }
    // Trigger only when '/' is the last character and is preceded by whitespace
    // or is the very first character. This avoids intercepting paths like "src/main.rs".
    input.ends_with('/')
        && input.len() >= 2
        && input.as_bytes()[input.len() - 2].is_ascii_whitespace()
}

fn composer_hint(app: &App) -> String {
    if app.view_state.turn == clarity_core::ui::TurnState::Stopping {
        app.t("Stopping current turn...").to_string()
    } else if app.chat_store().pending_send.is_some() {
        app.t("Message queued, will send after current response...")
            .to_string()
    } else if !app.chat_store().attachments.is_empty() {
        app.t("Type a message (files attached)...").to_string()
    } else {
        app.t("Type / for plugins, # for context...").to_string()
    }
}

fn handle_tui_keys(app: &mut App, ui: &egui::Ui, response: &egui::Response, prev_input: &str) {
    if app.context.ui_store.plugin_picker_state.open {
        // Let the plugin picker consume Enter/Escape; do not submit the composer.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.context.ui_store.plugin_picker_state.open = false;
        }
        return;
    }

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
        while app.chat_store_mut().input.ends_with('\n') {
            app.chat_store_mut().input.pop();
        }
        if app.chat_store_mut().input == *prev_input
            && !app.chat_store_mut().input.trim().is_empty()
            && app.view_state.turn != clarity_core::ui::TurnState::Loading
        {
            let trimmed = app.chat_store_mut().input.trim().to_string();
            if let Some(cmd) = trimmed.strip_prefix('!') {
                let cmd = cmd.trim().to_string();
                if !cmd.is_empty() {
                    app.chat_store_mut().input.clear();
                    app.execute_shell_direct(cmd);
                    return;
                }
            }
            push_input_history(app);
            app.chat_store_mut().stick_to_bottom = true;
            app.send();
        }
    }

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !app.chat_store_mut().input.is_empty() {
        app.chat_store_mut().input.clear();
        app.chat_store_mut().input_history_idx = None;
    }

    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !shift {
        recall_history(app, -1);
    }
    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && !shift {
        recall_history(app, 1);
    }
}

fn push_input_history(app: &mut App) {
    let text = app.chat_store_mut().input.trim().to_string();
    if text.is_empty() {
        return;
    }
    if app.chat_store_mut().input_history.last() != Some(&text) {
        app.chat_store_mut().input_history.push(text);
        if app.chat_store_mut().input_history.len() > 30 {
            app.chat_store_mut().input_history.remove(0);
        }
    }
    app.chat_store_mut().input_history_idx = None;
}

fn recall_history(app: &mut App, delta: isize) {
    let chat_store = app.chat_store_mut();
    let hist = &chat_store.input_history;
    if hist.is_empty() {
        return;
    }
    let max_idx = hist.len().saturating_sub(1);
    let new_idx = match chat_store.input_history_idx {
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
    chat_store.input = new_idx.map_or(String::new(), |i| hist[i].clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        let ctx = egui::Context::default();
        crate::apps::test_app(&ctx)
    }

    #[test]
    fn estimate_input_height_empty_is_within_expected_range() {
        let app = test_app();
        let h = estimate_input_height(&app);
        assert!(
            (90.0..=120.0).contains(&h),
            "empty input height {h} should be in [90, 120]"
        );
    }

    #[test]
    fn estimate_input_height_grows_with_line_count() {
        let mut app = test_app();
        app.chat_store_mut().input = "one".to_string();
        let h1 = estimate_input_height(&app);
        app.chat_store_mut().input = "one\ntwo\nthree\nfour\nfive".to_string();
        let h5 = estimate_input_height(&app);
        assert!(
            h5 > h1,
            "5-line height {h5} should exceed 1-line height {h1}"
        );
    }

    #[test]
    fn estimate_input_height_clamps_at_maximum() {
        let mut app = test_app();
        app.chat_store_mut().input = (0..20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        app.chat_store_mut()
            .context_items
            .push(clarity_apps::chat::ContextItem {
                display: "ctx".to_string(),
                source: clarity_apps::chat::ContextSource::File {
                    path: "src/main.rs".to_string(),
                    start_line: None,
                    end_line: None,
                },
                payload: String::new(),
            });
        app.chat_store_mut()
            .attachments
            .push(crate::ui::types::Attachment {
                path: std::path::PathBuf::from("foo.txt"),
                name: "foo.txt".to_string(),
            });
        let h = estimate_input_height(&app);
        assert!(h <= 180.0, "height {h} should be clamped to <= 180");
    }

    #[test]
    fn estimate_input_height_increases_with_context_items() {
        let mut app = test_app();
        app.chat_store_mut().input = "hi".to_string();
        let base = estimate_input_height(&app);
        app.chat_store_mut()
            .context_items
            .push(clarity_apps::chat::ContextItem {
                display: "src/main.rs".to_string(),
                source: clarity_apps::chat::ContextSource::File {
                    path: "src/main.rs".to_string(),
                    start_line: None,
                    end_line: None,
                },
                payload: String::new(),
            });
        let with_ctx = estimate_input_height(&app);
        assert!(
            with_ctx > base,
            "with context {with_ctx} should exceed base {base}"
        );
    }
}
