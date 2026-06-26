//! Model catalog service.

use std::collections::HashMap;
use std::sync::Arc;

use tracing;

use crate::catalog::CatalogError;
use crate::catalog::cache::CatalogCache;
use crate::catalog::entry::ModelCatalogEntry;
use crate::catalog::fetcher::CatalogFetcher;
use crate::registry_table;

/// Merges bootstrap defaults, on-disk cache, and remote fetches into a unified catalog.
///
/// This is the Phase 1 skeleton. It currently supports cache + bootstrap fallback.
/// User overrides from `models.toml` and live remote refresh will be wired in
/// subsequent phases.
pub struct ModelCatalogService {
    cache: CatalogCache,
    fetchers: Vec<Arc<dyn CatalogFetcher>>,
}

impl std::fmt::Debug for ModelCatalogService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelCatalogService")
            .field("cache", &self.cache)
            .field("fetcher_count", &self.fetchers.len())
            .finish()
    }
}

impl ModelCatalogService {
    /// Create a service backed by the default on-disk cache directory.
    pub fn default_cache() -> Result<Self, CatalogError> {
        Ok(Self::new(CatalogCache::new(CatalogCache::default_dir()?)))
    }

    /// Create a service with a custom cache.
    pub fn new(cache: CatalogCache) -> Self {
        Self {
            cache,
            fetchers: Vec::new(),
        }
    }

    /// Register a remote fetcher.
    pub fn register_fetcher(&mut self, fetcher: Arc<dyn CatalogFetcher>) {
        self.fetchers.push(fetcher);
    }

    /// Return the merged catalog for a provider family.
    ///
    /// Resolution order:
    /// 1. On-disk cache.
    /// 2. Minimal bootstrap defaults from [`registry_table`].
    pub async fn family_catalog(
        &self,
        family: &str,
    ) -> Result<Vec<ModelCatalogEntry>, CatalogError> {
        let cached = self.cache.load(family)?;
        if !cached.is_empty() {
            return Ok(cached);
        }

        if let Some(defaults) = registry_table::family_defaults(family) {
            let entries = defaults
                .known_models
                .into_iter()
                .map(|id| ModelCatalogEntry::new(family, id))
                .collect();
            return Ok(entries);
        }

        Ok(Vec::new())
    }

    /// Refresh all registered fetchers and persist their results to the cache.
    ///
    /// Fetchers that fail are logged but do not abort the overall refresh.
    pub async fn refresh_all(
        &self,
    ) -> Result<HashMap<String, Vec<ModelCatalogEntry>>, CatalogError> {
        let mut result = HashMap::new();
        for fetcher in &self.fetchers {
            match fetcher.fetch().await {
                Ok(entries) => {
                    if let Err(e) = self.cache.save(fetcher.family(), &entries) {
                        tracing::warn!(family = fetcher.family(), error = %e, "failed to cache catalog");
                    }
                    result.insert(fetcher.family().to_string(), entries);
                }
                Err(e) => {
                    tracing::warn!(family = fetcher.family(), error = %e, "failed to fetch catalog");
                }
            }
        }
        Ok(result)
    }
}
