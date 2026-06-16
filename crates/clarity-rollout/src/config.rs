//! Rollout configuration.
//!
//! Modeled after `codex_rollout::config` from the OpenAI Codex project, licensed
//! under Apache-2.0. See `NOTICES.md` for attribution.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Read-only view of rollout-related configuration.
pub trait RolloutConfigView {
    /// Root directory for Clarity runtime data.
    fn clarity_home(&self) -> &Path;
    /// Directory for SQLite metadata indices.
    fn sqlite_home(&self) -> &Path;
    /// Current working directory.
    fn cwd(&self) -> &Path;
    /// Active model provider identifier.
    fn model_provider_id(&self) -> &str;
    /// Whether memory generation is enabled.
    fn generate_memories(&self) -> bool;
}

/// Concrete rollout configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RolloutConfig {
    /// Root directory for Clarity runtime data.
    pub clarity_home: PathBuf,
    /// Directory for SQLite metadata indices.
    pub sqlite_home: PathBuf,
    /// Current working directory.
    pub cwd: PathBuf,
    /// Active model provider identifier.
    pub model_provider_id: String,
    /// Whether memory generation is enabled.
    pub generate_memories: bool,
}

impl RolloutConfig {
    /// Build a concrete config from any view.
    pub fn from_view(view: &impl RolloutConfigView) -> Self {
        Self {
            clarity_home: view.clarity_home().to_path_buf(),
            sqlite_home: view.sqlite_home().to_path_buf(),
            cwd: view.cwd().to_path_buf(),
            model_provider_id: view.model_provider_id().to_string(),
            generate_memories: view.generate_memories(),
        }
    }

    /// Directory where active rollout files are stored.
    pub fn sessions_dir(&self) -> PathBuf {
        self.clarity_home.join("sessions")
    }

    /// Directory where archived rollout files are stored.
    pub fn archived_sessions_dir(&self) -> PathBuf {
        self.clarity_home.join("archived_sessions")
    }
}

impl RolloutConfigView for RolloutConfig {
    fn clarity_home(&self) -> &Path {
        self.clarity_home.as_path()
    }

    fn sqlite_home(&self) -> &Path {
        self.sqlite_home.as_path()
    }

    fn cwd(&self) -> &Path {
        self.cwd.as_path()
    }

    fn model_provider_id(&self) -> &str {
        self.model_provider_id.as_str()
    }

    fn generate_memories(&self) -> bool {
        self.generate_memories
    }
}

impl<T: RolloutConfigView + ?Sized> RolloutConfigView for &T {
    fn clarity_home(&self) -> &Path {
        (*self).clarity_home()
    }

    fn sqlite_home(&self) -> &Path {
        (*self).sqlite_home()
    }

    fn cwd(&self) -> &Path {
        (*self).cwd()
    }

    fn model_provider_id(&self) -> &str {
        (*self).model_provider_id()
    }

    fn generate_memories(&self) -> bool {
        (*self).generate_memories()
    }
}

impl<T: RolloutConfigView + ?Sized> RolloutConfigView for Arc<T> {
    fn clarity_home(&self) -> &Path {
        self.as_ref().clarity_home()
    }

    fn sqlite_home(&self) -> &Path {
        self.as_ref().sqlite_home()
    }

    fn cwd(&self) -> &Path {
        self.as_ref().cwd()
    }

    fn model_provider_id(&self) -> &str {
        self.as_ref().model_provider_id()
    }

    fn generate_memories(&self) -> bool {
        self.as_ref().generate_memories()
    }
}
