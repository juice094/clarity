---
id: clarity-anthropic-proxy
name: clarity-anthropic-proxy
type: anthropic-proxy
layer: utility
depends_on: ["clarity-contract", "clarity-core", "clarity-llm"]
consumed_by: [""]
---

# clarity-anthropic-proxy

Anthropic Messages API → DeepSeek proxy utility.

## Responsibilities

- Translate Anthropic requests to DeepSeek
- Tool/schema conversion
- Streaming response adaptation

## Notes

Utility binary (`cc-proxy`).
