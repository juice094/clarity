---
id: clarity-anthropic-proxy
name: clarity-anthropic-proxy
type: anthropic-proxy
layer: utility
depends_on: ["clarity-contract", "clarity-llm"]
consumed_by: [""]
---

# clarity-anthropic-proxy

Anthropic Messages API → DeepSeek proxy utility.

## Responsibilities

- Expose Anthropic Messages API-compatible HTTP endpoint (`cc-proxy`).
- Load DeepSeek device credentials and instantiate the DeepSeek device provider.
- Delegate Anthropic request/response conversion to `clarity_llm::anthropic::AnthropicAdapter`.

## Notes

Utility binary (`cc-proxy`).
