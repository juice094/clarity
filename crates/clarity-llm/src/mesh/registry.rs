//! Provider registry for the LLM mesh.
//!
//! Builds a map of named providers from environment variables or explicit config.

use std::collections::HashMap;
use std::sync::Arc;

use crate::LlmFactory;
use clarity_contract::{AgentError, LlmProvider};

/// Provider registry for the LLM mesh.
pub struct MeshRegistry;

impl MeshRegistry {
    /// Build a provider map from a comma-separated list of provider names.
    ///
    /// Example `CLARITY_MESH_PROVIDERS=openai,kimi,ollama`
    pub async fn from_env() -> Result<HashMap<String, Arc<dyn LlmProvider>>, AgentError> {
        let mut map = HashMap::new();

        let names = std::env::var("CLARITY_MESH_PROVIDERS").unwrap_or_default();

        if names.trim().is_empty() {
            // No mesh config — return empty map so caller can fall back to single provider.
            return Ok(map);
        }

        for name in names.split(',') {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            match LlmFactory::create_arc(name).await {
                Ok(provider) => {
                    tracing::info!("Mesh registry loaded provider: {}", name);
                    map.insert(name.to_string(), provider);
                }
                Err(e) => {
                    tracing::warn!("Mesh registry failed to load provider '{}': {}", name, e);
                }
            }
        }

        if map.is_empty() {
            return Err(AgentError::Llm(
                "No mesh providers could be loaded from CLARITY_MESH_PROVIDERS".into(),
            ));
        }

        Ok(map)
    }
}
