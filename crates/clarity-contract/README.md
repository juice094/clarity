# clarity-contract

Shared contract types for the Clarity ecosystem: reliability primitives (`RetryConfig`, `ExponentialBackoff`, `RestartConfig`, `ConnectionState`, `HeartbeatConfig`, `ConnectionMetrics`), retention policies (`RetentionPolicy`), identity types (`User`, `Team`, `Organization`, `TeamPolicy`, `PermissionPolicy`), LLM abstractions (`LlmProvider`, `Tool`), and structured errors. Zero internal dependencies — all other Clarity crates build on this.

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
- **MSRV**: 1.85（跟随 workspace）
- **反向依赖禁止** (No reverse dependencies):
  - 不得依赖 clarity-core（它是 leaf crate）
- **Library/binary classification**:
  - Library: designed for `use` by other crates
