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

    fn try_load(path: &PathBuf) -> Option<SessionData> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str::<SessionData>(&content).ok()
    }

    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(session) = try_load(&path) {
                    sessions.push(session);
                } else {
                    // Try backup if main file is corrupted
                    let mut bak = path.clone();
                    bak.set_extension("json.bak");
                    if let Some(session) = try_load(&bak) {
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
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<SessionData>(&content) {
            Ok(session) => Ok(session),
            Err(parse_err) => {
                // Main file corrupted — try backup
                let mut bak = path.clone();
                bak.set_extension("json.bak");
                let bak_content = std::fs::read_to_string(&bak)
                    .map_err(|e| format!("Session corrupted and no backup: {} (backup err: {})", parse_err, e))?;
                serde_json::from_str(&bak_content)
                    .map_err(|e| format!("Session corrupted and backup unreadable: {}", e))
            }
        },
        Err(io_err) => {
            // Main file missing — try backup
            let mut bak = path.clone();
            bak.set_extension("json.bak");
            let bak_content = std::fs::read_to_string(&bak)
                .map_err(|e| format!("Session not found and no backup: {} (backup err: {})", io_err, e))?;
            serde_json::from_str(&bak_content)
                .map_err(|e| format!("Session not found and backup unreadable: {}", e))
        }
    }
}

#[tauri::command]
pub fn save_session(session: SessionData) -> Result<(), String> {
    let dir = SessionData::sessions_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let path = SessionData::file_path(&session.id);
    let tmp_path = path.with_extension("json.tmp");
    let bak_path = path.with_extension("json.bak");

    let content = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    std::fs::write(&tmp_path, content).map_err(|e| e.to_string())?;

    // If existing session exists, promote it to backup before overwriting
    if path.exists() {
        if let Err(e) = std::fs::copy(&path, &bak_path) {
            tracing::warn!("Failed to create session backup: {}", e);
        }
    }

    std::fs::rename(&tmp_path, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_session(id: String) -> Result<(), String> {
    let path = SessionData::file_path(&id);
    std::fs::remove_file(&path).map_err(|e| e.to_string())
}
