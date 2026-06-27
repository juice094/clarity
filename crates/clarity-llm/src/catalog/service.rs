//! Model catalog service.

use std::collections::HashMap;
use std::sync::Arc;

use tracing;

use crate::catalog::CatalogError;
use crate::catalog::cache::CatalogCache;
use crate::catalog::entry::ModelCatalogEntry;
use crate::catalog::fetcher::CatalogFetcher;
use crate::catalog::fetchers::{OllamaFetcher, OpenAiCompatibleFetcher};
use crate::model_registry::{ModelRegistry, ProtocolType};
use crate::registry_table;

/// Merges user overrides, on-disk cache, and bootstrap defaults into a unified catalog.
///
/// Resolution order for [`family_catalog`](Self::family_catalog):
/// 1. User override from a loaded [`ModelRegistry`] (`models.toml`).
/// 2. Cached remote catalog.
/// 3. Minimal offline bootstrap defaults from [`registry_table`].
pub struct ModelCatalogService {
    cache: CatalogCache,
    registry: Option<ModelRegistry>,
    fetchers: Vec<Arc<dyn CatalogFetcher>>,
}

impl std::fmt::Debug for ModelCatalogService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelCatalogService")
            .field("cache", &self.cache)
            .field("has_registry", &self.registry.is_some())
            .field("fetcher_count", &self.fetchers.len())
            .finish()
    }
}

impl ModelCatalogService {
    /// Create a service backed by the default on-disk cache directory.
    pub fn default_cache() -> Result<Self, CatalogError> {
        Ok(Self::new(CatalogCache::new(CatalogCache::default_dir()?)))
    }

    /// Create a service with the default cache and all canonical remote fetchers registered.
    ///
    /// ponytail: fetchers are registered optimistically; missing API keys or offline
    /// instances are handled gracefully during `refresh_all`.
    pub fn with_defaults() -> Result<Self, CatalogError> {
        let mut service = Self::default_cache()?;
        service.register_default_fetchers();
        Ok(service)
    }

    /// Create a service with a custom cache.
    pub fn new(cache: CatalogCache) -> Self {
        Self {
            cache,
            registry: None,
            fetchers: Vec::new(),
        }
    }

    /// Attach a user registry so its models take priority over cache/bootstrap.
    pub fn with_registry(mut self, registry: ModelRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Register a remote fetcher.
    pub fn register_fetcher(&mut self, fetcher: Arc<dyn CatalogFetcher>) {
        self.fetchers.push(fetcher);
    }

    /// Register canonical fetchers for all families that advertise a fetchable protocol.
    ///
    /// Currently supported:
    /// - `ProtocolType::Ollama` → [`OllamaFetcher`]
    /// - `ProtocolType::OpenAiChat` → [`OpenAiCompatibleFetcher`]
    pub fn register_default_fetchers(&mut self) {
        for family in registry_table::all_family_names() {
            let Some(defaults) = registry_table::family_defaults(family) else {
                continue;
            };

            match defaults.protocol {
                ProtocolType::Ollama => match OllamaFetcher::from_defaults() {
                    Ok(fetcher) => self.register_fetcher(Arc::new(fetcher)),
                    Err(e) => tracing::warn!(family, error = %e, "skipping ollama fetcher"),
                },
                ProtocolType::OpenAiChat => match OpenAiCompatibleFetcher::from_defaults(family) {
                    Ok(fetcher) => self.register_fetcher(Arc::new(fetcher)),
                    Err(e) => {
                        tracing::warn!(family, error = %e, "skipping openai-compatible fetcher")
                    }
                },
                _ => {}
            }
        }
    }

    /// Return the merged catalog for a provider family.
    ///
    /// Resolution order:
    /// 1. User override from the attached registry.
    /// 2. On-disk cache.
    /// 3. Minimal bootstrap defaults from [`registry_table`].
    pub fn family_catalog(&self, family: &str) -> Result<Vec<ModelCatalogEntry>, CatalogError> {
        // 1. User override.
        if let Some(registry) = &self.registry {
            let override_models: Vec<ModelCatalogEntry> = registry
                .list_models()
                .into_iter()
                .filter(|entry| entry.provider == family)
                .map(|entry| ModelCatalogEntry::new(family, entry.model_id.clone()))
                .collect();
            if !override_models.is_empty() {
                return Ok(override_models);
            }
        }

        // 2. Cached remote catalog.
        let cached = self.cache.load(family)?;
        if !cached.is_empty() {
            return Ok(cached);
        }

        // 3. Bootstrap defaults.
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

    /// Refresh a single provider from explicit runtime config and persist to cache.
    ///
    /// This supports both built-in families and custom providers added in the UI.
    /// The `api_format` values mirror [`crate::runtime::RuntimeProviderConfig`]:
    /// - `"ollama"` → [`OllamaFetcher`]
    /// - `"openai_chat"` / `"llama_server"` → [`OpenAiCompatibleFetcher`]
    /// - other formats return an empty list.
    pub async fn refresh_provider(
        &self,
        provider_id: &str,
        base_url: &str,
        api_key: Option<&str>,
        api_format: &str,
    ) -> Result<Vec<ModelCatalogEntry>, CatalogError> {
        let entries = match api_format {
            "ollama" => OllamaFetcher::new(base_url).fetch().await?,
            "openai_chat" | "llama_server" => {
                OpenAiCompatibleFetcher::new(provider_id, base_url, api_key.map(String::from))
                    .fetch()
                    .await?
            }
            _ => Vec::new(),
        };

        if !entries.is_empty() {
            self.cache.save(provider_id, &entries)?;
        }

        Ok(entries)
    }

    /// Refresh a single provider family and persist the result to the cache.
    ///
    /// If a fetcher is already registered for the family, it is reused.
    /// Otherwise the service tries to construct one from canonical registry defaults.
    pub async fn refresh_family(
        &self,
        family: &str,
    ) -> Result<Vec<ModelCatalogEntry>, CatalogError> {
        if let Some(fetcher) = self.fetchers.iter().find(|f| f.family() == family) {
            let entries = fetcher.fetch().await?;
            self.cache.save(family, &entries)?;
            return Ok(entries);
        }

        if let Some(defaults) = registry_table::family_defaults(family) {
            match defaults.protocol {
                ProtocolType::Ollama => {
                    let entries = OllamaFetcher::from_defaults()?.fetch().await?;
                    self.cache.save(family, &entries)?;
                    return Ok(entries);
                }
                ProtocolType::OpenAiChat => {
                    let entries = OpenAiCompatibleFetcher::from_defaults(family)?
                        .fetch()
                        .await?;
                    self.cache.save(family, &entries)?;
                    return Ok(entries);
                }
                _ => {}
            }
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
