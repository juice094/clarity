//! Simple mesh router — maintains a fallback chain of provider names.

/// Ordered list of provider names to try for each LLM request.
#[derive(Clone, Debug)]
pub struct MeshRouter {
    pub fallback_chain: Vec<String>,
}

impl MeshRouter {
    /// Build the chain from `CLARITY_MESH_FALLBACK` (comma-separated).
    /// Falls back to `CLARITY_MESH_PROVIDERS` order if the env var is absent.
    pub fn from_env() -> Self {
        let chain = std::env::var("CLARITY_MESH_FALLBACK")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self { fallback_chain: chain }
    }

    pub fn with_chain(chain: Vec<String>) -> Self {
        Self { fallback_chain: chain }
    }
}
