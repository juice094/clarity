---
title: Provider / Compute Source in Clarity
category: notes
date: 2026-06-26
---

# Where Compute Comes From in Clarity

This report explains how Clarity sources, configures, routes, and fails over LLM inference. All file paths are relative to the project root `C:/Users/22414/dev/clarity`.

---

## 1. The `LlmProvider` trait

The single abstraction used by `clarity-core`, frontends, and tools lives in the contract crate so that consumers do not depend on `clarity-llm`.

- **File:** `crates/clarity-contract/src/llm.rs`
- **Lines:** 79-129
- **Core methods:**
  - `async fn complete(&self, messages: &[Message], tools: &Value) -> Result<LlmResponse, AgentError>` — one-shot chat completion.
  - `fn stream(&self, messages: &[Message], tools: &Value) -> Result<mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError>` — streaming response.
  - `fn set_prompt_cache_key(&self, key: &str)` — enables provider-side prompt caching.
  - `fn clear_cache(&self)` — default no-op; overridden by stateful/local providers.
  - `fn capabilities(&self) -> ProviderCapabilities` — reports native tool calling, vision, prompt caching, pricing.
- **Related types in the same file:**
  - `LlmResponse` (line 65), `StreamDelta` (re-exported from `lib.rs`), `ProviderCapabilities` (line 37), `Pricing` (line 14), `LlmProviderFactory` (line 136).
- **Re-exports:**
  - `crates/clarity-llm/src/api.rs` re-exports everything from `clarity_contract::llm` for backward compatibility.
  - `crates/clarity-core/src/agent/mod.rs` line 78 re-exports `LlmProvider` etc. for core consumers.

---

## 2. Built-in providers

| Provider | File | Protocol / endpoint | Local or remote |
|---|---|---|---|
| `OpenAiCompatibleLlm` | `crates/clarity-llm/src/lib.rs:364` | OpenAI `/v1/chat/completions` (also covers Kimi, DeepSeek, custom endpoints) | Remote |
| `KimiLlm` | `crates/clarity-llm/src/lib.rs:690` | Wraps `OpenAiCompatibleLlm` with Moonshot defaults | Remote |
| `OAuthLlm` / `KimiCodeLlm` | `crates/clarity-llm/src/lib.rs:756` | OpenAI-compatible Kimi Code API with OAuth token refresh | Remote |
| `AnthropicLlm` | `crates/clarity-llm/src/lib.rs:872` | Anthropic Messages API `/v1/messages` | Remote |
| `DeepSeekProvider` | `crates/clarity-llm/src/deepseek.rs:23` | Wraps `OpenAiCompatibleLlm` with DeepSeek defaults | Remote |
| `DeepSeekDeviceProvider` | `crates/clarity-llm/src/deepseek_device.rs` | Native DeepSeek Android app device-login API | Remote |
| `OllamaProvider` | `crates/clarity-llm/src/ollama.rs:31` | Ollama native `/api/chat` HTTP endpoint | Local (external process) |
| `LlamaServerProvider` | `crates/clarity-llm/src/llama_server.rs:45` | llama.cpp server OpenAI-compatible HTTP endpoint | Local (external process) |
| `LocalGgufProvider` | `crates/clarity-llm/src/local_gguf.rs:361` | Native Rust Candle GGUF inference | Local (in-process) |
| `KalosmProvider` | `crates/clarity-llm/src/kalosm.rs:56` | Deprecated stub; always returns an error | — |
| `RouterLlmProvider` | `crates/clarity-llm/src/runtime_router.rs:80` | Routes by hint (`router:cheap`, `router:coding`, etc.) | Varies |

Key observations:
- Most providers are thin wrappers around `OpenAiCompatibleLlm` or bespoke HTTP/SSE parsers.
- `AnthropicLlm` still uses prompt-guided tool calling via `tool_payload::adapt_prompt_guided` because its `capabilities()` reports `native_tool_calling: false` (`lib.rs:1152`).
- `DeepSeekDeviceProvider` uses a non-standard API and is tagged `chat-only` in the registry.

