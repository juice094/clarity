# Agent 指引 — clarity-telemetry

## 构建

```bash
cargo build -p clarity-telemetry
```

## 测试

```bash
cargo test -p clarity-telemetry --lib
```

## 关键文件

- `src/lib.rs` — `WideEvent`, `EventSink`, public re-exports
- `src/audit.rs` — config audit trail
- `src/sink.rs` — sink abstraction and fan-out
- `src/backend/sqlite.rs` — SQLite backend
- `src/backend/greptime.rs` — GreptimeDB backend
- `src/tracing_layer.rs` — `tracing-subscriber` integration (feature-gated)

## 约定

- Telemetry write failures must be logged but must never block the hot path
- Feature-gate all optional backends (`sqlite`, `greptime`, `tracing-integration`)
- `WideEvent` fields are additive-only; removing fields is a breaking change
- Config audit entries are append-only and carry before/after hashes

## 红线

- 不得依赖 `clarity-core` 或任何 frontend crate
- 不得在生产代码中使用 `unwrap`/`expect`/`panic`（测试文件除外）
- 所有 `pub` 类型和函数必须有 `///` 文档注释
