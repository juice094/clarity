//! AutoDream — nightly memory consolidation scheduler
//!
//! Automatically triggers memory consolidation during low-activity periods
//! (typically nighttime). Uses the existing `CronScheduler` for timing and
//! `clarity_memory::MemoryCompiler` for the actual consolidation work.
//!
//! # Example
//!
//! ```rust,no_run
//! use clarity_core::autodream::{AutoDream, AutoDreamConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = AutoDreamConfig::default();
//!     let autodream = AutoDream::new(config);
//!
//!     // The dream task will run every day at 02:00
//!     autodream.start(|report| async move {
//!         println!("Dream report: {}", report.summary);
//!         Ok(())
//!     }).await?;
//!
//!     Ok(())
//! }
//! ```

use std::future::Future;
use std::str::FromStr;
use std::time::Duration;
use thiserror::Error;
use tracing::{info, warn};

/// Errors that can occur during AutoDream operations
#[derive(Debug, Error)]
pub enum AutoDreamError {
    /// Invalid cron expression
    #[error("Invalid cron expression: {0}")]
    InvalidCron(String),
    /// Consolidation task failed
    #[error("Consolidation failed: {0}")]
    ConsolidationFailed(String),
    /// Scheduler error
    #[error("Scheduler error: {0}")]
    SchedulerError(String),
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Configuration for AutoDream scheduling and consolidation
#[derive(Debug, Clone)]
pub struct AutoDreamConfig {
    /// Cron expression for when to run consolidation (default: "0 0 2 * * *" = 02:00 daily)
    pub cron_expr: String,
    /// Whether AutoDream is enabled
    pub enabled: bool,
    /// Timeout for a single consolidation run
    pub timeout_secs: u64,
    /// Maximum number of consolidation levels to run (1=today, 2=+week, 3=+longterm, 4=+facts)
    pub max_levels: u8,
}

impl Default for AutoDreamConfig {
    fn default() -> Self {
        Self {
            cron_expr: "0 0 2 * * *".to_string(), // 02:00 every day
            enabled: true,
            timeout_secs: 600, // 10 minutes
            max_levels: 4,
        }
    }
}

impl AutoDreamConfig {
    /// Create a new config with the given cron expression
    pub fn with_cron(mut self, expr: impl Into<String>) -> Self {
        self.cron_expr = expr.into();
        self
    }

    /// Disable AutoDream
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set the consolidation timeout
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set the maximum consolidation levels
    pub fn with_max_levels(mut self, levels: u8) -> Self {
        self.max_levels = levels.clamp(1, 4);
        self
    }
}

/// A report generated after a consolidation run
#[derive(Debug, Clone, Default)]
pub struct DreamReport {
    /// Summary of what was consolidated
    pub summary: String,
    /// Number of sessions processed
    pub sessions_processed: usize,
    /// Number of facts extracted
    pub facts_extracted: usize,
    /// Duration of the run in milliseconds
    pub elapsed_ms: u64,
    /// Whether the run succeeded
    pub success: bool,
}

impl DreamReport {
    /// Create a new successful report
    pub fn success(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            success: true,
            ..Default::default()
        }
    }

    /// Create a failure report
    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            summary: error.into(),
            success: false,
            ..Default::default()
        }
    }

    /// Add session count
    pub fn with_sessions(mut self, count: usize) -> Self {
        self.sessions_processed = count;
        self
    }

    /// Add fact count
    pub fn with_facts(mut self, count: usize) -> Self {
        self.facts_extracted = count;
        self
    }

    /// Add elapsed time
    pub fn with_elapsed_ms(mut self, ms: u64) -> Self {
        self.elapsed_ms = ms;
        self
    }
}

/// AutoDream scheduler and runner
///
/// Manages the cron-based scheduling of memory consolidation tasks.
/// The actual consolidation logic is provided by a user-supplied callback,
/// allowing flexibility in how memories are processed.
pub struct AutoDream {
    config: AutoDreamConfig,
}

impl AutoDream {
    /// Create a new AutoDream instance with the given configuration
    pub fn new(config: AutoDreamConfig) -> Self {
        Self { config }
    }

