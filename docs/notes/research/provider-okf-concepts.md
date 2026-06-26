---
title: OKF Concepts Extracted from Provider / Compute Layer
category: notes
date: 2026-06-26
---

# OKF Concepts for Provider / Compute Layer

These are candidate Open Knowledge Format (OKF) concepts extracted from `crates/clarity-llm/`, `crates/clarity-core/src/adaptive/`, `crates/clarity-secrets/`, and related frontend code. Existing OKF crate concepts are noted where applicable.

---

## 1. `llm-provider-trait`

- **name:** `llm-provider-trait`
- **definition:** The contract-layer trait that abstracts one-shot completion, streaming, prompt-cache key management, and capability reporting for every LLM backend in Clarity.
- **category:** provider
- **anchors:**
  - `crates/clarity-contract/src/llm.rs` (lines 79-129)
  - `crates/clarity-llm/src/api.rs` (re-export, lines 13-15)
  - `crates/clarity-core/src/agent/mod.rs` (re-export, line 78)
- **related_concepts:** `provider-capabilities`, `reliable-provider`, `llm-response`, `llm-provider-factory`
- **okf_exists:** No standalone OKF concept; the crate-level `clarity-llm` concept exists at `docs/okf/clarity-worktree/concepts/clarity-llm.md`.

---

## 2. `provider-capabilities`

- **name:** `provider-capabilities`
- **definition:** A value type describing what a provider can do (native tool calling, prompt-guided tool calling, vision, prompt caching) and optional pricing.
- **category:** provider
- **anchors:**
  - `crates/clarity-contract/src/llm.rs` (lines 37-60)
  - `crates/clarity-llm/src/lib.rs` (hard-coded flags in `OpenAiCompatibleLlm::capabilities`, line 676; `AnthropicLlm::capabilities`, line 1150; `LocalGgufProvider::capabilities`, line 701)
- **related_concepts:** `llm-provider-trait`, `pricing`, `adaptive-model-router`
- **okf_exists:** No standalone OKF concept.

---

## 3. `openai-compatible-provider`

- **name:** `openai-compatible-provider`
- **definition:** The generic HTTP provider that drives OpenAI, Kimi, DeepSeek, and any other `/v1/chat/completions` endpoint, plus the thin wrappers that inject provider-specific defaults.
- **category:** provider
- **anchors:**
  - `crates/clarity-llm/src/lib.rs` (`OpenAiCompatibleLlm`, line 364)
  - `crates/clarity-llm/src/deepseek.rs` (`DeepSeekProvider`, line 23)
  - `crates/clarity-llm/src/lib.rs` (`KimiLlm`, line 690)
  - `crates/clarity-llm/src/lib.rs` (`OAuthLlm`, line 756)
- **related_concepts:** `llm-provider-trait`, `runtime-provider-config`, `sse-parser`
- **okf_exists:** No standalone OKF concept.

---

## 4. `candle-gguf-local-inference`

- **name:** `candle-gguf-local-inference`
- **definition:** Native in-process GGUF inference using Candle, supporting Qwen2 / Qwen2.5 / DeepSeek-R1-Distill architectures with optional CUDA acceleration.
- **category:** local-inference
- **anchors:**
  - `crates/clarity-llm/src/local_gguf.rs` (entire file; `LocalGgufProvider` at line 361)
  - `crates/clarity-llm/Cargo.toml` (features `local-llm` / `local-llm-cuda`, lines 50-53)
  - `crates/clarity-llm/src/lib.rs` (`resolve_local_model_path`, line 86)
- **related_concepts:** `local-model-discovery`, `chat-template`, `tokenizer-loading`
- **okf_exists:** No standalone OKF concept; mentioned in the crate-level `clarity-llm` concept.

---

## 5. `local-model-discovery`

- **name:** `local-model-discovery`
- **definition:** The rule that resolves a local GGUF model path from `CLARITY_LOCAL_MODEL_PATH`, `~/models/`, or the frontend settings panel.
- **category:** local-inference
- **anchors:**
  - `crates/clarity-llm/src/lib.rs` (`resolve_local_model_path`, lines 86-129)
  - `crates/clarity-llm/src/model_listing.rs` (`scan_local_models`, lines 18-79)
  - `crates/clarity-egui/src/llm_loader.rs` (`try_load_local`, lines 78-131)
- **related_concepts:** `candle-gguf-local-inference`, `ollama-provider`, `llama-server-provider`
- **okf_exists:** No standalone OKF concept.

---

## 6. `model-registry`

