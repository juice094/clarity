---
title: Ponytail-Style Review of Provider / Compute Layer
category: notes
date: 2026-06-26
---

# Ponytail-Style Review of `crates/clarity-llm/` and Related Code

This review lists candidates for simplification based on the Ponytail lazy-senior-dev principles (YAGNI, stdlib/dependency reuse, delete-before-add, boring-over-clever, and explicit `// ponytail:` markers for shortcuts). No code was modified.

---

## Candidate 1: Duplicated message-to-API conversion

- **location:** `crates/clarity-llm/src/lib.rs:412-441` (`convert_api_messages`)
- **issue:** `OpenAiCompatibleLlm` has its own `convert_api_messages`. `OllamaProvider` (`ollama.rs:143-164`) and `AnthropicLlm` (`lib.rs:964-971`) repeat nearly identical role/content/tool-call mapping. The local GGUF chat template (`local_gguf.rs:65-192`) also re-implements message role dispatch.
- **suggestion:** Extract a single, provider-agnostic `messages_to_api_format(messages, role_formatter)` helper in `clarity-llm`. Each provider then only supplies the small wire-format differences (tool call shape, system prompt handling).
- **risk:** Low. All mappings are deterministic; tests already cover each provider.
- **note:** Keeps provider-specific SSE/JSON parsing where it belongs, but removes ~80 lines of duplicated role mapping.

---

## Candidate 2: Provider-specific flags hard-coded in generic `OpenAiCompatibleLlm`

- **location:** `crates/clarity-llm/src/lib.rs:461-477` and `574-588`
- **issue:** `complete()` and `stream()` each contain `if self.base_url.contains("kimi.com") { ... } else if self.base_url.contains("deepseek.com") { ... }` to decide the `thinking` JSON field. This leaks provider knowledge into the generic OpenAI-compatible provider.
- **suggestion:** Move `thinking` configuration into a small `OpenAiOptions` struct (or extend `ProviderCapabilities`) so `KimiLlm` and `DeepSeekProvider` construct their inner `OpenAiCompatibleLlm` with the right flags.
- **risk:** Low. Behavior stays identical; construction moves to the wrappers.
- **note:** Also removes the duplicated `thinking_opt` block between `complete` and `stream`.

---

## Candidate 3: `LocalGgufProvider::generate` duplicated as `generate_with_state`

- **location:** `crates/clarity-llm/src/local_gguf.rs:408-578` and `718-880`
- **issue:** `generate()` and `generate_with_state()` are almost line-for-line identical. The only reason for the split is that `stream()` needs to move state into a spawned task.
- **suggestion:** Make `generate()` take the state fields by `Arc`/clone and call the same shared implementation. Remove the duplicated tensor/sampling/penalty logic.
- **risk:** Low-medium. The code is performance-sensitive and has tests; refactoring should preserve token-cache behavior.
- **note:** Mark any retained shortcut with `// ponytail:` if KV-cache threshold logic stays hard-coded at 80%.

---

## Candidate 4: Two `resolve_key_ref` implementations

- **location:** `crates/clarity-llm/src/model_registry.rs:527-562` and `crates/clarity-egui/src/provider.rs:256-288`
- **issue:** The `${env:VAR}` and `${file:path:field}` resolution logic is copy-pasted between the backend registry and the frontend provider registry.
- **suggestion:** Move the resolver to `clarity-contract` or `clarity-secrets` as a pure function (it has no async/state needs). Both call sites then share one implementation.
- **risk:** Low. Function is pure string/env/file lookup; easy to unit-test.
- **note:** Avoids drift when a third reference syntax (e.g. `${keyring:}`) is added.

---

## Candidate 5: `LlmFactory::auto()` duplicates `ModelRegistry::built_in_fallback()`

