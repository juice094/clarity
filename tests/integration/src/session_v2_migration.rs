//! Session V2 migration test: verify append-only event log and compacted context.

use clarity_memory::session_store_v2::{EventType as V2EventType, SessionState, SessionStoreV2};

/// Full lifecycle: create → append events → compact → read back.
#[test]
fn test_session_v2_full_lifecycle() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let store = SessionStoreV2::new(tmp.path()).unwrap();

    // Create session
    store
        .create_session(
            "sess-1",
            Some("Test Session"),
            Some("soul-grey"),
            Some("hash-abc"),
        )
        .unwrap();

    let session = store.get_session("sess-1").unwrap().unwrap();
    assert_eq!(session.id, "sess-1");
    assert_eq!(session.state, SessionState::Active);

    // Append events (simulating a turn)
    store
        .append_event(
            "sess-1",
            1,
            V2EventType::UserMessage,
            &serde_json::json!({"role": "user", "content": "hello"}),
        )
        .unwrap();
    store
        .append_event(
            "sess-1",
            1,
            V2EventType::AssistantMessage,
            &serde_json::json!({"role": "assistant", "content": "hi there"}),
        )
        .unwrap();
    store
        .append_event(
            "sess-1",
            1,
            V2EventType::ToolCall,
            &serde_json::json!({"tool": "file_read", "args": {"path": "/tmp/test"}}),
        )
        .unwrap();

    // Read all events
    let events = store.read_events("sess-1").unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].event_type, V2EventType::UserMessage);
    assert_eq!(events[1].event_type, V2EventType::AssistantMessage);
    assert_eq!(events[2].event_type, V2EventType::ToolCall);

    // Verify event_id monotonicity
    assert!(events[1].event_id > events[0].event_id);
    assert!(events[2].event_id > events[1].event_id);

    // Store compacted context
    let compacted = serde_json::json!([
        {"role": "system", "content": "compacted summary"},
        {"role": "user", "content": "hello"}
    ]);
    store
        .store_compacted_context("sess-1", 1, 3, &compacted, "tier2", "hash-source")
        .unwrap();

    let loaded = store.load_compacted_context("sess-1").unwrap().unwrap();
    assert_eq!(loaded.compression_method, "tier2");
    assert_eq!(loaded.context_json, compacted);
    assert_eq!(loaded.source_hash, "hash-source");

    // Verify event_log survives session state transition
    store
        .set_session_state("sess-1", SessionState::Archived)
        .unwrap();
    let events_after = store.read_events("sess-1").unwrap();
    assert_eq!(events_after.len(), 3);
}

/// Verify parent_session_id (handoff lineage) works.
#[test]
fn test_session_v2_handoff_lineage() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let store = SessionStoreV2::new(tmp.path()).unwrap();

    store.create_session("parent", None, None, None).unwrap();
    store.create_session("child", None, None, None).unwrap();
    store.set_parent("child", "parent").unwrap();

    let child = store.get_session("child").unwrap().unwrap();
    assert_eq!(child.parent_session_id, Some("parent".to_string()));
}

/// Verify delete cascades to event_log and compacted_context.
#[test]
fn test_session_v2_delete_cascade() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let store = SessionStoreV2::new(tmp.path()).unwrap();

    store
        .create_session("sess-delete", None, None, None)
        .unwrap();
    store
        .append_event(
            "sess-delete",
            1,
            V2EventType::UserMessage,
            &serde_json::json!({}),
        )
        .unwrap();
    store
        .store_compacted_context("sess-delete", 1, 1, &serde_json::json!([]), "tier1", "h")
        .unwrap();

    store.delete_session("sess-delete").unwrap();

    assert!(store.get_session("sess-delete").unwrap().is_none());
    assert!(store.read_events("sess-delete").unwrap().is_empty());
    assert!(store
        .load_compacted_context("sess-delete")
        .unwrap()
        .is_none());
}

/// Simulate a long session and verify compaction boundary.
#[test]
fn test_session_v2_large_session() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let store = SessionStoreV2::new(tmp.path()).unwrap();

    store.create_session("long-sess", None, None, None).unwrap();

    // Simulate 100 turns
    for turn in 1..=100 {
        store
            .append_event(
                "long-sess",
                turn,
                V2EventType::UserMessage,
                &serde_json::json!({"content": format!("msg {}", turn)}),
            )
            .unwrap();
    }

    let all_events = store.read_events("long-sess").unwrap();
    assert_eq!(all_events.len(), 100);

    // Read events up to turn 50
    let events_until_50 = store.read_events_until("long-sess", 50, 50).unwrap();
    assert_eq!(events_until_50.len(), 50);

    // Verify turn_id monotonicity
    for i in 1..events_until_50.len() {
        assert!(
            events_until_50[i].turn_id >= events_until_50[i - 1].turn_id,
            "turn_id must be non-decreasing"
        );
    }
}
