//! Shared helpers for the OpenClaw client implementation.

use std::time::Duration;

/// Compute the exponential backoff delay for the Nth connection failure.
///
/// Sequence (1-indexed failures): 1s, 2s, 4s, 8s, 16s, 30s, 30s, ...
/// ponytail: duplicated in client.rs and gateway_client.rs; unified here.
pub fn next_backoff(failure_count: usize) -> Duration {
    let secs = 2usize
        .saturating_pow(failure_count.saturating_sub(1) as u32)
        .min(30);
    Duration::from_secs(secs as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_progression_capped_at_thirty_seconds() {
        let expected = [1, 2, 4, 8, 16, 30, 30];
        for (i, &secs) in expected.iter().enumerate() {
            let failures = i + 1;
            assert_eq!(next_backoff(failures).as_secs(), secs);
        }
    }
}
