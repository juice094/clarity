---
id: clarity-llm
name: clarity-llm
type: llm
layer: infrastructure
depends_on:
- clarity-contract
- clarity-mcp
- clarity-memory
- clarity-secrets
consumed_by:
- clarity-core
- clarity-mobile-core
- clarity-anthropic-proxy
title: clarity-llm
description: LLM provider abstraction + Candle GGUF local inference.
tags:
- clarity
- infrastructure
- llm
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-llm

LLM provider abstraction + Candle GGUF local inference.

## Responsibilities

- Provider registry
- `ReliableProvider` retry/failover
- `runtime_router` alias routing
- Candle GGUF local inference
- OAuth device flow auth

## Notes

Features: `local-llm`, `local-llm-cuda`.
