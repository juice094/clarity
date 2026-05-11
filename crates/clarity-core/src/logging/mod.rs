//! Tracing subscriber helpers with automatic credential redaction.
//!
//! All functions in this module initialise a [`tracing_subscriber::fmt`]
//! subscriber that writes to **stderr** and scrubs API keys, tokens,
//! passwords and other secrets before they reach the log stream.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use clarity_core::logging;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Equivalent to `tracing_subscriber::fmt::init()` but with redaction.
//!     logging::init();
//!
//!     tracing::info!(api_key = "sk-1234567890abcdef"); // printed as [REDACTED]
//! }
//! ```

mod redacting_writer;

pub use redacting_writer::{RedactingStderr, RedactingWriter};

use tracing_subscriber::EnvFilter;

/// Initialise the default subscriber with credential redaction.
///
/// Reads `RUST_LOG` via [`EnvFilter::from_default_env`].  If the variable
/// is missing or invalid the default level is `info`.
///
/// This is a drop-in replacement for `tracing_subscriber::fmt::init()`.
pub fn init() {
    let filter = EnvFilter::from_default_env();
    init_with_filter(filter);
}

/// Initialise the subscriber with a fallback default filter string.
///
/// First attempts to read `RUST_LOG`; if that fails or is unset the
/// provided `default` string is used.
pub fn init_with_default(default: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    init_with_filter(filter);
}

/// Initialise the subscriber with an explicit [`EnvFilter`].
pub fn init_with_filter(filter: EnvFilter) {
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(RedactingStderr)
        .init();
}
