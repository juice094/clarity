//! Catalog entry type.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single model entry in a provider's catalog.
///
/// This is the shared representation used by the bootstrap seed, on-disk
/// cache, and remote fetchers. It is intentionally plain: provider-specific
/// quirks are stored in [`metadata`](Self::metadata) rather than fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelCatalogEntry {
    /// Provider family name, e.g. `"openai"`.
    pub family: String,
    /// Model identifier accepted by the provider API.
    pub model_id: String,
    /// Human-readable display name shown in settings UIs.
    pub display_name: Option<String>,
    /// Capability tags such as `"chat"`, `"vision"`, or `"function-calling"`.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Provider-specific metadata, e.g. context-window size or pricing hints.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl ModelCatalogEntry {
    /// Create a minimal entry from a family name and model identifier.
    pub fn new(family: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            family: family.into(),
            model_id: model_id.into(),
            display_name: None,
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}
