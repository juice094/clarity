//! Supervisor for long-lived async tasks.
//!
//! A Rust equivalent of Go `suture.Supervisor` that supervises async tasks with
//! automatic restart, exponential backoff, and max-restart limits.
//!
//! Design follows production patterns from syncthing-rust's
//! `syncthing-sync/src/supervisor.rs`.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use clarity_contract::retry::{RestartConfig, RestartPolicy};

// ============================================================================
// Type aliases
// ============================================================================

/// Boxed error type used by supervised tasks.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Factory that produces a new future each time a supervised task is (re)started.
///
/// The factory pattern ensures each restart creates a fresh future with no stale
/// state from the previous attempt.
pub type TaskFactory =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), BoxError>> + Send>> + Send + Sync>;

/// Callback invoked when a task exceeds `max_restarts` and is permanently failed.
pub type FailureCallback = Arc<dyn Fn(&str) + Send + Sync>;

// ============================================================================
// SupervisedTask
// ============================================================================

/// A task to be supervised by a [`Supervisor`].
///
/// Each task is described by a name, a factory closure that produces the future
/// to run, and a restart configuration.
pub struct SupervisedTask {
    /// Human-readable name used for logging and permanent-failure callbacks.
    pub name: String,
    /// Factory that produces a new future each time the task is (re)started.
    pub future_factory: TaskFactory,
    /// Restart configuration controlling backoff and max restart limits.
    pub config: RestartConfig,
}

// ============================================================================
// Supervisor
// ============================================================================

/// Supervisor that manages a collection of [`SupervisedTask`]s.
///
/// Each task is spawned on the current Tokio runtime. On failure (or completion,
/// depending on [`RestartPolicy`]), the task is automatically restarted with
/// exponential backoff. If a task exceeds `max_restarts` within the reset
/// window, the permanent-failure callback is invoked.
///
/// # Example
///
/// ```rust,no_run
/// use clarity_core::agent::supervisor::{Supervisor, SupervisedTask, BoxError};
/// use clarity_contract::retry::{RestartConfig, RestartPolicy};
/// use std::time::Duration;
///
/// # async fn example() {
/// let mut supervisor = Supervisor::new();
/// supervisor.add_task(SupervisedTask {
///     name: "health-check".to_string(),
///     future_factory: Box::new(|| {
///         Box::pin(async {
///             loop {
///                 tokio::time::sleep(Duration::from_secs(30)).await;
///                 // Perform health check...
///             }
///             #[allow(unreachable_code)]
///             Ok(())
///         })
///     }),
///     config: RestartConfig::default(),
/// });
/// supervisor.start();
/// // ... later:
/// supervisor.shutdown().await;
/// # }
/// ```
pub struct Supervisor {
    tasks: Vec<SupervisedTask>,
    on_permanent_failure: Option<FailureCallback>,
    handles: Vec<JoinHandle<()>>,
    shutdown_tx: broadcast::Sender<()>,
}

