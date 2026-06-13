//! Shared retry / backoff policy for HTTP-based channel operations.
//!
//! `RetryPolicy` provides a generic, platform-agnostic exponential backoff helper
//! that channel implementations can use to wrap outbound network calls. Errors
//! opt-in to retry classification by implementing [`RetryableError`].

use std::error::Error;
use std::future::Future;
use std::time::Duration;

use tokio::time::sleep;

/// Default number of attempts for a retryable operation.
const DEFAULT_MAX_ATTEMPTS: u32 = 3;

/// Default base delay before the first retry.
const DEFAULT_BASE_DELAY_MS: u64 = 500;

/// Default maximum delay between retries.
const DEFAULT_MAX_DELAY_MS: u64 = 10_000;

/// Default exponential multiplier.
const DEFAULT_MULTIPLIER: f64 = 2.0;

/// An error that can be classified as transient and worth retrying.
pub trait RetryableError: Error + Send + Sync + 'static {
    /// Returns `true` if the operation should be retried.
    fn is_retryable(&self) -> bool;
}

/// Exponential backoff retry policy.
///
/// The policy is stateless and can be reused across many operations. It executes
/// a fallible async closure and retries while the error reports itself as
/// retryable, up to `max_attempts` total attempts.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of attempts (including the initial call).
    pub max_attempts: u32,
    /// Delay before the first retry.
    pub base_delay: Duration,
    /// Hard cap on the delay between any two attempts.
    pub max_delay: Duration,
    /// Exponential multiplier applied to `base_delay` after each attempt.
    pub multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            base_delay: Duration::from_millis(DEFAULT_BASE_DELAY_MS),
            max_delay: Duration::from_millis(DEFAULT_MAX_DELAY_MS),
            multiplier: DEFAULT_MULTIPLIER,
        }
    }
}

impl RetryPolicy {
    /// Create a retry policy with the library defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of attempts.
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }

    /// Set the base delay before the first retry.
    pub fn with_base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }

    /// Set the maximum delay between retries.
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set the exponential multiplier.
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier.max(1.0);
        self
    }

    /// Execute an async operation with retries.
    ///
    /// The operation closure is called once for each attempt. If it returns an
    /// error that [`RetryableError::is_retryable`] reports as `true`, and the
    /// attempt budget has not been exhausted, the policy sleeps according to the
    /// configured exponential backoff and tries again.
    pub async fn execute<F, Fut, T, E>(&self, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> Fut + Send,
        Fut: Future<Output = Result<T, E>> + Send,
        E: RetryableError + Send + Sync + std::fmt::Display,
    {
        let mut attempt: u32 = 1;

        loop {
            match operation().await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    if attempt >= self.max_attempts || !err.is_retryable() {
                        return Err(err);
                    }

                    let delay = self.delay_for_attempt(attempt);
                    tracing::warn!(
                        error = %err,
                        attempt,
                        max_attempts = self.max_attempts,
                        delay_ms = delay.as_millis() as u64,
                        "channel operation failed, retrying"
                    );
                    sleep(delay).await;
                    attempt += 1;
                }
            }
        }
    }

    fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let exponent = (attempt.saturating_sub(1)) as i32;
        let raw_ms = self.base_delay.as_millis() as f64 * self.multiplier.powi(exponent);
        let capped_ms = raw_ms.min(self.max_delay.as_millis() as f64);
        Duration::from_millis(capped_ms.max(0.0) as u64)
    }
}

impl RetryableError for super::ChannelError {
    fn is_retryable(&self) -> bool {
        match self {
            super::ChannelError::Network(_) => true,
            super::ChannelError::Platform { code, .. } => *code >= 500 || *code == 429,
            super::ChannelError::SendFailed(_) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, thiserror::Error)]
    enum TestError {
        #[error("transient")]
        Retryable,
        #[error("fatal")]
        Fatal,
    }

    impl RetryableError for TestError {
        fn is_retryable(&self) -> bool {
            matches!(self, TestError::Retryable)
        }
    }

    #[tokio::test]
    async fn test_retry_succeeds_on_second_attempt() {
        let counter = Arc::new(AtomicUsize::new(0));
        let policy = RetryPolicy::new().with_base_delay(Duration::from_millis(1));

        let result = policy
            .execute({
                let counter = counter.clone();
                move || {
                    let counter = counter.clone();
                    async move {
                        let n = counter.fetch_add(1, Ordering::SeqCst);
                        if n == 0 {
                            Err(TestError::Retryable)
                        } else {
                            Ok("success")
                        }
                    }
                }
            })
            .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_gives_up_after_max_attempts() {
        let counter = Arc::new(AtomicUsize::new(0));
        let policy = RetryPolicy::new()
            .with_max_attempts(3)
            .with_base_delay(Duration::from_millis(1));

        let result = policy
            .execute({
                let counter = counter.clone();
                move || {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Err::<(), _>(TestError::Retryable)
                    }
                }
            })
            .await;

        assert!(matches!(result, Err(TestError::Retryable)));
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_no_retry_on_non_retryable_error() {
        let counter = Arc::new(AtomicUsize::new(0));
        let policy = RetryPolicy::new();

        let result = policy
            .execute({
                let counter = counter.clone();
                move || {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Err::<(), _>(TestError::Fatal)
                    }
                }
            })
            .await;

        assert!(matches!(result, Err(TestError::Fatal)));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retryable_channel_error_classification() {
        use super::super::ChannelError;

        let counter = Arc::new(AtomicUsize::new(0));
        let policy = RetryPolicy::new()
            .with_max_attempts(2)
            .with_base_delay(Duration::from_millis(1));

        let result = policy
            .execute({
                let counter = counter.clone();
                move || {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Err::<(), ChannelError>(ChannelError::SendFailed("transient".to_string()))
                    }
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        assert!(
            ChannelError::Platform {
                code: 503,
                message: "unavailable".to_string(),
            }
            .is_retryable()
        );

        assert!(
            ChannelError::Platform {
                code: 429,
                message: "rate limited".to_string(),
            }
            .is_retryable()
        );

        assert!(!ChannelError::AuthFailed("bad token".to_string()).is_retryable());
        assert!(!ChannelError::ConfigError("missing".to_string()).is_retryable());
    }

    #[test]
    fn test_delay_grows_and_caps() {
        let policy = RetryPolicy::new()
            .with_base_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_millis(300))
            .with_multiplier(2.0);

        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(300));
        assert_eq!(policy.delay_for_attempt(10), Duration::from_millis(300));
    }
}
