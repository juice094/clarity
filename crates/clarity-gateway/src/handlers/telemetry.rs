#![cfg(feature = "telemetry-api")]

//! Telemetry query handlers — expose observability data via REST.
//!
//! Requires the `telemetry-api` feature. The SqliteBackend is passed via
//! axum `Extension<Arc<SqliteBackend>>` rather than `State`, so the
//! sub-router uses `Router<()>` and can be `.nest()`-ed into the main API router.

use axum::{Extension, extract::Query, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use clarity_telemetry::backend::sqlite::SqliteBackend;

// ============================================================================
// Query parameter types
// ============================================================================

#[derive(Debug, Deserialize, Default)]
pub struct MetricsQuery {
    pub event_type: Option<String>,
    pub metric_key: Option<String>,
    pub window_secs: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct TracesQuery {
    pub session_id: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Deserialize, Default)]
pub struct RecentQuery {
    pub event_type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    100
}

// ============================================================================
// Response types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub event_type: String,
    pub metric_key: String,
    pub avg: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub count: usize,
    pub window_secs: i64,
}

#[derive(Debug, Serialize)]
pub struct TraceGroup {
    pub trace_id: Option<String>,
    pub events: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct TracesResponse {
    pub traces: Vec<TraceGroup>,
    pub total_events: usize,
}

#[derive(Debug, Serialize)]
pub struct EventsResponse {
    pub events: Vec<serde_json::Value>,
    pub total: usize,
}

// ============================================================================
// Helper
// ============================================================================

fn get_backend(
    ext: &Extension<Arc<SqliteBackend>>,
) -> Result<&Arc<SqliteBackend>, (StatusCode, String)> {
    Ok(&ext.0)
}

// ============================================================================
// Metrics handler
// ============================================================================

/// GET /v1/metrics — aggregated metric over a time window.
pub async fn get_metrics(
    Extension(backend): Extension<Arc<SqliteBackend>>,
    Query(query): Query<MetricsQuery>,
) -> Result<Json<MetricsResponse>, (StatusCode, String)> {
    let event_type = query.event_type.unwrap_or_else(|| "tool_call".to_string());
    let metric_key = query.metric_key.unwrap_or_else(|| "latency_ms".to_string());
    let window_secs = query.window_secs.unwrap_or(3600);

    let now = chrono::Utc::now().timestamp_millis();
    let start = now - window_secs * 1000;

    match backend.aggregate_metrics(&event_type, &metric_key, start, now) {
        Ok(Some((avg, min, max, count))) => Ok(Json(MetricsResponse {
            event_type,
            metric_key,
            avg: Some(avg),
            min: Some(min),
            max: Some(max),
            count,
            window_secs,
        })),
        Ok(None) => Ok(Json(MetricsResponse {
            event_type,
            metric_key,
            avg: None,
            min: None,
            max: None,
            count: 0,
            window_secs,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("query error: {e}"),
        )),
    }
}

// ============================================================================
// Traces handler
// ============================================================================

/// GET /v1/traces — events grouped by trace context.
pub async fn get_traces(
    Extension(backend): Extension<Arc<SqliteBackend>>,
    Query(query): Query<TracesQuery>,
) -> Result<Json<TracesResponse>, (StatusCode, String)> {
    let limit = query.limit.min(500);
    let events = backend.query_by_type("", None, None, limit).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("query error: {e}"),
        )
    })?;

    let total_events = events.len();
    let mut groups: std::collections::HashMap<String, Vec<serde_json::Value>> =
        std::collections::HashMap::new();

    for event in &events {
        let payload = serde_json::to_value(event).unwrap_or_default();
        let trace_key = event
            .trace_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| format!("untraced-{}", event.payload_hash()));

        if let Some(ref s) = query.session_id {
            if event.attributes.get("session_id").and_then(|v| v.as_str()) != Some(s.as_str()) {
                continue;
            }
        }

        groups.entry(trace_key).or_default().push(payload);
    }

    let traces: Vec<TraceGroup> = groups
        .into_iter()
        .map(|(trace_id, events)| TraceGroup {
            trace_id: if trace_id.starts_with("untraced-") {
                None
            } else {
                Some(trace_id)
            },
            events,
        })
        .collect();

    Ok(Json(TracesResponse {
        traces,
        total_events,
    }))
}

// ============================================================================
// Events handler
// ============================================================================

/// GET /v1/events/recent — recent events as a flat JSON array.
pub async fn get_recent_events(
    Extension(backend): Extension<Arc<SqliteBackend>>,
    Query(query): Query<RecentQuery>,
) -> Result<Json<EventsResponse>, (StatusCode, String)> {
    let event_type = query.event_type.as_deref().unwrap_or("");
    let limit = query.limit.min(1000);

    let events = backend
        .query_by_type(event_type, None, None, limit)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("query error: {e}"),
            )
        })?;

    let total = events.len();
    let json_events: Vec<serde_json::Value> = events
        .iter()
        .map(|e| serde_json::to_value(e).unwrap_or_default())
        .collect();

    Ok(Json(EventsResponse {
        events: json_events,
        total,
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_query_defaults() {
        let q = MetricsQuery::default();
        assert!(q.event_type.is_none());
        assert!(q.metric_key.is_none());
    }

    #[test]
    fn test_traces_query_defaults() {
        let q = TracesQuery::default();
        assert!(q.session_id.is_none());
        assert_eq!(q.limit, 100);
    }

    #[test]
    fn test_recent_query_limit_clamp() {
        let q = RecentQuery {
            event_type: Some("tool_call".to_string()),
            limit: 2000,
        };
        assert_eq!(q.limit.min(1000), 1000);
    }

    #[test]
    fn test_metrics_response_serialization() {
        let resp = MetricsResponse {
            event_type: "tool_call".to_string(),
            metric_key: "latency_ms".to_string(),
            avg: Some(42.0),
            min: Some(10.0),
            max: Some(100.0),
            count: 5,
            window_secs: 3600,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"avg\":42.0"));
    }
}
