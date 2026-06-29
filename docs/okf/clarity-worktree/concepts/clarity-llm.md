---
id: clarity-llm
name: clarity-llm
type: llm
layer: infrastructure
depends_on: ["clarity-contract", "clarity-mcp", "clarity-memory", "clarity-secrets"]
consumed_by: ["clarity-core", "clarity-mobile-core", "clarity-anthropic-proxy"]
---

# clarity-llm

LLM provider abstraction + Candle GGUF local inference.

## Responsibilities

- Provider registry
- `ReliableProvider` retry/failover
- `runtime_router` alias routing
- Candle GGUF local inference
- OAuth device flow auth
- `AnthropicAdapter`: expose any `LlmProvider` behind an Anthropic Messages API facade

## Notes

Features: `local-llm`, `local-llm-cuda`.
