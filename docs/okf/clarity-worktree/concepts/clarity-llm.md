---
id: clarity-llm
name: clarity-llm
type: llm
layer: infrastructure
depends_on:
- clarity-contract
- clarity-secrets
consumed_by:
- clarity-core
- clarity-mobile-core
- clarity-anthropic-proxy
title: clarity-llm
description: LLM provider abstraction + built-in HTTP providers + Candle GGUF local inference.
tags:
- clarity
- infrastructure
- llm
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-llm

LLM provider abstraction + built-in HTTP providers + Candle GGUF local inference.

## Responsibilities

- `LlmProvider` trait implementation for DeepSeek, Kimi, OpenAI, Anthropic, Ollama, LlamaServer and local GGUF
- HTTP provider implementations under `providers/` (OpenAI-compatible, Anthropic, Kimi, OAuth/Kimi Code)
- Request-body size guards and OpenAI chat-completion types in `request.rs`
- Provider registry (`ModelRegistry`) and remote model catalog (`catalog/`)
- `ReliableProvider` retry/failover
- `runtime_router` alias routing
- Candle GGUF local inference
- OAuth device flow auth

## Notes

Features: `local-llm`, `local-llm-cuda`.
