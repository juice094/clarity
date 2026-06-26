# Plan: Derive `model_listing` fallback from `registry_table`

## Context

`crates/clarity-llm/src/model_listing.rs` currently keeps a ~70-line hard-coded fallback catalog of OpenAI / Anthropic / Kimi / DeepSeek / Ollama / Local models. The same canonical provider families already live in `crates/clarity-llm/src/registry_table.rs`, but that table only exposes a single `default_model` per family. The two sources drift risk is high and violates the Ponytail "single source of truth" discipline.

## Goal

Move the canonical **model catalog** into `registry_table` and make `model_listing::get_available_models()` derive its fallback branch from it. Only truly dynamic data — scanned local `.gguf` names — remains computed in `model_listing`.

## Scope

- Modify `crates/clarity-llm/src/registry_table.rs`
  - Add a `known_models: Vec<String>` field to `FamilyDefaults`.
  - Populate it for every provider family that has a stable public model list.
- Modify `crates/clarity-llm/src/model_listing.rs`
  - Replace the hard-coded `fallback` vec with a derivation from `registry_table::all_family_names()` + `family_defaults(name).known_models`.
  - Keep `scan_local_models()` and the local-provider placeholder unchanged.
- Add/update unit tests.

Out of scope: changing `ModelRegistry::built_in_fallback()` behavior (it continues to use only `default_model` for runtime alias construction).

---

## 1. Functions to change

### `crates/clarity-llm/src/registry_table.rs`

1. `FamilyDefaults` struct — add `known_models`.
2. `Default for FamilyDefaults` — initialize `known_models: Vec::new()`.
3. `family_defaults(name: &str)` — populate `known_models` in every `Some(...)` arm.
4. Add helper (optional but recommended):
   ```rust
   pub fn family_models(name: &str) -> Vec<String> {
       family_defaults(name)
           .map(|d| d.known_models)
           .unwrap_or_default()
   }
   ```

### `crates/clarity-llm/src/model_listing.rs`

1. `get_available_models()` — replace the hard-coded `fallback` vec (lines 147–210) with a derived catalog.
2. Keep `format_provider_name()` and `scan_local_models()` as-is.
3. Add a private helper `build_registry_fallback()` for clarity (optional).

---

## 2. How to import/use `registry_table` data

`model_listing.rs` is in the same crate as `registry_table.rs` (`clarity-llm`), so a plain module import is enough:

```rust
use crate::registry_table;
```

The fallback builder iterates over the canonical family list:

```rust
let mut fallback: Vec<(String, String, Vec<String>)> = Vec::new();
for family in registry_table::all_family_names() {
    let defaults = match registry_table::family_defaults(family) {
        Some(d) => d,
        None => continue,
    };

    let models = if family == "local" {
        local_model_names.clone()
    } else if defaults.known_models.is_empty() {
        // Defensive: at least expose the default_model so the provider is visible.
        defaults.default_model.into_iter().collect()
    } else {
        defaults.known_models.clone()
    };

    if !models.is_empty() {
        fallback.push((
            family.to_string(),
            format_provider_name(family),
            models,
        ));
    }
}
```

Type mapping:

| `registry_table` type | `model_listing` type | Conversion |
|---|---|---|
| `&str` (family name) | `String` | `.to_string()` |
| `Vec<String>` (`known_models`) | `Vec<String>` | clone |
| `Option<String>` (`default_model`) | `Vec<String>` | `.into_iter().collect()` |

---

## 3. Concrete `Edit` blocks

### 3.1 `registry_table.rs`: extend `FamilyDefaults`

**Edit 1 — struct field**

```rust
// old_string
    /// Default model identifier for this family.
    pub default_model: Option<String>,
    /// Capability / visibility tags (e.g. "chat-only").
    pub tags: Vec<String>,
}
```

```rust
// new_string
    /// Default model identifier for this family.
    pub default_model: Option<String>,
    /// Known public model identifiers for this family, shown in settings UIs
    /// when no registry config exists.
    pub known_models: Vec<String>,
    /// Capability / visibility tags (e.g. "chat-only").
    pub tags: Vec<String>,
}
```

**Edit 2 — `Default` impl**

```rust
// old_string
            default_model: None,
            tags: Vec::new(),
        }
    }
}
```

```rust
// new_string
            default_model: None,
            known_models: Vec::new(),
            tags: Vec::new(),
        }
    }
}
```

**Edit 3 — populate `known_models` for `openai`**

```rust
// old_string
        "openai" => Some(FamilyDefaults {
            base_url: Some("https://api.openai.com/v1".into()),
            api_key_env: Some("OPENAI_API_KEY".into()),
            default_model: Some("gpt-4o".into()),
            ..Default::default()
        }),
```

