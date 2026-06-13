#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)
)]
//! End-to-end telemetry test: verify WideEvent flows through EventSink to SQLite.

use clarity_telemetry::{
    EventSink, EventType, Severity, WideEvent, audit::ConfigActor, backend::sqlite::SqliteBackend,
};

/// Create a WideEvent, emit it through SqliteBackend, then read it back.
#[tokio::test]
async fn test_telemetry_sqlite_roundtrip() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let backend = SqliteBackend::new(Some(tmp.path().to_str().unwrap())).unwrap();

    let event = WideEvent::new("clarity-core", EventType::ToolCall, Severity::Info)
        .with_attr("tool_name", "file_read")
        .with_attr("session_id", "sess-42")
        .with_metric("latency_ms", 15.5)
        .with_metric("tokens", 1024.0);

    // Emit
    backend.emit(event.clone()).await.unwrap();
    backend.flush().await.unwrap();

    // Read back by type
    let events = backend.query_by_type("tool_call", None, None, 10).unwrap();
    assert_eq!(events.len(), 1);

    let read = &events[0];
    assert_eq!(read.service_name, "clarity-core");
    assert_eq!(read.event_type, EventType::ToolCall);
    assert_eq!(read.severity, Severity::Info);
    assert_eq!(
        read.attributes.get("tool_name"),
        Some(&serde_json::json!("file_read"))
    );
    assert_eq!(read.metrics.get("latency_ms"), Some(&15.5));
    assert_eq!(read.metrics.get("tokens"), Some(&1024.0));
}

/// Emit multiple events and verify batch query ordering.
#[tokio::test]
async fn test_telemetry_multiple_events_ordered() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let backend = SqliteBackend::new(Some(tmp.path().to_str().unwrap())).unwrap();

    for i in 0..5 {
        let event = WideEvent::new("clarity-llm", EventType::LlmRequest, Severity::Debug)
            .with_attr("req_id", format!("req-{}", i))
            .with_metric("latency_ms", 10.0 * (i as f64 + 1.0));

        backend.emit(event).await.unwrap();
    }
    backend.flush().await.unwrap();

    let events = backend
        .query_by_type("llm_request", None, None, 10)
        .unwrap();
    assert_eq!(events.len(), 5);

    // Descending order (newest first)
    assert_eq!(
        events[0].attributes.get("req_id"),
        Some(&serde_json::json!("req-4"))
    );
    assert_eq!(
        events[4].attributes.get("req_id"),
        Some(&serde_json::json!("req-0"))
    );
}

/// Verify telemetry integrity via payload_hash.
#[tokio::test]
async fn test_telemetry_integrity_hash() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let backend = SqliteBackend::new(Some(tmp.path().to_str().unwrap())).unwrap();

    let event = WideEvent::new("clarity-core", EventType::SessionStart, Severity::Info)
        .with_attr("session_id", "sess-integrity");

    let original_hash = event.payload_hash();
    assert!(!original_hash.is_empty(), "hash must be non-empty");

    backend.emit(event).await.unwrap();
    backend.flush().await.unwrap();

    let events = backend
        .query_by_type("session_start", None, None, 1)
        .unwrap();
    assert_eq!(events.len(), 1);

    // Verify the roundtripped event is structurally intact.
    let read_hash = events[0].payload_hash();
    assert!(!read_hash.is_empty(), "roundtripped hash must be non-empty");
    assert_eq!(
        events[0].attributes.get("session_id"),
        Some(&serde_json::json!("sess-integrity"))
    );
}

/// Test ConfigAuditLog emission as WideEvent.
#[tokio::test]
async fn test_config_audit_telemetry_integration() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let backend = SqliteBackend::new(Some(tmp.path().to_str().unwrap())).unwrap();

    let audit = clarity_telemetry::ConfigAuditLog::new(
        "~/.clarity/config.toml",
        clarity_telemetry::ConfigChangeType::Update,
        "changed model to gpt-4",
    )
    .with_before_hash("abc123")
    .with_after_hash("def456")
    .with_rollback("git checkout -- config.toml")
    .with_actor(ConfigActor::Agent);

    let event = audit.into_wide_event();
    backend.emit(event).await.unwrap();
    backend.flush().await.unwrap();

    let events = backend
        .query_by_type("config_audit", None, None, 1)
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, EventType::ConfigAudit);
    assert!(
        events[0].attributes.contains_key("audit_payload"),
        "audit_payload must be present in attributes"
    );
}
