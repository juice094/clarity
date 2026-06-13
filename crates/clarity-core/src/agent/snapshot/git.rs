//! Low-level Git command wrappers for the snapshot side-repository.

use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::time::{Duration, timeout};
use tracing::debug;

use crate::error::AgentError;

/// Represents a single snapshot commit in the side repo.
#[derive(Debug, Clone)]
pub struct SnapshotCommit {
    /// Snapshot hash.
    pub hash: String,
    /// Message text.
    pub message: String,
    /// Timestamp of the record.
    pub timestamp: String,
}

/// Wrapper around a bare Git repository used for workspace snapshots.
pub struct GitSnapshot {
    git_dir: PathBuf,
    work_tree: PathBuf,
}

impl GitSnapshot {
    pub(crate) fn new(git_dir: PathBuf, work_tree: PathBuf) -> Self {
        Self { git_dir, work_tree }
    }

    /// Initialise a bare repository at `side_dir/repo.git` and create the
    /// first commit from the current working-directory contents.
    pub async fn init_bare(work_tree: &Path, side_dir: &Path) -> Result<Self, AgentError> {
        let git_dir = side_dir.join("repo.git");
        std::fs::create_dir_all(&git_dir).map_err(|e| {
            AgentError::ToolExecutionFailed(
                "snapshot_init".to_string(),
                format!("Failed to create snapshot dir: {}", e),
            )
        })?;

        let snap = Self {
            git_dir: git_dir.clone(),
            work_tree: work_tree.to_path_buf(),
        };

        // Initialise bare repo (must NOT pass --work-tree)
        let mut cmd = Command::new("git");
        cmd.arg(format!("--git-dir={}", snap.git_dir.display()))
            .args(["init", "--bare"]);
        let out = timeout(Duration::from_secs(30), cmd.output())
            .await
            .map_err(|_| {
                AgentError::ToolExecutionFailed(
                    "git".to_string(),
                    "git init --bare timed out".into(),
                )
            })?
            .map_err(|e| AgentError::ToolExecutionFailed("git".to_string(), e.to_string()))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(AgentError::ToolExecutionFailed(
                "git".to_string(),
                format!("git init --bare failed: {}", stderr.trim()),
            ));
        }

        // Configure commit identity (required for first commit)
        snap.run_git(&["config", "user.email", "clarity@local"])
            .await?;
        snap.run_git(&["config", "user.name", "Clarity Agent"])
            .await?;

        // First commit from current work-tree state
        snap.run_git(&["add", "-A"]).await?;
        let _ = snap
            .run_git(&["commit", "--allow-empty", "-m", "snapshot-init"])
            .await;

        debug!(
            "Snapshot bare repo initialised: {} -> {}",
            work_tree.display(),
            git_dir.display()
        );
        Ok(snap)
    }

    /// Create a new snapshot commit with the given message.
    /// Returns the commit hash.
    pub async fn commit(&self, message: &str) -> Result<String, AgentError> {
        self.run_git(&["add", "-A"]).await?;
        self.run_git(&["commit", "--allow-empty", "-m", message])
            .await?;
        let hash = self.rev_parse("HEAD").await?;
        debug!("Snapshot committed: {} ({})", hash, message);
        Ok(hash)
    }

    /// Restore tracked files to the state at `hash`.
    /// Does **not** delete untracked files created after the snapshot.
    pub async fn checkout(&self, hash: &str) -> Result<(), AgentError> {
        self.run_git(&["checkout", hash, "--", "."]).await?;
        debug!("Snapshot restored: {}", hash);
        Ok(())
    }

    /// List all snapshot commits in chronological order (oldest first).
    pub async fn list_commits(&self) -> Result<Vec<SnapshotCommit>, AgentError> {
        let out = self
            .run_git(&["log", "--reverse", "--format=%H|%s|%ci"])
            .await?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut commits = Vec::new();
        for line in text.lines() {
            let mut parts = line.splitn(3, '|');
            if let (Some(hash), Some(msg), Some(ts)) = (parts.next(), parts.next(), parts.next()) {
                commits.push(SnapshotCommit {
                    hash: hash.to_string(),
                    message: msg.to_string(),
                    timestamp: ts.to_string(),
                });
            }
        }
        Ok(commits)
    }

    /// Run a git command with the side-repo context.
    async fn run_git(&self, args: &[&str]) -> Result<std::process::Output, AgentError> {
        let mut cmd = Command::new("git");
        cmd.arg(format!("--git-dir={}", self.git_dir.display()))
            .arg(format!("--work-tree={}", self.work_tree.display()))
            .args(args);

        let output = timeout(Duration::from_secs(30), cmd.output())
            .await
            .map_err(|_| {
                AgentError::ToolExecutionFailed("git".to_string(), "Git command timed out".into())
            })?
            .map_err(|e| AgentError::ToolExecutionFailed("git".to_string(), e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::ToolExecutionFailed(
                "git".to_string(),
                format!("git {} failed: {}", args.join(" "), stderr.trim()),
            ));
        }
        Ok(output)
    }

    /// Resolve a ref to its full hash.
    async fn rev_parse(&self, rev: &str) -> Result<String, AgentError> {
        let out = self.run_git(&["rev-parse", rev]).await?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn init_git_repo(dir: &Path) {
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("init").current_dir(dir);
        let _ = cmd.output().await.unwrap();

        let mut cmd = tokio::process::Command::new("git");
        cmd.args(["config", "user.email", "test@test.com"])
            .current_dir(dir);
        let _ = cmd.output().await.unwrap();

        let mut cmd = tokio::process::Command::new("git");
        cmd.args(["config", "user.name", "Test"]).current_dir(dir);
        let _ = cmd.output().await.unwrap();
    }

    #[tokio::test]
    async fn test_git_snapshot_init_and_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path().join("wd");
        let side = tmp.path().join("side");
        std::fs::create_dir(&wd).unwrap();

        init_git_repo(&wd).await;
        std::fs::write(wd.join("a.txt"), "hello").unwrap();

        let git = GitSnapshot::init_bare(&wd, &side).await.unwrap();
        let hash = git.commit("test-snapshot").await.unwrap();
        assert!(!hash.is_empty());

        let commits = git.list_commits().await.unwrap();
        assert!(!commits.is_empty());
        assert!(commits.iter().any(|c| c.message == "test-snapshot"));
    }

    #[tokio::test]
    async fn test_git_snapshot_checkout() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path().join("wd");
        let side = tmp.path().join("side");
        std::fs::create_dir(&wd).unwrap();

        init_git_repo(&wd).await;
        std::fs::write(wd.join("a.txt"), "v1").unwrap();

        let git = GitSnapshot::init_bare(&wd, &side).await.unwrap();
        let hash = git.commit("v1").await.unwrap();

        // Modify file
        std::fs::write(wd.join("a.txt"), "v2").unwrap();

        // Restore
        git.checkout(&hash).await.unwrap();
        let content = std::fs::read_to_string(wd.join("a.txt")).unwrap();
        assert_eq!(content, "v1");
    }
}
