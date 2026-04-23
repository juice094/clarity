//! Turn-based memory compilation trigger (OpenHanako-style)

use crate::types::{CompileStatus, Result};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};

/// Number of turns before triggering a summary
pub const DEFAULT_TURNS_PER_SUMMARY: u32 = 6;

/// Type alias for compilation callback
///
/// Note: This doesn't require Send because SQLite connections are not Send.
/// The callback will be executed on the same thread.
pub type CompileCallback = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<HashMap<String, CompileStatus>>> + Send>> + Send + Sync,
>;

/// Turn-based memory compilation trigger
///
/// Tracks the number of turns (message exchanges) in each session
/// and triggers compilation when the threshold is reached.
pub struct MemoryTicker {
    /// Maps session paths to their current turn count
    turn_count: HashMap<String, u32>,
    /// Threshold for triggering compilation
    turns_per_summary: u32,
    /// Output directory for compiled memories
    output_dir: PathBuf,
    /// Callback for triggering compilation
    compile_callback: Option<CompileCallback>,
    /// Tracks if compilation is currently running for each session
    compiling: HashMap<String, bool>,
}

/// Future type returned by notify_turn
///
/// Note: This doesn't require Send because SQLite connections are not Send.
pub type CompilationFuture = Pin<Box<dyn Future<Output = Result<HashMap<String, CompileStatus>>> + Send>>;

impl MemoryTicker {
    /// Create a new MemoryTicker without a compile callback
    ///
    /// Use `set_compile_callback` to set the callback later.
    pub fn new(output_dir: impl AsRef<Path>, turns_per_summary: Option<u32>) -> Self {
        Self {
            turn_count: HashMap::new(),
            turns_per_summary: turns_per_summary.unwrap_or(DEFAULT_TURNS_PER_SUMMARY),
            output_dir: output_dir.as_ref().to_path_buf(),
            compile_callback: None,
            compiling: HashMap::new(),
        }
    }

    /// Set the compilation callback
    pub fn set_compile_callback<F, Fut>(&mut self, callback: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<HashMap<String, CompileStatus>>> + Send + 'static,
    {
        self.compile_callback = Some(Arc::new(move || Box::pin(callback()) as CompilationFuture));
    }

    /// Notify the ticker of a new turn in a session
    ///
    /// Returns a future that will run compilation if the threshold is reached.
    /// Returns None if no compilation is triggered.
    #[instrument(skip(self))]
    pub fn notify_turn(&mut self, session_path: &str) -> Option<CompilationFuture> {
        // Increment turn count
        let count = self.turn_count.entry(session_path.to_string()).or_insert(0);
        *count += 1;

        debug!(
            "Turn {} for session {}, threshold is {}",
            *count, session_path, self.turns_per_summary
        );

        // Check if we should trigger compilation
        if *count >= self.turns_per_summary {
            // Check if already compiling for this session
            if self.compiling.get(session_path).copied().unwrap_or(false) {
                warn!("Compilation already in progress for {}", session_path);
                return None;
            }

            // Reset counter and mark as compiling
            *count = 0;
            self.compiling.insert(session_path.to_string(), true);

            info!("Triggering compilation for session {}", session_path);

            // Get callback
            let callback = self.compile_callback.clone()?;
            let _session = session_path.to_string();

            let future = Box::pin(async move {
                let result = callback().await;
                // Note: The caller should reset the compiling flag via check_and_reset
                result
            });

            Some(future)
        } else {
            None
        }
    }

    /// Notify of a turn and immediately await compilation if triggered
    #[instrument(skip(self))]
    pub async fn notify_turn_and_wait(
        &mut self,
        session_path: &str,
    ) -> Option<Result<HashMap<String, CompileStatus>>> {
        if let Some(future) = self.notify_turn(session_path) {
            let result = future.await;
            self.compiling.insert(session_path.to_string(), false);
            Some(result)
        } else {
            None
        }
    }

    /// Get the current turn count for a session
    pub fn get_turn_count(&self, session_path: &str) -> u32 {
        self.turn_count.get(session_path).copied().unwrap_or(0)
    }

    /// Reset turn count for a session
    pub fn reset_turn_count(&mut self, session_path: &str) {
        self.turn_count.remove(session_path);
        self.compiling.remove(session_path);
        debug!("Reset turn count for {}", session_path);
    }

    /// Reset all turn counts
    pub fn reset_all(&mut self) {
        self.turn_count.clear();
        self.compiling.clear();
        debug!("Reset all turn counts");
    }

    /// Set the turns per summary threshold
    pub fn set_threshold(&mut self, threshold: u32) {
        self.turns_per_summary = threshold;
        info!("Set turns per summary to {}", threshold);
    }

    /// Get the current threshold
    pub fn threshold(&self) -> u32 {
        self.turns_per_summary
    }

    /// Check if compilation is currently running for a session
    pub fn is_compiling(&self, session_path: &str) -> bool {
        self.compiling.get(session_path).copied().unwrap_or(false)
    }

    /// Get all tracked session paths
    pub fn get_sessions(&self) -> Vec<String> {
        self.turn_count.keys().cloned().collect()
    }

    /// Get compilation status for all sessions
    pub fn get_compilation_status(&self) -> HashMap<String, bool> {
        self.compiling.clone()
    }

    /// Get output directory
    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }
}

