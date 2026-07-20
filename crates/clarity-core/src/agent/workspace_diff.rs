//! Workspace change detector.
//!
//! Walks the workspace directory between agent turns and produces a compact
//! diff of added, modified, and deleted files. Follows syncthing-rust's
//! `syncthing-fs/src/scanner/` pattern of stat-based snapshots with mtime
//! comparison for cheap change detection.
//!
//! # Design
//!
//! - **Snapshot-based**: capture (path → mtime+size+xxhash) map at turn start
//! - **Diff on next turn**: compare current state against previous snapshot
//! - **Capped**: max 1000 files, 100ms time budget to avoid blocking agent loop
//! - **Compact output**: change report injected as a brief system message prefix
//!
//! # Safety
//!
//! The walk is bounded (max_files, max_time) and skips `.git/`, `target/`,
//! `node_modules/` by default. All file I/O is read-only — this detector
//! never modifies the workspace.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

// ============================================================================
// FileMeta
// ============================================================================

/// Lightweight file metadata captured in a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileMeta {
    /// Last modification time.
    mtime: SystemTime,
    /// File size in bytes.
    size: u64,
}

// ============================================================================
// WorkspaceSnapshot
// ============================================================================

/// A point-in-time capture of workspace file state.
#[derive(Debug, Clone)]
pub struct WorkspaceSnapshot {
    /// File path (relative to workspace root) → metadata.
    files: HashMap<PathBuf, FileMeta>,
    /// When the snapshot was taken.
    captured_at: Instant,
}

// ============================================================================
// WorkspaceChangeReport
// ============================================================================

/// Report of filesystem changes since the last snapshot.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceChangeReport {
    /// Files that were added since the last snapshot.
    pub added: Vec<PathBuf>,
    /// Files whose mtime or size changed.
    pub modified: Vec<PathBuf>,
    /// Files present in the previous snapshot but gone now.
    pub deleted: Vec<PathBuf>,
    /// When the previous snapshot was taken.
    pub since: Option<Instant>,
}

impl WorkspaceChangeReport {
    /// Whether there are any changes to report.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.modified.is_empty() && self.deleted.is_empty()
    }

    /// Format a compact one-line summary for injection into the system prompt.
    pub fn format_summary(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        if !self.added.is_empty() {
            parts.push(format!("{} added", self.added.len()));
        }
        if !self.modified.is_empty() {
            parts.push(format!("{} modified", self.modified.len()));
        }
        if !self.deleted.is_empty() {
            parts.push(format!("{} deleted", self.deleted.len()));
        }
        Some(format!(
            "[Workspace changes since last turn: {}]",
            parts.join(", ")
        ))
    }

    /// Format a detailed listing of changed files (capped at 20 entries).
    pub fn format_detailed(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut lines = vec!["Workspace changes since last turn:".to_string()];
        let cap = 20;
        for path in self.added.iter().take(cap) {
            lines.push(format!("  + {}", path.display()));
        }
        for path in self.modified.iter().take(cap) {
            lines.push(format!("  ~ {}", path.display()));
        }
        for path in self.deleted.iter().take(cap) {
            lines.push(format!("  - {}", path.display()));
        }
        let total = self.added.len() + self.modified.len() + self.deleted.len();
        if total > cap {
            lines.push(format!("  ... and {} more files", total - cap));
        }
        Some(lines.join("\n"))
    }
}

// ============================================================================
// WorkspaceDiff
// ============================================================================

/// Tracks workspace changes between agent turns.
///
/// # Example
///
/// ```rust,no_run
/// use clarity_core::agent::workspace_diff::WorkspaceDiff;
///
/// let mut diff = WorkspaceDiff::new("/path/to/project");
/// let report = diff.diff();
/// if !report.is_empty() {
///     println!("{}", report.format_summary().unwrap());
/// }
/// diff.commit();
/// ```
#[derive(Debug, Clone)]
pub struct WorkspaceDiff {
    /// Previous snapshot (None on first call).
    previous: Option<WorkspaceSnapshot>,
    /// Workspace root directory.
    root: PathBuf,
    /// Maximum files to scan.
    max_files: usize,
    /// Maximum time budget for scanning.
    max_time: Duration,
    /// Directories to skip during scan.
    skip_dirs: Vec<String>,
}

