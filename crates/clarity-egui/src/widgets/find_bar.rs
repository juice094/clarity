//! Find-in-session bar — search active session messages and navigate between matches.
//!
//! Extracted from `main.rs` per the egui panel render limit (300 lines).

use crate::App;
use crate::render_line_text;
use crate::design_system::{self, Space, TextStyle};
use clarity_ui::widgets::text_input::TextInput;
use crate::widgets::icon_button_toolbar;

impl App {
    /// S7 Phase 2D: navigate line cursor by `delta` lines (-1 = up, +1 = down).
    pub(crate) fn navigate_line(&mut self, delta: isize) {
        let total = self.ui_store.line_cursor_total_lines;
        if total == 0 {
            return;
        }
        let current = self.ui_store.line_cursor_selected.unwrap_or(0);
        let new_idx = if delta > 0 {
            (current + delta as usize).min(total.saturating_sub(1))
        } else {
            current.saturating_sub((-delta) as usize)
        };
        self.ui_store.line_cursor_selected = Some(new_idx);
    }

    /// S7 Phase 2D: return the text of the currently selected line (if any).
    pub(crate) fn selected_line_text(&self) -> Option<String> {
        let global_idx = self.ui_store.line_cursor_selected?;
        let active_id = self.session_store.active_session_id.clone();
        let session = self
            .session_store
            .sessions
            .iter()
            .find(|s| s.id == active_id)?;
        let mut acc = 0;
        for msg in &session.messages {
            let msg_lines = msg.lines.len();
            if global_idx >= acc && global_idx < acc + msg_lines {
                let local_idx = global_idx - acc;
                return msg.lines.get(local_idx).map(render_line_text);
            }
            acc += msg_lines;
        }
        None
    }

    /// Search the active session's messages for `find_query` and populate
    /// `find_matches` with the indices of matching messages.
    pub(crate) fn update_find_matches(&mut self) {
        // Skip recomputation when the query hasn't changed — avoids an O(n)
        // scan over all messages every frame while the find bar is open.
        if self.chat_store_mut().find_query == self.chat_store_mut().find_last_query {
            return;
        }
        self.chat_store_mut().find_last_query = self.chat_store_mut().find_query.clone();
        self.chat_store_mut().find_matches.clear();
        if self.chat_store_mut().find_query.is_empty() {
            self.chat_store_mut().find_current = 0;
            return;
        }
        let query_lower = self.chat_store_mut().find_query.to_lowercase();
        if let Some(session) = self.session_store.active_session() {
            for (i, msg) in session.messages.iter().enumerate() {
                if msg.content.to_lowercase().contains(&query_lower) {
                    self.chat_store_mut().find_matches.push(i);
                }
            }
        }
        if self.chat_store_mut().find_current >= self.chat_store_mut().find_matches.len() {
            self.chat_store_mut().find_current = self.chat_store_mut().find_matches.len().saturating_sub(1);
        }
    }

    /// Render the find-in-session bar above the chat message list.
    pub(crate) fn render_find_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.ui_store.theme.clone();
        let total = self.chat_store_mut().find_matches.len();
        let current = if total > 0 {
            self.chat_store_mut().find_current + 1
        } else {
            0
        };

        clarity_ui::design_system::Elevation::Base
            .frame(&theme)
            .fill(theme.bg_accent)
            .stroke(egui::Stroke::new(1.0, theme.border))
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .inner_margin(egui::Margin::symmetric(
                theme.space_8 as i8,
                theme.space_4 as i8,
            ))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Search input.
                    let text_edit = ui.add(
                        TextInput::singleline(&mut self.chat_store_mut().find_query)
                            .hint_text("Find in session…")
                            .width(ui.available_width() - 120.0)
                            .transparent(),
                    );
                    if text_edit.changed() {
                        self.chat_store_mut().find_current = 0;
                    }
                    if text_edit.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && total > 0
                    {
                        self.chat_store_mut().find_current = (self.chat_store_mut().find_current + 1) % total;
                    }

                    // Match counter: "2 of 5"
                    let count_text = if self.chat_store_mut().find_query.is_empty() {
                        String::new()
                    } else {
                        format!("{} of {}", current, total)
                    };
                    if !count_text.is_empty() {
                        design_system::text(ui, count_text, TextStyle::Small);
                    }

                    // Prev / Next buttons.
                    let prev_resp = icon_button_toolbar(
                        ui,
                        crate::theme::ICON_CARET_UP,
                        theme.text_xs,
                        &theme,
                    );
                    if prev_resp.clicked() && total > 0 {
                        self.chat_store_mut().find_current = if self.chat_store_mut().find_current > 0 {
                            self.chat_store_mut().find_current - 1
                        } else {
                            total.saturating_sub(1)
                        };
                    }
                    let next_resp = icon_button_toolbar(
                        ui,
                        crate::theme::ICON_CARET_DOWN,
                        theme.text_xs,
                        &theme,
                    );
                    if next_resp.clicked() && total > 0 {
                        self.chat_store_mut().find_current =
                            (self.chat_store_mut().find_current + 1) % total.max(1);
                    }

                    // Close.
                    if icon_button_toolbar(ui, crate::theme::ICON_X, theme.text_sm, &theme)
                        .on_hover_text("Close find")
                        .clicked()
                    {
                        self.chat_store_mut().find_open = false;
                    }
                });
            });
    }
}