    /// Start the AutoDream scheduler
    ///
    /// The `consolidate` callback is invoked each time the cron expression fires.
    /// It should perform the actual memory consolidation and return a `DreamReport`.
    ///
    /// This method blocks until the scheduler is stopped or an unrecoverable error occurs.
    pub async fn start<F, Fut>(&self, consolidate: F) -> Result<(), AutoDreamError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<DreamReport, AutoDreamError>> + Send + 'static,
    {
        if !self.config.enabled {
            info!("AutoDream is disabled, not starting scheduler");
            return Ok(());
        }

        // Validate cron expression by attempting to parse it
        let schedule = cron::Schedule::from_str(&self.config.cron_expr)
            .map_err(|e| AutoDreamError::InvalidCron(format!("{}", e)))?;

        info!(
            "AutoDream scheduler started. Next run: {:?}",
            schedule.upcoming(chrono::Utc).next()
        );

        // Simple loop: sleep until next scheduled time, then run
        loop {
            let now = chrono::Utc::now();
            let next = schedule
                .upcoming(chrono::Utc)
                .next()
                .ok_or_else(|| AutoDreamError::SchedulerError("No upcoming schedule time".to_string()))?;

            let wait_duration = (next - now).to_std().unwrap_or(Duration::from_secs(60));
            info!("AutoDream next run at {} (waiting {:?})", next, wait_duration);

            tokio::time::sleep(wait_duration).await;

            info!("AutoDream starting consolidation run...");
            let start = std::time::Instant::now();

            let result = tokio::time::timeout(
                Duration::from_secs(self.config.timeout_secs),
                consolidate(),
            )
            .await;

            match result {
                Ok(Ok(report)) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    info!(
                        "AutoDream consolidation completed in {}ms: {} (sessions={}, facts={})",
                        elapsed, report.summary, report.sessions_processed, report.facts_extracted
                    );
                }
                Ok(Err(e)) => {
                    warn!("AutoDream consolidation failed: {}", e);
                }
                Err(_) => {
                    warn!(
                        "AutoDream consolidation timed out after {}s",
                        self.config.timeout_secs
                    );
                }
            }
        }
    }

    /// Run a single consolidation immediately (for manual triggering)
    pub async fn run_once<F, Fut>(&self, consolidate: F) -> Result<DreamReport, AutoDreamError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<DreamReport, AutoDreamError>> + Send + 'static,
    {
        if !self.config.enabled {
            return Err(AutoDreamError::ConsolidationFailed(
                "AutoDream is disabled".to_string(),
            ));
        }

        info!("AutoDream running single consolidation...");
        let start = std::time::Instant::now();

        let result = tokio::time::timeout(
            Duration::from_secs(self.config.timeout_secs),
            consolidate(),
        )
        .await;

        match result {
            Ok(Ok(report)) => {
                let elapsed = start.elapsed().as_millis() as u64;
                info!(
                    "AutoDream single run completed in {}ms: {}",
                    elapsed, report.summary
                );
                Ok(report.with_elapsed_ms(elapsed))
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(AutoDreamError::ConsolidationFailed(format!(
                "Timed out after {}s",
                self.config.timeout_secs
            ))),
        }
    }

    /// Validate that the configured cron expression is parseable
    pub fn validate_cron(&self) -> Result<(), AutoDreamError> {
        cron::Schedule::from_str(&self.config.cron_expr)
            .map_err(|e| AutoDreamError::InvalidCron(format!("{}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_config_default() {
        let config = AutoDreamConfig::default();
        assert_eq!(config.cron_expr, "0 0 2 * * *");
        assert!(config.enabled);
        assert_eq!(config.timeout_secs, 600);
        assert_eq!(config.max_levels, 4);
    }

    #[test]
    fn test_config_builder() {
        let config = AutoDreamConfig::default()
            .with_cron("0 0 3 * * *")
            .with_timeout(300)
            .with_max_levels(2)
            .disabled();

        assert_eq!(config.cron_expr, "0 0 3 * * *");
        assert_eq!(config.timeout_secs, 300);
        assert_eq!(config.max_levels, 2);
        assert!(!config.enabled);
    }

    #[test]
    fn test_validate_cron_valid() {
        let autodream = AutoDream::new(AutoDreamConfig::default());
        assert!(autodream.validate_cron().is_ok());
    }

    #[test]
    fn test_validate_cron_invalid() {
        let config = AutoDreamConfig::default().with_cron("not-a-cron");
        let autodream = AutoDream::new(config);
        assert!(autodream.validate_cron().is_err());
    }

    #[tokio::test]
    async fn test_run_once_success() {
        let autodream = AutoDream::new(AutoDreamConfig::default());
        let result = autodream
            .run_once(|| async {
                Ok(DreamReport::success("Consolidated 5 sessions").with_sessions(5).with_facts(12))
            })
            .await;

        assert!(result.is_ok());
        let report = result.unwrap();
        assert!(report.success);
        assert_eq!(report.sessions_processed, 5);
        assert_eq!(report.facts_extracted, 12);
    }

    #[tokio::test]
    async fn test_run_once_disabled() {
        let config = AutoDreamConfig::default().disabled();
        let autodream = AutoDream::new(config);
        let result = autodream
            .run_once(|| async { Ok(DreamReport::success("test")) })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_once_timeout() {
        let config = AutoDreamConfig::default().with_timeout(1);
        let autodream = AutoDream::new(config);
        let result = autodream
            .run_once(|| async {
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok(DreamReport::success("too late"))
            })
            .await;

        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Timed out"));
    }

    #[tokio::test]
    async fn test_start_triggers_callback() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        // Use a cron expression that fires immediately (every second)
        let config = AutoDreamConfig::default()
            .with_cron("* * * * * *")
            .with_timeout(5);
        let autodream = AutoDream::new(config);

        // Run for a short time then cancel
        let handle = tokio::spawn(async move {
            autodream
                .start(move || {
                    let c = counter_clone.clone();
                    async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        Ok(DreamReport::success("tick"))
                    }
                })
                .await
        });

        // Let it fire a couple of times
        tokio::time::sleep(Duration::from_millis(2500)).await;
        handle.abort();

        let count = counter.load(Ordering::SeqCst);
        assert!(
            count >= 1,
            "Expected at least 1 trigger, got {}",
            count
        );
    }
}
