# clarity-wire

Clarity 事件总线：基于 `tokio::sync::broadcast` 的 Soul-UI 跨模块通信通道。

## 职责

- **SPMC 广播通道** — 单生产者（Soul）多消费者（UI）的消息总线
- **消息生命周期** — 覆盖对话回合的完整生命周期：`TurnBegin` → `StepBegin` → `ContentPart` / `ToolCall` / `ToolResult` → `TurnEnd`
- **消息合并** — 自动合并连续的 `ContentPart`，减少 UI 渲染压力
- **双通道设计** — `raw` 通道保留原始消息，`merged` 通道提供合并后的高效消费
- **优雅关闭** — `shutdown()` 刷新缓冲区并关闭通道，消费者收到 `RecvError::Closed`

## 关键类型

- `Wire` — 总线主体，管理 raw / merged 两个广播通道
- `WireSoulSide` — 生产者端，提供 `send()` 与 `flush()`
- `WireUISide` — 消费者端，提供阻塞 `recv()` 与非阻塞 `try_recv()`
- `WireMessage` — 核心消息枚举，覆盖对话全生命周期

## 测试

```bash
cargo test -p clarity-wire --lib
```
