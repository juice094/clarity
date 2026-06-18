# clarity-rollout

`clarity-rollout` 提供 Clarity 的 JSONL rollout 持久化层：以追加-only 的 JSONL 事件日志形式记录 Thread/Session 生命周期事件，支持压缩、归档与回放。

## 职责

- 定义 `RolloutRecorder`、`RolloutPolicy`、`RolloutConfig` 等核心类型。
- 将 Agent 运行事件序列化为 JSONL 并落盘。
- 提供按线程/会话读取、压缩与回放的 API。

## 设计参考

本 crate 的 JSONL rollout 概念与事件词汇受到 OpenAI Codex（Apache-2.0）的架构启发；实现为 Clarity 原创代码，按 AGPL-3.0-or-later 发布。详见 [`NOTICES.md`](./NOTICES.md)。

## 依赖

- `clarity-contract`
- 标准序列化/异步/ tracing 依赖

## 测试

```bash
cargo test -p clarity-rollout --lib
```
