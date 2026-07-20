//! Session / Thread event handlers.
//!
//! These handlers centralise mutations to `SessionStore` so that both local UI
//! actions and future backend `WireMessage::Thread*` events converge on a
//! single event-driven path.

use crate::stores::SessionStore;
use crate::ui::types::Session;

/// Activate the thread with the given id.
///
/// If the thread is not present in the store, this is a no-op. The caller is
/// responsible for ensuring the thread exists (or creating it first).
pub fn on_thread_active(session_store: &mut SessionStore, thread_id: String) {
    if session_store.sessions.iter().any(|s| s.id == thread_id) {
        session_store.active_session_id = thread_id;
    }
}

/// Replace the in-memory session list.
pub fn on_thread_list(session_store: &mut SessionStore, threads: Vec<Session>) {
    if threads.is_empty() {
        return;
    }
    session_store.sessions = threads;
    // Ensure the active session id still points to a valid session.
    if !session_store
        .sessions
        .iter()
        .any(|s| s.id == session_store.active_session_id)
    {
        session_store.active_session_id = session_store
            .sessions
            .first()
            .map(|s| s.id.clone())
            .unwrap_or_default();
    }
}

/// Insert a newly created session and make it active.
pub fn on_thread_created(session_store: &mut SessionStore, session: Session) {
    let id = session.id.clone();
    session_store.sessions.push(session);
    session_store.active_session_id = id;
}

/// Update thread metadata.
pub fn on_thread_updated(
    session_store: &mut SessionStore,
    thread_id: String,
    title: Option<String>,
    archived: Option<bool>,
) {
    if let Some(session) = session_store
        .sessions
        .iter_mut()
        .find(|s| s.id == thread_id)
    {
        if let Some(title) = title {
            session.title = title;
        }
        if let Some(archived) = archived {
            session.archived = archived;
        }
        session.updated_at = crate::session::now_millis();
    }
}

/// Delete a thread.
pub fn on_thread_deleted(session_store: &mut SessionStore, thread_id: String) {
    session_store.sessions.retain(|s| s.id != thread_id);
    session_store.drafts.remove(&thread_id);

    if session_store.sessions.is_empty() {
        return;
    }
    if session_store.active_session_id == thread_id {
        session_store.active_session_id = session_store
            .sessions
            .first()
            .map(|s| s.id.clone())
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::now_millis;
    use crate::ui::types::{SessionContext, SessionLifecycle};
    use std::collections::HashMap;

    fn make_session(id: &str, title: &str, category: &str) -> Session {
        Session {
            id: id.to_string(),
            title: title.to_string(),
            category: category.to_string(),
            project_id: None,
            context: SessionContext::default(),
            lifecycle: SessionLifecycle::default(),
            archived: false,
            messages: Vec::new(),
            updated_at: now_millis(),
            last_saved_at: now_millis(),
            turn_heights: Vec::new(),
            estimate_buffer: Vec::new(),
            line_offset_buffer: Vec::new(),
            estimate_key: None,
            cached_total_height: None,
            provider_state: HashMap::new(),
            in_flight: false,
            diff_stats: None,
        }
    }

    fn make_store(sessions: Vec<Session>, active: &str) -> SessionStore {
        SessionStore {
            sessions,
            active_session_id: active.to_string(),
            drafts: HashMap::new(),
            turn_cache: HashMap::new(),
        }
    }

    #[test]
    fn thread_created_inserts_and_activates() {
        let mut store = make_store(vec![], "");
        let s = make_session("s1", "First", "engineering");
        on_thread_created(&mut store, s);
        assert_eq!(store.sessions.len(), 1);
        assert_eq!(store.active_session_id, "s1");
    }

    #[test]
    fn thread_active_ignores_unknown_id() {
        let mut store = make_store(vec![make_session("s1", "A", "engineering")], "s1");
        on_thread_active(&mut store, "unknown".to_string());
        assert_eq!(store.active_session_id, "s1");
    }

    #[test]
    fn thread_active_switches_when_present() {
        let mut store = make_store(
            vec![
                make_session("s1", "A", "engineering"),
                make_session("s2", "B", "engineering"),
            ],
            "s1",
        );
        on_thread_active(&mut store, "s2".to_string());
        assert_eq!(store.active_session_id, "s2");
    }

    #[test]
    fn thread_updated_changes_title_and_archived() {
        let mut store = make_store(vec![make_session("s1", "Old", "engineering")], "s1");
        on_thread_updated(
            &mut store,
            "s1".to_string(),
            Some("New".to_string()),
            Some(true),
        );
        let s = store.sessions.iter().find(|s| s.id == "s1").unwrap();
        assert_eq!(s.title, "New");
        assert!(s.archived);
    }

    #[test]
    fn thread_deleted_removes_and_falls_back() {
        let mut store = make_store(
            vec![
                make_session("s1", "A", "engineering"),
                make_session("s2", "B", "engineering"),
            ],
            "s1",
        );
        store.drafts.insert("s1".to_string(), "draft".to_string());
        on_thread_deleted(&mut store, "s1".to_string());
        assert_eq!(store.sessions.len(), 1);
        assert!(!store.drafts.contains_key("s1"));
        assert_eq!(store.active_session_id, "s2");
    }

    #[test]
    fn thread_list_replaces_sessions() {
        let mut store = make_store(vec![make_session("s1", "A", "engineering")], "s1");
        let list = vec![make_session("s2", "B", "engineering")];
        on_thread_list(&mut store, list);
        assert_eq!(store.sessions.len(), 1);
        assert_eq!(store.sessions[0].id, "s2");
        assert_eq!(store.active_session_id, "s2");
    }
}
