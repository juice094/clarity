//! Model enumeration helpers for settings UIs.
//!
//! These helpers were originally in `clarity-core::view_models::settings` but
//! depend on `clarity_llm::ModelRegistry`, so they live here to keep the core
//! crate decoupled from concrete LLM provider construction.

use crate::model_registry::ModelRegistry;
use std::collections::HashSet;
use std::path::PathBuf;

/// Scan the filesystem for local `.gguf` models.
///
/// Searches:
/// 1. `CLARITY_LOCAL_MODEL_PATH` (file or directory)
/// 2. `~/models/`
///
/// Returns `(path, name)` tuples sorted by name.
pub fn scan_local_models() -> Vec<(String, String)> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    fn add_ggufs_from_dir(
        dir: &PathBuf,
        results: &mut Vec<(String, String)>,
        seen: &mut HashSet<String>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("gguf"))
                        .unwrap_or(false)
                {
                    let path_str = path.to_string_lossy().into_owned();
                    if seen.insert(path_str.clone()) {
                        let name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        results.push((path_str, name));
                    }
                }
            }
        }
    }

    if let Ok(path_str) = std::env::var("CLARITY_LOCAL_MODEL_PATH") {
        let p = PathBuf::from(&path_str);
        if p.is_dir() {
            add_ggufs_from_dir(&p, &mut results, &mut seen);
        } else if p.is_file()
            && p.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("gguf"))
                .unwrap_or(false)
        {
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if seen.insert(path_str.clone()) {
                results.push((path_str, name));
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        let models_dir = home.join("models");
        if models_dir.is_dir() {
            add_ggufs_from_dir(&models_dir, &mut results, &mut seen);
        }
    }

    results.sort_by(|a, b| a.1.cmp(&b.1));
    results
}

/// Format a provider ID into a human-friendly display name.
fn format_provider_name(id: &str) -> String {
    match id {
        "openai" => "OpenAI".into(),
        "anthropic" => "Anthropic".into(),
        "kimi" => "Kimi".into(),
        "kimi-code" => "Kimi Code".into(),
        "deepseek" => "DeepSeek".into(),
        "ollama" => "Ollama".into(),
        "local" => "Local (GGUF)".into(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => other.into(),
            }
        }
    }
}

/// Return the list of available providers and their models for settings UIs.
///
/// Merges the dynamic `ModelRegistry` with a hardcoded fallback list for
/// backward compatibility when no registry is configured.
pub fn get_available_models() -> Vec<(String, String, Vec<String>)> {
    let mut result: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut seen_providers = HashSet::new();

    // Phase 2: Merge dynamic registry with hardcoded fallback
    // Registry takes precedence; hardcoded fills gaps for backward compat.
    if let Ok(registry) = ModelRegistry::load() {
        for provider_id in registry.list_providers() {
            seen_providers.insert(provider_id.clone());
            let models: Vec<String> = if provider_id == "local" {
                let local_models = scan_local_models();
                if local_models.is_empty() {
                    vec!["No models found — place .gguf in ~/models/".into()]
                } else {
                    local_models.into_iter().map(|(_, name)| name).collect()
                }
            } else {
                registry
                    .list_models()
                    .into_iter()
                    .filter(|m| &m.provider == provider_id)
                    .map(|m| m.model_id.clone())
                    .collect()
            };
            if !models.is_empty() {
                result.push((
                    provider_id.clone(),
                    format_provider_name(provider_id),
                    models,
                ));
            }
        }
    }

    // Hardcoded fallback for providers not present in registry
    let local_models = scan_local_models();
    let local_model_names: Vec<String> = if local_models.is_empty() {
        vec!["No models found — place .gguf in ~/models/".into()]
    } else {
        local_models.into_iter().map(|(_, name)| name).collect()
    };

    let fallback = vec![
        (
            "openai".to_string(),
            "OpenAI".to_string(),
            vec![
                "gpt-4o".into(),
                "gpt-4o-mini".into(),
                "gpt-4.1".into(),
                "gpt-4.1-mini".into(),
                "gpt-4.1-nano".into(),
                "o1".into(),
                "o1-mini".into(),
                "o3-mini".into(),
            ],
        ),
        (
            "anthropic".to_string(),
            "Anthropic".to_string(),
            vec![
                "claude-3-7-sonnet-20250219".into(),
                "claude-3-5-sonnet-20241022".into(),
                "claude-3-5-haiku-20241022".into(),
                "claude-3-opus-20240229".into(),
            ],
        ),
        (
            "kimi".to_string(),
            "Kimi".to_string(),
            vec![
                "kimi-k2.6".into(),
                "kimi-k2-07132k".into(),
                "kimi-k1.5".into(),
                "kimi-latest".into(),
            ],
        ),
        (
            "deepseek".to_string(),
            "DeepSeek".to_string(),
            vec![
                "deepseek-v4-flash".into(),
                "deepseek-v4-pro".into(),
                "deepseek-chat".into(),
                "deepseek-reasoner".into(),
                "deepseek-coder".into(),
            ],
        ),
        (
            "ollama".to_string(),
            "Ollama".to_string(),
            vec![
                "llama3.2".into(),
                "llama3.1".into(),
                "qwen2.5".into(),
                "qwen2.5-coder".into(),
                "deepseek-r1".into(),
                "phi4".into(),
            ],
        ),
        (
            "local".to_string(),
            "Local (GGUF)".to_string(),
            local_model_names,
        ),
    ];

    for (id, label, models) in fallback {
        if seen_providers.insert(id.clone()) {
            result.push((id, label, models));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_available_models_has_providers() {
        let models = get_available_models();
        assert!(!models.is_empty());
        let keys: Vec<String> = models.iter().map(|(k, _, _)| k.clone()).collect();
        assert!(keys.contains(&"openai".to_string()));
        assert!(keys.contains(&"local".to_string()));
    }

    #[test]
    fn test_get_available_models_local_label() {
        let models = get_available_models();
        let local = models.iter().find(|(k, _, _)| k == "local");
        assert!(local.is_some());
        let (_, label, _) = local.unwrap();
        assert_eq!(label, "Local (GGUF)");
    }
}
