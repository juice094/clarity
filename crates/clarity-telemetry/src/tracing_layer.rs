//! `tracing-subscriber` Layer that forwards spans/events into [`WideEvent`](crate::WideEvent).
//!
//! Install this layer alongside your existing tracing subscriber to automatically
//! capture all `tracing` instrumentation as structured wide events.
//!
//! # Example
//!
//! ```no_run
//! use tracing_subscriber::layer::SubscriberExt;
//! use clarity_telemetry::tracing_layer::TelemetryLayer;
//!
//! let sink = clarity_telemetry::sink::MultiSink::from_config(&Default::default()
//! ).unwrap();
//!
//! tracing_subscriber::registry()
//!     .with(TelemetryLayer::new(sink))
//!     .init();
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use tracing::span::Id as SpanId;
use tracing::Event as TracingEvent;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

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
