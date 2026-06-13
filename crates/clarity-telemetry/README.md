# clarity-telemetry

Unified telemetry foundation for the Clarity ecosystem — wide events, metrics, traces, and config audit.

## 职责

- **Wide event model** — normalize metrics, logs, and traces into `WideEvent`
- **Event sink abstraction** — fan out to SQLite, GreptimeDB, or custom sinks
- **Tracing integration** — optional `tracing-subscriber` layer
- **Config audit trail** — before/after hashes, rollback commands, actor tracking

## 关键类型

- `WideEvent` — unified observability event
- `EventSink` / `FanOutSink` — sink trait and fan-out implementation
- `audit::ConfigAudit` — immutable config change log
- `backend::sqlite::SqliteSink` — local-first SQLite backend
- `backend::greptime::GreptimeSink` — GreptimeDB HTTP backend (feature `greptime`)

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `sqlite` | yes | Local SQLite backend |
| `greptime` | no | GreptimeDB HTTP backend |
| `tracing-integration` | yes | `tracing-subscriber` layer |

## 测试

```bash
cargo test -p clarity-telemetry --lib
```

## 边界与稳定性

- **Stability tier**: Stable
  - `WideEvent` schema is a compatibility surface
- **MSRV**: 1.85.0
- **反向依赖禁止** (No reverse dependencies):
  - 不得依赖任何 frontend crate（egui/tui/gateway/slint）
- **Library/binary classification**:
  - Library: designed for `use` by other crates