```rust
// new_string
        "openai" => Some(FamilyDefaults {
            base_url: Some("https://api.openai.com/v1".into()),
            api_key_env: Some("OPENAI_API_KEY".into()),
            default_model: Some("gpt-4o".into()),
            known_models: vec![
                "gpt-4o".into(),
                "gpt-4o-mini".into(),
                "gpt-4.1".into(),
                "gpt-4.1-mini".into(),
                "gpt-4.1-nano".into(),
                "o1".into(),
                "o1-mini".into(),
                "o3-mini".into(),
            ],
            ..Default::default()
        }),
```

**Edit 4 — populate `known_models` for `deepseek`**

```rust
// old_string
        "deepseek" => Some(FamilyDefaults {
            base_url: Some("https://api.deepseek.com/v1".into()),
            api_key_env: Some("DEEPSEEK_API_KEY".into()),
            default_model: Some("deepseek-chat".into()),
            ..Default::default()
        }),
```

```rust
// new_string
        "deepseek" => Some(FamilyDefaults {
            base_url: Some("https://api.deepseek.com/v1".into()),
            api_key_env: Some("DEEPSEEK_API_KEY".into()),
            default_model: Some("deepseek-chat".into()),
            known_models: vec![
                "deepseek-v4-flash".into(),
                "deepseek-v4-pro".into(),
                "deepseek-chat".into(),
                "deepseek-reasoner".into(),
                "deepseek-coder".into(),
            ],
            ..Default::default()
        }),
```

**Edit 5 — populate `known_models` for `kimi` and `moonshot`**

```rust
// old_string
        "kimi" => Some(FamilyDefaults {
            base_url: Some("https://api.moonshot.cn/v1".into()),
            api_key_env: Some("KIMI_API_KEY".into()),
            default_model: Some("kimi-k2.6".into()),
            ..Default::default()
        }),
        "moonshot" => Some(FamilyDefaults {
            base_url: Some("https://api.moonshot.cn/v1".into()),
            api_key_env: Some("KIMI_API_KEY".into()),
            default_model: Some("kimi-k2.6".into()),
            ..Default::default()
        }),
```

```rust
// new_string
        "kimi" => Some(FamilyDefaults {
            base_url: Some("https://api.moonshot.cn/v1".into()),
            api_key_env: Some("KIMI_API_KEY".into()),
            default_model: Some("kimi-k2.6".into()),
            known_models: vec![
                "kimi-k2.6".into(),
                "kimi-k2-07132k".into(),
                "kimi-k1.5".into(),
                "kimi-latest".into(),
            ],
            ..Default::default()
        }),
        "moonshot" => Some(FamilyDefaults {
            base_url: Some("https://api.moonshot.cn/v1".into()),
            api_key_env: Some("KIMI_API_KEY".into()),
            default_model: Some("kimi-k2.6".into()),
            known_models: vec![
                "kimi-k2.6".into(),
                "kimi-k2-07132k".into(),
                "kimi-k1.5".into(),
                "kimi-latest".into(),
            ],
            ..Default::default()
        }),
```

> Note: `moonshot` intentionally mirrors `kimi` because it is the same provider family under a different ID.

**Edit 6 — populate `known_models` for `kimi-code`**

```rust
// old_string
        "kimi-code" => Some(FamilyDefaults {
            base_url: Some("https://api.kimi.com/coding/v1".into()),
            api_key_env: Some("KIMI_CODE_API_KEY".into()),
            auth_type: AuthType::OAuth,
            auth_token_key: Some("kimi-code".into()),
            oauth: Some(OAuthProviderConfig {
                client_id: "17e5f671-d194-4dfb-9706-5516cb48c098".into(),
                ..Default::default()
            }),
            default_model: Some("kimi-k2.6".into()),
            ..Default::default()
        }),
```

```rust
// new_string
        "kimi-code" => Some(FamilyDefaults {
            base_url: Some("https://api.kimi.com/coding/v1".into()),
            api_key_env: Some("KIMI_CODE_API_KEY".into()),
            auth_type: AuthType::OAuth,
            auth_token_key: Some("kimi-code".into()),
            oauth: Some(OAuthProviderConfig {
                client_id: "17e5f671-d194-4dfb-9706-5516cb48c098".into(),
                ..Default::default()
            }),
            default_model: Some("kimi-k2.6".into()),
            known_models: vec!["kimi-k2.6".into()],
            ..Default::default()
        }),
```

**Edit 7 — populate `known_models` for `anthropic`**

```rust
// old_string
        "anthropic" => Some(FamilyDefaults {
            protocol: ProtocolType::AnthropicMessages,
            base_url: Some("https://api.anthropic.com".into()),
            api_key_env: Some("ANTHROPIC_AUTH_TOKEN".into()),
            default_model: Some("claude-sonnet".into()),
            ..Default::default()
        }),
```

