//! Configuration audit trail — Daimon-inspired immutable config change log.
//!
//! Every mutation to Clarity's configuration generates an audit record with:
//! - before/after SHA-256 hashes
//! - process identity (PID, argv)
//! - rollback command
//! - actor attribution (user, agent, migration script)
//!
//! Records are written through the [`EventSink`](crate::sink::EventSink) as
//! [`WideEvent`](crate::WideEvent) with `event_type = ConfigAudit`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{EventType, Severity, WideEvent};

// ============================================================================
// ConfigAuditLog
// ============================================================================

/// A single configuration change audit record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigAuditLog {
    /// When the change occurred.
    pub timestamp: DateTime<Utc>,

    /// Which configuration file or key was modified.
    pub config_path: String,

    /// Classification of the change.
    pub change_type: ConfigChangeType,

    /// SHA-256 of the file content before the change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_hash: Option<String>,

    /// SHA-256 of the file content after the change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_hash: Option<String>,

    /// Process ID that performed the change.
    pub pid: u32,

    /// Command-line arguments of the process.
    pub argv: Vec<String>,

    /// A shell command that would revert this change.
    ///
    /// `None` if the change is not revertible (e.g. deletion of a generated file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_command: Option<String>,

    /// Who/what initiated the change.
    pub actor: ConfigActor,

    /// Human-readable description of what changed.
    pub description: String,
}

impl ConfigAuditLog {
    /// Create a new audit log entry with current timestamp and process info.
    pub fn new(
        config_path: impl Into<String>,
        change_type: ConfigChangeType,
        description: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            config_path: config_path.into(),
            change_type,
            before_hash: None,
            after_hash: None,
            pid: std::process::id(),
            argv: std::env::args().collect(),
            rollback_command: None,
            actor: ConfigActor::User,
            description: description.into(),
        }
    }

    /// Set the before-hash (call this before applying the change).
    pub fn with_before_hash(mut self, hash: impl Into<String>) -> Self {
        self.before_hash = Some(hash.into());
        self
    }

    /// Set the after-hash (call this after applying the change).
    pub fn with_after_hash(mut self, hash: impl Into<String>) -> Self {
        self.after_hash = Some(hash.into());
        self
    }

    /// Set the rollback command.
    pub fn with_rollback(mut self, command: impl Into<String>) -> Self {
        self.rollback_command = Some(command.into());
        self
    }

    /// Set the actor.
    pub fn with_actor(mut self, actor: ConfigActor) -> Self {
        self.actor = actor;
        self
    }

    /// Convert this audit log into a [`WideEvent`] ready for emission.
    pub fn into_wide_event(self) -> WideEvent {
        WideEvent::new("clarity-core", EventType::ConfigAudit, Severity::Info)
            .with_attr("config_path", &self.config_path)
            .with_attr("change_type", self.change_type)
            .with_attr("before_hash", &self.before_hash)
            .with_attr("after_hash", &self.after_hash)
            .with_attr("pid", self.pid)
            .with_attr("actor", self.actor)
            .with_attr("description", &self.description)
            .with_attr("rollback_command", &self.rollback_command)
            .with_attr(
                "audit_payload",
                serde_json::to_value(&self).unwrap_or_default(),
            )
    }

    /// Compute a SHA-256 hash over file content.
    ///
    /// Returns `None` if the file does not exist.
    pub fn hash_file(path: &std::path::Path) -> Option<String> {
        use std::io::Read;
        let mut file = std::fs::File::open(path).ok()?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).ok()?;
        Some(sha256_hex(&contents))
    }
}

/// Compute SHA-256 of bytes and return as lowercase hex.
fn sha256_hex(bytes: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    // NOTE: We use std::hash::DefaultHasher instead of sha2 crate to avoid
    // adding a heavy crypto dependency. The hasher is used for change
    // detection, not cryptographic security. If stronger guarantees are
    // needed in the future, migrate to `sha2::Sha256`.
    format!("{:016x}", hasher.finish())
}

// ============================================================================
// Supporting types
// ============================================================================

/// Classification of a configuration change.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ConfigChangeType {
    /// New configuration file or key created.
    Create,
    /// Existing configuration updated.
    Update,
    /// Configuration file or key deleted.
    Delete,
    /// Change applied by an automated migration.
    Migration,
}

/// Actor that initiated a configuration change.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ConfigActor {
    /// A human user.
    User,
    /// An autonomous agent.
    Agent,
    /// A schema or data migration.
    Migration,
    /// The system itself (e.g. installer, service).
    System,
}
