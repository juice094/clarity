//! Dynamic model catalog: bootstrap defaults, on-disk cache, and remote fetches.
//!
//! The catalog layer replaces the previous large hard-coded model lists in
//! [`crate::registry_table`] with a three-tier lookup:
//!
//! 1. User override (planned, will read `models.toml`).
//! 2. Cached remote catalog fetched on demand.
//! 3. Minimal offline bootstrap seed from [`crate::registry_table`].

pub mod cache;
pub mod entry;
pub mod fetcher;
pub mod service;

use thiserror::Error;

/// Errors returned by catalog operations.
#[derive(Debug, Error)]
pub enum CatalogError {
    /// I/O error while reading or writing the on-disk cache.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// The user's home directory could not be determined.
    #[error("home directory not found")]
    NoHomeDir,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cache::CatalogCache;
    use entry::ModelCatalogEntry;
    use std::collections::HashMap;

    #[test]
    fn catalog_entry_round_trip() {
        let mut metadata = HashMap::new();
        metadata.insert("context_window".into(), "128000".into());
        let entry = ModelCatalogEntry {
            family: "openai".into(),
            model_id: "gpt-4o".into(),
            display_name: Some("GPT-4o".into()),
            tags: vec!["chat".into(), "vision".into()],
            metadata,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: ModelCatalogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, parsed);
    }

    #[test]
    fn cache_round_trip_in_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CatalogCache::new(dir.path());
        let entries = vec![
            ModelCatalogEntry::new("openai", "gpt-4o"),
            ModelCatalogEntry::new("openai", "o1-preview"),
        ];

        cache.save("openai", &entries).unwrap();
        let loaded = cache.load("openai").unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].model_id, "gpt-4o");
        assert_eq!(loaded[1].model_id, "o1-preview");
    }

    #[test]
    fn load_missing_family_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CatalogCache::new(dir.path());
        let loaded = cache.load("nonexistent").unwrap();
        assert!(loaded.is_empty());
    }
}
