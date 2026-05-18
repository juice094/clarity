# clarity-subagents

Subagent management system for Project Clarity.

## 职责

- **子代理生命周期** — 构建、调度、执行、结果收集的完整闭环
- **并行执行** — `ParallelExecutor` 支持批量子代理并发运行，带取消令牌
- **团队协调** — `TeamCoordinator` 管理角色分工与状态同步
- **Jumpy 预测** — 可选集成 `OutcomePredictor`，执行前预测工具调用结果
- **契约重导出** — 所有子代理类型定义在 `clarity-contract` 中，本 crate 提供逻辑实现

## 关键类型

- `SubagentManager` — 整合存储、执行器、预测器的高级接口
- `SubagentBuilder` — `std::process::Command` 风格的构建器模式
- `SubagentRunner` — 单个子代理的执行引擎
- `ParallelExecutor` / `SubagentBatch` — 批量并发执行
- `TeamCoordinator` — 团队级角色与状态管理
- `SubagentStore` — 持久化存储子代理状态与输出

## 测试

```bash
cargo test -p clarity-subagents --lib
```
