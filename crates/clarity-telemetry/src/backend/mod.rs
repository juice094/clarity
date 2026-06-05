//! Telemetry storage backends.

#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "greptime")]
pub mod greptime;
