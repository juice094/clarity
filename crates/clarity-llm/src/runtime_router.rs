//! Runtime router for model aliases.
//!
//! Provides a single `LlmProvider` implementation that resolves a routing hint
//! to a concrete alias at call time, builds that provider (with encrypted keys),
//! and delegates the request.  Combined with `ReliableProvider`, this gives
//! hint-based failover without hard-coding provider choices in callers.
//!
//! Supported hints (case-insensitive):
//! - `cheapest` / `cheap` — lowest estimated cost
//! - `coding` — prefers providers tagged `coding`
//! - `vision` — prefers providers tagged `vision`
//! - `tools` / `native-tools` — prefers providers tagged `tools`
//! - `fast` — alias for `cheapest` until latency telemetry is available
//! - any other string — treated as an explicit alias to fall back to

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::api::{LlmProvider, LlmResponse, Message, ProviderCapabilities, StreamDelta};
use crate::{
    ModelRegistry, ReliableProvider, build_provider_from_registry_entry, default_secret_store,
};
use clarity_contract::AgentError;

/// A routing hint parsed from an alias such as `router:cheap`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouterHint {
    /// Minimize estimated cost.
    Cheapest,
    /// Prefer coding-capable providers.
    Coding,
    /// Prefer vision-capable providers.
    Vision,
    /// Prefer native tool-calling providers.
    Tools,
    /// Latency-optimized (currently aliases `Cheapest`).
    Fast,
    /// Use a specific alias.
    Explicit(String),
}

impl RouterHint {
    /// Parse a hint string after the `router:` prefix.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "" | "auto" => Self::Cheapest,
            "cheap" | "cheapest" | "cost" => Self::Cheapest,
            "coding" | "code" => Self::Coding,
            "vision" | "image" | "multimodal" => Self::Vision,
            "tools" | "native-tools" | "tool" => Self::Tools,
            "fast" | "quick" => Self::Fast,
            other => Self::Explicit(other.to_string()),
        }
    }

    /// Returns true if the given alias is itself a router alias.
    pub fn is_router_alias(alias: &str) -> bool {
        alias.eq_ignore_ascii_case("router") || alias.to_lowercase().starts_with("router:")
    }

    /// Extract the hint from a router alias.
    pub fn from_alias(alias: &str) -> Option<Self> {
        if !Self::is_router_alias(alias) {
            return None;
        }
        if alias.eq_ignore_ascii_case("router") {
            Some(Self::Cheapest)
        } else {
            alias
                .split_once(':')
                .map(|(_, hint)| Self::parse(hint))
                .or(Some(Self::Cheapest))
        }
    }
}

/// Provider that routes each request according to a hint.
pub struct RouterLlmProvider {
    registry: ModelRegistry,
    secrets: Option<clarity_secrets::SecretStore>,
    hint: RouterHint,
}

impl RouterLlmProvider {
    /// Create a router from a registry and hint.
    pub fn new(registry: ModelRegistry, hint: RouterHint) -> Self {
        Self {
            registry,
            secrets: default_secret_store().ok(),
            hint,
        }
    }

    /// Create a router from an alias such as `router:cheap`.
    pub fn from_alias(alias: &str, registry: ModelRegistry) -> Option<Self> {
        RouterHint::from_alias(alias).map(|hint| Self::new(registry, hint))
    }

    /// Select the best concrete alias for the current hint.
    fn select_alias(&self) -> Option<String> {
        let candidates: Vec<_> = self.registry.list_models();
        if candidates.is_empty() {
            return None;
        }

        match self.hint {
            RouterHint::Explicit(ref alias) => self.registry.get(alias).map(|e| e.alias.clone()),
            RouterHint::Cheapest | RouterHint::Fast => {
                self.select_by_score(&candidates, score_cheap)
            }
            RouterHint::Coding => self.select_by_score(&candidates, score_coding),
            RouterHint::Vision => self.select_by_score(&candidates, score_vision),
            RouterHint::Tools => self.select_by_score(&candidates, score_tools),
        }
    }

    fn select_by_score<F>(&self, candidates: &[&crate::ModelEntry], score: F) -> Option<String>
    where
        F: Fn(&crate::ModelEntry, Option<&crate::ProviderConfig>) -> i64,
    {
        candidates
            .iter()
            .filter(|e| !RouterHint::is_router_alias(&e.alias))
            .map(|e| {
                let provider_cfg = self.registry.get_provider(&e.provider);
                (e.alias.clone(), score(e, provider_cfg))
            })
            .max_by_key(|(_, s)| *s)
            .map(|(alias, _)| alias)
    }

    async fn build_delegate(&self, alias: &str) -> Result<Arc<dyn LlmProvider>, AgentError> {
        let entry = self
            .registry
            .get(alias)
            .ok_or_else(|| AgentError::Llm(format!("Router selected unknown alias '{}'", alias)))?
            .clone();
        let provider_cfg = self
            .registry
            .get_provider(&entry.provider)
            .ok_or_else(|| {
                AgentError::Llm(format!(
                    "Provider '{}' for alias '{}' not found",
                    entry.provider, alias
                ))
            })?
            .clone();

        let provider =
            build_provider_from_registry_entry(&provider_cfg, &entry, None, self.secrets.as_ref())
                .await?;

        // Wrap the selected provider with retry/fallback semantics.
        Ok(Arc::new(ReliableProvider::new(vec![Arc::from(provider)])))
    }
}

