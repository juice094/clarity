//! `tracing-subscriber` Layer that forwards spans/events into [`WideEvent`](crate::WideEvent).
//!
//! Install this layer alongside your existing tracing subscriber to automatically
//! capture all `tracing` instrumentation as structured wide events.
//!
//! # Example
//!
//! ```no_run
//! use tracing_subscriber::layer::SubscriberExt;
//! use tracing_subscriber::util::SubscriberInitExt;
//! use clarity_telemetry::tracing_layer::TelemetryLayer;
//!
//! fn install() -> Result<(), Box<dyn std::error::Error>> {
//!     let sink = clarity_telemetry::sink::MultiSink::from_config(&Default::default())?;
//!
//!     tracing_subscriber::registry()
//!         .with(TelemetryLayer::new(sink))
//!         .init();
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use tracing::Event as TracingEvent;
use tracing::span::Id as SpanId;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use crate::{EventSink, EventType, Severity, WideEvent};

// ============================================================================
// TelemetryLayer
// ============================================================================

/// A `tracing-subscriber` Layer that translates tracing spans/events into [`WideEvent`]s.
pub struct TelemetryLayer {
    sink: Arc<dyn EventSink>,
}

impl TelemetryLayer {
    /// Create a new telemetry layer wrapping the given sink.
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        Self { sink }
    }
}

impl<S> Layer<S> for TelemetryLayer
where
    S: tracing::Subscriber,
    S: for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &TracingEvent, ctx: Context<'_, S>) {
        let mut attrs = HashMap::new();
        let mut metrics = HashMap::new();

        // Extract fields from the tracing event.
        let mut visitor = FieldVisitor {
            attrs: &mut attrs,
            metrics: &mut metrics,
            message: None,
        };
        event.record(&mut visitor);

        // Determine event type from the target or message.
        let event_type = infer_event_type(event.metadata().target(), visitor.message.as_deref());

        let severity = Severity::from(*event.metadata().level());

        let mut wide = WideEvent::new(event.metadata().target(), event_type, severity)
            .with_attr("span_name", event.metadata().name())
            .with_attr("file", event.metadata().file().unwrap_or("unknown"))
            .with_attr("line", event.metadata().line().unwrap_or(0));

        if let Some(msg) = visitor.message {
            wide = wide.with_attr("message", msg);
        }

        // Attach trace context from the current span, if any.
        if let Some(span_ref) = ctx.event_span(event) {
            let span_id = span_id_to_uuid(&span_ref.id());
            wide = wide.with_attr("span_id", span_id.to_string());

            if let Some(trace_id) = span_ref.extensions().get::<TraceId>() {
                wide = wide.with_trace(trace_id.0, span_id);
            }
        }

        // Add collected attributes and metrics.
        for (k, v) in attrs {
            wide = wide.with_attr(k, v);
        }
        for (k, v) in metrics {
            wide = wide.with_metric(k, v);
        }

        // Emit asynchronously via tokio::spawn so we never block the tracing hot path.
        let sink = Arc::clone(&self.sink);
        tokio::spawn(async move {
            let _ = sink.emit(wide).await;
        });
    }

    fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &SpanId, ctx: Context<'_, S>) {
        // Store trace_id in span extensions if provided as a field.
        if let Some(span) = ctx.span(id) {
            let mut visitor = TraceIdVisitor { trace_id: None };
            attrs.record(&mut visitor);
            if let Some(trace_id) = visitor.trace_id {
                span.extensions_mut().insert(TraceId(trace_id));
            }
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

struct TraceId(pub uuid::Uuid);

struct FieldVisitor<'a> {
    attrs: &'a mut HashMap<String, serde_json::Value>,
    metrics: &'a mut HashMap<String, f64>,
    message: Option<String>,
}

impl<'a> tracing::field::Visit for FieldVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let key = field.name().to_string();
        let val_str = format!("{:?}", value);

        // Heuristic: fields named with `_ms`, `_bytes`, `_count`, `_tokens` go to metrics.
        if key.ends_with("_ms")
            || key.ends_with("_bytes")
            || key.ends_with("_count")
            || key.ends_with("_tokens")
        {
            if let Ok(v) = val_str.parse::<f64>() {
                self.metrics.insert(key, v);
                return;
            }
        }

        if key == "message" {
            self.message = Some(val_str.trim_matches('"').to_string());
        } else {
            self.attrs.insert(key, serde_json::Value::String(val_str));
        }
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.metrics.insert(field.name().to_string(), value);
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.metrics.insert(field.name().to_string(), value as f64);
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.metrics.insert(field.name().to_string(), value as f64);
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.attrs.insert(
                field.name().to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }
}

struct TraceIdVisitor {
    trace_id: Option<uuid::Uuid>,
}

impl tracing::field::Visit for TraceIdVisitor {
    fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {}

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "trace_id" {
            self.trace_id = uuid::Uuid::parse_str(value).ok();
        }
    }
}

fn infer_event_type(target: &str, message: Option<&str>) -> EventType {
    // Infer from target module path.
    if target.contains("tool") {
        return EventType::ToolCall;
    }
    if target.contains("llm") || target.contains("model") {
        return EventType::LlmRequest;
    }
    if target.contains("memory") {
        return EventType::MemoryQuery;
    }
    if target.contains("config") {
        return EventType::ConfigChange;
    }
    if target.contains("gateway") {
        return EventType::GatewayHealth;
    }
    if target.contains("agent") {
        return EventType::AgentSpawn;
    }
    if target.contains("task") || target.contains("background") {
        return EventType::TaskSchedule;
    }

    // Fallback: check message content for hints.
    if let Some(msg) = message {
        if msg.contains("error") || msg.contains("failed") || msg.contains("panic") {
            return EventType::Unknown; // Keep severity-driven alerting
        }
    }

    EventType::Unknown
}