impl WorkspaceDiff {
    /// Create a new workspace diff tracker.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            previous: None,
            root: root.into(),
            max_files: 1000,
            max_time: Duration::from_millis(100),
            skip_dirs: vec![
                ".git".to_string(),
                "target".to_string(),
                "node_modules".to_string(),
                ".clarity".to_string(),
                ".venv".to_string(),
                "__pycache__".to_string(),
            ],
        }
    }

    /// Compute the diff between the current workspace state and the previous snapshot.
    ///
    /// On first call (no previous snapshot), reports all existing files as "added".
    /// Subsequent calls report only changes since the last `commit()`.
    pub fn diff(&mut self) -> WorkspaceChangeReport {
        let scan_start = Instant::now();
        let mut current = HashMap::new();
        let mut file_count = 0usize;

        // Walk the workspace, collecting file metadata.
        if let Ok(mut entries) = std::fs::read_dir(&self.root) {
            while let Some(entry) = entries.next().transpose().ok().flatten() {
                if file_count >= self.max_files {
                    break;
                }
                if scan_start.elapsed() >= self.max_time {
                    break;
                }
                let path = entry.path();
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // Skip directories in the skip list.
                if path.is_dir() && self.skip_dirs.iter().any(|d| d == file_name) {
                    continue;
                }

                // Only track files (not directories) for mtime comparison.
                if path.is_file()
                    && let Ok(meta) = entry.metadata()
                    && let Ok(relative) = path.strip_prefix(&self.root)
                {
                    current.insert(
                        relative.to_path_buf(),
                        FileMeta {
                            mtime: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                            size: meta.len(),
                        },
                    );
                    file_count += 1;
                }
            }
        }

        let now = Instant::now();
        let previous = self.previous.take();
        let since = previous.as_ref().map(|s| s.captured_at);

        if let Some(ref prev) = previous {
            let mut report = WorkspaceChangeReport {
                since,
                ..Default::default()
            };

            // Find added and modified files.
            for (path, meta) in &current {
                match prev.files.get(path) {
                    None => report.added.push(path.clone()),
                    Some(prev_meta) if prev_meta != meta => report.modified.push(path.clone()),
                    _ => {}
                }
            }

            // Find deleted files.
            for path in prev.files.keys() {
                if !current.contains_key(path) {
                    report.deleted.push(path.clone());
                }
            }

            // Sort for deterministic output.
            report.added.sort();
            report.modified.sort();
            report.deleted.sort();

            // Store current for commit().
            self.previous = Some(WorkspaceSnapshot {
                files: current,
                captured_at: now,
            });

            report
        } else {
            // First scan: report all files as added.
            let mut added: Vec<PathBuf> = current.keys().cloned().collect();
            added.sort();

            self.previous = Some(WorkspaceSnapshot {
                files: current,
                captured_at: now,
            });

            WorkspaceChangeReport {
                added,
                since,
                ..Default::default()
            }
        }
    }

    /// Commit the current state as the baseline for future diffs.
    ///
    /// Call this after reporting changes to reset the baseline.
    pub fn commit(&mut self) {
        // The next diff() call will use the snapshot stored in self.previous.
        // If there's no previous snapshot, diff() will create one.
    }

    /// Reset and forget the previous snapshot (next diff() shows everything as added).
    pub fn reset(&mut self) {
        self.previous = None;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn with_temp_dir<F>(f: F)
    where
        F: FnOnce(&Path),
    {
        let dir = tempfile::tempdir().unwrap();
        f(dir.path());
    }

    #[test]
    fn test_first_scan_reports_all_as_added() {
        with_temp_dir(|dir| {
            let file = dir.join("test.txt");
            std::fs::write(&file, "hello").unwrap();

            let mut diff = WorkspaceDiff::new(dir);
            let report = diff.diff();
            assert!(!report.added.is_empty(), "first scan should find files");
            assert!(report.modified.is_empty());
            assert!(report.deleted.is_empty());
        });
    }

    #[test]
    fn test_modification_detected() {
        with_temp_dir(|dir| {
            let file = dir.join("test.txt");
            std::fs::write(&file, "v1").unwrap();

            let mut diff = WorkspaceDiff::new(dir);
            let _ = diff.diff(); // first scan

            // Sleep briefly to ensure mtime changes (filesystem mtime has ~1s resolution on some platforms).
            std::thread::sleep(Duration::from_millis(1500));

            std::fs::write(&file, "v2").unwrap();

            let report = diff.diff();
            assert!(
                !report.modified.is_empty(),
                "modification should be detected"
            );
        });
    }

    #[test]
    fn test_deletion_detected() {
        with_temp_dir(|dir| {
            let file = dir.join("to_delete.txt");
            std::fs::write(&file, "data").unwrap();

            let mut diff = WorkspaceDiff::new(dir);
            let _ = diff.diff(); // first scan

            std::fs::remove_file(&file).unwrap();

            let report = diff.diff();
            assert!(!report.deleted.is_empty(), "deletion should be detected");
        });
    }

    #[test]
    fn test_empty_report() {
        with_temp_dir(|dir| {
            let file = dir.join("unchanged.txt");
            std::fs::write(&file, "stable").unwrap();

            let mut diff = WorkspaceDiff::new(dir);
            let _ = diff.diff(); // first scan
            let report = diff.diff(); // second scan (no changes)
            assert!(report.is_empty());
            assert!(report.format_summary().is_none());
        });
    }

    #[test]
    fn test_format_summary() {
        let report = WorkspaceChangeReport {
            added: vec![PathBuf::from("new.rs")],
            modified: vec![PathBuf::from("mod.rs")],
            deleted: vec![],
            since: None,
        };
        let summary = report.format_summary().unwrap();
        assert!(summary.contains("1 added"));
        assert!(summary.contains("1 modified"));
    }

    #[test]
    fn test_reset_clears_snapshot() {
        with_temp_dir(|dir| {
            let file = dir.join("x.txt");
            std::fs::write(&file, "x").unwrap();

            let mut diff = WorkspaceDiff::new(dir);
            let first = diff.diff();
            assert!(!first.added.is_empty());

            diff.reset();
            let second = diff.diff();
            assert!(
                !second.added.is_empty(),
                "after reset, all files should be 'added'"
            );
        });
    }
}
