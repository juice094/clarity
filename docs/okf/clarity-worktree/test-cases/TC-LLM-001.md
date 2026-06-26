---
type: test-case
id: TC-LLM-001
title: OpenAI-compatible provider completes a chat request
description: Verify that OpenAiCompatibleLlm can call /v1/chat/completions and return a parsed LlmResponse.
component: clarity-llm
priority: high
status: implemented
tags: [test, clarity-llm, provider, openai]
related_concepts: [openai-compatible-provider, llm-provider-trait]
timestamp: 2026-06-26T12:00:00Z
---

# TC-LLM-001: OpenAI-compatible provider completes a chat request

## Background

`OpenAiCompatibleLlm` is the generic HTTP provider backing OpenAI, Kimi,
DeepSeek, and any `/v1/chat/completions` endpoint. This test ensures the
provider correctly serializes messages, parses the response, and returns an
`LlmResponse`.

## Preconditions

- A mock HTTP server exposing `/v1/chat/completions` is running, or tests use
  a request-recording stub.
- The provider is constructed with a valid `base_url`, `api_key`, and
  `model_id`.

## Test Data

- Messages: one user message `"Hello"`.
- Tools: empty JSON array.
- Mock response body:
  ```json
  {
    "choices": [{
      "message": { "role": "assistant", "content": "Hi there" }
    }]
  }
  ```

## Steps

1. Construct `OpenAiCompatibleLlm` with the mock endpoint.
2. Call `complete(&messages, &tools).await`.
3. Inspect the returned `LlmResponse`.

## Expected Results

- The response content equals `"Hi there"`.
- No tool calls are present.
- No error is returned.

## Actual Results

- Covered by existing provider unit tests in `crates/clarity-llm/src/lib.rs`.

## Notes

- Streaming variant (`stream`) should be tested separately (TC-LLM-009).
