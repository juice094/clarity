//! Rate-limit handling helpers for HTTP-based LLM providers.

use std::time::Duration;

/// Maximum duration we will wait when a provider asks us to back off.
const MAX_RETRY_AFTER_SECONDS: u64 = 60;

/// Parse a `Retry-After` header value into a capped [`Duration`].
///
/// Supports the delay-seconds form (`"10"`). HTTP-date form is accepted but
/// currently treated as opaque / unparseable; providers in this codebase
/// overwhelmingly emit seconds.
pub(crate) fn parse_retry_after(value: &str) -> Option<Duration> {
    let trimmed = value.trim();
    trimmed
        .parse::<u64>()
        .ok()
        .map(|secs| Duration::from_secs(secs.min(MAX_RETRY_AFTER_SECONDS)))
}

/// Return `true` for status codes that commonly carry a meaningful
/// `Retry-After` header.
fn should_honor_retry_after(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status == reqwest::StatusCode::SERVICE_UNAVAILABLE
}

/// If the response indicates rate limiting and the provider supplied a
/// `Retry-After` header, sleep for the requested duration (capped).
///
/// This is called *before* the response body is consumed, so it must take the
/// response by reference.
pub(crate) async fn wait_for_retry_after(response: &reqwest::Response) {
    if !should_honor_retry_after(response.status()) {
        return;
    }
    if let Some(value) = response.headers().get(reqwest::header::RETRY_AFTER) {
        if let Ok(text) = value.to_str() {
            if let Some(delay) = parse_retry_after(text) {
                tracing::debug!(
                    status = %response.status(),
                    ?delay,
                    "Provider asked us to back off; sleeping before returning error"
                );
                tokio::time::sleep(delay).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_retry_after_seconds() {
        assert_eq!(parse_retry_after("10"), Some(Duration::from_secs(10)));
    }

    #[test]
    fn test_parse_retry_after_caps_at_max() {
        assert_eq!(parse_retry_after("120"), Some(Duration::from_secs(60)));
        assert_eq!(parse_retry_after("60"), Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_parse_retry_after_zero_is_allowed() {
        assert_eq!(parse_retry_after("0"), Some(Duration::from_secs(0)));
    }

    #[test]
    fn test_parse_retry_after_rejects_garbage() {
        assert_eq!(parse_retry_after("soon"), None);
        assert_eq!(parse_retry_after(""), None);
    }
}