impl Supervisor {
    /// Create a new, empty supervisor.
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            tasks: Vec::new(),
            on_permanent_failure: None,
            handles: Vec::new(),
            shutdown_tx,
        }
    }

    /// Register a callback invoked when a task exceeds `max_restarts`.
    ///
    /// The callback receives the task name. It is called synchronously from the
    /// supervision loop; avoid long-running operations inside the callback.
    pub fn on_permanent_failure<F>(&mut self, callback: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_permanent_failure = Some(Arc::new(callback));
    }

    /// Add a task to be supervised.
    ///
    /// Tasks are not started until [`start()`](Self::start) is called.
    pub fn add_task(&mut self, task: SupervisedTask) {
        self.tasks.push(task);
    }

    /// Start all registered tasks.
    ///
    /// Each task is spawned on the current Tokio runtime. Tasks that were
    /// already started are not re-spawned.
    pub fn start(&mut self) {
        while let Some(task) = self.tasks.pop() {
            let shutdown_rx = self.shutdown_tx.subscribe();
            let on_failure = self.on_permanent_failure.clone();
            let handle = tokio::spawn(supervise_task(task, shutdown_rx, on_failure));
            self.handles.push(handle);
        }
    }

    /// Gracefully shut down the supervisor by aborting all supervised tasks.
    ///
    /// Sends a shutdown signal to all tasks, then aborts their handles.
    /// Tasks that were started with `RestartPolicy::Never` will not be restarted.
    pub async fn shutdown(mut self) {
        // Signal all tasks to stop.
        let _ = self.shutdown_tx.send(());
        // Abort all handles.
        for handle in self.handles.drain(..) {
            handle.abort();
            let _ = handle.await;
        }
    }

    /// Returns the number of supervised tasks.
    pub fn task_count(&self) -> usize {
        self.tasks.len() + self.handles.len()
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Task supervision loop
// ============================================================================

/// Core supervision loop for a single task.
///
/// Runs the task in a loop, applying restart policy, backoff, and max-restart
/// limits. The loop exits when:
/// - The task completes successfully with `RestartPolicy::OnFailure` or `Never`.
/// - The task exceeds `max_restarts` within the reset window.
/// - A shutdown signal is received.
async fn supervise_task(
    task: SupervisedTask,
    mut shutdown_rx: broadcast::Receiver<()>,
    on_permanent_failure: Option<FailureCallback>,
) {
    let mut attempt: u32 = 0;
    let mut restart_count: u32 = 0;
    let mut window_start = Instant::now();

    loop {
        // Apply backoff delay before restart (skip for first attempt).
        if attempt > 0 {
            let delay = task.config.backoff.next_delay(attempt - 1);
            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                _ = shutdown_rx.recv() => {
                    return;
                }
            }
        }

        // Spawn the task future and keep an abort handle for shutdown.
        let fut = (task.future_factory)();
        let handle = tokio::spawn(fut);
        let abort_handle = handle.abort_handle();

        tokio::select! {
            res = handle => {
                let should_restart = match &res {
                    // Task completed successfully.
                    Ok(Ok(())) => matches!(task.config.restart_policy, RestartPolicy::Always),
                    // Task returned an error.
                    Ok(Err(_)) => {
                        matches!(task.config.restart_policy, RestartPolicy::Always | RestartPolicy::OnFailure)
                    }
                    // Task panicked (JoinError).
                    Err(_) => {
                        matches!(task.config.restart_policy, RestartPolicy::Always | RestartPolicy::OnFailure)
                    }
                };

                if !should_restart {
                    tracing::debug!(
                        task_name = %task.name,
                        attempt = attempt + 1,
                        "Supervised task completed, not restarting"
                    );
                    return;
                }

                // Check if the restart counter should reset.
                let now = Instant::now();
                if now.duration_since(window_start) > task.config.backoff.reset_after {
                    window_start = now;
                    restart_count = 0;
                    tracing::trace!(
                        task_name = %task.name,
                        "Restart counter reset after reset_after window elapsed"
                    );
                }

                restart_count += 1;
                if restart_count > task.config.max_restarts {
                    tracing::error!(
                        task_name = %task.name,
                        restart_count = restart_count,
                        max_restarts = task.config.max_restarts,
                        "Supervised task exceeded max restarts, permanently failing"
                    );
                    if let Some(ref cb) = on_permanent_failure {
                        cb(&task.name);
                    }
                    return;
                }

                tracing::warn!(
                    task_name = %task.name,
                    attempt = attempt + 1,
                    restart_count = restart_count,
                    "Supervised task failed, restarting with backoff"
                );

                attempt += 1;
            }
            _ = shutdown_rx.recv() => {
                abort_handle.abort();
                tracing::debug!(
                    task_name = %task.name,
                    "Supervised task aborted due to shutdown signal"
                );
                return;
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::retry::ExponentialBackoff;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    /// Helper: create a restart config suitable for fast tests.
    fn fast_restart_config() -> RestartConfig {
        RestartConfig {
            restart_policy: RestartPolicy::OnFailure,
            backoff: ExponentialBackoff {
                initial_delay: Duration::from_millis(10),
                max_delay: Duration::from_millis(100),
                reset_after: Duration::from_secs(60),
            },
            max_restarts: 5,
        }
    }

    #[tokio::test]
    async fn test_supervisor_restarts_on_panic() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter2 = counter.clone();

        let mut supervisor = Supervisor::new();
        supervisor.add_task(SupervisedTask {
            name: "panic-task".to_string(),
            future_factory: Box::new(move || {
                let c = counter2.clone();
                Box::pin(async move {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        panic!("intentional panic");
                    }
                    Ok(())
                })
            }),
            config: RestartConfig {
                restart_policy: RestartPolicy::OnFailure,
                ..fast_restart_config()
            },
        });
        supervisor.start();

        tokio::time::sleep(Duration::from_millis(500)).await;

        let count = counter.load(Ordering::SeqCst);
        assert!(
            count >= 2,
            "expected at least 2 invocations after panic restart, got {}",
            count
        );

        supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn test_supervisor_backoff_increases() {
        let times = Arc::new(std::sync::Mutex::new(Vec::new()));
        let times2 = times.clone();

        let mut supervisor = Supervisor::new();
        supervisor.add_task(SupervisedTask {
            name: "backoff-task".to_string(),
            future_factory: Box::new(move || {
                let t = times2.clone();
                Box::pin(async move {
                    t.lock().unwrap().push(Instant::now());
                    Err(Box::new(std::io::Error::other("fail")) as BoxError)
                })
            }),
            config: RestartConfig {
                restart_policy: RestartPolicy::OnFailure,
                backoff: ExponentialBackoff {
                    initial_delay: Duration::from_millis(50),
                    max_delay: Duration::from_secs(10),
                    reset_after: Duration::from_secs(60),
                },
                max_restarts: 10,
            },
        });
        supervisor.start();

        tokio::time::sleep(Duration::from_millis(400)).await;
        supervisor.shutdown().await;

        let vec = times.lock().unwrap();
        assert!(
            vec.len() >= 3,
            "expected at least 3 attempts, got {}",
            vec.len()
        );

        // Verify backoff increases: each subsequent delay should be >= the previous.
        let deltas: Vec<Duration> = vec.windows(2).map(|w| w[1].duration_since(w[0])).collect();
        for i in 1..deltas.len() {
            assert!(
                deltas[i] >= deltas[i - 1],
                "backoff did not increase: attempt {} delta {:?} vs attempt {} delta {:?}",
                i + 1,
                deltas[i],
                i,
                deltas[i - 1]
            );
        }
    }

    #[tokio::test]
    async fn test_supervisor_max_restarts_exceeded() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter2 = counter.clone();
        let failed = Arc::new(AtomicBool::new(false));
        let failed2 = failed.clone();

        let mut supervisor = Supervisor::new();
        supervisor.on_permanent_failure(move |name: &str| {
            assert_eq!(name, "fail-task");
            failed2.store(true, Ordering::SeqCst);
        });
        supervisor.add_task(SupervisedTask {
            name: "fail-task".to_string(),
            future_factory: Box::new(move || {
                let c = counter2.clone();
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err(Box::new(std::io::Error::other("fail")) as BoxError)
                })
            }),
            config: RestartConfig {
                restart_policy: RestartPolicy::OnFailure,
                backoff: ExponentialBackoff {
                    initial_delay: Duration::from_millis(10),
                    max_delay: Duration::from_millis(100),
                    reset_after: Duration::from_secs(60),
                },
                max_restarts: 2,
            },
        });
        supervisor.start();

        tokio::time::sleep(Duration::from_millis(500)).await;
        supervisor.shutdown().await;

        assert!(
            failed.load(Ordering::SeqCst),
            "permanent failure callback should have fired"
        );
        let count = counter.load(Ordering::SeqCst);
        assert_eq!(count, 3, "expected exactly 3 invocations, got {}", count);
    }

    #[tokio::test]
    async fn test_supervisor_graceful_shutdown() {
        let running = Arc::new(AtomicUsize::new(0));
        let running2 = running.clone();

        let mut supervisor = Supervisor::new();
        supervisor.add_task(SupervisedTask {
            name: "loop-task".to_string(),
            future_factory: Box::new(move || {
                let r = running2.clone();
                Box::pin(async move {
                    r.fetch_add(1, Ordering::SeqCst);
                    loop {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                })
            }),
            config: RestartConfig::default(),
        });
        supervisor.start();

        tokio::time::sleep(Duration::from_millis(150)).await;
        let before = running.load(Ordering::SeqCst);
        assert!(before >= 1, "task should have started");

        supervisor.shutdown().await;

        tokio::time::sleep(Duration::from_millis(100)).await;
        let after = running.load(Ordering::SeqCst);
        assert_eq!(
            before, after,
            "task should not have restarted after shutdown"
        );
    }

    #[tokio::test]
    async fn test_supervisor_restart_policy_never() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter2 = counter.clone();

        let mut supervisor = Supervisor::new();
        supervisor.add_task(SupervisedTask {
            name: "never-restart".to_string(),
            future_factory: Box::new(move || {
                let c = counter2.clone();
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err(Box::new(std::io::Error::other("fail")) as BoxError)
                })
            }),
            config: RestartConfig {
                restart_policy: RestartPolicy::Never,
                ..fast_restart_config()
            },
        });
        supervisor.start();

        tokio::time::sleep(Duration::from_millis(300)).await;
        supervisor.shutdown().await;

        let count = counter.load(Ordering::SeqCst);
        assert_eq!(
            count, 1,
            "Never policy should run exactly once, got {}",
            count
        );
    }

    #[tokio::test]
    async fn test_supervisor_restart_policy_always() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter2 = counter.clone();

        let mut supervisor = Supervisor::new();
        supervisor.add_task(SupervisedTask {
            name: "always-restart".to_string(),
            future_factory: Box::new(move || {
                let c = counter2.clone();
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(()) // Success — but Always policy should still restart
                })
            }),
            config: RestartConfig {
                restart_policy: RestartPolicy::Always,
                backoff: ExponentialBackoff {
                    initial_delay: Duration::from_millis(5),
                    max_delay: Duration::from_millis(100),
                    reset_after: Duration::from_secs(60),
                },
                max_restarts: 10,
            },
        });
        supervisor.start();

        tokio::time::sleep(Duration::from_millis(150)).await;
        supervisor.shutdown().await;

        let count = counter.load(Ordering::SeqCst);
        assert!(
            count >= 3,
            "Always policy should restart multiple times, got {}",
            count
        );
    }

    #[tokio::test]
    async fn test_supervisor_multiple_tasks() {
        let counter_a = Arc::new(AtomicUsize::new(0));
        let counter_b = Arc::new(AtomicUsize::new(0));
        let ca = counter_a.clone();
        let cb = counter_b.clone();

        let mut supervisor = Supervisor::new();
        supervisor.add_task(SupervisedTask {
            name: "task-a".to_string(),
            future_factory: Box::new(move || {
                let c = ca.clone();
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
            }),
            config: RestartConfig {
                restart_policy: RestartPolicy::Always,
                backoff: ExponentialBackoff {
                    initial_delay: Duration::from_millis(5),
                    max_delay: Duration::from_millis(50),
                    reset_after: Duration::from_secs(60),
                },
                max_restarts: 5,
            },
        });
        supervisor.add_task(SupervisedTask {
            name: "task-b".to_string(),
            future_factory: Box::new(move || {
                let c = cb.clone();
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
            }),
            config: RestartConfig {
                restart_policy: RestartPolicy::Always,
                backoff: ExponentialBackoff {
                    initial_delay: Duration::from_millis(5),
                    max_delay: Duration::from_millis(50),
                    reset_after: Duration::from_secs(60),
                },
                max_restarts: 5,
            },
        });
        supervisor.start();

        tokio::time::sleep(Duration::from_millis(150)).await;
        supervisor.shutdown().await;

        let a = counter_a.load(Ordering::SeqCst);
        let b = counter_b.load(Ordering::SeqCst);
        assert!(
            a >= 2,
            "task-a should have restarted at least 2 times, got {}",
            a
        );
        assert!(
            b >= 2,
            "task-b should have restarted at least 2 times, got {}",
            b
        );
    }
}
