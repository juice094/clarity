# Agent Guidance for clarity-thread-store

> **Scope:** `crates/clarity-thread-store` 及其子目录。  
> **类型:** Library crate。

## 1. 职责边界

`clarity-thread-store` 是 Clarity 的 Thread 持久化抽象层：

- 定义 `ThreadStore` trait。
- 提供 `InMemoryThreadStore` 与基于 `rusqlite` 的 `LocalThreadStore`。
- 与 `clarity-rollout` 协同记录事件日志。

本 crate **不** 直接处理 UI、网络或 LLM 调用。

## 2. 关键不变量

1. 只依赖 `clarity-contract` 与 `clarity-rollout`，不依赖 `clarity-core` 或任何前端 crate。
2. SQLite schema 变更需同步提供迁移逻辑与回归测试。
3. 所有 `pub` 项必须有文档注释（workspace `missing_docs = "deny"`）。

## 3. 常用命令

```bash
# 编译
cargo check -p clarity-thread-store

# 测试
cargo test -p clarity-thread-store --lib

# Clippy
cargo clippy -p clarity-thread-store --lib --tests -- -D warnings
```

## 4. 设计来源说明

`ThreadStore` trait 与相关类型受到 OpenAI Codex（Apache-2.0）的架构启发；实现为原创代码。详见 [`NOTICES.md`](./NOTICES.md)。