---

## 3. Local inference

### Candle GGUF
- **Implementation:** `crates/clarity-llm/src/local_gguf.rs`
- **Entry type:** `LocalGgufProvider` at line 361; `LocalGgufConfig` at line 202.
- **Supported architectures:** Qwen2, Qwen2.5, DeepSeek-R1-Distill-Qwen variants (doc comment line 6; `ChatTemplate::detect` line 48).
- **Feature gating:**
  - `local-llm` is **default** in `crates/clarity-llm/Cargo.toml` line 51.
  - `local-llm-cuda` enables `candle-core/cuda` (`Cargo.toml` line 53).
- **Device selection:** `pick_device()` at `local_gguf.rs:883` prefers CUDA when the `local-llm-cuda` feature is enabled and `cuda_is_available()` returns true; otherwise CPU.
- **Model discovery:** `resolve_local_model_path()` at `crates/clarity-llm/src/lib.rs:86` checks:
  1. `CLARITY_LOCAL_MODEL_PATH` env var.
  2. First `.gguf` file in `~/models/`.
- **Tokenizer loading:** `load_tokenizer()` at `local_gguf.rs:925` tries `tokenizer_path`, then HuggingFace `tokenizer_repo`, then a `tokenizer.json` sibling.
- **KV-cache reuse:** the provider caches prompt tokens and reuses the longest common prefix when it exceeds 80% of the new prompt (`local_gguf.rs:430-443`).
- **Tool calling:** implemented by injecting a JSON schema into the system prompt and parsing `{"tool_calls": [...]}` from generated text (`local_gguf.rs:581-617`).

### Ollama / llama.cpp server
- These are out-of-process local options; Clarity speaks HTTP to them.
- Default URLs: `http://localhost:11434` (Ollama), `http://localhost:8080` (llama-server).

---

## 4. Configuration

### `models.toml` (registry-driven)
- **Search paths** (`crates/clarity-llm/src/model_registry.rs:299`):
  1. `CLARITY_MODELS_CONFIG`
  2. `./.clarity/models.toml`
  3. `~/.config/clarity/models.toml`
- **Schema:**
  - `[providers.<name>]` defines `protocol`, `base_url`, `api_key_env`, `auth_type`, `pricing`, `tags`.
  - `[[models]]` defines `alias`, `provider`, `model_id`, per-alias overrides (`api_key`, `base_url`, `pricing`, `tags`, `fallback_aliases`).
- **Loading:** `ModelRegistry::load()` (line 244) and `ModelRegistry::load_async()` (line 260).
- **Built-in env fallback:** `ModelRegistry::built_in_fallback()` (line 329) mirrors the old `LlmFactory::auto()` behavior and reads canonical defaults from `registry_table`.
- **Per-alias overrides:** `ModelEntry::merge_into()` at line 200 merges alias settings into provider settings.

### Frontend provider registry (`clarity-egui`)
- **File:** `crates/clarity-egui/src/provider.rs`
- `ProviderRegistry::load()` (line 530) loads built-in definitions plus custom TOML files from `~/.config/clarity/providers/*.toml`.
- `ProviderDefinition` (line 136) supports `api_key_ref`, `auth_type`, `tags`, `extra`, encrypted `password_enc`.
- `ProviderDefinition::resolve_api_key()` (line 231) supports `${env:VAR}` and `${file:path:field}`.

### Runtime value type
- `RuntimeProviderConfig` in `crates/clarity-llm/src/runtime.rs:37` is the value passed from frontend settings to `build_provider()` (line 61).
- `build_provider()` dispatches on `api_format`: `openai_chat`, `anthropic_messages`, `ollama`, `llama_server`.

### Settings UI model
- `SettingsViewModel` / `SettingsSnapshot` in `crates/clarity-core/src/view_models/settings.rs` hold provider, model, API key, local model path, approval mode.

