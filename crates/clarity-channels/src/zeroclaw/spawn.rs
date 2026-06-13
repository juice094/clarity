//! Sanctioned interface against `tokio::spawn`.
//!
//! Mirrors `zeroclaw_spawn::spawn!`: propagates the current tracing span
//! and emits a lifecycle record when the task completes.

/// Event name used for task spawn/completion lifecycle records.
pub const TASK_EVENT_NAME: &str = "runtime.task.spawn";

/// Spawn a future onto the current tokio runtime with span propagation.
#[macro_export]
macro_rules! spawn {
    ($body:expr) => {{
        use ::tracing::Instrument as _;
        use $crate::zeroclaw::log::{Action, Event, EventOutcome};

        const __ZC_TASK_MODULE: &'static str = module_path!();
        const __ZC_TASK_FILE: &'static str = file!();
        const __ZC_TASK_LINE: u32 = line!();

        $crate::record!(
            INFO,
            Event::new($crate::zeroclaw::spawn::TASK_EVENT_NAME, Action::Spawn)
                .with_attrs(::serde_json::json!({
                    "task_site": format!("{}:{}", __ZC_TASK_FILE, __ZC_TASK_LINE),
                    "task_module": __ZC_TASK_MODULE,
                })),
            "task spawned"
        );

        let __zc_task_started_at = ::tokio::time::Instant::now();
        let __zc_task_future = async move {
            let __zc_task_output = { $body }.await;
            let __zc_task_elapsed_ms = __zc_task_started_at.elapsed().as_millis() as u64;
            $crate::record!(
                INFO,
                Event::new($crate::zeroclaw::spawn::TASK_EVENT_NAME, Action::Complete)
                    .with_outcome(EventOutcome::Success)
                    .with_attrs(::serde_json::json!({
                        "task_site": format!("{}:{}", __ZC_TASK_FILE, __ZC_TASK_LINE),
                        "task_module": __ZC_TASK_MODULE,
                        "duration_ms": __zc_task_elapsed_ms,
                    })),
                "task completed"
            );
            __zc_task_output
        };

        ::tokio::spawn(__zc_task_future.in_current_span())
    }};
}

/// Re-export the macro at the `zeroclaw::spawn` path.
#[doc(hidden)]
pub use crate::spawn;
