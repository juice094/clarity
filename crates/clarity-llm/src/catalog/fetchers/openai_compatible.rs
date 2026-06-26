//! OpenAI-compatible `/v1/models` catalog fetcher.

use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

use crate::catalog::CatalogError;
use crate::catalog::entry::ModelCatalogEntry;
use crate::catalog::fetcher::CatalogFetcher;

/// Response shape returned by `GET /v1/models`.
#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

/// Fetches the model list from any OpenAI-compatible provider.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleFetcher {
    family: String,
    base_url: String,
    api_key: Option<String>,
}

impl OpenAiCompatibleFetcher {
    /// Create a fetcher for a provider family and base URL.
    pub fn new(
        family: impl Into<String>,
        base_url: impl Into<String>,
        api_key: Option<String>,
    ) -> Self {
        Self {
            family: family.into(),
            base_url: base_url.into(),
            api_key,
        }
    }

    /// Build a fetcher from canonical registry defaults for a family.
    ///
    /// The API key is resolved from the family's `api_key_env` using
    /// [`clarity_contract::resolve_key_ref`].
    pub fn from_defaults(family: &str) -> Result<Self, CatalogError> {
        let defaults = crate::registry_table::family_defaults(family)
            .ok_or_else(|| CatalogError::MissingBaseUrl(family.into()))?;
        let base_url = defaults
            .base_url
            .ok_or_else(|| CatalogError::MissingBaseUrl(family.into()))?;
        let api_key = defaults
            .api_key_env
            .as_deref()
            .and_then(clarity_contract::resolve_key_ref);
        Ok(Self::new(family, base_url, api_key))
    }
}

#[async_trait]
impl CatalogFetcher for OpenAiCompatibleFetcher {
    fn family(&self) -> &str {
        &self.family
    }

    async fn fetch(&self) -> Result<Vec<ModelCatalogEntry>, CatalogError> {
        let url = format!("{}/v1/models", self.base_url.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;
        let mut request = client.get(&url);
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }

        let response = request.send().await?.error_for_status()?;
        let payload: OpenAiModelsResponse = response.json().await?;
        Ok(payload
            .data
            .into_iter()
            .map(|m| ModelCatalogEntry::new(&self.family, m.id))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_openai_models_response() {
        let json = r#"{"object":"list","data":[
            {"id":"gpt-4o","object":"model"},
            {"id":"gpt-4o-mini","object":"model"}
        ]}"#;
        let parsed: OpenAiModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.data.len(), 2);
        assert_eq!(parsed.data[0].id, "gpt-4o");
    }

    #[test]
    fn from_defaults_resolves_api_key_env() {
        let fetcher = OpenAiCompatibleFetcher::from_defaults("openai").unwrap();
        assert_eq!(fetcher.family(), "openai");
        assert!(fetcher.base_url.contains("openai.com"));
        let expected = std::env::var("OPENAI_API_KEY").ok();
        assert_eq!(fetcher.api_key, expected);
    }
}
