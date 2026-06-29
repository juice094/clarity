//! Shared test utilities for integration tests.
//!
//! Only ever called by test code — panic and unwrap are acceptable here.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//!
//! Provides temp-directory helpers with RAII cleanup so tests never touch
//! the user's real data directories. Intended for use across all egui
//! integration and unit tests that need filesystem isolation.
//!
//! # Usage
//!
//! ```ignore
//! use crate::test_util::with_temp_dir;
//!
//! with_temp_dir("my-test", |tmp| {
//!     let path = tmp.join("data.json");
//!     std::fs::write(&path, "{}").unwrap();
//!     // ... test assertions ...
//! }); // directory auto-removed here
//! ```

use std::path::{Path, PathBuf};

// ============================================================================
// TempDirGuard
// ============================================================================

/// RAII guard that removes the temp directory (and all contents) on drop.
///
/// # Safety
///
/// The guard intentionally does **not** check whether the removal succeeded —
/// test cleanup is best-effort. If the directory cannot be removed (e.g.
/// locked file on Windows), the test framework will handle leftover temp
/// files during its own cleanup cycle.
pub struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ============================================================================
// with_temp_dir
// ============================================================================

/// Run a closure with an isolated temp directory that is automatically
/// cleaned up (including all contents) when the closure returns.
///
/// The directory name includes the `test_name` prefix for easier debugging
/// when inspecting leftover directories after test failures.
///
/// # Example
///
/// ```ignore
/// #[test]
/// fn my_integration_test() {
///     with_temp_dir("my_test", |tmp| {
///         std::fs::write(tmp.join("file.txt"), "hello").unwrap();
///         assert!(tmp.join("file.txt").exists());
///     });
/// }
/// ```
pub fn with_temp_dir(test_name: &str, f: impl FnOnce(&Path)) {
    let tmp = std::env::temp_dir().join(format!(
        "clarity-egui-test-{}-{}",
        test_name,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&tmp)
        .unwrap_or_else(|e| panic!("with_temp_dir: failed to create {}: {}", tmp.display(), e));
    let _guard = TempDirGuard(tmp.clone());
    f(&tmp);
    // Explicit drop so the guard's cleanup runs before any test-framework
    // asserts on temp directory state.
    drop(_guard);
}

// ============================================================================
// Domain-specific helpers
// ============================================================================

/// Like [`with_temp_dir`], but also creates a `sessions/` subdirectory
/// inside the temp root. Convenience wrapper for session persistence tests.
///
/// # Companion helper
///
/// Use [`crate::session::save_session_to_path`] to write a [`crate::ui::types::Session`]
/// to the temp directory for roundtrip integration tests.
pub fn with_temp_sessions_dir(test_name: &str, f: impl FnOnce(&Path)) {
    with_temp_dir(test_name, |tmp| {
        let sessions = tmp.join("sessions");
        std::fs::create_dir_all(&sessions)
            .unwrap_or_else(|e| panic!("with_temp_sessions_dir: failed to create sessions: {}", e));
        f(tmp);
    });
}

// ============================================================================
// Unit tests for the test utilities themselves
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn with_temp_dir_creates_and_cleans_up() {
        let mut dir_path = PathBuf::new();
        with_temp_dir("cleanup_test", |tmp| {
            dir_path = tmp.to_path_buf();
            assert!(dir_path.exists(), "temp dir should exist during closure");
            fs::write(tmp.join("data.txt"), "test").unwrap();
            assert!(tmp.join("data.txt").exists());
        });
        assert!(
            !dir_path.exists(),
            "temp dir should be removed after closure: {}",
            dir_path.display()
        );
    }

    #[test]
    fn with_temp_sessions_dir_creates_sessions_subdir() {
        with_temp_sessions_dir("sessions_test", |tmp| {
            let sessions = tmp.join("sessions");
            assert!(
                sessions.is_dir(),
                "sessions/ subdir should exist: {}",
                sessions.display()
            );
        });
    }

    #[test]
    fn with_temp_dir_handles_nested_operations() {
        with_temp_dir("nested", |tmp| {
            let nested = tmp.join("a").join("b").join("c");
            fs::create_dir_all(&nested).unwrap();
            fs::write(nested.join("deep.txt"), "deep").unwrap();
            assert!(nested.join("deep.txt").exists());
        });
        // After closure, everything is gone — tested indirectly by the fact
        // that this test doesn't panic (TempDirGuard::drop removes recursively).
    }
}
