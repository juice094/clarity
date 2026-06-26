---
type: test-case
id: TC-LLM-005
title: Local GGUF model discovery respects CLARITY_LOCAL_MODEL_PATH
description: Verify that scan_local_models finds .gguf files in the configured path and home models directory.
component: clarity-llm
priority: medium
status: implemented
tags: [test, clarity-llm, local-inference, model-discovery]
related_concepts: [local-model-discovery, candle-gguf-local-inference]
timestamp: 2026-06-26T12:00:00Z
---

# TC-LLM-005: Local GGUF model discovery respects CLARITY_LOCAL_MODEL_PATH

## Background

`scan_local_models()` is used by settings UIs and `LocalGgufProvider` to
locate locally available GGUF models without requiring a `models.toml` entry.

## Preconditions

- A temporary directory contains at least one `.gguf` file.
- `CLARITY_LOCAL_MODEL_PATH` can be set to that directory.

## Test Data

- File: `tempdir/qwen2-7b-q4_0.gguf`
- Env: `CLARITY_LOCAL_MODEL_PATH=tempdir`

## Steps

1. Create temporary directory with a `.gguf` file.
2. Set `CLARITY_LOCAL_MODEL_PATH`.
3. Call `scan_local_models()`.
4. Clear the env var.

## Expected Results

- Returns a non-empty vector.
- Each entry is `(absolute_path, file_name)`.
- The discovered model name is `qwen2-7b-q4_0.gguf`.

## Actual Results

- Covered by model-listing tests in `crates/clarity-llm/src/model_listing.rs`.

## Notes

- Ensure tests do not leak env vars to other tests.