```rust
// new_string
        "anthropic" => Some(FamilyDefaults {
            protocol: ProtocolType::AnthropicMessages,
            base_url: Some("https://api.anthropic.com".into()),
            api_key_env: Some("ANTHROPIC_AUTH_TOKEN".into()),
            default_model: Some("claude-sonnet".into()),
            known_models: vec![
                "claude-3-7-sonnet-20250219".into(),
                "claude-3-5-sonnet-20241022".into(),
                "claude-3-5-haiku-20241022".into(),
                "claude-3-opus-20240229".into(),
            ],
            ..Default::default()
        }),
```

**Edit 8 — populate `known_models` for `ollama`**

```rust
// old_string
        "ollama" => Some(FamilyDefaults {
            protocol: ProtocolType::Ollama,
            base_url: Some("http://localhost:11434".into()),
            auth_type: AuthType::None,
            default_model: Some("ollama-llama3".into()),
            ..Default::default()
        }),
```

```rust
// new_string
        "ollama" => Some(FamilyDefaults {
            protocol: ProtocolType::Ollama,
            base_url: Some("http://localhost:11434".into()),
            auth_type: AuthType::None,
            default_model: Some("ollama-llama3".into()),
            known_models: vec![
                "llama3.2".into(),
                "llama3.1".into(),
                "qwen2.5".into(),
                "qwen2.5-coder".into(),
                "deepseek-r1".into(),
                "phi4".into(),
            ],
            ..Default::default()
        }),
```

**Edit 9 — `llama-server`, `deepseek-device`, and `local`**

These families currently have no equivalent hard-coded list in `model_listing.rs`. To avoid introducing new UI entries silently, set `known_models: Vec::new()` (the default) for now and let `default_model` act as the fallback visibility item:

```rust
// old_string
        "llama-server" => Some(FamilyDefaults {
            protocol: ProtocolType::LlamaServer,
            base_url: Some("http://localhost:8080".into()),
            auth_type: AuthType::None,
            default_model: Some("llama-server-default".into()),
            ..Default::default()
        }),
        "deepseek-device" => Some(FamilyDefaults {
            protocol: ProtocolType::DeepSeekDevice,
            base_url: Some("https://chat.deepseek.com".into()),
            api_key_env: Some("DEEPSEEK_DEVICE_TOKEN".into()),
            default_model: Some("deepseek-chat".into()),
            tags: vec!["chat-only".to_string()],
            ..Default::default()
        }),
        #[cfg(feature = "local-llm")]
        "local" => Some(FamilyDefaults {
            protocol: ProtocolType::KalosmLocal,
            auth_type: AuthType::None,
            default_model: Some("local-qwen".into()),
            ..Default::default()
        }),
```

```rust
// new_string
        "llama-server" => Some(FamilyDefaults {
            protocol: ProtocolType::LlamaServer,
            base_url: Some("http://localhost:8080".into()),
            auth_type: AuthType::None,
            default_model: Some("llama-server-default".into()),
            ..Default::default()
        }),
        "deepseek-device" => Some(FamilyDefaults {
            protocol: ProtocolType::DeepSeekDevice,
            base_url: Some("https://chat.deepseek.com".into()),
            api_key_env: Some("DEEPSEEK_DEVICE_TOKEN".into()),
            default_model: Some("deepseek-chat".into()),
            tags: vec!["chat-only".to_string()],
            ..Default::default()
        }),
        #[cfg(feature = "local-llm")]
        "local" => Some(FamilyDefaults {
            protocol: ProtocolType::KalosmLocal,
            auth_type: AuthType::None,
            default_model: Some("local-qwen".into()),
            ..Default::default()
        }),
```

> These arms rely on `Default::default()` to produce an empty `known_models`, which keeps the behavior conservative.

**Edit 10 — add helper (optional)**

Insert after `family_defaults`:

```rust
// new_string only
/// Return the canonical model list for a provider family.
pub fn family_models(name: &str) -> Vec<String> {
    family_defaults(name)
        .map(|d| d.known_models)
        .unwrap_or_default()
}
```

### 3.2 `model_listing.rs`: replace hard-coded fallback

**Edit 11 — add import**

```rust
// old_string
use crate::model_registry::ModelRegistry;
use std::collections::HashSet;
use std::path::PathBuf;
```

```rust
// new_string
use crate::model_registry::ModelRegistry;
use crate::registry_table;
use std::collections::HashSet;
use std::path::PathBuf;
```

**Edit 12 — replace fallback construction**

```rust
// old_string
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
```

```rust
// new_string
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

        let models = if family == "local" {
            local_model_names.clone()
        } else if defaults.known_models.is_empty() {
            // Defensive fallback: if no curated list exists, at least surface
            // the family's default model so the provider remains selectable.
            defaults.default_model.into_iter().collect()
        } else {
            defaults.known_models
        };

        if !models.is_empty() {
            result.push((
                family.to_string(),
                format_provider_name(family),
                models,
            ));
        }
    }

    result
}
```

