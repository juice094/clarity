//! Telemetry storage backends.

#[cfg(feature = "sqlite")]
/// Local-first SQLite backend.
pub mod sqlite;

#[cfg(feature = "greptime")]
/// Remote GreptimeDB HTTP backend.
pub mod greptime;
