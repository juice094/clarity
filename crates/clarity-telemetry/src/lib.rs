#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)
)]
//! `clarity-telemetry` — Unified telemetry foundation for the Clarity ecosystem.
//!
//! Provides a **wide event** model that unifies metrics, logs, and traces into a single
//! structured data type (`WideEvent`). Events flow through an `EventSink` abstraction
//! that supports multiple backends:
//!
//! | Backend | Feature | Purpose |
//! |---------|---------|---------|
//! | SQLite  | `sqlite` (default) | Local-first fallback, single-file store |
//! | GreptimeDB | `greptime` | Remote time-series analytics (HTTP API) |
//! | Multi   | — | Fan-out to multiple sinks simultaneously |
//!
//! ## Architecture
//!
//! ```text
//! Agent / LLM / Tool          tracing span/event
//!        │                           │
//!        └──────────┬────────────────┘
//!                   ▼
//!            WideEvent (normalized)
//!                   │
//!         ┌─────────┴─────────┐
//!         ▼                   ▼
//!    EventSink trait    tracing-subscriber Layer
//!         │
//!    ┌────┴────┐
//!    ▼         ▼
//! SQLite   GreptimeDB
//! ```
//!
//! ## Design constraints (CCP-v2)
//!
//! - **Zero internal deps**: Only depends on `clarity-contract` and external crates.
//! - **Local-first**: SQLite backend works without any network or external service.
//! - **Fail-silent-safe**: Event write failures are logged but never block the hot path.
//! - **Audit-ready**: Every config change carries before/after hashes and rollback commands.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

// ============================================================================
// Public modules
// ============================================================================

/// Configuration audit trail support.
pub mod audit;
/// Telemetry storage backends.
pub mod backend;
/// Event sink abstraction and fan-out.
pub mod sink;

#[cfg(feature = "tracing-integration")]
/// `tracing-subscriber` integration layer.
pub mod tracing_layer;

// ============================================================================
// Core type: WideEvent
// ============================================================================

/// A unified observability event that carries metrics, logs, and trace context.
///
/// Designed to be directly ingestible by GreptimeDB's wide-table model and
/// compatible with OpenTelemetry's event representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WideEvent {
    /// Event timestamp in UTC.
    pub timestamp: DateTime<Utc>,

    /// OpenTelemetry trace ID, if this event belongs to a distributed trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<Uuid>,

    /// OpenTelemetry span ID, if this event belongs to a span.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<Uuid>,

    /// Parent span ID for hierarchical span relationships.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<Uuid>,

    /// Source service name (e.g. `"clarity-core"`, `"clarity-llm"`).
    pub service_name: String,

    /// Event type — used as a GreptimeDB tag for efficient filtering.
    pub event_type: EventType,

    /// Severity level — used for routing and alerting.
    pub severity: Severity,

    /// Structured attributes (string-keyed, JSON-valued).
    /// These become GreptimeDB string/JSON columns.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub attributes: HashMap<String, Value>,

    /// Numeric metrics associated with this event.
    /// These become GreptimeDB float columns and are the primary target for aggregation.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metrics: HashMap<String, f64>,
}

impl WideEvent {
    /// Create a new wide event with the current UTC timestamp.
    pub fn new(service_name: impl Into<String>, event_type: EventType, severity: Severity) -> Self {
        Self {
            timestamp: Utc::now(),
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            service_name: service_name.into(),
            event_type,
            severity,
            attributes: HashMap::new(),
            metrics: HashMap::new(),
        }
    }

    /// Attach an OpenTelemetry trace context.
    pub fn with_trace(mut self, trace_id: Uuid, span_id: Uuid) -> Self {
        self.trace_id = Some(trace_id);
        self.span_id = Some(span_id);
        self
    }

    /// Attach a parent span ID.
    pub fn with_parent_span(mut self, parent_span_id: Uuid) -> Self {
        self.parent_span_id = Some(parent_span_id);
        self
    }

    /// Add a string attribute.
    pub fn with_attr(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        match serde_json::to_value(value) {
            Ok(v) => {
                self.attributes.insert(key.into(), v);
            }
            Err(e) => {
                // NOTE: attribute serialization failure must not block the hot path.
                // The error is silently dropped; the event still carries other data.
                let _ = e;
            }
        }
        self
    }

