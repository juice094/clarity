//! Ollama `/api/tags` catalog fetcher.

use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

use crate::catalog::entry::ModelCatalogEntry;
use crate::catalog::fetcher::CatalogFetcher;
use crate::catalog::CatalogError;

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
    ///
    /// Honors the `OLLAMA_HOST` environment variable if set, matching the
    /// Ollama CLI convention.
    pub fn from_defaults() -> Result<Self, CatalogError> {
        let base_url = if let Ok(host) = std::env::var("OLLAMA_HOST") {
            normalize_ollama_host(&host)
        } else {
            let defaults = crate::registry_table::family_defaults("ollama")
                .ok_or_else(|| CatalogError::MissingBaseUrl("ollama".into()))?;
            defaults
                .base_url
                .ok_or_else(|| CatalogError::MissingBaseUrl("ollama".into()))?
        };
        Ok(Self::new(base_url))
    }
}

fn normalize_ollama_host(host: &str) -> String {
    let host = host.trim();
    if let Some(rest) = host.strip_prefix("http://") {
        return format!("http://{}", replace_zero_addr(rest));
    }
    if let Some(rest) = host.strip_prefix("https://") {
        return format!("https://{}", replace_zero_addr(rest));
    }
    format!("http://{}", replace_zero_addr(host))
}

/// `0.0.0.0` is a valid bind address but not a valid connect address.
/// Treat it as localhost so users don't have to manually reconfigure.
fn replace_zero_addr(addr: &str) -> String {
    let addr = addr.trim();
    if addr == "0.0.0.0" {
        return "127.0.0.1".to_string();
    }
    if let Some(rest) = addr.strip_prefix("0.0.0.0:") {
        return format!("127.0.0.1:{}", rest);
    }
    addr.to_string()
}

#[async_trait]
impl CatalogFetcher for OllamaFetcher {
    fn family(&self) -> &str {
        "ollama"
    }

    async fn fetch(&self) -> Result<Vec<ModelCatalogEntry>, CatalogError> {
        let url = format!("{}/api/tags", self.base_url.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;
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

    #[test]
    fn normalize_ollama_host_adds_scheme() {
        assert_eq!(
            normalize_ollama_host("localhost:11434"),
            "http://localhost:11434"
        );
        assert_eq!(
            normalize_ollama_host("http://192.168.1.5:11434"),
            "http://192.168.1.5:11434"
        );
    }

    #[test]
    fn normalize_ollama_host_maps_zero_addr_to_localhost() {
        assert_eq!(
            normalize_ollama_host("0.0.0.0:11434"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(normalize_ollama_host("0.0.0.0"), "http://127.0.0.1");
        assert_eq!(
            normalize_ollama_host("http://0.0.0.0:11434"),
            "http://127.0.0.1:11434"
        );
    }
}
