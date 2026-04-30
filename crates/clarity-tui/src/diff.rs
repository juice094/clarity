//! Diff utilities — re-exported from `clarity-core::diff`.
//!
//! The original implementation has been下沉 to `clarity-core` to serve both
//! TUI and egui frontends. This module preserves backward compatibility for
//! existing TUI imports.

pub use clarity_core::diff::{compute_diff, parse_unified_diff, DiffHunk, DiffLine};
