//! Message-level actions: inline-edit, regenerate.
//!
//! ARCHITECTURE:
//!   - Pure UI-state mutations (no LLM logic here).
//!   - `regenerate` re-uses the existing `App::send()` path by setting input
//!     and calling send, keeping a single source of truth for agent invocation.
//!
//! Sprint 33: compensates missing modern frontend affordances (Edit/Regenerate).

use crate::App;
use crate::ui::types::Role;

impl App {
    // ── Inline Edit ──

    /// Enter inline-edit mode for the message at `idx`.
    /// Only user messages are editable.
    pub(crate) fn start_edit(&mut self, idx: usize) {
        let content = self
            .context
            .session_store
            .active_session()
            .and_then(|session| session.messages.get(idx))
            .filter(|msg| msg.role == Role::User)
            .map(|msg| msg.content.clone());
        if let Some(content) = content {
            let chat_store = self.chat_store_mut();
            chat_store.editing_message_idx = Some(idx);
            chat_store.edit_buffer = content;
        }
    }

    /// Commit the edit: overwrite the message, truncate everything after it,
    /// and re-submit so the agent regenerates the response from the new prompt.
    pub(crate) fn commit_edit(&mut self) {
        let Some(idx) = self.chat_store_mut().editing_message_idx else {
            return;
        };
        let new_text = self.chat_store_mut().edit_buffer.trim().to_string();
        if new_text.is_empty() {
            self.cancel_edit();
            return;
        }

        if let Some(session) = self.context.session_store.active_session_mut() {
            if idx < session.messages.len() {
                session.messages[idx].content = new_text.clone();
                session.messages[idx].blocks.clear();
                session.messages[idx].prepare();
                // Truncate history after this message.
                session.messages.truncate(idx + 1);
                session.turn_heights.clear();
                session.updated_at = crate::session::now_millis();
            }
        }
        self.chat_store_mut().editing_message_idx = None;
        self.chat_store_mut().edit_buffer.clear();
        self.chat_store_mut().input = new_text;
        self.send();
    }

    /// Cancel inline edit without mutating session history.
    pub(crate) fn cancel_edit(&mut self) {
        self.chat_store_mut().editing_message_idx = None;
        self.chat_store_mut().edit_buffer.clear();
    }

    // ── Regenerate ──

    /// Regenerate the AI response starting at `ai_idx`.
    ///
    /// Behaviour:
    /// 1. Find the nearest preceding user message.
    /// 2. Delete the AI message at `ai_idx` and everything after it.
    /// 3. Re-submit the user prompt so the agent streams a new response.
    pub(crate) fn regenerate(&mut self, ai_idx: usize) {
        let session = match self.context.session_store.active_session_mut() {
            Some(s) => s,
            None => return,
        };
        if ai_idx >= session.messages.len() {
            return;
        }

        // Find the nearest preceding user message.
        let user_idx = session.messages[..ai_idx]
            .iter()
            .rposition(|m| m.role == Role::User);

        let Some(user_idx) = user_idx else {
            crate::handlers::system::push_toast(
                &mut self.context.ui_store,
                "Cannot regenerate: no preceding user message",
                crate::ui::types::ToastLevel::Warn,
            );
            return;
        };

        let query = session.messages[user_idx].content.clone();
        // Remove the AI message and anything after it.
        session.messages.truncate(ai_idx);
        session.turn_heights.clear();
        session.updated_at = crate::session::now_millis();
        // Re-submit.
        self.chat_store_mut().input = query;
        self.send();
    }

    // ── Keyboard message selection (A2) ──

    /// Move the keyboard message selection by `delta` turn units
    /// (`-1` = previous, `+1` = next) and scroll it into view.
    ///
    /// The stored selection is the first message index of the selected unit
    /// so edit / regenerate / copy can reuse the message-level helpers.
    pub(crate) fn navigate_message_selection(&mut self, delta: isize) {
        let Some(session) = self.context.session_store.active_session() else {
            return;
        };
        let units = crate::panels::chat::message_list::aggregate_turns(&session.messages);
        let unit_starts: Vec<usize> = units.iter().map(|u| u.start).collect();
        let Some(next) = move_selection(
            self.context.ui_store.selected_message_idx,
            &unit_starts,
            delta,
        ) else {
            return;
        };
        self.context.ui_store.selected_message_idx = Some(next);
        // Browsing history implies the user is no longer pinned to the tail.
        self.chat_store_mut().stick_to_bottom = false;
        self.scroll_selection_into_view(&units);
    }

