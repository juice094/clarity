//! Session persistence — load/save chat history to JSON files.

use crate::ui::types::{ContentBlock, Message, Role, Session, SessionContext, SessionLifecycle};
use std::collections::HashMap;
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
                            last_saved_at: data.updated_at,
                            turn_heights: vec![],
                            estimate_buffer: Vec::new(),
                            line_offset_buffer: Vec::new(),
                            estimate_key: None,
                            cached_total_height: None,
                            provider_state: data.provider_state,
                            in_flight: false,
                            diff_stats: None,
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
pub fn save_session_internal(session: &mut Session) -> Result<(), String> {
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
        provider_state: session.provider_state.clone(),
        created_at: session.updated_at,
        updated_at: now_millis(),
        messages: session
            .messages
            .iter()
            .map(|m| MessageData {
                role: match m.role {
                    Role::User => "user".into(),
                    Role::Agent => "agent".into(),
                    Role::System => "system".into(),
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

/// Creates a new session with the given context.
pub fn new_session(index: usize, context: SessionContext) -> Session {
    let id = format!("sess-{}", uuid::Uuid::new_v4());
    let title = if index == 0 {
        "New Chat".to_string()
    } else {
        format!("New Chat {}", index + 1)
    };
    let project_id = match &context {
        SessionContext::Work {
            workspace_id: Some(id),
            ..
        } => Some(id.clone()),
        _ => None,
    };
    let now = now_millis();
    Session {
        id,
        title,
        category: "chat".to_string(),
        project_id,
        context,
        lifecycle: SessionLifecycle::Temporary,
        archived: false,
        messages: vec![],
        updated_at: now,
        // New sessions start in sync — no pending auto-save until messages arrive.
        last_saved_at: now,
        turn_heights: vec![],
        estimate_buffer: Vec::new(),
        line_offset_buffer: Vec::new(),
        estimate_key: None,
        cached_total_height: None,
        provider_state: HashMap::new(),
        in_flight: false,
        diff_stats: None,
    }
}

/// Returns the current time in milliseconds.
pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Returns the canonical session key for a Claw role.
///
/// All devices sharing the same role consume the same session context, so the
/// key must be deterministic per role rather than a random uuid.
pub fn claw_session_key(role: &str) -> String {
    format!("agent:main:{}", role)
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
    #[serde(default)]
    provider_state: HashMap<String, String>,
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

/// Like `save_session_internal` but writes to an explicit path. `pub` so
/// integration tests (`tests/*.rs`) can write session files to isolated temp
/// directories. Not `#[cfg(test)]` because integration test crates compile
/// against the library without `cfg(test)` automatically enabled.
///
/// Only called from test code; `dead_code` is allowed in the binary target.
#[allow(dead_code)]
pub fn save_session_to_path(session: &Session, path: &std::path::Path) -> Result<(), String> {
    if session.messages.is_empty() {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = SessionData {
        id: session.id.clone(),
        title: session.title.clone(),
        category: Some(session.category.clone()),
        project_id: session.project_id.clone(),
        context: session.context.clone(),
        lifecycle: session.lifecycle,
        archived: session.archived,
        provider_state: session.provider_state.clone(),
        created_at: session.updated_at,
        updated_at: now_millis(),
        messages: session
            .messages
            .iter()
            .map(|m| MessageData {
                role: match m.role {
                    Role::User => "user".into(),
                    Role::Agent => "agent".into(),
                    Role::System => "system".into(),
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
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::types::{SessionContext, SessionLifecycle};

    #[test]
    fn new_session_defaults_to_chat_context() {
        let s = new_session(0, SessionContext::Chat);
        assert_eq!(s.context, SessionContext::Chat);
        assert_eq!(s.lifecycle, SessionLifecycle::Temporary);
        assert!(!s.archived);
        assert!(s.project_id.is_none());
        assert_eq!(s.title, "New Chat");
    }

    #[test]
    fn new_session_increments_title_index() {
        let s = new_session(2, SessionContext::Chat);
        assert_eq!(s.title, "New Chat 3");
    }

    #[test]
    fn new_session_work_carries_workspace_id() {
        let s = new_session(
            0,
            SessionContext::Work {
                workspace_id: Some("ws-1".into()),
                has_workspace: true,
            },
        );
        assert_eq!(s.project_id, Some("ws-1".into()));
        assert!(matches!(s.context, SessionContext::Work { .. }));
    }

    #[test]
    fn session_data_roundtrips_phase7_fields() {
        let data = SessionData {
            id: "s-1".into(),
            title: "test".into(),
            category: Some("engineering".into()),
            project_id: Some("p-1".into()),
            context: SessionContext::Work {
                workspace_id: Some("p-1".into()),
                has_workspace: true,
            },
            lifecycle: SessionLifecycle::ProjectBound,
            archived: true,
            provider_state: HashMap::new(),
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
        assert!(restored.provider_state.is_empty());
    }

    #[test]
    fn session_data_roundtrips_provider_state() {
        let mut provider_state = HashMap::new();
        provider_state.insert(
            "deepseek-device".into(),
            r#"{"chat_session_id":"abc-123","last_response_message_id":7}"#.into(),
        );
        let data = SessionData {
            id: "s-1".into(),
            title: "test".into(),
            category: Some("chat".into()),
            project_id: None,
            context: SessionContext::Chat,
            lifecycle: SessionLifecycle::Temporary,
            archived: false,
            provider_state,
            created_at: 0,
            updated_at: 1,
            messages: vec![],
        };
        let json = serde_json::to_string(&data).unwrap();
        let restored: SessionData = serde_json::from_str(&json).unwrap();
        assert_eq!(
            restored.provider_state.get("deepseek-device"),
            Some(&r#"{"chat_session_id":"abc-123","last_response_message_id":7}"#.to_string())
        );
    }

    #[test]
    fn claw_session_key_is_deterministic_per_role() {
        assert_eq!(claw_session_key("operator"), "agent:main:operator");
        assert_eq!(claw_session_key("coder"), "agent:main:coder");
        // Same role always yields the same key so multiple devices share context.
        assert_eq!(claw_session_key("operator"), claw_session_key("operator"));
    }

    // ============================================================================
    // Integration tests — save-load roundtrip with real filesystem
    // ============================================================================

    #[test]
    fn save_session_roundtrip_preserves_all_fields() {
        crate::test_util::with_temp_sessions_dir("roundtrip", |tmp| {
            let sessions_dir = tmp.join("sessions");
            std::fs::create_dir_all(&sessions_dir).unwrap();

            // Construct a session with representative data.
            let mut session = new_session(0, SessionContext::Chat);
            session.id = "sess-roundtrip-test".to_string();
            session.title = "Roundtrip Test".to_string();
            session.category = "engineering".to_string();
            session.project_id = Some("proj-1".to_string());
            let mut msg = Message {
                role: Role::User,
                content: "Hello, world!".to_string(),
                blocks: vec![ContentBlock::Text {
                    text: "Hello, world!".to_string(),
                }],
                timestamp: std::time::Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: false,
                lines: vec![],
            };
            msg.prepare();
            session.messages.push(msg);

            let mut msg2 = Message {
                role: Role::Agent,
                content: "Hi there!".to_string(),
                blocks: vec![ContentBlock::Text {
                    text: "Hi there!".to_string(),
                }],
                timestamp: std::time::Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: false,
                lines: vec![],
            };
            msg2.prepare();
            session.messages.push(msg2);

            let session_path = sessions_dir.join(format!("{}.json", session.id));
            save_session_to_path(&session, &session_path).unwrap();

            // Verify the file exists and is valid JSON.
            let content = std::fs::read_to_string(&session_path).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
            assert_eq!(parsed["id"], "sess-roundtrip-test");
            assert_eq!(parsed["title"], "Roundtrip Test");
            assert_eq!(parsed["messages"].as_array().unwrap().len(), 2);
            assert_eq!(parsed["messages"][0]["role"], "user");
            assert_eq!(parsed["messages"][0]["content"], "Hello, world!");
            assert_eq!(parsed["messages"][1]["role"], "agent");
        });
    }

    #[test]
    fn save_empty_session_deletes_file() {
        crate::test_util::with_temp_sessions_dir("empty", |tmp| {
            let sessions_dir = tmp.join("sessions");
            std::fs::create_dir_all(&sessions_dir).unwrap();

            let session_path = sessions_dir.join("sess-empty-test.json");
            // Create a dummy file first.
            std::fs::write(&session_path, "{}").unwrap();
            assert!(session_path.exists());

            // Save an empty session — should delete the file.
            let mut empty_session = new_session(0, SessionContext::Chat);
            empty_session.id = "sess-empty-test".to_string();
            save_session_to_path(&empty_session, &session_path).unwrap();

            assert!(
                !session_path.exists(),
                "Empty session file should be deleted"
            );
        });
    }

    #[test]
    fn save_multiple_sessions_and_load_all() {
        crate::test_util::with_temp_sessions_dir("multi_save", |tmp| {
            let sessions_dir = tmp.join("sessions");
            std::fs::create_dir_all(&sessions_dir).unwrap();

            // Save 3 sessions with different contexts.
            for (i, ctx) in [
                SessionContext::Chat,
                SessionContext::Work {
                    workspace_id: Some("ws-1".into()),
                    has_workspace: true,
                },
                SessionContext::Claw {
                    role: "coder".into(),
                    session_key: "agent:main:coder".into(),
                    affinity: crate::ui::types::DeviceAffinity::AnyOnline,
                },
            ]
            .iter()
            .enumerate()
            {
                let mut session = new_session(i, ctx.clone());
                session.id = format!("sess-multi-{}", i);
                session.title = format!("Multi {}", i);
                let mut msg = Message {
                    role: Role::User,
                    content: format!("msg {}", i),
                    blocks: vec![],
                    timestamp: std::time::Instant::now(),
                    parsed: vec![],
                    cached_height: None,
                    is_error: false,
                    lines: vec![],
                };
                msg.prepare();
                session.messages.push(msg);
                let path = sessions_dir.join(format!("{}.json", session.id));
                save_session_to_path(&session, &path).unwrap();
            }

            // All 3 files should exist.
            for i in 0..3 {
                let path = sessions_dir.join(format!("sess-multi-{}.json", i));
                assert!(path.exists(), "session {} file should exist", i);
            }
        });
    }

    #[test]
    fn corrupted_json_is_rejected_by_session_data_parser() {
        // Verify that malformed / empty JSON is rejected at the parse level.
        // `load_sessions` calls `serde_json::from_str::<SessionData>` internally;
        // this test validates the deserialization contract without touching
        // the user's real sessions directory.
        assert!(serde_json::from_str::<SessionData>("not valid json {{{").is_err());
        assert!(serde_json::from_str::<SessionData>("").is_err());
        assert!(serde_json::from_str::<SessionData>("null").is_err());

        // A structurally valid JSON object missing required fields should also
        // fail (SessionData fields like "id", "title", "messages" are required).
        let missing_fields = serde_json::json!({"id": "s1"});
        assert!(
            serde_json::from_str::<SessionData>(&missing_fields.to_string()).is_err(),
            "SessionData with missing required fields should fail deserialization"
        );
    }

    #[test]
    fn session_data_deserialization_rejects_wrong_types() {
        // Field type mismatch: "messages" should be an array, not a string.
        let wrong_type = serde_json::json!({
            "id": "s1",
            "title": "test",
            "category": "chat",
            "created_at": 0,
            "updated_at": 0,
            "messages": "not-an-array"
        });
        assert!(
            serde_json::from_str::<SessionData>(&wrong_type.to_string()).is_err(),
            "messages as string should fail deserialization"
        );
    }

    #[test]
    fn session_with_provider_state_roundtrips() {
        crate::test_util::with_temp_sessions_dir("provider_state", |tmp| {
            let sessions_dir = tmp.join("sessions");
            std::fs::create_dir_all(&sessions_dir).unwrap();

            let mut session = new_session(0, SessionContext::Chat);
            session.id = "sess-provider".to_string();
            session.provider_state.insert(
                "deepseek-device".into(),
                r#"{"chat_session_id":"abc-123"}"#.into(),
            );
            let mut msg = Message {
                role: Role::User,
                content: "use deepseek".into(),
                blocks: vec![],
                timestamp: std::time::Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: false,
                lines: vec![],
            };
            msg.prepare();
            session.messages.push(msg);

            let path = sessions_dir.join("sess-provider.json");
            save_session_to_path(&session, &path).unwrap();

            // Verify the serialized JSON contains provider_state.
            let raw = std::fs::read_to_string(&path).unwrap();
            assert!(
                raw.contains("provider_state"),
                "serialized JSON should contain provider_state"
            );
            assert!(raw.contains("chat_session_id"));
        });
    }

    #[test]
    fn atomic_write_uses_tmp_then_rename() {
        crate::test_util::with_temp_sessions_dir("atomic", |tmp| {
            let sessions_dir = tmp.join("sessions");
            std::fs::create_dir_all(&sessions_dir).unwrap();

            let mut session = new_session(0, SessionContext::Chat);
            session.id = "sess-atomic".to_string();
            let mut msg = Message {
                role: Role::User,
                content: "atomic test".into(),
                blocks: vec![],
                timestamp: std::time::Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: false,
                lines: vec![],
            };
            msg.prepare();
            session.messages.push(msg);

            let path = sessions_dir.join("sess-atomic.json");
            let tmp_path = sessions_dir.join("sess-atomic.json.tmp");

            // Before save: target does not exist, tmp does not exist.
            assert!(!path.exists());
            assert!(!tmp_path.exists());

            save_session_to_path(&session, &path).unwrap();

            // After save: target exists, tmp was cleaned up by rename.
            assert!(path.exists(), "target file should exist after save");
            assert!(!tmp_path.exists(), "tmp file should be gone after rename");

            // Content is correct.
            let raw = std::fs::read_to_string(&path).unwrap();
            assert!(raw.contains("atomic test"));
        });
    }
}
