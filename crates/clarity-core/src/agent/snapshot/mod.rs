//! Workspace snapshot service — automatic per-turn Git snapshots via a side
//! bare repository so the Agent can safely roll back file changes.

pub mod config;
pub mod git;
pub mod tool;

pub use config::SnapshotConfig;
pub use git::{GitSnapshot, SnapshotCommit};
pub use tool::GitRestoreTool;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{info, warn};

use crate::error::AgentError;

/// Unique identifier for a single snapshot.
#[derive(Debug, Clone)]
pub struct SnapshotId {
    pub id: usize,
    pub hash: String,
    pub label: String,
    pub timestamp: String,
}

/// Information about a stored snapshot (for listing / UI).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotInfo {
    pub id: usize,
    pub hash: String,
    pub label: String,
    pub timestamp: String,
}

/// Service that manages workspace snapshots.
pub struct SnapshotService {
    git: GitSnapshot,
    index_path: PathBuf,
    counter: AtomicUsize,
    max_snapshots: usize,
}

impl SnapshotService {
    /// Attempt to create a `SnapshotService`.
    ///
    /// Returns `None` if snapshots are disabled or the working directory is
    /// not inside a Git repository.
    pub async fn try_new(
        config: &SnapshotConfig,
        working_dir: &Path,
    ) -> Option<Self> {
        if !config.enabled {
            return None;
        }

        let wd = match std::fs::canonicalize(working_dir) {
            Ok(p) => p,
            Err(_) => return None,
        };

        // Must be inside a git repo
        if !wd.join(".git").exists() {
            info!(
                "Snapshots disabled — working dir '{}' is not a git repository",
                wd.display()
            );
            return None;
        }

        let repo_id = wd.to_string_lossy().replace('\\', "/").replace(':', "_");

        let snapshot_base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".clarity")
            .join("snapshots")
            .join(&repo_id);

        let index_path = snapshot_base.join("index.json");
        let side_dir = snapshot_base.join("repo.git");

        let git = if side_dir.exists() {
            GitSnapshot::new(side_dir, wd.clone())
        } else {
            match GitSnapshot::init_bare(&wd, &snapshot_base).await {
                Ok(g) => g,
                Err(e) => {
                    warn!("Failed to init snapshot bare repo: {}", e);
                    return None;
                }
            }
        };

        // Load existing counter from index
        let counter = if index_path.exists() {
            match std::fs::read_to_string(&index_path) {
                Ok(text) => serde_json::from_str::<Vec<SnapshotInfo>>(&text)
                    .map(|v| v.len())
                    .unwrap_or(0),
                Err(_) => 0,
            }
        } else {
            0
        };

        info!(
            "Snapshot service active for {} (repo_id={})",
            wd.display(),
            &repo_id[..8]
        );

        Some(Self {
            git,
            index_path,
            counter: AtomicUsize::new(counter),
            max_snapshots: config.max_snapshots,
        })
    }

    /// Create a pre-turn snapshot.
    pub async fn snapshot_pre_turn(&self) -> Result<SnapshotId, AgentError> {
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        let label = format!("pre-turn-{}", id);
        self.snapshot_inner(id, &label).await
    }

    /// Create a post-turn snapshot.
    pub async fn snapshot_post_turn(&self) -> Result<SnapshotId, AgentError> {
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        let label = format!("post-turn-{}", id);
        self.snapshot_inner(id, &label).await
    }

    async fn snapshot_inner(&self, id: usize, label: &str) -> Result<SnapshotId, AgentError> {
        let hash = self.git.commit(label).await?;
        let timestamp = chrono::Utc::now().to_rfc3339();

        let info = SnapshotInfo {
            id,
            hash: hash.clone(),
            label: label.to_string(),
            timestamp: timestamp.clone(),
        };

        self.append_index(info).await?;

        Ok(SnapshotId {
            id,
            hash,
            label: label.to_string(),
            timestamp,
        })
    }

    /// List all stored snapshots (oldest first).
    pub fn list(&self) -> Vec<SnapshotInfo> {
        if !self.index_path.exists() {
            return Vec::new();
        }
        std::fs::read_to_string(&self.index_path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Restore the workspace to the state of snapshot `id`.
    pub async fn restore(&self, id: usize) -> Result<(), AgentError> {
        let list = self.list();
        let target = list
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| AgentError::ToolExecutionFailed(
                "git_restore".to_string(),
                format!("Snapshot id {} not found", id),
            ))?;
        self.git.checkout(&target.hash).await
    }

    async fn append_index(&self, info: SnapshotInfo) -> Result<(), AgentError> {
        let mut list = self.list();
        list.push(info);

        // Trim to max_snapshots (drop oldest from index)
        if list.len() > self.max_snapshots {
            let drop_count = list.len() - self.max_snapshots;
            list = list.into_iter().skip(drop_count).collect();
        }

        let json = serde_json::to_string_pretty(&list).map_err(|e| {
            AgentError::ToolExecutionFailed(
                "snapshot_index".to_string(),
                format!("Failed to serialize index: {}", e),
            )
        })?;

        if let Some(parent) = self.index_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AgentError::ToolExecutionFailed(
                    "snapshot_index".to_string(),
                    format!("Failed to create index dir: {}", e),
                )
            })?;
        }

        tokio::fs::write(&self.index_path, json).await.map_err(|e| {
            AgentError::ToolExecutionFailed(
                "snapshot_index".to_string(),
                format!("Failed to write index: {}", e),
            )
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_snapshot_service_disabled() {
        let cfg = SnapshotConfig::new().with_enabled(false);
        let tmp = tempfile::tempdir().unwrap();
        let svc = SnapshotService::try_new(&cfg, tmp.path()).await;
        assert!(svc.is_none());
    }

    #[tokio::test]
    async fn test_snapshot_service_not_git_repo() {
        let cfg = SnapshotConfig::new();
        let tmp = tempfile::tempdir().unwrap();
        let svc = SnapshotService::try_new(&cfg, tmp.path()).await;
        assert!(svc.is_none());
    }
}
