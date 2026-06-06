//! SQLite backend for local-first telemetry storage.
//!
//! Uses a single wide table (`wide_events`) with JSON columns for flexible
//! schema evolution. This matches GreptimeDB's wide-event model while
//! requiring zero external services.
//!
//! # Schema
//!
//! ```sql
//! CREATE TABLE wide_events (
//!     id          INTEGER PRIMARY KEY AUTOINCREMENT,
//!     timestamp   INTEGER NOT NULL,          -- Unix millis
//!     trace_id    TEXT,
//!     span_id     TEXT,
//!     parent_span_id TEXT,
//!     service_name TEXT NOT NULL,
//!     event_type  TEXT NOT NULL,
//!     severity    TEXT NOT NULL,
//!     attributes  TEXT,                      -- JSON object
//!     metrics     TEXT,                      -- JSON object
//!     payload_hash TEXT                    -- integrity check
//! );
//!
//! CREATE INDEX idx_wide_events_timestamp ON wide_events(timestamp);
//! CREATE INDEX idx_wide_events_type ON wide_events(event_type);
//! CREATE INDEX idx_wide_events_service ON wide_events(service_name);
//! CREATE INDEX idx_wide_events_trace ON wide_events(trace_id);
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension};
use uuid::Uuid;

use crate::{EventSink, TelemetryError, TelemetryResult, WideEvent};

// ============================================================================
// SqliteBackend
// ============================================================================

/// Local SQLite storage for wide events.
pub struct SqliteBackend {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl SqliteBackend {
    /// Open or create a SQLite database at the given path.
    ///
    /// If `path` is `None`, uses the default location:
    /// `~/.clarity/telemetry.sqlite`.
    pub fn new(path: Option<&str>) -> TelemetryResult<Self> {
        let path = match path {
            Some(p) => PathBuf::from(p),
            None => default_db_path()?,
        };

        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                TelemetryError::Backend(format!("failed to create db directory: {e}"))
            })?;
        }

        let conn = Connection::open(&path)
            .map_err(|e| TelemetryError::Backend(format!("failed to open sqlite: {e}")))?;

        // SAFETY: PRAGMA journal_mode may return a result row on some SQLite builds.
        // We use execute_batch to silently ignore it; failure is non-fatal.
        let _ = conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;");

        let this = Self {
            conn: Arc::new(Mutex::new(conn)),
            path,
        };
        this.init_schema()?;
        Ok(this)
    }

    /// Initialize the database schema.
    fn init_schema(&self) -> TelemetryResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch(SCHEMA_SQL)
            .map_err(|e| TelemetryError::Backend(format!("schema init: {e}")))?;
        Ok(())
    }

    /// Path to the database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Query events by type within a time range.
    pub fn query_by_type(
        &self,
        event_type: &str,
        start: Option<i64>,
        end: Option<i64>,
        limit: usize,
    ) -> TelemetryResult<Vec<WideEvent>> {
        let conn = self.conn.lock();

        // Build SQL and owned parameters to avoid lifetime issues.
        let sql = match (start, end) {
            (None, None) => {
                let sql = "SELECT timestamp, trace_id, span_id, parent_span_id, service_name, \
                           event_type, severity, attributes, metrics, payload_hash \
                           FROM wide_events WHERE event_type = ?1 \
                           ORDER BY timestamp DESC LIMIT ?2";
                let params: Vec<Box<dyn rusqlite::ToSql>> =
                    vec![Box::new(event_type.to_string()), Box::new(limit as i64)];
                (sql.to_string(), params)
            }
            (Some(s), None) => {
                let sql = "SELECT timestamp, trace_id, span_id, parent_span_id, service_name, \
                           event_type, severity, attributes, metrics, payload_hash \
                           FROM wide_events WHERE event_type = ?1 AND timestamp >= ?2 \
                           ORDER BY timestamp DESC LIMIT ?3";
                let params: Vec<Box<dyn rusqlite::ToSql>> = vec![
                    Box::new(event_type.to_string()),
                    Box::new(s),
                    Box::new(limit as i64),
                ];
                (sql.to_string(), params)
            }
            (None, Some(e)) => {
                let sql = "SELECT timestamp, trace_id, span_id, parent_span_id, service_name, \
                           event_type, severity, attributes, metrics, payload_hash \
                           FROM wide_events WHERE event_type = ?1 AND timestamp <= ?2 \
                           ORDER BY timestamp DESC LIMIT ?3";
                let params: Vec<Box<dyn rusqlite::ToSql>> = vec![
                    Box::new(event_type.to_string()),
                    Box::new(e),
                    Box::new(limit as i64),
                ];
                (sql.to_string(), params)
            }
            (Some(s), Some(e)) => {
                let sql = "SELECT timestamp, trace_id, span_id, parent_span_id, service_name, \
                           event_type, severity, attributes, metrics, payload_hash \
                           FROM wide_events WHERE event_type = ?1 \
                           AND timestamp >= ?2 AND timestamp <= ?3 \
                           ORDER BY timestamp DESC LIMIT ?4";
                let params: Vec<Box<dyn rusqlite::ToSql>> = vec![
                    Box::new(event_type.to_string()),
                    Box::new(s),
                    Box::new(e),
                    Box::new(limit as i64),
                ];
                (sql.to_string(), params)
            }
        };

        let ref_params: Vec<&dyn rusqlite::ToSql> = sql.1.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn
            .prepare(&sql.0)
            .map_err(|e| TelemetryError::Backend(format!("query prepare: {e}")))?;

        let rows = stmt
            .query_map(rusqlite::params_from_iter(ref_params.iter()), row_to_event)
            .map_err(|e| TelemetryError::Backend(format!("query execute: {e}")))?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(|e| TelemetryError::Backend(format!("row mapping: {e}")))?);
        }
        Ok(events)
    }

    /// Get aggregated metrics for a given event type over a time window.
    pub fn aggregate_metrics(
        &self,
        event_type: &str,
        metric_key: &str,
        start: i64,
        end: i64,
    ) -> TelemetryResult<Option<(f64, f64, f64, usize)>> {
        let conn = self.conn.lock();
        let result: Option<(f64, f64, f64, i64)> = conn
            .query_row(
                "SELECT AVG(CAST(json_extract(metrics, ?1) AS REAL)), \
                 MIN(CAST(json_extract(metrics, ?1) AS REAL)), \
                 MAX(CAST(json_extract(metrics, ?1) AS REAL)), \
                 COUNT(*) \
                 FROM wide_events \
                 WHERE event_type = ?2 AND timestamp >= ?3 AND timestamp <= ?4 \
                 AND json_extract(metrics, ?1) IS NOT NULL",
                rusqlite::params![format!("$.{}", metric_key), event_type, start, end,],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|e| TelemetryError::Backend(format!("aggregate: {e}")))?;

        Ok(result.map(|(avg, min, max, count)| (avg, min, max, count as usize)))
    }
}

