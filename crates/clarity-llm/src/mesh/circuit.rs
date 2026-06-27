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
    /// Create a new circuit breaker with the given failure threshold and recovery timeout.
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
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let should_try = self
                    .last_failure
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .is_some_and(|t| t.elapsed() >= self.recovery_timeout);
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
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.failure_count.store(0, Ordering::SeqCst);
        *state = CircuitState::Closed;
    }

    /// Record a failed call — increment count and possibly open the circuit.
    pub fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        if count >= self.threshold {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = CircuitState::Open;
        }
        *self.last_failure.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    }

    /// Return the current circuit state.
    pub fn state(&self) -> CircuitState {
        *self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, 30)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_starts_closed() {
        let cb = CircuitBreaker::new(3, 1);
        assert!(cb.allow());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_opens_after_threshold() {
        let cb = CircuitBreaker::new(2, 60);
        cb.record_failure();
        assert!(cb.allow()); // still closed after 1 failure
        cb.record_failure();
        assert!(!cb.allow()); // open after 2 failures
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_closes_after_success() {
        let cb = CircuitBreaker::new(2, 60);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        cb.record_success();
        assert!(cb.allow());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_after_timeout() {
        let cb = CircuitBreaker::new(1, 0); // 0s recovery timeout
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        std::thread::sleep(Duration::from_millis(10));
        assert!(cb.allow()); // should transition to HalfOpen
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }
}
