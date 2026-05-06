//! Message-level actions: inline-edit, regenerate.
//!
//! ARCHITECTURE:
//!   - Pure UI-state mutations (no LLM logic here).
//!   - `regenerate` re-uses the existing `App::send()` path by setting input
//!     and calling send, keeping a single source of truth for agent invocation.
//!
//! Sprint 33: compensates missing modern frontend affordances (Edit/Regenerate).

use crate::ui::types::Role;
use crate::App;

impl App {
    // ── Inline Edit ──

    /// Enter inline-edit mode for the message at `idx`.
    /// Only user messages are editable.
    pub(crate) fn start_edit(&mut self, idx: usize) {
        if let Some(session) = self.session_store.active_session() {
            if let Some(msg) = session.messages.get(idx) {
                if msg.role == Role::User {
                    self.chat_store.editing_message_idx = Some(idx);
                    self.chat_store.edit_buffer = msg.content.clone();
                }
            }
        }
    }

    /// Commit the edit: overwrite the message, truncate everything after it,
    /// and re-submit so the agent regenerates the response from the new prompt.
    pub(crate) fn commit_edit(&mut self) {
        let Some(idx) = self.chat_store.editing_message_idx else {
            return;
        };
        let new_text = self.chat_store.edit_buffer.trim().to_string();
        if new_text.is_empty() {
            self.cancel_edit();
            return;
        }

        if let Some(session) = self.session_store.active_session_mut() {
            if idx < session.messages.len() {
                session.messages[idx].content = new_text.clone();
                session.messages[idx].blocks.clear();
                session.messages[idx].prepare();
                // Truncate history after this message.
                session.messages.truncate(idx + 1);
                session.turn_heights.clear();
            }
        }
        self.chat_store.editing_message_idx = None;
        self.chat_store.edit_buffer.clear();
        self.chat_store.input = new_text;
        self.send();
    }

    /// Cancel inline edit without mutating session history.
    pub(crate) fn cancel_edit(&mut self) {
        self.chat_store.editing_message_idx = None;
        self.chat_store.edit_buffer.clear();
    }

    // ── Regenerate ──

    /// Regenerate the AI response starting at `ai_idx`.
    ///
    /// Behaviour:
    /// 1. Find the nearest preceding user message.
    /// 2. Delete the AI message at `ai_idx` and everything after it.
    /// 3. Re-submit the user prompt so the agent streams a new response.
    pub(crate) fn regenerate(&mut self, ai_idx: usize) {
        let session = match self.session_store.active_session_mut() {
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
                &mut self.ui_store,
                "Cannot regenerate: no preceding user message",
                crate::ui::types::ToastLevel::Warn,
            );
            return;
        };

        let query = session.messages[user_idx].content.clone();
        // Remove the AI message and anything after it.
        session.messages.truncate(ai_idx);
        session.turn_heights.clear();
        // Re-submit.
        self.chat_store.input = query;
        self.send();
    }
}