#[async_trait]
impl EventSink for SqliteBackend {
    async fn emit(&self, event: WideEvent) -> TelemetryResult<()> {
        let conn = self.conn.lock();
        let timestamp = event.timestamp.timestamp_millis();
        let trace_id = event.trace_id.map(|u| u.to_string());
        let span_id = event.span_id.map(|u| u.to_string());
        let parent_span_id = event.parent_span_id.map(|u| u.to_string());
        let attributes =
            serde_json::to_string(&event.attributes).unwrap_or_else(|_| "{}".to_string());
        let metrics = serde_json::to_string(&event.metrics).unwrap_or_else(|_| "{}".to_string());
        let payload_hash = event.payload_hash();

        conn.execute(
            "INSERT INTO wide_events \
             (timestamp, trace_id, span_id, parent_span_id, service_name, \
              event_type, severity, attributes, metrics, payload_hash) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                timestamp,
                trace_id.as_deref(),
                span_id.as_deref(),
                parent_span_id.as_deref(),
                &event.service_name,
                event.event_type.to_string(),
                serde_json::to_string(&event.severity)
                    .unwrap_or_default()
                    .trim_matches('"'),
                &attributes,
                &metrics,
                &payload_hash,
            ],
        )
        .map_err(|e| TelemetryError::Backend(format!("sqlite insert: {e}")))?;

