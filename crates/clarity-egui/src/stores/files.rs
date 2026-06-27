//! Files store — local workspace browser state.
//!
//! Holds the current workspace root, expanded directories, selected
//! path, and recent-file history for the right-rail Files panel.
//!
//! Extension points for future GitHub integration are declared as
//! `Option` fields that renderers silently skip when `None`.

use std::collections::HashSet;
use std::path::PathBuf;

/// State for the right-rail Files panel.
pub struct FilesStore {
    /// Root directory for the file tree.
    pub workspace_root: PathBuf,
    /// Set of directories currently expanded in the tree.
    pub expanded_dirs: HashSet<PathBuf>,
    /// Currently selected (clicked) file path.
    pub selected_path: Option<PathBuf>,
    /// Recently opened files, most recent first (cap 20).
    pub recent_files: Vec<PathBuf>,

    // === Extension points for future backend features ===
    /// Git working-tree status cache. When `Some`, the file tree renders
    /// M/A/U status icons alongside filenames. Set by a future GitHub
    /// integration or workspace watcher.
    pub git_status: Option<GitStatusCache>,
    /// GitHub / remote repository URL. When `Some`, an "Open on GitHub"
    /// button is rendered in the panel header.
    pub repo_url: Option<String>,
}

impl Default for FilesStore {
    fn default() -> Self {
        Self {
            workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            expanded_dirs: HashSet::new(),
            selected_path: None,
            recent_files: Vec::new(),
            git_status: None,
            repo_url: None,
        }
    }
}

impl FilesStore {
    /// Add a file to the recent-files list, trimming to capacity.
    pub fn touch_recent(&mut self, path: PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(20);
    }
}

/// Git working-tree status for a single workspace.
///
/// Populated by a future `git status` polling loop or filesystem
/// watcher.  All paths are relative to the workspace root.
#[derive(Clone, Debug)]
pub struct GitStatusCache {
    pub branch: String,
    pub modified: Vec<String>,
    pub staged: Vec<String>,
    pub untracked: Vec<String>,
}