- **name:** `model-registry`
- **definition:** The TOML-driven registry (`models.toml`) that maps user-facing aliases to concrete provider + model_id configurations, with per-alias overrides and encrypted keys.
- **category:** config
- **anchors:**
  - `crates/clarity-llm/src/model_registry.rs` (`ModelRegistry`, line 236; `ModelEntry`, line 154; `ProviderConfig`, line 119)
  - `docs/development/provider-config.md` (`models.toml` schema)
- **related_concepts:** `runtime-router`, `build-provider-from-registry`, `api-key-ref-resolution`
- **okf_exists:** No standalone OKF concept.

---

## 7. `build-provider-from-registry`

- **name:** `build-provider-from-registry`
- **definition:** The async factory that turns a `ProviderConfig` + `ModelEntry` into a concrete `LlmProvider`, applying alias overrides, decrypting `enc2:` keys, and selecting the correct protocol adapter.
- **category:** provider
- **anchors:**
  - `crates/clarity-llm/src/model_registry.rs` (`build_provider_from_registry_entry`, line 766; `build_provider_from_registry_with_key`, line 605)
- **related_concepts:** `model-registry`, `enc2-secret-store`, `api-key-ref-resolution`, `oauth-token-manager`
- **okf_exists:** No standalone OKF concept.

---

## 8. `reliable-provider`

- **name:** `reliable-provider`
- **definition:** A decorator that chains one or more providers, retrying with exponential backoff, honoring rate limits, truncating context windows, re-rolling empty completions, and failing over through the chain.
- **category:** routing
- **anchors:**
  - `crates/clarity-contract/src/reliable_provider.rs` (lines 195-360)
  - `crates/clarity-core/src/agent/construct.rs` (`with_fallback_llms`, line 138)
- **related_concepts:** `llm-provider-trait`, `adaptive-model-router`, `runtime-router`
- **okf_exists:** No standalone OKF concept; mentioned in the crate-level `clarity-llm` concept.

---

## 9. `runtime-router`

- **name:** `runtime-router`
- **definition:** A provider that resolves `router:<hint>` aliases at request time (cheap / coding / vision / tools / fast / explicit alias) using pricing and tag scoring.
- **category:** routing
- **anchors:**
  - `crates/clarity-llm/src/runtime_router.rs` (`RouterLlmProvider`, line 80; `RouterHint`, line 28)
  - `crates/clarity-llm/src/model_registry.rs` (`fallback_aliases`, line 189)
- **related_concepts:** `model-registry`, `reliable-provider`, `provider-capabilities`, `pricing`
- **okf_exists:** No standalone OKF concept; mentioned in the crate-level `clarity-llm` concept.

---

## 10. `adaptive-model-router`

- **name:** `adaptive-model-router`
- **definition:** The core-layer router that selects providers using historical telemetry (EWMA latency, error rate, quality, cost) and task-type-specific weights.
- **category:** routing
- **anchors:**
  - `crates/clarity-core/src/adaptive/router.rs` (`AdaptiveModelRouter`, line 362; `ProviderProfile`, line 160; `TaskDescriptor`, line 68)
  - `crates/clarity-core/src/adaptive/mod.rs` (module doc, lines 1-45)
- **related_concepts:** `reliable-provider`, `provider-capabilities`, `telemetry`, `task-descriptor`
- **okf_exists:** No standalone OKF concept.

---

## 11. `enc2-secret-store`

- **name:** `enc2-secret-store`
- **definition:** ChaCha20-Poly1305 encrypted secret storage using the `enc2:` ciphertext prefix, used for per-alias API keys and DeepSeek device passwords.
- **category:** security
- **anchors:**
  - `crates/clarity-secrets/src/lib.rs` (`SecretStore`, line 54; `CIPHER_PREFIX`, line 32)
  - `crates/clarity-llm/src/model_registry.rs` (`resolve_api_key`, line 582)
  - `crates/clarity-egui/src/provider.rs` (`set_password`, line 317)
- **related_concepts:** `api-key-ref-resolution`, `build-provider-from-registry`, `oauth-token-manager`
- **okf_exists:** Yes — crate-level `clarity-secrets` concept at `docs/okf/clarity-worktree/concepts/clarity-secrets.md`.

---

## 12. `api-key-ref-resolution`

- **name:** `api-key-ref-resolution`
- **definition:** The syntax layer that turns key references (`${env:VAR}`, `${file:path:field}`, literal strings, and `enc2:` blobs) into resolved API keys at provider construction time.
- **category:** security
- **anchors:**
  - `crates/clarity-llm/src/model_registry.rs` (`r
