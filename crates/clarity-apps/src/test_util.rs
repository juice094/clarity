//! Shared test utilities for `clarity-apps`.
//!
//! Only ever called by test code — panic and unwrap are acceptable here.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

/// RAII guard that removes the temp directory (and all contents) on drop.
pub struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run a closure with an isolated temp directory that is automatically cleaned up.
pub fn with_temp_dir(test_name: &str, f: impl FnOnce(&Path)) {
    let tmp = std::env::temp_dir().join(format!(
        "clarity-apps-test-{}-{}",
        test_name,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&tmp)
        .unwrap_or_else(|e| panic!("with_temp_dir: failed to create {}: {}", tmp.display(), e));
    let _guard = TempDirGuard(tmp.clone());
    f(&tmp);
    drop(_guard);
}
