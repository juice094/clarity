//! In-memory cache for loaded OKF bundles.
//!
//! Parsing an OKF bundle from disk involves reading every `.md` file in a
//! directory, splitting YAML frontmatter, and resolving cross-links. This is
//! fast for small bundles but becomes noticeable when multiple agent tools
//! access the same bundle in quick succession (e.g. `okf_search` followed by
//! `okf_read`). The cache keeps recently loaded bundles in memory, keyed by
//! their canonical filesystem path, so repeated tool calls avoid redundant
//! disk I/O.
//!
//! The cache is intentionally simple: bundles are immutable after loading, so
//! no invalidation happens automatically. Callers that know the bundle has
//! changed on disk can use [`OkfBundleCache::invalidate`] to force a reload on
//! the next access.

use super::{OkfBundle, OkfError, OkfResult, load_bundle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, RwLock};

/// Global in-memory cache shared by all OKF tools.
///
/// Using a global cache keeps the tool implementations stateless while still
/// avoiding repeated disk reads for the same bundle path.
static GLOBAL_CACHE: LazyLock<OkfBundleCache> = LazyLock::new(OkfBundleCache::new);

/// In-memory cache for OKF bundles.
#[derive(Debug, Clone, Default)]
pub struct OkfBundleCache {
    inner: Arc<RwLock<HashMap<PathBuf, OkfBundle>>>,
}

impl OkfBundleCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the global OKF bundle cache used by the OKF tools.
    pub fn global() -> &'static OkfBundleCache {
        &GLOBAL_CACHE
    }

    /// Load a bundle from `path` or return a previously cached copy.
    ///
    /// `path` is canonicalized before lookup, so relative and absolute paths
    /// that point to the same directory share the same cache entry.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be canonicalized, the directory
    /// cannot be read, or the cache lock is poisoned.
    pub fn get_or_load(&self, path: impl AsRef<Path>) -> OkfResult<OkfBundle> {
        let canonical = path.as_ref().canonicalize()?;

        {
            let guard = self.inner.read().map_err(lock_error)?;
            if let Some(bundle) = guard.get(&canonical) {
                return Ok(bundle.clone());
            }
        }

        let bundle = load_bundle(&canonical)?;
        {
            let mut guard = self.inner.write().map_err(lock_error)?;
            guard.insert(canonical, bundle.clone());
        }

        Ok(bundle)
    }

    /// Remove a bundle from the cache so the next `get_or_load` reloads it.
    ///
    /// Returns `true` if an entry was removed.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be canonicalized or the cache lock
    /// is poisoned.
    pub fn invalidate(&self, path: impl AsRef<Path>) -> OkfResult<bool> {
        let canonical = path.as_ref().canonicalize()?;
        let mut guard = self.inner.write().map_err(lock_error)?;
        Ok(guard.remove(&canonical).is_some())
    }

    /// Clear the entire cache.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.clear();
        }
    }

    /// Number of bundles currently cached.
    pub fn len(&self) -> usize {
        self.inner.read().map_or(0, |g| g.len())
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Convert a poisoned lock error into a generic I/O error.
fn lock_error<T>(_: std::sync::PoisonError<T>) -> OkfError {
    OkfError::Io(std::io::Error::other("OKF bundle cache lock poisoned"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_bundle() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();

        let mut concept = std::fs::File::create(root.join("concept.md")).unwrap();
        concept
            .write_all(b"---\ntype: Concept\ntitle: Example\n---\n\nBody.\n")
            .unwrap();

        (dir, root)
    }

    #[test]
    fn cache_loads_and_returns_bundle() {
        let (_dir, root) = create_bundle();
        let cache = OkfBundleCache::new();

        let bundle = cache.get_or_load(&root).unwrap();
        assert_eq!(bundle.len(), 1);

        // A second load should return the cached copy without re-reading disk.
        let cached = cache.get_or_load(&root).unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn invalidate_forces_reload() {
        let (dir, root) = create_bundle();
        let cache = OkfBundleCache::new();

        let bundle = cache.get_or_load(&root).unwrap();
        assert_eq!(bundle.len(), 1);

        // Add a new concept to the bundle on disk.
        let mut extra = std::fs::File::create(root.join("extra.md")).unwrap();
        extra
            .write_all(b"---\ntype: Concept\ntitle: Extra\n---\n\nMore.\n")
            .unwrap();

        // Without invalidation the cache still returns the old bundle.
        let stale = cache.get_or_load(&root).unwrap();
        assert_eq!(stale.len(), 1);

        // After invalidation the next load sees the new file.
        assert!(cache.invalidate(&root).unwrap());
        let fresh = cache.get_or_load(&root).unwrap();
        assert_eq!(fresh.len(), 2);

        // Keep TempDir alive until the end of the test.
        let _ = dir;
    }

    #[test]
    fn global_cache_is_accessible() {
        OkfBundleCache::global().clear();
        assert!(OkfBundleCache::global().is_empty());
    }
}
