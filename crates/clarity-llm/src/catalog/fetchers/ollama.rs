//! Ollama `/api/tags` catalog fetcher.

use async_trait::async_trait;
use serde::Deserialize;

use crate::catalog::CatalogError;
use crate::catalog::entry::ModelCatalogEntry;
use crate::catalog::fetcher::CatalogFetcher;

/// Response shape returned by `GET /api/tags`.
#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

/// Fetches the list of models from a local Ollama instance.
#[derive(Debug, Clone)]
pub struct OllamaFetcher {
    base_url: String,
}

impl OllamaFetcher {
    /// Create a fetcher for the given Ollama base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    /// Build a fetcher from canonical registry defaults.
    pub fn from_defaults() -> Result<Self, CatalogError> {
        let defaults = crate::registry_table::family_defaults("ollama")
            .ok_or_else(|| CatalogError::MissingBaseUrl("ollama".into()))?;
        let base_url = defaults
            .base_url
            .ok_or_else(|| CatalogError::MissingBaseUrl("ollama".into()))?;
        Ok(Self::new(base_url))
    }
}

#[async_trait]
impl CatalogFetcher for OllamaFetcher {
    fn family(&self) -> &str {
        "ollama"
    }

    async fn fetch(&self) -> Result<Vec<ModelCatalogEntry>, CatalogError> {
        let url = format!("{}/api/tags", self.base_url.trim_end_matches('/'));
        let client = reqwest::Client::new();
        let response = client.get(&url).send().await?.error_for_status()?;

        let payload: OllamaTagsResponse = response.json().await?;
        Ok(payload
            .models
            .into_iter()
            .map(|m| ModelCatalogEntry::new(self.family(), m.name))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ollama_tags_response() {
        let json = r#"{"models":[
            {"name":"llama3.2:latest","model":"llama3.2:latest","modified_at":"2024-01-01","size":100,"digest":"abc"},
            {"name":"qwen2.5-coder:latest","model":"qwen2.5-coder:latest"}
        ]}"#;
        let parsed: OllamaTagsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.models.len(), 2);
        assert_eq!(parsed.models[0].name, "llama3.2:latest");
    }

    #[test]
    fn from_defaults_uses_registry_url() {
        let fetcher = OllamaFetcher::from_defaults().unwrap();
        assert_eq!(fetcher.family(), "ollama");
        assert!(fetcher.base_url.contains("11434"));
    }
}
