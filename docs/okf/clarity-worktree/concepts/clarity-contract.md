---
id: clarity-contract
name: clarity-contract
type: contract
layer: contract
depends_on: [""]
consumed_by: ["clarity-wire", "clarity-memory", "clarity-mcp", "clarity-llm", "clarity-tools", "clarity-channels", "clarity-secrets", "clarity-openclaw", "clarity-rollout", "clarity-thread-store", "clarity-telemetry", "clarity-core", "clarity-anthropic-proxy", "clarity-slint", "clarity-mobile-core"]
---

# clarity-contract

Shared trait/type contract with zero internal dependencies.

## Responsibilities

- `LlmProvider` trait
- `Tool` trait
- `AgentError` unified error type
- `FederationMessage`
- `ThreadId`
- `RolloutItem`

## Notes

Everything builds on this crate.
