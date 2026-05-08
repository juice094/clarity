# clarity-contract

Minimal trait contracts for AI agent tools: `Tool` trait, capability tokens, and structured errors. Extract this crate when you need a zero-dependency foundation for tool interoperability without pulling in the full Clarity engine.

## Why use this instead of...

- **mcp-sdk-rs** — MCP is transport-centric (stdio/SSE); clarity-contract is trait-centric, letting you define tools without committing to a wire protocol.
- **rig-core** — Rig couples tools to its own executor and provider abstractions; clarity-contract keeps traits standalone so you can plug them into any runtime.

## Test

```bash
cargo test -p clarity-contract --lib
```

## 边界与稳定性

- **Stability tier**: Stable
  - Stable: API unlikely to change in minor releases
- **MSRV**: 1.78.0
- **反向依赖禁止** (No reverse dependencies):
  - 不得依赖 clarity-core（它是 leaf crate）
- **Library/binary classification**:
  - Library: designed for `use` by other crates
