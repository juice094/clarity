//! EventSink abstraction — the primary interface for emitting wide events.
//!
//! Consumers (agents, LLM providers, tools, gateways) hold an `Arc<dyn EventSink>`
//! and call `emit(event).await` without knowledge of the underlying storage.

use std::sync::Arc;

use async_trait::async_trait;

use crate::{TelemetryResult, WideEvent};

// ============================================================================
// EventSink trait
// ============================================================================

/// Abstraction over telemetry storage backends.
///
/// Implementations must be `Send + Sync` so they can be shared across
/// async tasks and threads. The `emit` method is async to allow for
/// network I/O (GreptimeDB) or disk I/O (SQLite) without blocking.
#[async_trait]
pub trait EventSink: Send + Sync {
    /// Emit a single wide event.
    ///
    /// # Errors
    ///
    /// Returns `TelemetryError::Backend` if the underlying storage fails.
    /// Callers should **not** propagate this error to users — telemetry
    /// write failures are non-fatal by design.
    async fn emit(&self, event: WideEvent) -> TelemetryResult<()>;

    /// Emit a batch of events atomically where possible.
    ///
    /// The default implementation calls `emit` sequentially. Backends that
    /// support true batching (e.g. SQLite transactions, GreptimeDB bulk insert)
    /// should override this for better throughput.
    async fn emit_batch(&self, events: Vec<WideEvent>) -> TelemetryResult<()> {
        for event in events {
            self.emit(event).await?;
        }
        Ok(())
    }

    /// Flush any buffered events to persistent storage.
    ///
    /// Called on graceful shutdown and after critical operations.
    async fn flush(&self) -> TelemetryResult<()> {
        Ok(())
    }

    /// Human-readable backend name for diagnostics.
    fn name(&self) -> &str;
}

// ============================================================================
// MultiSink — fan-out to multiple backends
// ============================================================================

/// A composite sink that forwards events to multiple backends.
///
/// Use this when you want events to land in both local SQLite **and**
/// remote GreptimeDB simultaneously. Errors from individual sinks are
/// collected but do not short-circuit other sinks.
#[derive(Default)]
pub struct MultiSink {
    sinks: Vec<Arc<dyn EventSink>>,
}

impl MultiSink {
    /// Create an empty multi-sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a backend sink into this multi-sink.
    pub fn with_sink(mut self, sink: Arc<dyn EventSink>) -> Self {
        self.sinks.push(sink);
        self
    }

    /// Build from a configuration.
    pub fn from_config(config: &SinkConfig) -> TelemetryResult<Arc<dyn EventSink>> {
        // Intentionally retained because `multi` is only mutated when backend
        // features are enabled; without them it would trigger an unused_mut warning.
        #[allow(unused_mut)]
        let mut multi = Self::new();

        #[cfg(feature = "sqlite")]
        if config.sqlite_enabled {
            let sqlite = crate::backend::sqlite::SqliteBackend::new(config.sqlite_path.as_deref())?;
            multi = multi.with_sink(Arc::new(sqlite));
        }

        #[cfg(feature = "greptime")]
        if config.greptime_enabled {
            let greptime = crate::backend::greptime::GreptimeBackend::new(
                config
                    .greptime_url
                    .as_deref()
                    .unwrap_or("http://localhost:4000"),
                config.greptime_db.as_deref().unwrap_or("clarity"),
            )?;
            multi = multi.with_sink(Arc::new(greptime));
        }

        if multi.sinks.is_empty() {
            // SAFETY: No backend configured. Install a no-op sink so callers
            // don't need to handle `Option<Arc<dyn EventSink>>` everywhere.
            return Ok(Arc::new(NoOpSink));
        }

        Ok(Arc::new(multi))
    }
}

#[async_trait]
impl EventSink for MultiSink {
    async fn emit(&self, event: WideEvent) -> TelemetryResult<()> {
        let mut last_err = None;
        for sink in &self.sinks {
            if let Err(e) = sink.emit(event.clone()).await {
                // NOTE: We deliberately do not short-circuit on error.
                // One backend failing (e.g. network partition to GreptimeDB)
                // must not prevent local SQLite from receiving the event.
                last_err = Some(e);
            }
        }
        match last_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    async fn emit_batch(&self, events: Vec<WideEvent>) -> TelemetryResult<()> {
        let mut last_err = None;
        for sink in &self.sinks {
            if let Err(e) = sink.emit_batch(events.clone()).await {
                last_err = Some(e);
            }
        }
        match last_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    async fn flush(&self) -> TelemetryResult<()> {
        let mut last_err = None;
        for sink in &self.sinks {
            if let Err(e) = sink.flush().await {
                last_err = Some(e);
            }
        }
        match last_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn name(&self) -> &str {
        "multi"
    }
}

// ============================================================================
// NoOpSink — used when no backend is configured
// ============================================================================

struct NoOpSink;

#[async_trait]
impl EventSink for NoOpSink {
    async fn emit(&self, _event: WideEvent) -> TelemetryResult<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "noop"
    }
}

// ============================================================================
// SinkConfig — declarative configuration
// ============================================================================

/// Configuration for initializing telemetry sinks.
///
/// Loaded from `~/.clarity/config.toml` under the `[telemetry]` section.
#[derive(Debug, Clone, Default)]
pub struct SinkConfig {
    /// Enable the SQLite local backend.
    pub sqlite_enabled: bool,
    /// Path to the SQLite database file. `None` means default (`~/.clarity/telemetry.sqlite`).
    pub sqlite_path: Option<String>,

    /// Enable the GreptimeDB remote backend.
    pub greptime_enabled: bool,
    /// GreptimeDB HTTP API endpoint.
    pub greptime_url: Option<String>,
    /// GreptimeDB database name.
    pub greptime_db: Option<String>,
}