fn span_id_to_uuid(id: &SpanId) -> uuid::Uuid {
    // SpanId is an opaque 64-bit value. We deterministically map it to UUIDv4
    // by zero-extending into a UUID and setting the version bits.
    let inner: u64 = id.into_u64();
    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&inner.to_be_bytes());
    // Set UUID v4 version (0100) and variant (10) bits.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventSink, EventType, Severity, TelemetryResult, WideEvent};
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::Arc;
    use tracing::span::Id as SpanId;
    use tracing_subscriber::layer::SubscriberExt;

    struct TestSink {
        tx: tokio::sync::mpsc::UnboundedSender<WideEvent>,
    }

    #[async_trait]
    impl EventSink for TestSink {
        async fn emit(&self, event: WideEvent) -> TelemetryResult<()> {
            let _ = self.tx.send(event);
            Ok(())
        }

        fn name(&self) -> &str {
            "test"
        }
    }

    #[test]
    fn infer_event_type_from_target() {
        assert_eq!(
            infer_event_type("my::tool::handler", None),
            EventType::ToolCall
        );
        assert_eq!(infer_event_type("llm::client", None), EventType::LlmRequest);
        assert_eq!(
            infer_event_type("memory::store", None),
            EventType::MemoryQuery
        );
        assert_eq!(
            infer_event_type("config::audit", None),
            EventType::ConfigChange
        );
        assert_eq!(
            infer_event_type("gateway::health", None),
            EventType::GatewayHealth
        );
        assert_eq!(
            infer_event_type("agent::spawn", None),
            EventType::AgentSpawn
        );
        assert_eq!(
            infer_event_type("task::scheduler", None),
            EventType::TaskSchedule
        );
        assert_eq!(
            infer_event_type("background::worker", None),
            EventType::TaskSchedule
        );
    }

    #[test]
    fn infer_event_type_fallback_to_message() {
        assert_eq!(
            infer_event_type("unknown", Some("an error occurred")),
            EventType::Unknown
        );
        assert_eq!(
            infer_event_type("unknown", Some("request failed")),
            EventType::Unknown
        );
        assert_eq!(
            infer_event_type("unknown", Some("panic in worker")),
            EventType::Unknown
        );
    }

    #[test]
    fn infer_event_type_unknown_when_no_match() {
        assert_eq!(infer_event_type("other", Some("ok")), EventType::Unknown);
        assert_eq!(infer_event_type("other", None), EventType::Unknown);
    }

    #[test]
    fn span_id_to_uuid_sets_version_and_variant() {
        let id = SpanId::from_u64(1);
        let uuid = span_id_to_uuid(&id);
        assert_eq!(uuid.get_version_num(), 4);
        assert_eq!(uuid.get_variant(), uuid::Variant::RFC4122);

        let id = SpanId::from_u64(0x1234_5678_9abc_def0);
        let uuid = span_id_to_uuid(&id);
        assert_eq!(uuid.get_version_num(), 4);
        assert_eq!(uuid.get_variant(), uuid::Variant::RFC4122);
    }

    #[test]
    fn span_id_to_uuid_is_deterministic() {
        let id = SpanId::from_u64(42);
        assert_eq!(span_id_to_uuid(&id), span_id_to_uuid(&id));
    }

    #[tokio::test]
    async fn telemetry_layer_forwards_event() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sink = Arc::new(TestSink { tx });
        let layer = TelemetryLayer::new(sink);
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "clarity_core::tool", "user requested file read");
        });

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out waiting for event")
            .expect("channel closed");

        assert_eq!(event.event_type, EventType::ToolCall);
        assert_eq!(event.severity, Severity::Info);
        assert_eq!(
            event.attributes.get("message"),
            Some(&json!("user requested file read"))
        );
        assert!(event.attributes.contains_key("span_name"));
    }

    #[tokio::test]
    async fn telemetry_layer_extracts_metrics() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sink = Arc::new(TestSink { tx });
        let layer = TelemetryLayer::new(sink);
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                target: "clarity_core::llm",
                latency_ms = 42u64,
                "prompt sent"
            );
        });

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out waiting for event")
            .expect("channel closed");

        assert_eq!(event.event_type, EventType::LlmRequest);
        assert_eq!(event.metrics.get("latency_ms"), Some(&42.0));
    }

    #[tokio::test]
    async fn telemetry_layer_attaches_trace_context() {
        let trace_id = uuid::Uuid::new_v4();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sink = Arc::new(TestSink { tx });
        let layer = TelemetryLayer::new(sink);
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!(
                target: "clarity_core::agent",
                "process_request",
                trace_id = trace_id.to_string()
            );
            span.in_scope(|| {
                tracing::info!(target: "clarity_core::agent", "inside span");
            });
        });

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out waiting for event")
            .expect("channel closed");

        assert_eq!(event.event_type, EventType::AgentSpawn);
        assert_eq!(event.trace_id, Some(trace_id));
        assert!(event.span_id.is_some());
    }

    #[tokio::test]
    async fn telemetry_layer_falls_back_to_unknown() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sink = Arc::new(TestSink { tx });
        let layer = TelemetryLayer::new(sink);
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(target: "some_module", "something failed");
        });

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out waiting for event")
            .expect("channel closed");

        assert_eq!(event.event_type, EventType::Unknown);
        assert_eq!(event.severity, Severity::Warn);
    }
}
