# clarity-thread-store

`clarity-thread-store` 提供 Clarity 的 Thread 持久化抽象：定义 `ThreadStore` trait，并包含内存实现与基于 SQLite 的本地实现，支撑 Session/Thread 的生命周期管理。

## 职责

- 定义 `ThreadStore` trait 及 `Thread`、`ThreadSource`、`LiveThread` 等核心类型。
- 提供 `InMemoryThreadStore` 与 `LocalThreadStore`（SQLite）两种实现。
- 与 `clarity-rollout` 协同完成事件日志持久化。

## 设计参考

本 crate 的 `ThreadStore` trait 与相关类型受到 OpenAI Codex（Apache-2.0）的架构启发；实现为 Clarity 原创代码，按 AGPL-3.0-or-later 发布。详见 [`NOTICES.md`](./NOTICES.md)。

## 依赖

- `clarity-contract`
- `clarity-rollout`
- `rusqlite`（bundled-full）用于本地 SQLite 实现

## 测试

```bash
cargo test -p clarity-thread-store --lib
```
