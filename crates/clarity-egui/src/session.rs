//! Session persistence — load/save chat history to JSON files.

use crate::ui::types::{Message, Role, Session};
use std::path::PathBuf;
use std::time::Instant;

pub fn sessions_dir() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("clarity");
    path.push("sessions");
    path
}

pub fn session_path(id: &str) -> PathBuf {
    let mut path = sessions_dir();
    path.push(format!("{}.json", id));
    path
}

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
                        sessions.push(Session {
                            id: data.id,
                            title: data.title,
                            messages: data.messages.into_iter().map(|m| {
                                let mut msg = Message {
                                    role: if m.role == "user" { Role::User } else { Role::Agent },
                                    content: m.content,
                                    timestamp: Instant::now(),
                                    parsed: vec![],
                                    cached_height: None,
                                    is_error: false,
                                };
                                msg.prepare();
                                msg
                            }).collect(),
                            updated_at: data.updated_at,
                        });
                    }
                }
            }
        }
    }
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions
}

pub fn save_session_internal(session: &Session) -> Result<(), String> {
    let dir = sessions_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = session_path(&session.id);
    let data = SessionData {
        id: session.id.clone(),
        title: session.title.clone(),
        created_at: session.updated_at,
        updated_at: now_millis(),
        messages: session.messages.iter().map(|m| MessageData {
            role: match m.role {
                Role::User => "user".into(),
                Role::Agent => "agent".into(),
            },
            content: m.content.clone(),
        }).collect(),
    };
    let content = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

pub fn new_session() -> Session {
    let id = format!("sess-{}", uuid::Uuid::new_v4());
    Session {
        id: id.clone(),
        title: "New Chat".into(),
        messages: vec![],
        updated_at: now_millis(),
    }
}

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
    created_at: u64,
    updated_at: u64,
    messages: Vec<MessageData>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MessageData {
    role: String,
    content: String,
}