### Environment variables
- `OPENAI_API_KEY`, `DEEPSEEK_API_KEY`, `KIMI_API_KEY`, `KIMI_CODE_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `OLLAMA_HOST`, `CLARITY_LOCAL_MODEL_PATH`, `CLARITY_LOCAL_TOKENIZER_REPO`, `CLARITY_MODELS_CONFIG`, `CLARITY_SECRETS_KEY`.

---

## 5. Routing / failover

### `ReliableProvider` (contract layer)
- **File:** `crates/clarity-contract/src/reliable_provider.rs`
- **Lines:** 195-360
- Wraps a chain of `Arc<dyn LlmProvider>` and provides:
  - Exponential-backoff retries (max 3 by default, capped at 10 s).
  - Rate-limit honoring via `Retry-After` heuristic (lines 87-105).
  - Context-window truncation and one retry (lines 117-130, 285-308).
  - Empty-completion re-roll (lines 107-115, 251-266).
  - Fallback chain when the primary provider fails.
- Integrated into `Agent` construction in `crates/clarity-core/src/agent/construct.rs:138` (`with_fallback_llms` wraps the primary LLM automatically).

### `AdaptiveModelRouter` (core layer)
- **File:** `crates/clarity-core/src/adaptive/router.rs`
- **Lines:** 362-466
- Maintains `ProviderProfile`s with EWMA latency, sliding-window error rate, quality score, and cost.
- Routes a `TaskDescriptor` (task type, token estimate, required capabilities, budget) to the highest-scoring provider.
- Weights are task-type dependent: `Coding` optimizes for latency; `Plan` and `Explore` weight quality/reasoning more; `Background` favors cost (`crates/clarity-core/src/adaptive/router.rs:478-484`).
- The `capable()` filter is currently a stub returning `true` (line 460-465); capability-aware filtering is not yet implemented.

---

## 6. Security and key management

### API key resolution hierarchy
- **File:** `crates/clarity-llm/src/model_registry.rs:564-620`
- Priority (highest first):
  1. Runtime override key.
  2. Alias-level literal or encrypted key (`enc2:`).
  3. Alias-level environment variable name.
  4. Provider-level environment variable name.
  5. Plain string fallback.

### Key reference syntax
- `${env:VAR}` — read `VAR` from environment (`model_registry.rs:556`).
- `${file:path:field}` — read JSON field from file (`model_registry.rs:534-553`).
- `enc2:<ciphertext>` — ChaCha20-Poly1305 encrypted secret, decrypted by `clarity-secrets` (`crates/clarity-secrets/src/lib.rs:31`).

### Frontend security
- `clarity-egui` keeps API keys in `SettingsSnapshot` and writes encrypted passwords via `ProviderDefinition::set_password` (`crates/clarity-egui/src/provider.rs:317`).
- OAuth device flow is supported for Kimi Code (`crates/clarity-llm/src/lib.rs:756`).

---

## 7. Summary

Clarity 的算力来源是一个分层混合架构：

- **本地优先**：默认启用 Candle GGUF，支持 Qwen/DeepSeek-R1-Distill，可选 CUDA。
- **外部本地进程**：Ollama、llama.cpp server 通过 HTTP 接入。
- **云端 API**：OpenAI-compatible（覆盖 OpenAI/Kimi/DeepSeek）、Anthropic、Ollama remote、llama-server remote。
- **路由与韧性**：`ReliableProvider` 负责重试和 failover；`AdaptiveModelRouter` 基于历史遥测动态选择；`RouterLlmProvider` 支持 `router:<hint>` 别名。
- **配置与安全**：`models.toml` + 环境变量 + `enc2:` 加密 + OAuth，构成完整的算力接入治理。

> 关键入口：任何新增算力源，只需要实现 `clarity-contract::llm::LlmProvider` 并在 `ModelRegistry` / `registry_table` 中注册即可。
