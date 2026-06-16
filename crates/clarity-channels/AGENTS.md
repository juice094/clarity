# Agent 指引 — clarity-channels

## 构建

```bash
cargo build -p clarity-channels
```

## 测试

```bash
cargo test -p clarity-channels --lib
```

## 关键文件

- `src/lib.rs` — `Channel` trait, `ChannelMessage`, `ChannelError`
- `src/retry.rs` — exponential backoff `RetryPolicy` + `RetryableError`
- `src/chkit/mod.rs` — shared channel primitives
- `src/chkit/wechat/` — WeChat iLink implementation
- `src/chkit/channel.rs` — channel adapter types

## 约定

- 所有平台实现必须 `#[async_trait]` 实现 `Channel`
- 网络错误统一收敛到 `ChannelError::Network`
- 平台错误统一收敛到 `ChannelError::Platform { code, message }`
- 新增平台时优先复用 `RetryPolicy`，禁止在 handler 中手写退避逻辑
- 测试使用 mock HTTP server；禁止在单元测试中依赖真实平台 token

## 红线

- 不得依赖 `clarity-core` 或任何 frontend crate
- 不得在生产代码中使用 `unwrap`/`expect`/`panic`（测试文件除外）
- 所有 `pub` 类型和函数必须有 `///` 文档注释
