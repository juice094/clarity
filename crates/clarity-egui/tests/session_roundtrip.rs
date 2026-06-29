#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Integration test: session save → load roundtrip via the library target.
//!
//! Uses `clarity_egui::test_util` for isolated temp directories and
//! `clarity_egui::session` for session persistence — all accessed through
//! the library's `pub` API (no internal `pub(crate)` access).

use clarity_egui::session::{new_session, save_session_to_path};
use clarity_egui::test_util::with_temp_sessions_dir;
use clarity_egui::types::{ContentBlock, Message, Role, SessionContext, SessionLifecycle};

#[test]
fn integration_session_save_and_reread_json() {
    with_temp_sessions_dir("integration_roundtrip", |tmp| {
        let sessions_dir = tmp.join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        let mut session = new_session(0, SessionContext::Chat);
        session.id = "integration-sess-1".to_string();
        session.title = "Integration Test".to_string();
        session.category = "engineering".to_string();
        session.lifecycle = SessionLifecycle::ProjectBound;

        let mut msg = Message {
            role: Role::User,
            content: "Hello from integration test".to_string(),
            blocks: vec![ContentBlock::Text {
                text: "Hello from integration test".to_string(),
            }],
            timestamp: std::time::Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: false,
            lines: Vec::new(),
        };
        msg.prepare();
        session.messages.push(msg);

        let path = sessions_dir.join("integration-sess-1.json");
        save_session_to_path(&session, &path).expect("save_session_to_path should succeed");

        // Verify the file exists and contains valid JSON.
        let raw = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&raw).expect("saved session must be valid JSON");

        assert_eq!(parsed["id"], "integration-sess-1");
        assert_eq!(parsed["title"], "Integration Test");
        assert_eq!(parsed["messages"].as_array().unwrap().len(), 1);
        assert_eq!(parsed["messages"][0]["role"], "user");
        assert!(
            parsed["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("integration test")
        );
    });
}

#[test]
fn integration_session_contexts_persist() {
    with_temp_sessions_dir("integration_contexts", |tmp| {
        let sessions_dir = tmp.join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        // Work session with workspace_id
        let mut work_session = new_session(
            0,
            SessionContext::Work {
                workspace_id: Some("ws-integration".into()),
                has_workspace: true,
            },
        );
        work_session.id = "sess-work".to_string();
        add_text_message(&mut work_session, Role::User, "/work test");
        let path = sessions_dir.join("sess-work.json");
        save_session_to_path(&work_session, &path).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("Work"), "Work context should be serialized");
        assert!(
            raw.contains("ws-integration"),
            "workspace_id should be serialized"
        );

        // Claw session
        let claw_session = new_session(
            1,
            SessionContext::Claw {
                role: "coder".into(),
                session_key: "agent:main:coder".into(),
                affinity: clarity_egui::types::DeviceAffinity::AnyOnline,
            },
        );
        // NOTE: new_session clears id, so we set it manually.
        let mut claw_session = claw_session;
        claw_session.id = "sess-claw".to_string();
        add_text_message(&mut claw_session, Role::User, "claw test");
        let path = sessions_dir.join("sess-claw.json");
        save_session_to_path(&claw_session, &path).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("Claw"), "Claw context should be serialized");
        assert!(
            raw.contains("agent:main:coder"),
            "session_key should be serialized"
        );
    });
}

#[test]
fn integration_empty_session_deletes_file() {
    with_temp_sessions_dir("integration_empty", |tmp| {
        let sessions_dir = tmp.join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        let path = sessions_dir.join("should-be-deleted.json");
        std::fs::write(&path, "{}").unwrap();
        assert!(path.exists());

        let empty = new_session(0, SessionContext::Chat);
        // new_session creates a session with empty messages.
        let mut empty = empty;
        empty.id = "should-be-deleted".to_string();
        save_session_to_path(&empty, &path).unwrap();

        assert!(!path.exists(), "empty session file should be deleted");
    });
}

#[test]
fn integration_atomic_write_renames_tmp() {
    with_temp_sessions_dir("integration_atomic", |tmp| {
        let sessions_dir = tmp.join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        let mut session = new_session(0, SessionContext::Chat);
        session.id = "sess-atomic-int".to_string();
        add_text_message(&mut session, Role::User, "atomic");

        let path = sessions_dir.join("sess-atomic-int.json");
        let tmp_path = sessions_dir.join("sess-atomic-int.json.tmp");

        assert!(!tmp_path.exists());
        save_session_to_path(&session, &path).unwrap();

        assert!(path.exists());
        assert!(!tmp_path.exists(), "tmp file should be gone after rename");
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("atomic"));
    });
}

// ── Helpers ──

fn add_text_message(session: &mut clarity_egui::types::Session, role: Role, content: &str) {
    let mut msg = Message {
        role,
        content: content.to_string(),
        blocks: vec![ContentBlock::Text {
            text: content.to_string(),
        }],
        timestamp: std::time::Instant::now(),
        parsed: vec![],
        cached_height: None,
        is_error: false,
        lines: Vec::new(),
    };
    msg.prepare();
    session.messages.push(msg);
}
