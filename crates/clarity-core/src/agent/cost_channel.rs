//! Process-level cost bypass channel.
//!
//! Background tasks (subagents, compaction, memory operations) run in separate
//! Agent instances or async tasks. Their token costs are invisible to the main
//! Agent's budget tracker. This module provides a global pending-cost buffer
//! that any component can report into, and the primary Agent drains each turn.
//!
//! Design: `OnceLock<Mutex<f64>>` — thread-safe, lazy-initialised, zero-cost
//! when unused. No channels or async required.

use std::sync::Mutex;
use std::sync::OnceLock;

static PENDING_COST_USD: OnceLock<Mutex<f64>> = OnceLock::new();

fn ensure_init() -> &'static Mutex<f64> {
    PENDING_COST_USD.get_or_init(|| Mutex::new(0.0))
}

/// Report a cost (USD) from any component in the process.
///
/// Called by subagents, compaction service, memory operations, etc.
/// The cost is accumulated in a global pending buffer until the main
/// Agent drains it at the start of its next turn.
pub fn report_cost(cost: f64) {
    if cost <= 0.0 {
        return;
    }
    if let Ok(mut guard) = ensure_init().lock() {
        *guard += cost;
    }
}

/// Drain all pending cost and return the total.
///
/// Call this once per main-Agent turn (e.g. inside `record_cost` or
/// at turn start). Returns 0.0 if nothing is pending.
pub fn drain_pending_cost() -> f64 {
    if let Ok(mut guard) = ensure_init().lock() {
        let cost = *guard;
        *guard = 0.0;
        cost
    } else {
        0.0
    }
}

/// Peek at the current pending cost without draining.
///
/// Useful for budget pre-checks.
pub fn pending_cost() -> f64 {
    if let Ok(guard) = ensure_init().lock() {
        *guard
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_and_drain() {
        // Ensure a fresh state for this test.
        let _ = drain_pending_cost();
        report_cost(0.5);
        report_cost(0.3);
        assert!((pending_cost() - 0.8).abs() < f64::EPSILON);
        assert!((drain_pending_cost() - 0.8).abs() < f64::EPSILON);
        assert_eq!(pending_cost(), 0.0);
    }

    #[test]
    fn test_drain_empty() {
        let _ = drain_pending_cost();
        assert_eq!(drain_pending_cost(), 0.0);
    }

    #[test]
    fn test_negative_and_zero_cost_ignored() {
        let _ = drain_pending_cost();
        report_cost(-1.0);
        report_cost(0.0);
        assert_eq!(pending_cost(), 0.0);
    }
}