    /// Add a numeric metric.
    pub fn with_metric(mut self, key: impl Into<String>, value: f64) -> Self {
        self.metrics.insert(key.into(), value);
        self
    }

    /// Add multiple metrics at once.
    pub fn with_metrics(mut self, metrics: HashMap<String, f64>) -> Self {
        self.metrics.extend(metrics);
        self
    }

    /// Compute a content hash over the serialized event payload.
    ///
    /// Used for integrity verification in audit trails and deduplication.
    pub fn payload_hash(&self) -> String {
        match serde_json::to_vec(self) {
            Ok(bytes) => {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                bytes.hash(&mut hasher);
                format!("{:016x}", hasher.finish())
            }
            Err(_) => String::new(),
        }
    }
}

// ============================================================================
// EventType — dimension for categorization and filtering
// ============================================================================

/// Types of observability events emitted by Clarity subsystems.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // Session lifecycle
    /// Start of a new session.
    SessionStart,
    /// End of a session.
    SessionEnd,

    // Message flow
    /// Outbound message.
    MessageSend,
    /// Inbound message.
    MessageRecv,

    // Tool execution
    /// Tool invocation.
    ToolCall,
    /// Successful tool result.
    ToolResult,
    /// Tool execution failure.
    ToolError,

    // LLM routing and inference
    /// Model routing decision.
    ModelRoute,
    /// LLM request emitted.
    LlmRequest,
    /// LLM response received.
    LlmResponse,
    /// Chunk of a streaming LLM response.
    LlmStreamChunk,

    // Context management
    /// Context compression occurred.
    ContextCompress,
    /// Context truncation occurred.
    ContextTruncate,

    // Memory system
    /// Memory query executed.
    MemoryQuery,
    /// Memory compilation.
    MemoryCompile,
    /// Memory store operation.
    MemoryStore,

    // Configuration and audit
    /// Configuration value changed.
    ConfigChange,
    /// Configuration audit record.
    ConfigAudit,

    // Gateway health
    /// Gateway health check.
    GatewayHealth,
    /// Gateway probe event.
    GatewayProbe,

    // Agent lifecycle
    /// Agent spawned.
    AgentSpawn,
    /// Agent terminated.
    AgentTerminate,

    // Background tasks
    /// Task scheduled.
    TaskSchedule,
    /// Task started.
    TaskStart,
    /// Task completed.
    TaskComplete,
    /// Task failed.
    TaskFail,

    // User interaction
    /// User feedback received.
    UserFeedback,
    /// User interrupted execution.
    UserInterrupt,

    // Catch-all for forward compatibility
    /// Unknown or unrecognized event type.
    #[default]
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self)
            .unwrap_or_else(|_| "\"unknown\"".to_string())
            .trim_matches('"')
            .to_string();
        write!(f, "{}", s)
    }
}

// ============================================================================
// Severity — dimension for alerting and log level routing
// ============================================================================

/// Severity level of a wide event.
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Verbose diagnostic information.
    Debug,
    /// General informational message.
    #[default]
    Info,
    /// Non-fatal warning.
    Warn,
    /// Recoverable error.
    Error,
    /// Unrecoverable error.
    Fatal,
}

impl From<tracing::Level> for Severity {
    fn from(level: tracing::Level) -> Self {
        match level {
            tracing::Level::TRACE => Severity::Debug,
            tracing::Level::DEBUG => Severity::Debug,
            tracing::Level::INFO => Severity::Info,
            tracing::Level::WARN => Severity::Warn,
            tracing::Level::ERROR => Severity::Error,
        }
    }
}

// ============================================================================
// Error types
// ============================================================================

/// Errors emitted by the telemetry subsystem.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    /// The storage backend returned an error.
    #[error("backend error: {0}")]
    Backend(String),

    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The supplied configuration is invalid or incomplete.
    #[error("invalid configuration: {0}")]
    Config(String),
}

/// Result type alias for telemetry operations.
pub type TelemetryResult<T> = Result<T, TelemetryError>;

// ============================================================================
// Re-exports
// ============================================================================

pub use audit::{ConfigActor, ConfigAuditLog, ConfigChangeType};
pub use sink::{EventSink, MultiSink, SinkConfig};

#[cfg(feature = "sqlite")]
pub use backend::sqlite::SqliteBackend;

#[cfg(feature = "greptime")]
pub use backend::greptime::GreptimeBackend;

#[cfg(test)]
mod tests;
