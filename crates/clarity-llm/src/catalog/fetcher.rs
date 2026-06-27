//! Catalog fetcher trait.

use async_trait::async_trait;

use crate::catalog::entry::ModelCatalogEntry;
use crate::catalog::CatalogError;

/// Fetches a provider's model catalog from a remote or local source.
///
/// Implementations are registered with [`ModelCatalogService`](super::service::ModelCatalogService)
/// and called during refresh. A fetcher should return an empty vector when the
/// source is reachable but advertises no models, and an error only when the
/// source is reachable yet responds in an unexpected way.
#[async_trait]
pub trait CatalogFetcher: Send + Sync {
    /// Provider family this fetcher handles, e.g. `"ollama"`.
    fn family(&self) -> &str;

    /// Fetch the current catalog for the provider family.
    async fn fetch(&self) -> Result<Vec<ModelCatalogEntry>, CatalogError>;
}
