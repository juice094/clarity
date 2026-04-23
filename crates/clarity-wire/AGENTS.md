# Agent 指引 — clarity-wire

## 构建

```bash
cargo build -p clarity-wire
```

## 测试

```bash
cargo test -p clarity-wire --lib
```

## 关键文件

- `src/lib.rs` — `Wire`、`WireSoulSide`、`WireUISide`、`WireMessage`

## 约定

- 错误处理使用标准 `broadcast::error` 类型
- 异步使用 `tokio::sync::broadcast`
- `ContentPart` 消息在 merged 通道会自动合并，非 mergeable 消息触发 flush
- 消费者通过 `recv().await` 阻塞接收，或通过 `try_recv()` 非阻塞轮询
