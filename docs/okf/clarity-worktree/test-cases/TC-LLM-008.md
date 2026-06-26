---
type: test-case
id: TC-LLM-008
title: Anthropic provider uses prompt-guided tool calling
description: Verify that AnthropicLlm injects tool schemas into the system prompt and parses tool_calls from generated XML/text.
component: clarity-llm
priority: medium
status: planned
tags: [test, clarity-llm, provider, anthropic, tool-calling]
related_concepts: [anthropic-llm, provider-capabilities, tool-payload]
timestamp: 2026-06-26T12:00:00Z
---

# TC-LLM-008: Anthropic provider uses prompt-guided tool calling

## Background

`AnthropicLlm` does not use native Anthropic tool calling. Instead, it uses
`tool_payload::adapt_prompt_guided` to inject a JSON schema into the system
prompt and parses `{"tool_calls": [...]}` from the generated text.

## Preconditions

- A mock Anthropic Messages API endpoint is available.
- A tool schema is provided.

## Test Data

- Tool: `file_read` with parameter `path`.
- Mock response text containing:
  ```json
  {"tool_calls": [{"name": "file_read", "arguments": {"path": "/tmp/x"}}]}
  ```

## Steps

1. Construct `AnthropicLlm` with the mock endpoint.
2. Call `complete` with messages and the tool schema.
3. Inspect `LlmResponse.tool_calls`.

## Expected Results

- `tool_calls` contains one `file_read` call.
- The `path` argument is `"/tmp/x"`.

## Actual Results

- Not yet implemented as a focused unit test.

## Notes

- This is a provider-specific parser test; it should be isolated from real
  network calls.
