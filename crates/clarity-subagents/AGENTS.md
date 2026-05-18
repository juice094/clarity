# Agent 指引 — clarity-subagents

## 构建

```bash
cargo build -p clarity-subagents
```

## 测试

```bash
cargo test -p clarity-subagents --lib
```

## 关键文件

- `src/lib.rs` — 入口、`SubagentManager`、契约重导出
- `src/builder.rs` — `SubagentBuilder` 构建器模式实现
- `src/runner.rs` — `SubagentRunner` 执行引擎、`OutputCollector`
- `src/parallel.rs` — `ParallelExecutor`、`SubagentBatch` 并发执行
- `src/team.rs` — `TeamCoordinator` 团队角色协调
- `src/store.rs` — `SubagentStore` 状态持久化
- `src/registry.rs` — 子代理类型注册表
