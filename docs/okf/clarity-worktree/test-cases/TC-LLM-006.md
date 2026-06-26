---
type: test-case
id: TC-LLM-006
title: RouterLlmProvider routes by hint when no explicit alias is given
description: Verify that router:cheap, router:coding, and other hints select an appropriate provider from the registry.
component: clarity-llm
priority: medium
status: planned
tags: [test, clarity-llm, routing, runtime-router]
related_concepts: [runtime-router, model-registry, pricing]
timestamp: 2026-06-26T12:00:00Z
---

# TC-LLM-006: RouterLlmProvider routes by hint when no explicit alias is given

## Background

`RouterLlmProvider` resolves `router:<hint>` aliases at request time using the
`ModelRegistry`. Hints like `cheap`, `coding`, `vision`, `tools`, `fast`, and
explicit alias names allow callers to express intent without hard-coding a
model.

## Preconditions

- A `models.toml` registry is loaded with multiple aliases tagged by price,
  capability, or purpose.

## Test Data

```toml
[[models]]
alias = "cheap-local"
provider = "local"
model_id = "qwen2-1.5b"
tags = ["cheap"]

[[models]]
alias = "coding-deepseek"
provider = "deepseek"
model_id = "deepseek-coder"
tags = ["coding"]
```

## Steps

1. Load `ModelRegistry`.
2. Construct `RouterLlmProvider`.
3. Call `complete` with messages whose model hint is `router:cheap`.
4. Call `complete` with messages whose model hint is `router:coding`.

## Expected Results

- `router:cheap` resolves to a provider matching the `cheap` tag.
- `router:coding` resolves to a provider matching the `coding` tag.
- Unknown hints return an error rather than silently picking a default.

## Actual Results

- Not yet implemented as a focused end-to-end test.

## Notes

- The scoring logic uses `pricing`, `tags`, and `fallback_aliases`; tests
  should cover both tag hits and fallback behavior.
