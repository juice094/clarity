---
type: test-case
id: TC-LLM-002
title: Model registry loads models.toml and resolves an alias
description: Verify that ModelRegistry::load reads the TOML config and build_for_alias constructs the right provider.
component: clarity-llm
priority: high
status: implemented
tags: [test, clarity-llm, config, model-registry]
related_concepts: [model-registry, build-provider-from-registry]
timestamp: 2026-06-26T12:00:00Z
---

# TC-LLM-002: Model registry loads models.toml and resolves an alias

## Background

`ModelRegistry` is the TOML-driven source of truth that maps user-facing
aliases to concrete provider + model configurations. This test validates the
load and resolution pipeline.

## Preconditions

- A `models.toml` file exists at one of the supported search paths
  (`CLARITY_MODELS_CONFIG`, `./.clarity/models.toml`, or
  `~/.config/clarity/models.toml`).
- The TOML contains at least one `[[models]]` entry with alias `"default"`.

## Test Data

```toml
[providers.openai]
protocol = "openai_chat"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"

[[models]]
alias = "default"
provider = "openai"
model_id = "gpt-4o"
```

## Steps

1. Call `ModelRegistry::load_async().await`.
2. Call `registry.build_for_alias("default").await`.
3. Verify the returned provider is an `OpenAiCompatibleLlm` configured for
   `gpt-4o`.

## Expected Results

- `load_async` succeeds.
- `build_for_alias("default")` returns an `Arc<dyn LlmProvider>`.
- The underlying provider targets `gpt-4o`.

## Actual Results

- Covered by `model_registry::tests` in `crates/clarity-llm/src/model_registry.rs`.

## Notes

- Per-alias overrides (`api_key`, `base_url`) should be honored; add a
  separate case if not already covered.
