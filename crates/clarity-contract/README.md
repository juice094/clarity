# clarity-contract

Minimal trait contracts for AI agent tools: `Tool` trait, capability tokens, and structured errors. Extract this crate when you need a zero-dependency foundation for tool interoperability without pulling in the full Clarity engine.

## Why use this instead of...

- **mcp-sdk-rs** — MCP is transport-centric (stdio/SSE); clarity-contract is trait-centric, letting you define tools without committing to a wire protocol.
- **rig-core** — Rig couples tools to its own executor and provider abstractions; clarity-contract keeps traits standalone so you can plug them into any runtime.

## Test

```bash
cargo test -p clarity-contract --lib
```
