//! On-disk cache for fetched model catalogs.

use std::path::PathBuf;

use crate::catalog::entry::ModelCatalogEntry;
use crate::catalog::CatalogError;

/// Persists fetched catalogs as JSON files under a directory.
///
/// Each provider family gets its own file named `{family}.json`.
/// The directory is created lazily on the first save.
#[derive(Debug, Clone)]
pub struct CatalogCache {
    dir: PathBuf,
}

impl CatalogCache {
    /// Open a cache at the given directory.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Return the default cache directory (`~/.clarity/catalogs/`).
    pub fn default_dir() -> Result<PathBuf, CatalogError> {
        dirs::home_dir()
            .map(|h| h.join(".clarity").join("catalogs"))
            .ok_or(CatalogError::NoHomeDir)
    }

    /// Load cached entries for a provider family.
    ///
    /// Returns an empty vector if no cache file exists yet.
    pub fn load(&self, family: &str) -> Result<Vec<ModelCatalogEntry>, CatalogError> {
        let path = self.path_for(family);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&path)?;
        let entries: Vec<ModelCatalogEntry> = serde_json::from_str(&text)?;
        Ok(entries)
    }

    /// Persist entries for a provider family, creating the cache directory if needed.
    pub fn save(&self, family: &str, entries: &[ModelCatalogEntry]) -> Result<(), CatalogError> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.path_for(family);
        let text = serde_json::to_string_pretty(entries)?;
        std::fs::write(&path, text)?;
        Ok(())
    }

    fn path_for(&self, family: &str) -> PathBuf {
        self.dir.join(format!("{family}.json"))
    }
}