- **location:** `crates/clarity-llm/src/lib.rs:1177-1249` and `crates/clarity-llm/src/model_registry.rs:329-406`
- **issue:** `LlmFactory::auto()` re-implements the env-var scanning logic that `ModelRegistry::built_in_fallback()` already centralizes via `registry_table`. Both have the same provider priority and key checks.
- **suggestion:** Deprecate `LlmFactory::auto()` and make it call `ModelRegistry::load_async()` + `build_for_alias()` (via the `LlmProviderFactory` impl already on `ModelRegistry`).
- **risk:** Medium. `LlmFactory` is widely used in examples/headless/gateway; tests must cover the switch.
- **note:** This aligns with the module's own deprecation note: "Frozen for new providers — use `ModelRegistry::load()`".

---

## Candidate 6: Frontend `ProviderRegistry` duplicates canonical defaults

- **location:** `crates/clarity-egui/src/provider.rs:519-644` (built-in providers) and `crates/clarity-llm/src/registry_table.rs:47-123`
- **issue:** `clarity-egui` defines its own hard-coded list of built-in providers, base URLs, models, and OAuth client IDs, while `clarity-llm` already maintains `registry_table` as the canonical source.
- **suggestion:** Expose `registry_table` (or a serializable `ProviderConfig` view) from `clarity-llm` and have `ProviderRegistry::load_builtin()` derive from it. Custom user providers still load from `~/.config/clarity/providers/`.
- **risk:** Medium. Changes the shape of the egui `ProviderDefinition` conversion and may affect the Settings UI.
- **note:** Eliminates a second place where OAuth client IDs and base URLs can drift.

---

## Candidate 7: Hardcoded fallback model list duplicates registry defaults

- **location:** `crates/clarity-llm/src/model_listing.rs:147-217`
- **issue:** `get_available_models()` contains a large hard-coded fallback list of OpenAI/Anthropic/Kimi/DeepSeek/Ollama models. Much of this overlaps with `registry_table` defaults and the frontend `ProviderRegistry`.
- **suggestion:** Derive the fallback from the canonical registry defaults; only keep truly dynamic additions (e.g. scanned local GGUF names).
- **risk:** Low. This is UI/catalog data, not runtime behavior.
- **note:** Reduces the chance that a new default model is added in one place but missing from the settings dropdown.

---

## Candidate 8: `AdaptiveModelRouter::capable` is a stub

- **location:** `crates/clarity-core/src/adaptive/router.rs:460-465`
- **issue:** The capability filter always returns `true`, making the `requires_reasoning`, `requires_vision`, and `requires_tools` fields on `TaskDescriptor` dead weight. The routing docs claim capability filtering is part of the pipeline.
- **suggestion:** Either (a) wire `ProviderCapabilities` into `ProviderProfile` and implement real filtering, or (b) delete `capable()` and the capability flags from `TaskDescriptor` until they are needed, with a `// ponytail: capability routing deferred` comment.
- **risk:** Low for option (b); medium for option (a) because it changes routing decisions.
- **note:** Per Ponytail, don't carry dead code just because it looks like a "future feature".

---

## Summary table

| # | Location | Issue | Suggestion | Risk |
|---|---|---|---|---|
| 1 | `lib.rs:412`, `ollama.rs:143`, `local_gguf.rs:65` | Duplicated message-to-API conversion | Shared helper | Low |
| 2 | `lib.rs:461`, `lib.rs:574` | Provider flags in generic provider | Move to wrapper options | Low |
| 3 | `local_gguf.rs:408`, `local_gguf.rs:718` | `generate` / `generate_with_state` duplication | Shared state-aware implementation | Low-medium |
| 4 | `model_registry.rs:527`, `provider.rs:256` | Two `${env/file}` resolvers | One pure function | Low |
| 5 | `lib.rs:1177`, `model_registry.rs:329` | `LlmFactory::auto` duplicates fallback | Delegate to `ModelRegistry` | Medium |
| 6 | `provider.rs:519`, `registry_table.rs:47` | Frontend duplicates canonical defaults | Derive from `registry_table` | Medium |
| 7 | `model_listing.rs:147` | Hard-coded fallback model list | Derive from registry | Low |
| 8 | `router.rs:460` | `capable()` stub | Implement or delete | Low-medium |

---

*Generated from a read-only codebase survey on 2026-06-26.*
