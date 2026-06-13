//! Configuration audit trail — Daimon-inspired immutable config change log.
//!
//! Every mutation to Clarity's configuration generates an audit record with
//! before/after hashes, process identity, rollback command, and actor attribution.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use clarity_core::config::audit::{ConfigAudit, ConfigChangeType};
//!
//! let mut audit = ConfigAudit::new();
//! audit.record_change(
//!     "~/.clarity/config.toml",
//!     ConfigChangeType::Update,
//!     "changed default model to gpt-4",
//!     None,
//!     None,
//! );
//! ```

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

    /// Content hash before the change (if file existed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_hash: Option<String>,

    /// Content hash after the change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_hash: Option<String>,

    /// Process ID that performed the change.
    pub pid: u32,

    /// Command-line arguments of the process.
    pub argv: Vec<String>,

    /// Shell command that would revert this change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_command: Option<String>,

    /// Who/what initiated the change.
    pub actor: ConfigActor,

    /// Human-readable description.
    pub description: String,
}

impl ConfigAuditLog {
    /// Start building a new audit log entry.
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

    /// Set the before-hash (call this **before** applying the change).
    pub fn with_before_hash(mut self, hash: impl Into<String>) -> Self {
        self.before_hash = Some(hash.into());
        self
    }

    /// Set the after-hash (call this **after** applying the change).
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
}

// ============================================================================
// Supporting types
// ============================================================================

/// Classification of a configuration change.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ConfigChangeType {
    /// Configuration created.
    Create,
    /// Configuration updated.
    Update,
    /// Configuration deleted.
    Delete,
    /// Configuration migration.
    Migration,
}

/// Actor that initiated a configuration change.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ConfigActor {
    /// User-initiated change.
    User,
    /// Agent-initiated change.
    Agent,
    /// Configuration migration.
    Migration,
    /// System-initiated change.
    System,
}

// ============================================================================
// ConfigAudit
// ============================================================================

/// Mutable audit trail that writes to a JSONL log file.
pub struct ConfigAudit {
    /// In-memory buffer of recent logs (flushed on demand or drop).
    buffer: Vec<ConfigAuditLog>,
    /// Path to the audit log file.
    log_path: std::path::PathBuf,
}

impl ConfigAudit {
    /// Create a new audit trail with the default log path.
    pub fn new() -> Self {
        Self::with_path(Self::default_path())
    }

    /// Create a new audit trail with a custom log path.
    pub fn with_path(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            buffer: Vec::new(),
            log_path: path.into(),
        }
    }

    /// Default audit log path: `~/.clarity/logs/config-audit.jsonl`.
    pub fn default_path() -> std::path::PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".clarity")
            .join("logs")
            .join("config-audit.jsonl")
    }

    /// Record a configuration change.
    ///
    /// The caller should compute `before_hash` **before** mutation and
    /// `after_hash` **after** mutation, then pass both to this method.
    pub fn record_change(
        &mut self,
        config_path: impl Into<String>,
        change_type: ConfigChangeType,
        description: impl Into<String>,
        before_hash: Option<String>,
        after_hash: Option<String>,
    ) {
        let log = ConfigAuditLog::new(config_path, change_type, description)
            .with_before_hash(before_hash.unwrap_or_default())
            .with_after_hash(after_hash.unwrap_or_default());
        self.buffer.push(log);
    }

    /// Flush the in-memory buffer to the JSONL log file.
    pub fn flush(&mut self) -> std::io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        if let Some(parent) = self.log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;

        for log in &self.buffer {
            let line = serde_json::to_string(log)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            use std::io::Write;
            writeln!(file, "{}", line)?;
        }

        self.buffer.clear();
        Ok(())
    }

    /// Read all audit logs from the file.
    pub fn read_logs(&self) -> std::io::Result<Vec<ConfigAuditLog>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let contents = std::fs::read_to_string(&self.log_path)?;
        let mut logs = Vec::new();
        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(log) = serde_json::from_str(line) {
                logs.push(log);
            }
        }
        Ok(logs)
    }
}

impl Default for ConfigAudit {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Hash helper
// ============================================================================

/// Compute a fast content hash for file integrity checks.
///
/// Uses std::hash (not cryptographic). Suitable for change detection,
/// not for security guarantees.
pub fn hash_content(bytes: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Compute the hash of a file, returning `None` if the file does not exist.
pub fn hash_file(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|b| hash_content(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_audit_log_builder() {
        let log = ConfigAuditLog::new("config.toml", ConfigChangeType::Update, "test change")
            .with_before_hash("abc")
            .with_after_hash("def")
            .with_rollback("git checkout config.toml")
            .with_actor(ConfigActor::Agent);

        assert_eq!(log.config_path, "config.toml");
        assert_eq!(log.change_type, ConfigChangeType::Update);
        assert_eq!(log.before_hash, Some("abc".to_string()));
        assert_eq!(log.after_hash, Some("def".to_string()));
        assert_eq!(
            log.rollback_command,
            Some("git checkout config.toml".to_string())
        );
        assert_eq!(log.actor, ConfigActor::Agent);
        assert_eq!(log.pid, std::process::id());
    }

    #[test]
    fn test_hash_content_deterministic() {
        let h1 = hash_content(b"hello");
        let h2 = hash_content(b"hello");
        let h3 = hash_content(b"world");

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_audit_roundtrip() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut audit = ConfigAudit::with_path(tmp.path());

        audit.record_change(
            "config.toml",
            ConfigChangeType::Create,
            "initial config",
            None,
            Some("hash1".to_string()),
        );
        audit.record_change(
            "config.toml",
            ConfigChangeType::Update,
            "update model",
            Some("hash1".to_string()),
            Some("hash2".to_string()),
        );
        audit.flush().unwrap();

        let logs = audit.read_logs().unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].change_type, ConfigChangeType::Create);
        assert_eq!(logs[1].change_type, ConfigChangeType::Update);
    }
}