        Ok(())
    }

    async fn emit_batch(&self, events: Vec<WideEvent>) -> TelemetryResult<()> {
        let mut conn = self.conn.lock();
        let tx = conn
            .transaction()
            .map_err(|e| TelemetryError::Backend(format!("sqlite transaction: {e}")))?;

        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO wide_events \
                 (timestamp, trace_id, span_id, parent_span_id, service_name, \
                  event_type, severity, attributes, metrics, payload_hash) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                )
                .map_err(|e| TelemetryError::Backend(format!("sqlite prepare: {e}")))?;

            for event in events {
                let timestamp = event.timestamp.timestamp_millis();
                let trace_id = event.trace_id.map(|u| u.to_string());
                let span_id = event.span_id.map(|u| u.to_string());
                let parent_span_id = event.parent_span_id.map(|u| u.to_string());
                let attributes =
                    serde_json::to_string(&event.attributes).unwrap_or_else(|_| "{}".to_string());
                let metrics =
                    serde_json::to_string(&event.metrics).unwrap_or_else(|_| "{}".to_string());
                let payload_hash = event.payload_hash();

                stmt.execute(rusqlite::params![
                    timestamp,
                    trace_id.as_deref(),
                    span_id.as_deref(),
                    parent_span_id.as_deref(),
                    &event.service_name,
                    event.event_type.to_string(),
                    serde_json::to_string(&event.severity)
                        .unwrap_or_default()
                        .trim_matches('"'),
                    &attributes,
                    &metrics,
                    &payload_hash,
                ])
                .map_err(|e| TelemetryError::Backend(format!("sqlite batch insert: {e}")))?;
            }
        }

        tx.commit()
            .map_err(|e| TelemetryError::Backend(format!("sqlite commit: {e}")))?;
        Ok(())
    }

    async fn flush(&self) -> TelemetryResult<()> {
        let conn = self.conn.lock();
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        Ok(())
    }

    fn name(&self) -> &str {
        "sqlite"
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn default_db_path() -> TelemetryResult<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| TelemetryError::Config("unable to determine home directory".to_string()))?;
    Ok(home.join(".clarity").join("telemetry.sqlite"))
}

fn row_to_event(row: &rusqlite::Row) -> Result<WideEvent, rusqlite::Error> {
    use chrono::TimeZone;

    let timestamp_ms: i64 = row.get(0)?;
    let trace_id: Option<String> = row.get(1)?;
    let span_id: Option<String> = row.get(2)?;
    let parent_span_id: Option<String> = row.get(3)?;
    let service_name: String = row.get(4)?;
    let event_type_str: String = row.get(5)?;
    let severity_str: String = row.get(6)?;
    let attributes_str: String = row.get(7)?;
    let metrics_str: String = row.get(8)?;
    // payload_hash is column 9, read but not used in reconstruction
    let _payload_hash: String = row.get(9)?;

    let timestamp = Utc
        .timestamp_millis_opt(timestamp_ms)
        .single()
        .unwrap_or_else(Utc::now);

    let trace_id = trace_id.and_then(|s| Uuid::parse_str(&s).ok());
    let span_id = span_id.and_then(|s| Uuid::parse_str(&s).ok());
    let parent_span_id = parent_span_id.and_then(|s| Uuid::parse_str(&s).ok());

    let event_type = serde_json::from_str(&format!("\"{}\"", event_type_str)).unwrap_or_default();
    let severity = serde_json::from_str(&format!("\"{}\"", severity_str)).unwrap_or_default();
    let attributes = serde_json::from_str(&attributes_str).unwrap_or_default();
    let metrics = serde_json::from_str(&metrics_str).unwrap_or_default();

    Ok(WideEvent {
        timestamp,
        trace_id,
        span_id,
        parent_span_id,
        service_name,
        event_type,
        severity,
        attributes,
        metrics,
    })
}

// ============================================================================
// Schema
// ============================================================================

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS wide_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   INTEGER NOT NULL,
    trace_id    TEXT,
    span_id     TEXT,
    parent_span_id TEXT,
    service_name TEXT NOT NULL,
    event_type  TEXT NOT NULL,
    severity    TEXT NOT NULL,
    attributes  TEXT,
    metrics     TEXT,
    payload_hash TEXT
);

CREATE INDEX IF NOT EXISTS idx_wide_events_timestamp ON wide_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_wide_events_type ON wide_events(event_type);
CREATE INDEX IF NOT EXISTS idx_wide_events_service ON wide_events(service_name);
CREATE INDEX IF NOT EXISTS idx_wide_events_trace ON wide_events(trace_id);
CREATE INDEX IF NOT EXISTS idx_wide_events_severity ON wide_events(severity);
"#;
