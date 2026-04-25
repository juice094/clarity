use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MessageData {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionData {
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<MessageData>,
}

impl SessionData {
    fn sessions_dir() -> PathBuf {
        let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("clarity");
        path.push("sessions");
        path
    }

    fn file_path(id: &str) -> PathBuf {
        let mut path = Self::sessions_dir();
        path.push(format!("{}.json", id));
        path
    }
}

#[tauri::command]
pub fn list_sessions() -> Vec<SessionData> {
    let dir = SessionData::sessions_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(session) = serde_json::from_str::<SessionData>(&content) {
                        sessions.push(session);
                    }
                }
            }
        }
    }

    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions
}

#[tauri::command]
pub fn load_session(id: String) -> Result<SessionData, String> {
    let path = SessionData::file_path(&id);
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_session(session: SessionData) -> Result<(), String> {
    let dir = SessionData::sessions_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let path = SessionData::file_path(&session.id);
    let content = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_session(id: String) -> Result<(), String> {
    let path = SessionData::file_path(&id);
    std::fs::remove_file(&path).map_err(|e| e.to_string())
}
