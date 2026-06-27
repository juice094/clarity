//! Diff data types — pure contract, no external dependencies.
//!
//! These types represent structured diff output and can be shared
//! across all crates without pulling in the `similar` crate used
//! by the computation functions in `clarity-tools`.

/// A single line in a diff hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// Unchanged context line.
    Context(String),
    /// Removed line (old version).
    Removed(String),
    /// Added line (new version).
    Added(String),
}

/// A hunk in a unified diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// Starting line number in the old file (1-based).
    pub old_start: usize,
    /// Starting line number in the new file (1-based).
    pub new_start: usize,
    /// Lines in this hunk.
    pub lines: Vec<DiffLine>,
}
