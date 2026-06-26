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
pub mod fetchers;
pub mod service;

pub use fetchers::{OllamaFetcher, OpenAiCompatibleFetcher};

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

    /// HTTP request or response error.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// A provider has no base URL, so no remote catalog can be fetched.
    #[error("provider '{0}' has no base url configured")]
    MissingBaseUrl(String),

    /// The user's home directory could not be determined.
    #[error("home directory not found")]
    NoHomeDir,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cache::CatalogCache;
    use entry::ModelCatalogEntry;
    use service::ModelCatalogService;
    use std::collections::HashMap;

    use crate::model_registry::{AuthType, ModelConfigFile, ModelEntry, ProviderConfig};

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

    #[test]
    fn service_registry_override_wins_over_bootstrap() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CatalogCache::new(dir.path());
        let service = ModelCatalogService::new(cache);

        // Bootstrap fallback for openai still includes gpt-4o.
        let bootstrap = service.family_catalog("openai").unwrap();
        assert!(bootstrap.iter().any(|e| e.model_id == "gpt-4o"));

        // User override narrows openai to only o1-preview.
        let mut providers = HashMap::new();
        providers.insert(
            "openai".into(),
            ProviderConfig {
                auth_type: AuthType::ApiKey,
                ..Default::default()
            },
        );
        let config = ModelConfigFile {
            providers,
            models: vec![ModelEntry {
                alias: "o1-preview".into(),
                provider: "openai".into(),
                model_id: "o1-preview".into(),
                ..Default::default()
            }],
        };
        let registry = crate::model_registry::ModelRegistry::from_config(config).unwrap();
        let service = service.with_registry(registry);

        let overridden = service.family_catalog("openai").unwrap();
        assert_eq!(overridden.len(), 1);
        assert_eq!(overridden[0].model_id, "o1-preview");
    }

    #[test]
    fn service_cache_wins_over_bootstrap() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CatalogCache::new(dir.path());
        cache
            .save(
                "openai",
                &[ModelCatalogEntry::new("openai", "cached-model")],
            )
            .unwrap();
        let service = ModelCatalogService::new(cache);

        let models = service.family_catalog("openai").unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model_id, "cached-model");
    }
}