fn score_cheap(entry: &crate::ModelEntry, provider_cfg: Option<&crate::ProviderConfig>) -> i64 {
    let pricing = entry
        .pricing
        .or(provider_cfg.and_then(|p| p.pricing))
        .unwrap_or_else(|| default_family_pricing(&entry.provider));
    // Higher score = cheaper.  Use negative mills per 1M tokens.
    let cost = pricing.input_per_1m + pricing.output_per_1m;
    if cost > 0.0 {
        (-cost * 1000.0) as i64
    } else {
        // No known pricing: prefer providers that look like cheap distills.
        if entry.tags.iter().any(|t| t.eq_ignore_ascii_case("cheap")) {
            -100
        } else {
            -1000
        }
    }
}

fn score_coding(entry: &crate::ModelEntry, provider_cfg: Option<&crate::ProviderConfig>) -> i64 {
    let tag_bonus = if entry.tags.iter().any(|t| t.eq_ignore_ascii_case("coding")) {
        1000
    } else {
        0
    };
    let family_bonus = match entry.provider.as_str() {
        "kimi-code" => 500,
        "deepseek" => 300,
        "openai" => 200,
        _ => 0,
    };
    tag_bonus + family_bonus + score_cheap(entry, provider_cfg)
}

fn score_vision(entry: &crate::ModelEntry, _provider_cfg: Option<&crate::ProviderConfig>) -> i64 {
    if entry.tags.iter().any(|t| t.eq_ignore_ascii_case("vision")) {
        1000
    } else {
        0
    }
}

fn score_tools(entry: &crate::ModelEntry, _provider_cfg: Option<&crate::ProviderConfig>) -> i64 {
    if entry.tags.iter().any(|t| t.eq_ignore_ascii_case("tools")) {
        1000
    } else {
        0
    }
}

/// Best-effort default pricing when none is configured.
fn default_family_pricing(family: &str) -> clarity_contract::llm::Pricing {
    use clarity_contract::llm::Pricing;
    match family {
        "deepseek" => Pricing {
            input_per_1m: 0.14,
            output_per_1m: 0.28,
        },
        "kimi" | "moonshot" => Pricing {
            input_per_1m: 0.50,
            output_per_1m: 0.50,
        },
        "kimi-code" => Pricing {
            input_per_1m: 0.30,
            output_per_1m: 0.30,
        },
        "openai" => Pricing {
            input_per_1m: 2.50,
            output_per_1m: 10.0,
        },
        "anthropic" => Pricing {
            input_per_1m: 3.0,
            output_per_1m: 15.0,
        },
        _ => Pricing {
            input_per_1m: 1.0,
            output_per_1m: 1.0,
        },
    }
}

#[async_trait]
impl LlmProvider for RouterLlmProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<LlmResponse, AgentError> {
        let alias = self
            .select_alias()
            .ok_or_else(|| AgentError::Llm("Router could not select any alias".into()))?;
        tracing::info!("Routing request to alias: {}", alias);
        let delegate = self.build_delegate(&alias).await?;
        delegate.complete(messages, tools).await
    }

    fn stream(
        &self,
        messages: &[Message],
        tools: &Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let alias = self
            .select_alias()
            .ok_or_else(|| AgentError::Llm("Router could not select any alias".into()))?;
        let delegate = tokio::task::block_in_place(|| {
            // Build is async; stream is sync.  Spawn a tiny runtime block.
            tokio::runtime::Handle::current().block_on(self.build_delegate(&alias))
        })?;
        delegate.stream(messages, tools)
    }

    fn set_prompt_cache_key(&self, key: &str) {
        // No-op: delegates are created per-request.
        let _ = key;
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::llm::Pricing;

    fn sample_registry() -> ModelRegistry {
        use crate::model_registry::{ModelConfigFile, ModelEntry, ProviderConfig};
        let mut file = ModelConfigFile::default();
        file.providers.insert(
            "deepseek".into(),
            ProviderConfig {
                ..Default::default()
            },
        );
        file.providers.insert(
            "openai".into(),
            ProviderConfig {
                pricing: Some(Pricing {
                    input_per_1m: 2.5,
                    output_per_1m: 10.0,
                }),
                ..Default::default()
            },
        );
        file.models.push(ModelEntry {
            alias: "deepseek-chat".into(),
            provider: "deepseek".into(),
            model_id: "deepseek-chat".into(),
            tags: vec!["cheap".into(), "coding".into()],
            ..Default::default()
        });
        file.models.push(ModelEntry {
            alias: "gpt-4o".into(),
            provider: "openai".into(),
            model_id: "gpt-4o".into(),
            tags: vec!["vision".into(), "tools".into()],
            ..Default::default()
        });
        ModelRegistry::from_config(file).unwrap()
    }

    #[test]
    fn test_router_hint_parse() {
        assert_eq!(RouterHint::parse("cheap"), RouterHint::Cheapest);
        assert_eq!(RouterHint::parse("coding"), RouterHint::Coding);
        assert_eq!(RouterHint::parse("vision"), RouterHint::Vision);
        assert_eq!(RouterHint::parse("tools"), RouterHint::Tools);
        assert_eq!(
            RouterHint::parse("explicit-alias"),
            RouterHint::Explicit("explicit-alias".into())
        );
    }

    #[test]
    fn test_select_cheap() {
        let registry = sample_registry();
        let router = RouterLlmProvider::new(registry, RouterHint::Cheapest);
        assert_eq!(router.select_alias(), Some("deepseek-chat".into()));
    }

    #[test]
    fn test_select_vision() {
        let registry = sample_registry();
        let router = RouterLlmProvider::new(registry, RouterHint::Vision);
        assert_eq!(router.select_alias(), Some("gpt-4o".into()));
    }

    #[test]
    fn test_from_alias() {
        let registry = sample_registry();
        let router = RouterLlmProvider::from_alias("router:cheap", registry).unwrap();
        assert_eq!(router.select_alias(), Some("deepseek-chat".into()));
    }
}
