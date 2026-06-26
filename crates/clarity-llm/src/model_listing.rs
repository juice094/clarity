//! Model enumeration helpers for settings UIs.
//!
//! These helpers were originally in `clarity-core::view_models::settings` but
//! depend on `clarity_llm::ModelRegistry`, so they live here to keep the core
//! crate decoupled from concrete LLM provider construction.

use crate::model_registry::ModelRegistry;
use crate::registry_table;
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
/// Merges the dynamic `ModelRegistry` with a fallback derived from
/// `registry_table` canonical defaults when no registry is configured.
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

    // Fallback catalog derived from the canonical registry defaults.
    // Only local GGUF scanning stays dynamic here.
    let local_models = scan_local_models();
    let local_model_names: Vec<String> = if local_models.is_empty() {
        vec!["No models found — place .gguf in ~/models/".into()]
    } else {
        local_models.into_iter().map(|(_, name)| name).collect()
    };

    for family in registry_table::all_family_names() {
        if !seen_providers.insert(family.to_string()) {
            continue;
        }

        let defaults = match registry_table::family_defaults(family) {
            Some(d) => d,
            None => continue,
        };

        let models = if *family == "local" {
            local_model_names.clone()
        } else if defaults.known_models.is_empty() {
            // Defensive fallback: if no curated list exists, at least surface
            // the family's default model so the provider remains selectable.
            defaults.default_model.into_iter().collect()
        } else {
            defaults.known_models
        };

        if !models.is_empty() {
            result.push((family.to_string(), format_provider_name(family), models));
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

    #[test]
    fn test_fallback_derives_from_registry_table() {
        let models = get_available_models();
        let openai = models.iter().find(|(id, _, _)| id == "openai");
        assert!(openai.is_some(), "openai should appear in fallback");
        let (_, _, openai_models) = openai.unwrap();
        assert!(
            openai_models.contains(&"gpt-4o".to_string()),
            "openai fallback should include gpt-4o"
        );
    }

    #[test]
    fn test_no_duplicate_moonshot_when_kimi_present() {
        // The registry-derived fallback keeps alias families distinct.
        let models = get_available_models();
        let ids: Vec<&str> = models.iter().map(|(id, _, _)| id.as_str()).collect();
        assert!(ids.contains(&"kimi"));
        assert!(
            ids.iter().filter(|&&id| id == "moonshot").count() <= 1,
            "moonshot alias should appear at most once"
        );
    }
}
