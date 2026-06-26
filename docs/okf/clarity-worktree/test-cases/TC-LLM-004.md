---
type: test-case
id: TC-LLM-004
title: ReliableProvider retries primary and falls back on failure
description: Verify that ReliableProvider retries the primary provider and then walks the fallback chain.
component: clarity-contract
priority: high
status: planned
tags: [test, clarity-contract, routing, reliable-provider]
related_concepts: [reliable-provider, llm-provider-trait]
timestamp: 2026-06-26T12:00:00Z
---

# TC-LLM-004: ReliableProvider retries primary and falls back on failure

## Background

`ReliableProvider` wraps a chain of `LlmProvider`s. It retries the primary
provider (with exponential backoff, rate-limit honoring, and context-window
truncation) and falls through the chain on persistent failure.

## Preconditions

- Two or more mock providers are available.
- The first mock provider is configured to fail permanently.
- The second mock provider returns a valid response.

## Test Data

- Primary provider: always returns `AgentError::Llm("down")`.
- Fallback provider: returns `LlmResponse` with content `"fallback-ok"`.
- Retry config: default (max 3 retries, 10 s cap).

## Steps

1. Wrap `[primary, fallback]` in `ReliableProvider::new(...)`.
2. Call `complete(&messages, &tools).await`.
3. Observe that the primary is retried, then the fallback is invoked.

## Expected Results

- Final response content equals `"fallback-ok"`.
- Primary provider is called more than once (retries).
- Fallback provider is called exactly once.

## Actual Results

- Not yet implemented as a focused unit test.

## Notes

- Existing tests in `crates/clarity-contract/src/reliable_provider.rs` cover
  retry internals; an end-to-end chain test is missing.
