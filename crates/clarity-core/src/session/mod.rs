//! Session management — V1/V2 storage and migration.

#[cfg(feature = "session-migration")]
pub mod migration;
#[cfg(feature = "session-migration")]
pub mod thread_migration;