---

## 4. Type conversions

| Source | Target | How |
|---|---|---|
| `&'static str` from `all_family_names()` | `String` provider id | `.to_string()` |
| `FamilyDefaults::known_models: Vec<String>` | `Vec<String>` model list | move/clone |
| `FamilyDefaults::default_model: Option<String>` | `Vec<String>` | `into_iter().collect()` (yields 0 or 1 item) |
| `local_model_names: Vec<String>` | per-provider model list | `.clone()` because it is reused only for the `local` family |

No new structs or traits are required. `ModelInfo` is not used in this file; the existing tuple type `(String, String, Vec<String>)` is preserved.

---

## 5. Tests to add/update

### 5.1 Update existing tests in `model_listing.rs`

The two existing tests should still pass without changes:

```rust
#[test]
fn test_get_available_models_has_providers() { ... }

#[test]
fn test_get_available_models_local_label() { ... }
```

Add a regression test that asserts the fallback is derived from `registry_table`:

```rust
#[test]
fn test_fallback_derives_from_registry_table() {
    let models = get_available_models();
    let openai = models.iter().find(|(id, _, _)| id == "openai");
    assert!(openai.is_some(), "openai should appear in fallback");
    let (_, _, openai_models) = openai.unwrap();
    assert!(openai_models.contains(&"gpt-4o".to_string()));
    assert!(openai_models.contains(&"gpt-4o-mini".to_string()));
}
```

Add a test that ensures `moonshot` (or any future alias family) does not produce duplicate UI entries when `kimi` is already present via the registry:

```rust
#[test]
fn test_no_duplicate_moonshot_when_kimi_present() {
    // This test documents the invariant that registry-derived fallback
    // keeps alias families distinct; it is most meaningful when neither
    // family is configured in the runtime registry.
    let models = get_available_models();
    let ids: Vec<&str> = models.iter().map(|(id, _, _)| id.as_str()).collect();
    assert!(ids.contains(&"kimi"));
    assert!(
        ids.iter().filter(|&&id| id == "moonshot").count() <= 1,
        "moonshot alias should appear at most once"
    );
}
```

### 5.2 Add tests in `registry_table.rs`

```rust
#[test]
fn test_family_models_matches_defaults() {
    let openai = super::family_defaults("openai").unwrap();
    assert!(openai.known_models.contains(&"gpt-4o".to_string()));
    assert!(openai.known_models.contains(&"o3-mini".to_string()));
}

#[test]
fn test_family_models_helper() {
    let models = super::family_models("anthropic");
    assert!(!models.is_empty());
    assert!(models.contains(&"claude-3-7-sonnet-20250219".to_string()));
}

#[test]
fn test_unknown_family_models_empty() {
    assert!(super::family_models("unknown-provider").is_empty());
}
```

---

## 6. Risk level and verification commands

### Risk level: **Low to Medium**

- **Behavior change**: Settings UI fallback now includes `moonshot`, `llama-server`, and `deepseek-device` families (via `all_family_names()`) when they are absent from the runtime registry. This is generally desirable but may surprise UI tests that assert an exact provider count.
- **Data loss / functional risk**: None. The function only affects the model listing returned to UIs; actual LLM routing still goes through `ModelRegistry`.
- **Compile risk**: Low. Changes are localized to `clarity-llm` and use only existing public items.

### Verification commands

Run these after applying the edits:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Clippy for clarity-llm (zero warnings)
cargo clippy -p clarity-llm --lib --tests -- -D warnings

# 3. Unit tests for clarity-llm
cargo test -p clarity-llm --lib model_listing
cargo test -p clarity-llm --lib registry_table

# 4. Full clarity-llm test suite
cargo test -p clarity-llm --lib
```

### Manual QA

1. Temporarily move `~/.config/clarity/models.toml` and `./.clarity/models.toml` aside.
2. Run `cargo run -p clarity-egui`.
3. Open Settings → Provider/Model selector.
4. Confirm that OpenAI, Anthropic, Kimi, DeepSeek, and Ollama still appear with their previous model lists.
5. Confirm that `local` shows either scanned `.gguf` names or the placeholder message.

---

## 7. Follow-up ideas (out of scope for this refactor)

- Consider deduplicating `kimi`/`moonshot` known_models by extracting a shared constant if more alias families appear.
- Consider fetching Ollama's actual tag list via `/api/tags` when the daemon is reachable, making the Ollama catalog fully dynamic.
- Consider adding a `display_order` or `category` field to `FamilyDefaults` so the UI can sort families consistently without hard-coding order in `model_listing`.