/// Thread-safe version of MemoryTicker for use across async boundaries
#[derive(Clone)]
pub struct SharedMemoryTicker {
    inner: Arc<Mutex<MemoryTicker>>,
}

impl SharedMemoryTicker {
    /// Create a new shared ticker
    pub fn new(ticker: MemoryTicker) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ticker)),
        }
    }

    /// Notify of a turn and potentially trigger compilation
    pub async fn notify_turn(&self, session_path: &str) -> Option<CompilationFuture> {
        let mut ticker = self.inner.lock().await;
        ticker.notify_turn(session_path)
    }

    /// Notify and wait for compilation
    pub async fn notify_turn_and_wait(
        &self,
        session_path: &str,
    ) -> Option<Result<HashMap<String, CompileStatus>>> {
        let mut ticker = self.inner.lock().await;
        ticker.notify_turn_and_wait(session_path).await
    }

    /// Get turn count for a session
    pub async fn get_turn_count(&self, session_path: &str) -> u32 {
        let ticker = self.inner.lock().await;
        ticker.get_turn_count(session_path)
    }

    /// Reset turn count
    pub async fn reset_turn_count(&self, session_path: &str) {
        let mut ticker = self.inner.lock().await;
        ticker.reset_turn_count(session_path);
    }

    /// Set threshold
    pub async fn set_threshold(&self, threshold: u32) {
        let mut ticker = self.inner.lock().await;
        ticker.set_threshold(threshold);
    }

    /// Set compile callback
    pub async fn set_compile_callback<F, Fut>(&self, callback: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<HashMap<String, CompileStatus>>> + Send + 'static,
    {
        let mut ticker = self.inner.lock().await;
        ticker.set_compile_callback(callback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_turn_counting() {
        let temp_dir = TempDir::new().unwrap();
        let mut ticker = MemoryTicker::new(temp_dir.path(), Some(3));

        // Set a no-op callback
        ticker.set_compile_callback(|| async { Ok(HashMap::new()) });

        assert_eq!(ticker.get_turn_count("session-1"), 0);

        // First 2 turns should not trigger
        assert!(ticker.notify_turn("session-1").is_none());
        assert_eq!(ticker.get_turn_count("session-1"), 1);

        assert!(ticker.notify_turn("session-1").is_none());
        assert_eq!(ticker.get_turn_count("session-1"), 2);

        // Third turn should trigger
        assert!(ticker.notify_turn("session-1").is_some());
        assert_eq!(ticker.get_turn_count("session-1"), 0); // Reset after trigger
    }

    #[test]
    fn test_multiple_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let mut ticker = MemoryTicker::new(temp_dir.path(), Some(3));
        ticker.set_compile_callback(|| async { Ok(HashMap::new()) });

        // Track turns independently
        ticker.notify_turn("session-a");
        ticker.notify_turn("session-a");
        ticker.notify_turn("session-b");

        assert_eq!(ticker.get_turn_count("session-a"), 2);
        assert_eq!(ticker.get_turn_count("session-b"), 1);

        // Trigger only for session-a
        assert!(ticker.notify_turn("session-a").is_some());
        assert!(ticker.notify_turn("session-b").is_none());

        assert_eq!(ticker.get_turn_count("session-a"), 0);
        assert_eq!(ticker.get_turn_count("session-b"), 2);
    }

    #[test]
    fn test_reset() {
        let temp_dir = TempDir::new().unwrap();
        let mut ticker = MemoryTicker::new(temp_dir.path(), Some(3));
        ticker.set_compile_callback(|| async { Ok(HashMap::new()) });

        ticker.notify_turn("session-1");
        ticker.notify_turn("session-1");
        assert_eq!(ticker.get_turn_count("session-1"), 2);

        ticker.reset_turn_count("session-1");
        assert_eq!(ticker.get_turn_count("session-1"), 0);

        // Should need 3 more turns to trigger
        assert!(ticker.notify_turn("session-1").is_none());
        assert!(ticker.notify_turn("session-1").is_none());
        assert!(ticker.notify_turn("session-1").is_some());
    }

    #[test]
    fn test_threshold_configuration() {
        let temp_dir = TempDir::new().unwrap();
        let mut ticker = MemoryTicker::new(temp_dir.path(), Some(3));
        ticker.set_compile_callback(|| async { Ok(HashMap::new()) });

        // Change threshold to 5
        ticker.set_threshold(5);
        assert_eq!(ticker.threshold(), 5);

        // Should need 5 turns now
        for _ in 0..4 {
            assert!(ticker.notify_turn("session-1").is_none());
        }
        assert!(ticker.notify_turn("session-1").is_some());
    }

    #[test]
    fn test_default_threshold() {
        let temp_dir = TempDir::new().unwrap();
        let mut ticker = MemoryTicker::new(temp_dir.path(), None);
        ticker.set_compile_callback(|| async { Ok(HashMap::new()) });

        assert_eq!(ticker.threshold(), DEFAULT_TURNS_PER_SUMMARY);
    }

    #[tokio::test]
    async fn test_shared_ticker() {
        let temp_dir = TempDir::new().unwrap();
        let mut ticker = MemoryTicker::new(temp_dir.path(), Some(2));
        ticker.set_compile_callback(|| async { Ok(HashMap::new()) });

        let shared = SharedMemoryTicker::new(ticker);

        assert_eq!(shared.get_turn_count("session-1").await, 0);

        shared.notify_turn("session-1").await;
        assert_eq!(shared.get_turn_count("session-1").await, 1);

        shared.notify_turn("session-1").await;
        assert_eq!(shared.get_turn_count("session-1").await, 0); // Triggered and reset
    }

    #[tokio::test]
    async fn test_callback_invocation() {
        let temp_dir = TempDir::new().unwrap();
        let mut ticker = MemoryTicker::new(temp_dir.path(), Some(2));

        // Track if callback was called
        let call_count = Arc::new(Mutex::new(0));
        let call_count_clone = Arc::clone(&call_count);

        ticker.set_compile_callback(move || {
            let count = Arc::clone(&call_count_clone);
            async move {
                let mut c = count.lock().await;
                *c += 1;
                Ok(HashMap::new())
            }
        });

        // First turn - no trigger
        ticker.notify_turn("session-1");
        assert_eq!(*call_count.lock().await, 0);

        // Second turn - triggers
        let future = ticker.notify_turn("session-1").expect("Should trigger");
        future.await.ok();

        assert_eq!(*call_count.lock().await, 1);
    }
}
