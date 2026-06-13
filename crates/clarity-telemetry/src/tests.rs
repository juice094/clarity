//! Unit tests for clarity-telemetry.

use crate::{
    EventType, Severity, WideEvent,
    audit::{ConfigActor, ConfigAuditLog, ConfigChangeType},
};

// ============================================================================
// WideEvent tests
// ============================================================================

#[test]
fn test_wide_event_basic() {
    let event = WideEvent::new("clarity-core", EventType::SessionStart, Severity::Info)
        .with_attr("session_id", "test-session-123")
        .with_metric("latency_ms", 42.0);

    assert_eq!(event.service_name, "clarity-core");
    assert_eq!(event.event_type, EventType::SessionStart);
    assert_eq!(event.severity, Severity::Info);
    assert_eq!(
        event.attributes.get("session_id"),
        Some(&serde_json::json!("test-session-123"))
    );
    assert_eq!(event.metrics.get("latency_ms"), Some(&42.0));
}

#[test]
fn test_wide_event_with_trace() {
    let trace_id = uuid::Uuid::new_v4();
    let span_id = uuid::Uuid::new_v4();

    let event = WideEvent::new("clarity-llm", EventType::LlmRequest, Severity::Debug)
        .with_trace(trace_id, span_id)
        .with_parent_span(span_id);

    assert_eq!(event.trace_id, Some(trace_id));
    assert_eq!(event.span_id, Some(span_id));
    assert_eq!(event.parent_span_id, Some(span_id));
}

#[test]
fn test_wide_event_payload_hash() {
    let event = WideEvent::new("clarity-core", EventType::ToolCall, Severity::Info)
        .with_attr("tool_name", "file_read");

    let hash1 = event.payload_hash();
    let hash2 = event.clone().payload_hash();
    assert_eq!(hash1, hash2, "same event must produce same hash");

    let different = WideEvent::new("clarity-core", EventType::ToolCall, Severity::Info)
        .with_attr("tool_name", "bash");
    let hash3 = different.payload_hash();
    assert_ne!(hash1, hash3, "different event must produce different hash");
}

#[test]
fn test_event_type_display() {
    assert_eq!(EventType::SessionStart.to_string(), "session_start");
    assert_eq!(EventType::ToolError.to_string(), "tool_error");
    assert_eq!(EventType::Unknown.to_string(), "unknown");
}

#[test]
fn test_severity_from_tracing_level() {
    assert_eq!(Severity::from(tracing::Level::TRACE), Severity::Debug);
    assert_eq!(Severity::from(tracing::Level::DEBUG), Severity::Debug);
    assert_eq!(Severity::from(tracing::Level::INFO), Severity::Info);
    assert_eq!(Severity::from(tracing::Level::WARN), Severity::Warn);
    assert_eq!(Severity::from(tracing::Level::ERROR), Severity::Error);
}

// ============================================================================
// ConfigAudit tests
// ============================================================================

#[test]
fn test_config_audit_log_basic() {
    let log = ConfigAuditLog::new(
        "~/.clarity/config.toml",
        ConfigChangeType::Update,
        "changed default provider to kimi-coding",
    )
    .with_before_hash("abc123")
    .with_after_hash("def456")
    .with_rollback("git checkout -- config.toml")
    .with_actor(ConfigActor::User);

    assert_eq!(log.config_path, "~/.clarity/config.toml");
    assert_eq!(log.change_type, ConfigChangeType::Update);
    assert_eq!(log.before_hash, Some("abc123".to_string()));
    assert_eq!(log.after_hash, Some("def456".to_string()));
    assert_eq!(
        log.rollback_command,
        Some("git checkout -- config.toml".to_string())
    );
    assert_eq!(log.actor, ConfigActor::User);
    assert_eq!(log.pid, std::process::id());
}

#[test]
fn test_config_audit_into_wide_event() {
    let log = ConfigAuditLog::new(
        "config.toml",
        ConfigChangeType::Migration,
        "schema v2 migration",
    );

    let event = log.into_wide_event();
    assert_eq!(event.event_type, EventType::ConfigAudit);
    assert_eq!(event.severity, Severity::Info);
    assert_eq!(
        event.attributes.get("config_path"),
        Some(&serde_json::json!("config.toml"))
    );
    assert!(event.attributes.contains_key("audit_payload"));
}

// ============================================================================
// Serialization roundtrip
// ============================================================================

#[test]
fn test_wide_event_serde_roundtrip() {
    let original = WideEvent::new("clarity-test", EventType::MemoryCompile, Severity::Warn)
        .with_attr("session_id", "sess-42")
        .with_attr("tool_name", "file_read")
        .with_metric("latency_ms", 15.5)
        .with_metric("tokens", 1024.0);

    let json = serde_json::to_string(&original).expect("serialize");
    let deserialized: WideEvent = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(original.service_name, deserialized.service_name);
    assert_eq!(original.event_type, deserialized.event_type);
    assert_eq!(original.severity, deserialized.severity);
    assert_eq!(original.attributes, deserialized.attributes);
    assert_eq!(original.metrics, deserialized.metrics);
}
