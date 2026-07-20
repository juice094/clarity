//! Unified SQLite-backed session storage (V2).
//!
//! This module provides `SessionStoreV2` and related types for append-only
//! event logs, compacted contexts, rollout tracking, and identity management.

pub mod compaction;
pub mod event_log;
pub mod identity;
pub mod rollout;
pub mod schema;
pub mod session;

pub(crate) mod payload_hash;

pub use compaction::CompactedContext;
pub use event_log::{EventRecord, EventType};
pub use identity::DeviceIdentityRecord;
pub use session::{SessionState, SessionStoreV2, SessionV2};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_store_v2::session::SessionStoreV2;

    fn temp_store() -> SessionStoreV2 {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        SessionStoreV2::new(tmp.path()).unwrap()
    }

    #[test]
    fn test_create_and_get_session() {
        let store = temp_store();
        store
            .create_session(
                "sess-1",
                Some("Test Session"),
                Some("soul-a"),
                Some("abc123"),
            )
            .unwrap();

        let sess = store.get_session("sess-1").unwrap().unwrap();
        assert_eq!(sess.id, "sess-1");
        assert_eq!(sess.title, Some("Test Session".to_string()));
        assert_eq!(sess.soul_id, Some("soul-a".to_string()));
        assert_eq!(sess.state, SessionState::Active);
    }

    #[test]
    fn test_event_log_append_and_read() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        let payload = serde_json::json!({"role": "user", "content": "hello"});
        store
            .append_event("sess-1", 1, EventType::UserMessage, &payload)
            .unwrap();
        store
            .append_event(
                "sess-1",
                1,
                EventType::AssistantMessage,
                &serde_json::json!({"content": "hi"}),
            )
            .unwrap();
        store
            .append_event(
                "sess-1",
                2,
                EventType::UserMessage,
                &serde_json::json!({"content": "world"}),
            )
            .unwrap();

        let events = store.read_events("sess-1").unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, EventType::UserMessage);
        assert_eq!(events[0].event_id, 1);
        assert_eq!(events[1].event_id, 2);
        assert_eq!(events[2].turn_id, 2);
    }

    #[test]
    fn test_read_events_until() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        for i in 1..=5 {
            store
                .append_event(
                    "sess-1",
                    i,
                    EventType::UserMessage,
                    &serde_json::json!({"i": i}),
                )
                .unwrap();
        }

        // turn_id 3 has event_id 3 (auto-assigned by append_event).
        let events = store.read_events_until("sess-1", 3, 3).unwrap();
        assert_eq!(events.len(), 3); // turns 1,2,3
    }

    #[test]
    fn test_compacted_context_roundtrip() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        let context = serde_json::json!([{"role": "system", "content": "compacted"}]);
        store
            .store_compacted_context("sess-1", 5, 12, &context, "tier2", "hash-abc")
            .unwrap();

        let loaded = store.load_compacted_context("sess-1").unwrap().unwrap();
        assert_eq!(loaded.turn_id, 5);
        assert_eq!(loaded.event_id, 12);
        assert_eq!(loaded.compression_method, "tier2");
        assert_eq!(loaded.context_json, context);
    }

    #[test]
    fn test_session_state_transition() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        store
            .set_session_state("sess-1", SessionState::Compacting)
            .unwrap();
        let sess = store.get_session("sess-1").unwrap().unwrap();
        assert_eq!(sess.state, SessionState::Compacting);
    }

    #[test]
    fn test_parent_session_lineage() {
        let store = temp_store();
        store.create_session("parent", None, None, None).unwrap();
        store.create_session("child", None, None, None).unwrap();
        store.set_parent("child", "parent").unwrap();

        let child = store.get_session("child").unwrap().unwrap();
        assert_eq!(child.parent_session_id, Some("parent".to_string()));
    }

    #[test]
    fn test_delete_session_cascades() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();
        store
            .append_event("sess-1", 1, EventType::UserMessage, &serde_json::json!({}))
            .unwrap();
        store
            .store_compacted_context("sess-1", 1, 1, &serde_json::json!([]), "tier1", "h")
            .unwrap();

        store.delete_session("sess-1").unwrap();
        assert!(store.get_session("sess-1").unwrap().is_none());
        assert!(store.read_events("sess-1").unwrap().is_empty());
        assert!(store.load_compacted_context("sess-1").unwrap().is_none());
    }

    #[test]
    fn test_rollout_index_roundtrip() {
        let store = temp_store();
        store.create_session("thread-1", None, None, None).unwrap();

        let path = std::path::PathBuf::from("/tmp/rollouts/thread-1.jsonl");
        store.register_rollout("thread-1", &path, 42).unwrap();

        let (got_path, seq) = store.get_rollout("thread-1").unwrap().unwrap();
        assert_eq!(got_path, path);
        assert_eq!(seq, 42);

        store.update_rollout_seq("thread-1", 99).unwrap();
        let (_, seq) = store.get_rollout("thread-1").unwrap().unwrap();
        assert_eq!(seq, 99);

        store.delete_session("thread-1").unwrap();
        assert!(store.get_rollout("thread-1").unwrap().is_none());
    }

    // ------------------------------------------------------------------
    // Identity CRUD tests
    // ------------------------------------------------------------------

    fn make_user(id: &str, name: &str, provider: &str) -> clarity_contract::User {
        clarity_contract::User {
            id: id.into(),
            display_name: name.into(),
            avatar_url: None,
            email: None,
            provider: provider.into(),
            provider_user_id: None,
            created_at: 1700000000,
            updated_at: 1700000001,
        }
    }

    #[test]
    fn test_user_upsert_get_delete() {
        let store = temp_store();
        let user = make_user("u-1", "Alice", "local");
        store.upsert_user(&user).unwrap();

        let got = store.get_user("u-1").unwrap().unwrap();
        assert_eq!(got.id, "u-1");
        assert_eq!(got.display_name, "Alice");
        assert_eq!(got.provider, "local");

        store.delete_user("u-1").unwrap();
        assert!(store.get_user("u-1").unwrap().is_none());
    }

    #[test]
    fn test_user_by_provider_lookup() {
        let store = temp_store();
        let mut user = make_user("u-2", "Bob", "wechat");
        user.provider_user_id = Some("wx-openid-123".into());
        store.upsert_user(&user).unwrap();

        let got = store
            .get_user_by_provider("wechat", "wx-openid-123")
            .unwrap()
            .unwrap();
        assert_eq!(got.id, "u-2");

        // Missing provider/user combo returns None.
        assert!(
            store
                .get_user_by_provider("github", "nobody")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_team_upsert_get_list() {
        let store = temp_store();
        let team = clarity_contract::Team {
            id: "t-1".into(),
            org_id: "o-1".into(),
            name: "Engineering".into(),
            description: None,
            created_at: 1700000000,
        };
        store.upsert_team(&team).unwrap();

        let got = store.get_team("t-1").unwrap().unwrap();
        assert_eq!(got.name, "Engineering");

        let all = store.list_teams_for_org("o-1").unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_team_member_add_list() {
        let store = temp_store();
        let user = make_user("u-3", "Carol", "local");
        store.upsert_user(&user).unwrap();
        let team = clarity_contract::Team {
            id: "t-2".into(),
            org_id: "o-1".into(),
            name: "Design".into(),
            description: None,
            created_at: 1700000000,
        };
        store.upsert_team(&team).unwrap();

        let member = clarity_contract::TeamMember {
            user_id: "u-3".into(),
            team_id: "t-2".into(),
            role: clarity_contract::TeamRole::Admin,
            joined_at: 1700000000,
        };
        store.add_team_member(&member).unwrap();

        let members = store.list_team_members("t-2").unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_id, "u-3");
        assert_eq!(members[0].role, clarity_contract::TeamRole::Admin);

        let role = store.get_team_member_role("u-3", "t-2").unwrap().unwrap();
        assert_eq!(role, clarity_contract::TeamRole::Admin);
    }

    #[test]
    fn test_org_upsert_get() {
        let store = temp_store();
        let org = clarity_contract::Organization {
            id: "o-1".into(),
            name: "Acme Corp".into(),
            description: Some("Enterprise".into()),
            created_at: 1700000000,
        };
        store.upsert_org(&org).unwrap();

        let got = store.get_org("o-1").unwrap().unwrap();
        assert_eq!(got.name, "Acme Corp");
    }

    #[test]
    fn test_org_member_add_list() {
        let store = temp_store();
        let user = make_user("u-4", "Dave", "local");
        store.upsert_user(&user).unwrap();
        let org = clarity_contract::Organization {
            id: "o-2".into(),
            name: "Startup Inc".into(),
            description: None,
            created_at: 1700000000,
        };
        store.upsert_org(&org).unwrap();

        let member = clarity_contract::OrgMember {
            user_id: "u-4".into(),
            org_id: "o-2".into(),
            role: clarity_contract::TeamRole::Owner,
            joined_at: 1700000000,
        };
        store.add_org_member(&member).unwrap();

        let members = store.list_org_members("o-2").unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_id, "u-4");
        assert_eq!(members[0].role, clarity_contract::TeamRole::Owner);
    }

    #[test]
    fn test_cascade_delete_user_removes_memberships() {
        let store = temp_store();
        let user = make_user("u-del", "Eve", "local");
        store.upsert_user(&user).unwrap();
        let org = clarity_contract::Organization {
            id: "o-del".into(),
            name: "Temp Org".into(),
            description: None,
            created_at: 1700000000,
        };
        store.upsert_org(&org).unwrap();
        let team = clarity_contract::Team {
            id: "t-del".into(),
            org_id: "o-del".into(),
            name: "Temp Team".into(),
            description: None,
            created_at: 1700000000,
        };
        store.upsert_team(&team).unwrap();

        store
            .add_org_member(&clarity_contract::OrgMember {
                user_id: "u-del".into(),
                org_id: "o-del".into(),
                role: clarity_contract::TeamRole::Member,
                joined_at: 1700000000,
            })
            .unwrap();
        store
            .add_team_member(&clarity_contract::TeamMember {
                user_id: "u-del".into(),
                team_id: "t-del".into(),
                role: clarity_contract::TeamRole::Member,
                joined_at: 1700000000,
            })
            .unwrap();

        // Verify memberships exist.
        assert_eq!(store.list_org_members("o-del").unwrap().len(), 1);
        assert_eq!(store.list_team_members("t-del").unwrap().len(), 1);

        // Delete user → cascade deletes memberships.
        store.delete_user("u-del").unwrap();
        assert!(store.list_org_members("o-del").unwrap().is_empty());
        assert!(store.list_team_members("t-del").unwrap().is_empty());
    }
}
