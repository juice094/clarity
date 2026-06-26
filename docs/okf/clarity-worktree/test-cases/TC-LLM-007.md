---
type: test-case
id: TC-LLM-007
title: model_listing fallback derives from canonical registry defaults
description: Verify that get_available_models populates its fallback catalog from registry_table rather than a duplicated hard-coded list.
component: clarity-llm
priority: medium
status: implemented
tags: [test, clarity-llm, ui, model-listing, registry-table]
related_concepts: [model-listing, registry-table]
timestamp: 2026-06-26T12:00:00Z
---

# TC-LLM-007: model_listing fallback derives from canonical registry defaults

## Background

Settings UIs need a fallback list of providers and models when no
`models.toml` is configured. This fallback should come from the same canonical
source as runtime provider construction to avoid drift.

## Preconditions

- No `models.toml` is loaded, or the registry does not override the families
  under test.

## Test Data

- Expected families: `openai`, `anthropic`, `kimi`, `deepseek`, `ollama`,
  `local`.

## Steps

1. Call `get_available_models()`.
2. Inspect the returned vector.

## Expected Results

- `openai` appears with at least `"gpt-4o"`.
- `local` appears with either scanned `.gguf` names or the placeholder.
- `moonshot` appears at most once (alias families should not duplicate).

## Actual Results

- Covered by `model_listing::tests` in `crates/clarity-llm/src/model_listing.rs`.

## Notes

- This test documents the single-source-of-truth invariant introduced by the
  Ponytail refactor.
