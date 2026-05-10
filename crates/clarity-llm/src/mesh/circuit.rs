//! Simple circuit breaker for LLM provider failover.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    /// Normal operation — requests pass through.
    Closed,
    /// Failure threshold reached — requests are rejected fast.
    Open,
    /// Testing whether the provider has recovered.
    HalfOpen,
}

/// Per-provider circuit breaker.
pub struct CircuitBreaker {
    state: Mutex<CircuitState>,
    failure_count: AtomicU32,
    last_failure: Mutex<Option<Instant>>,
    threshold: u32,
    recovery_timeout: Duration,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, recovery_timeout_secs: u64) -> Self {
        Self {
            state: Mutex::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            last_failure: Mutex::new(None),
            threshold,
            recovery_timeout: Duration::from_secs(recovery_timeout_secs),
        }
    }

    /// Check whether the circuit allows a request through.
    pub fn allow(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let should_try = self.last_failure.lock().unwrap().map_or(false, |t| {
                    t.elapsed() >= self.recovery_timeout
                });
                if should_try {
                    *state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful call — reset failure count and close the circuit.
    pub fn record_success(&self) {
        let mut state = self.state.lock().unwrap();
        self.failure_count.store(0, Ordering::SeqCst);
        *state = CircuitState::Closed;
    }

    /// Record a failed call — increment count and possibly open the circuit.
    pub fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        if count >= self.threshold {
            let mut state = self.state.lock().unwrap();
            *state = CircuitState::Open;
        }
        *self.last_failure.lock().unwrap() = Some(Instant::now());
    }

    pub fn state(&self) -> CircuitState {
        *self.state.lock().unwrap()
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, 30)
    }
}
