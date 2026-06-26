---
type: test-case
id: TC-LLM-003
title: resolve_key_ref handles env, file, and literal references
description: Verify that clarity-contract::resolve_key_ref correctly resolves ${env:VAR}, ${file:path:field}, plain literals, and env fallbacks.
component: clarity-contract
priority: high
status: implemented
tags: [test, clarity-contract, security, api-key]
related_concepts: [api-key-ref-resolution, enc2-secret-store]
timestamp: 2026-06-26T12:00:00Z
---

# TC-LLM-003: resolve_key_ref handles env, file, and literal references

## Background

API keys can be stored as environment variables, JSON file fields, or literal
strings. `resolve_key_ref` is the single shared resolver used by both the
backend registry and the frontend provider registry.

## Preconditions

- Test environment variables can be set/cleared safely.
- A temporary JSON file can be created.

## Test Data

- `${env:OPENAI_API_KEY}` where `OPENAI_API_KEY=sk-openai`.
- `${file:/tmp/keys.json:api_key}` where `keys.json` contains
  `{"api_key":"sk-file"}`.
- Plain literal `"sk-literal"`.
- Plain string matching an env var name `"RESOLVE_TEST_KEY"` where the env var
  is set.

## Steps

1. Set test env vars.
2. Create temporary JSON file.
3. Call `resolve_key_ref` with each reference form.
4. Compare returned values.

## Expected Results

| Input | Output |
|-------|--------|
| `${env:OPENAI_API_KEY}` | `Some("sk-openai")` |
| `${file:/tmp/keys.json:api_key}` | `Some("sk-file")` |
| `"sk-literal"` | `Some("sk-literal")` |
| `"RESOLVE_TEST_KEY"` | env value if set, else literal |
| `""` | `None` |

## Actual Results

- Implemented in `crates/clarity-contract/src/key_ref.rs` with 7 unit tests.

## Notes

- On Windows, absolute paths like `C:\keys.json` must be parsed correctly;
  `rsplit_once(':')` is used to separate path from JSON field.
