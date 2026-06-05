//! GreptimeDB HTTP API backend for remote telemetry storage.
//!
//! Uses GreptimeDB's HTTP `v1/sql` endpoint for writes and queries.
//! This avoids pulling in the heavy `greptimedb-client` crate and its
//! transitive dependencies (arrow-flight, tonic, etc.).
//!
//! # Configuration
//!
//! ```toml
//! [telemetry]
//! greptime_enabled = true
//! greptime_url = "http://localhost:4000"
//! greptime_db = "clarity"
//! ```
//!
//! # Write path
//!
//! Events are batched in memory and flushed periodically or on `flush()`.
//! Each flush sends an `INSERT INTO` SQL statement over HTTP.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;

use crate::{EventSink, TelemetryError, TelemetryResult, WideEvent};

// ============================================================================
// GreptimeBackend
// ============================================================================

/// Remote GreptimeDB storage via HTTP API.
pub struct GreptimeBackend {
    client: reqwest::Client,
    base_url: String,
    database: String,
    /// In-memory buffer for batching.
    buffer: Arc<Mutex<Vec<WideEvent>>>,
    /// Maximum buffer size before auto-flush.
    max_buffer_size: usize,
    /// Track consecutive failures for circuit breaker logic.
    consecutive_failures: AtomicUsize,
    /// Max failures before entering open circuit state.
    failure_threshold: usize,
}

impl GreptimeBackend {
    /// Create a new GreptimeDB backend.
    pub fn new(base_url: &str, database: &str) -> TelemetryResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| TelemetryError::Backend(format!("http client: {e}")))?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            database: database.to_string(),
            buffer: Arc::new(Mutex::new(Vec::new())),
            max_buffer_size: 100,
            consecutive_failures: AtomicUsize::new(0),
            failure_threshold: 5,
        })
    }

    /// Set the auto-flush buffer size.
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.max_buffer_size = size;
        self
    }

    /// Check if the circuit breaker is open (too many consecutive failures).
    fn is_circuit_open(&self) -> bool {
        self.consecutive_failures.load(Ordering::Relaxed) >= self.failure_threshold
    }

    /// Build the INSERT SQL for a batch of events.
    fn build_insert_sql(events: &[WideEvent]) -> String {
        let mut sql = String::from(
            "INSERT INTO wide_events \
             (timestamp, trace_id, span_id, parent_span_id, service_name, \
              event_type, severity, attributes, metrics, payload_hash) \
             VALUES ",
        );

        for (i, event) in events.iter().enumerate() {
            if i > 0 {
                sql.push(',');
            }
            let ts = event.timestamp.timestamp_millis();
            let trace = event
                .trace_id
                .map(|u| format!("'{}'", u))
                .unwrap_or_else(|| "NULL".to_string());
            let span = event
                .span_id
                .map(|u| format!("'{}'", u))
                .unwrap_or_else(|| "NULL".to_string());
            let parent = event
                .parent_span_id
                .map(|u| format!("'{}'", u))
                .unwrap_or_else(|| "NULL".to_string());
            let attrs =
                serde_json::to_string(&event.attributes).unwrap_or_else(|_| "{}".to_string());
            let metrics =
                serde_json::to_string(&event.metrics).unwrap_or_else(|_| "{}".to_string());
            let hash = event.payload_hash();

            sql.push_str(&format!(
                "({ts}, {trace}, {span}, {parent}, '{}', '{}', '{}', '{}', '{}', '{}')",
                escape_sql(&event.service_name),
                event.event_type,
                serde_json::to_string(&event.severity)
                    .unwrap_or_default()
                    .trim_matches('"'),
                escape_sql(&attrs),
                escape_sql(&metrics),
                escape_sql(&hash),
            ));
        }

        sql
    }

    /// Send a raw SQL query to GreptimeDB.
    async fn send_sql(&self, sql: &str) -> TelemetryResult<()> {
        let url = format!("{}/v1/sql?db={}", self.base_url, self.database);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("sql={}", urlencoding::encode(sql)))
            .send()
            .await
            .map_err(|e| TelemetryError::Backend(format!("greptime http: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(TelemetryError::Backend(format!(
                "greptime error ({}): {}",
                status, body
            )));
        }

        Ok(())
    }

    /// Ensure the target table exists.
    async fn ensure_table(&self) -> TelemetryResult<()> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS wide_events (
                timestamp TIMESTAMP NOT NULL,
                trace_id STRING,
                span_id STRING,
                parent_span_id STRING,
                service_name STRING,
                event_type STRING,
                severity STRING,
                attributes STRING,
                metrics STRING,
                payload_hash STRING,
                TIME INDEX (timestamp)
            );
        "#;
        self.send_sql(sql).await?;
        Ok(())
    }
}

#[async_trait]
impl EventSink for GreptimeBackend {
    async fn emit(&self, event: WideEvent) -> TelemetryResult<()> {
        {
            let mut buf = self.buffer.lock();
            buf.push(event);
            if buf.len() < self.max_buffer_size {
                return Ok(());
            }
        }
        // Buffer is full — flush.
        self.flush().await
    }

    async fn emit_batch(&self, events: Vec<WideEvent>) -> TelemetryResult<()> {
        {
            let mut buf = self.buffer.lock();
            buf.extend(events);
            if buf.len() < self.max_buffer_size {
                return Ok(());
            }
        }
        self.flush().await
    }

    async fn flush(&self) -> TelemetryResult<()> {
        if self.is_circuit_open() {
            // Circuit breaker is open — silently drop the flush.
            // Events remain in the SQLite fallback if MultiSink is used.
            return Ok(());
        }

        let events: Vec<WideEvent> = {
            let mut buf = self.buffer.lock();
            if buf.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *buf)
        };

        let sql = Self::build_insert_sql(&events);

        match self.send_sql(&sql).await {
            Ok(()) => {
                self.consecutive_failures.store(0, Ordering::Relaxed);
                Ok(())
            }
            Err(e) => {
                self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
        }
    }

    fn name(&self) -> &str {
        "greptimedb"
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn escape_sql(s: &str) -> String {
    s.replace('\'', "''")
}
