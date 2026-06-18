# Agent Guidance for clarity-rollout

> **Scope:** `crates/clarity-rollout` 及其子目录。  
> **类型:** Library crate。

## 1. 职责边界

`clarity-rollout` 是 Clarity 的 JSONL rollout 持久化层：

- 以追加-only JSONL 记录 Thread/Session 生命周期事件。
- 提供 `RolloutRecorder` / `RolloutPolicy` / `RolloutConfig`。
- 支持事件日志的压缩、归档与回放。

本 crate **不** 直接处理 UI、网络或 LLM 调用。

## 2. 关键不变量

1. 只依赖 `clarity-contract` 与其他外部 crate，不依赖 `clarity-core` 或任何前端 crate。
2. 持久化格式保持向后兼容；变更磁盘格式需同步更新迁移/回放逻辑与测试。
3. 所有 `pub` 项必须有文档注释（workspace `missing_docs = "deny"`）。

## 3. 常用命令

```bash
# 编译
cargo check -p clarity-rollout

# 测试
cargo test -p clarity-rollout --lib

# Clippy
cargo clippy -p clarity-rollout --lib --tests -- -D warnings
```

## 4. 设计来源说明

JSONL rollout 概念与事件词汇受到 OpenAI Codex（Apache-2.0）的架构启发；实现为原创代码。详见 [`NOTICES.md`](./NOTICES.md)。