    /// Copy text of the currently selected message unit, mirroring the hover
    /// action row: a user unit copies its single message, an agent unit joins
    /// all messages of the turn with a blank line.
    pub(crate) fn selected_message_text(&self) -> Option<String> {
        let sel = self.context.ui_store.selected_message_idx?;
        let session = self.context.session_store.active_session()?;
        let units = crate::panels::chat::message_list::aggregate_turns(&session.messages);
        let unit = units.iter().find(|u| u.start == sel)?;
        if unit.is_user {
            Some(session.messages.get(unit.start)?.content.clone())
        } else {
            Some(
                session.messages[unit.start..unit.end]
                    .iter()
                    .map(|m| m.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            )
        }
    }

    /// Enter inline-edit for the selected message (user messages only, same
    /// guard as [`App::start_edit`]). Clears the selection on success.
    pub(crate) fn edit_selected_message(&mut self) {
        let Some(sel) = self.context.ui_store.selected_message_idx else {
            return;
        };
        self.start_edit(sel);
        if self.chat_store().editing_message_idx.is_some() {
            self.context.ui_store.selected_message_idx = None;
        }
    }

    /// Regenerate from the selected message. Only applies to agent messages,
    /// mirroring the hover action row which offers Regenerate on agent turns.
    pub(crate) fn regenerate_selected_message(&mut self) {
        let Some(sel) = self.context.ui_store.selected_message_idx else {
            return;
        };
        let is_agent = self
            .context
            .session_store
            .active_session()
            .and_then(|s| s.messages.get(sel))
            .is_some_and(|m| m.role == Role::Agent);
        if !is_agent {
            return;
        }
        self.context.ui_store.selected_message_idx = None;
        self.regenerate(sel);
    }

    /// Align the scroll offset so the selected unit sits near the top of the
    /// viewport. Uses the per-session height cache; when it is cold the
    /// selection simply stays put for a frame until the cache is built.
    fn scroll_selection_into_view(
        &mut self,
        units: &[crate::panels::chat::message_list::RenderUnit],
    ) {
        /// Gap kept above the selected unit so it does not hug the viewport edge.
        const SELECTION_SCROLL_MARGIN: f32 = 120.0;
        let Some(sel) = self.context.ui_store.selected_message_idx else {
            return;
        };
        let Some(unit_idx) = units.iter().position(|u| u.start == sel) else {
            return;
        };
        let Some(session) = self.context.session_store.active_session() else {
            return;
        };
        let Some(cache) = session.height_cache.as_ref() else {
            return;
        };
        if unit_idx >= cache.unit_heights.len() {
            return;
        }
        let top: f32 = cache.unit_heights[..unit_idx].iter().sum();
        self.context.ui_store.last_scroll_offset = (top - SELECTION_SCROLL_MARGIN).max(0.0);
    }
}

/// Compute the next keyboard selection when moving by `delta` turn units.
///
/// `current` is the selected unit's first message index (if any) and
/// `unit_starts` the first message index of every turn unit. Movement clamps
/// at both ends; with no prior selection (or a stale one that no longer
/// matches a unit, e.g. after history truncation) any direction selects the
/// last — most recent — unit. Returns `None` only for an empty list.
pub(crate) fn move_selection(
    current: Option<usize>,
    unit_starts: &[usize],
    delta: isize,
) -> Option<usize> {
    if unit_starts.is_empty() {
        return None;
    }
    let pos = current.and_then(|c| unit_starts.iter().position(|&s| s == c));
    let next = match pos {
        Some(p) if delta < 0 => p.saturating_sub(delta.unsigned_abs()),
        Some(p) => (p + delta as usize).min(unit_starts.len() - 1),
        None => unit_starts.len() - 1,
    };
    Some(unit_starts[next])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_selection_empty_list_yields_none() {
        assert_eq!(move_selection(None, &[], 1), None);
        assert_eq!(move_selection(Some(0), &[], -1), None);
    }

    #[test]
    fn move_selection_without_current_selects_last() {
        let starts = [0, 1, 3];
        assert_eq!(move_selection(None, &starts, -1), Some(3));
        assert_eq!(move_selection(None, &starts, 1), Some(3));
    }

    #[test]
    fn move_selection_steps_and_clamps() {
        let starts = [0, 1, 3];
        assert_eq!(move_selection(Some(3), &starts, -1), Some(1));
        assert_eq!(move_selection(Some(1), &starts, -1), Some(0));
        // Clamp at the top.
        assert_eq!(move_selection(Some(0), &starts, -1), Some(0));
        // Clamp at the bottom.
        assert_eq!(move_selection(Some(3), &starts, 1), Some(3));
        assert_eq!(move_selection(Some(1), &starts, 1), Some(3));
    }

    #[test]
    fn move_selection_stale_current_falls_back_to_last() {
        let starts = [0, 1, 3];
        // 2 is not a unit start (e.g. history was truncated).
        assert_eq!(move_selection(Some(2), &starts, -1), Some(3));
        assert_eq!(move_selection(Some(99), &starts, 1), Some(3));
    }
}
