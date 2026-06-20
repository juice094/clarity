//! Session persistence — load/save chat history to JSON files.

use crate::ui::types::{ContentBlock, Message, Role, Session, SessionContext, SessionLifecycle};
use std::path::PathBuf;
use std::time::Instant;

/// Sessions dir.
pub fn sessions_dir() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("clarity");
    path.push("sessions");
    path
}

/// Session path.
pub fn session_path(id: &str) -> PathBuf {
    let mut path = sessions_dir();
    path.push(format!("{}.json", id));
    path
}

/// Loads sessions from disk.
pub fn load_sessions() -> Vec<Session> {
    let dir = sessions_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(data) = serde_json::from_str::<SessionData>(&content) {
                        let messages: Vec<Message> = data
                            .messages
                            .into_iter()
                            .map(|m| {
                                let mut msg = Message {
                                    role: if m.role == "user" {
                                        Role::User
                                    } else {
                                        Role::Agent
                                    },
                                    content: m.content,
                                    blocks: m.blocks.unwrap_or_default(),
                                    timestamp: Instant::now(),
                                    parsed: vec![],
                                    cached_height: None,
                                    is_error: false,
                                    lines: Vec::new(),
                                };
                                msg.prepare();
                                msg
                            })
                            .collect();
                        // Empty sessions are transient — don't load them and clean up
                        // the orphaned file so they never clutter the tab bar.
                        if messages.is_empty() {
                            let _ = std::fs::remove_file(&path);
                            continue;
                        }
                        sessions.push(Session {
                            id: data.id,
                            title: data.title,
                            category: data.category.unwrap_or_else(|| "chat".to_string()),
                            project_id: data.project_id,
                            context: data.context,
                            lifecycle: data.lifecycle,
                            archived: data.archived,
                            messages,
                            updated_at: data.updated_at,
                            turn_heights: vec![],
                        });
                    }
                }
            }
        }
    }
    sessions.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
    sessions
}

/// Persists session internal to disk.
pub fn save_session_internal(session: &Session) -> Result<(), String> {
    let path = session_path(&session.id);
    // Empty sessions are transient — don't write them to disk.
    // If a previously-non-empty session became empty, delete its file.
    if session.messages.is_empty() {
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        return Ok(());
    }
    let dir = sessions_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let data = SessionData {
        id: session.id.clone(),
        title: session.title.clone(),
        category: Some(session.category.clone()),
        project_id: session.project_id.clone(),
        context: session.context.clone(),
        lifecycle: session.lifecycle,
        archived: session.archived,
        created_at: session.updated_at,
        updated_at: now_millis(),
        messages: session
            .messages
            .iter()
            .map(|m| MessageData {
                role: match m.role {
                    Role::User => "user".into(),
                    Role::Agent => "agent".into(),
                },
                content: m.content.clone(),
                blocks: if m.blocks.is_empty() {
                    None
                } else {
                    Some(m.blocks.clone())
                },
            })
            .collect(),
    };
    let content = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    // Write to a temp file and rename into place so a crash during write does
    // not leave a half-written session file.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Creates a new plain chat session.
pub fn new_session(index: usize) -> Session {
    let id = format!("sess-{}", uuid::Uuid::new_v4());
    let title = if index == 0 {
        "New Chat".to_string()
    } else {
        format!("New Chat {}", index + 1)
    };
    Session {
        id: id.clone(),
        title,
        category: "chat".to_string(),
        project_id: None,
        context: SessionContext::Chat,
        lifecycle: SessionLifecycle::Temporary,
        archived: false,
        messages: vec![],
        updated_at: now_millis(),
        turn_heights: vec![],
    }
}

/// Returns the current time in milliseconds.
pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SessionData {
    id: String,
    title: String,
    category: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    context: SessionContext,
    #[serde(default)]
    lifecycle: SessionLifecycle,
    #[serde(default)]
    archived: bool,
    created_at: u64,
    updated_at: u64,
    messages: Vec<MessageData>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MessageData {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocks: Option<Vec<ContentBlock>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::types::{SessionContext, SessionLifecycle};

    #[test]
    fn new_session_defaults_to_chat_context() {
        let s = new_session(0);
        assert_eq!(s.context, SessionContext::Chat);
        assert_eq!(s.lifecycle, SessionLifecycle::Temporary);
        assert!(!s.archived);
        assert!(s.project_id.is_none());
        assert_eq!(s.title, "New Chat");
    }

    #[test]
    fn new_session_increments_title_index() {
        let s = new_session(2);
        assert_eq!(s.title, "New Chat 3");
    }

    #[test]
    fn session_data_roundtrips_phase7_fields() {
        let data = SessionData {
            id: "s-1".into(),
            title: "test".into(),
            category: Some("engineering".into()),
            project_id: Some("p-1".into()),
            context: SessionContext::Project {
                project_id: "p-1".into(),
                has_workspace: true,
            },
            lifecycle: SessionLifecycle::ProjectBound,
            archived: true,
            created_at: 0,
            updated_at: 1,
            messages: vec![],
        };
        let json = serde_json::to_string(&data).unwrap();
        let restored: SessionData = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.project_id, Some("p-1".into()));
        assert_eq!(restored.lifecycle, SessionLifecycle::ProjectBound);
        assert!(restored.archived);
    }

    #[test]
    fn session_data_defaults_missing_phase7_fields() {
        let json = r#"{"id":"s-legacy","title":"legacy","category":"engineering","created_at":0,"updated_at":1,"messages":[]}"#;
        let restored: SessionData = serde_json::from_str(json).unwrap();
        assert!(restored.project_id.is_none());
        assert_eq!(restored.context, SessionContext::Chat);
        assert_eq!(restored.lifecycle, SessionLifecycle::Temporary);
        assert!(!restored.archived);
    }
}
