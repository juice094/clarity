---
title: J7/J8 Design: Flow Extension + SubagentManager Integration
category: Design
date: 2026-05-16
tags: [design, agent]
---

# J7/J8 Design: Flow Extension + SubagentManager Integration

## J7: Flow 节点扩展

### 目标

在现有 Flow 引擎中新增两种节点类型，支持 Jumpy 预测驱动的动态编排。

### 现有节点

```rust
pub enum FlowNodeKind {
    Begin,
    End,
    Task,
    Decision,
}
```

### 新增节点

```rust
pub enum FlowNodeKind {
    Begin,
    End,
    Task,
    Decision,
    InvokeSkill,      // 调用外部 Skill（通过 devbase skill-run MCP）
    PredictCheckpoint, // 在关键决策点执行 Jumpy 预测并验证
}
```

#### InvokeSkill 节点

- **语义**：调用 devbase 注册的 Skill
- **参数**：`skill_id: String`, `params: String`
- **执行**：通过 `devbase_skill_run` MCP 工具执行
- **输出**：Skill 执行结果写入 `JumpyState.memory`

#### PredictCheckpoint 节点

- **语义**：在当前状态执行预测，验证是否达到预期目标
- **参数**：`expected_tags: Vec<String>`, `min_progress: f32`
- **执行**：
  1. 调用 `HybridPredictor.predict()` 预测执行后的状态
  2. 对比预测状态与预期条件
  3. 若预测不满足条件 → 触发重新规划（回到 Decision 节点）
  4. 若预测满足条件 → 继续执行

### FlowRunner 修改

```rust
impl<'a> FlowRunner<'a> {
    async fn execute_node(&mut self, node_id: &str) -> Result<String, FlowError> {
        let node = self.flow.nodes.get(node_id).ok_or(...)?;
        match node.kind {
            FlowNodeKind::Task => self.execute_task(node).await,
            FlowNodeKind::Decision => self.execute_decision(node).await,
            FlowNodeKind::InvokeSkill => self.execute_invoke_skill(node).await,
            FlowNodeKind::PredictCheckpoint => self.execute_predict_checkpoint(node).await,
            _ => ...
        }
    }
}
```

---

## J8: SubagentManager 打通

### 目标

让 `SubagentManager` 能接收 Jumpy 预测输出，并根据预测结果智能路由子代理任务。

### 集成点

```rust
pub struct SubagentManager {
    store: SubagentStore,
    runner: SubagentRunner,
    predictor: Option<Arc<dyn OutcomePredictor>>, // 新增
}
```

### 路由策略

1. **执行前预测**：在调用子代理前，预测执行后的状态
   ```rust
   async fn run_with_prediction(&self, spec: RunSpec) -> SubagentResult {
       let current = self.capture_current_state();
       let predicted = self.predictor
           .predict(&spec.skill_id, &spec.params, &current, 0.9)
           .await;
       
       // 根据预测结果选择执行策略
       match predicted {
           Ok(state) if state.satisfies(&spec.goal_tags) => {
               // 预测成功 → 直接执行，无需监控
               self.runner.run(spec).await
           }
           Ok(state) => {
               // 预测不确定 → 执行并启用详细监控
               self.runner.run_with_monitoring(spec).await
           }
           Err(_) => {
               // 预测失败 → 使用默认策略
               self.runner.run(spec).await
           }
       }
   }
   ```

2. **子代理类型选择**：根据 `JumpyState.tags` 选择最合适的子代理类型
   - `tags` 包含 "explore" → 启动 explore 子代理
   - `tags` 包含 "code" → 启动 coder 子代理
   - `tags` 包含 "plan" → 启动 plan 子代理

3. **并行执行优化**：`ParallelExecutor` 使用预测结果来分组兼容的任务
   - 预测不会冲突的任务可以并行执行
   - 预测有冲突的任务需要串行执行

### 状态捕获

```rust
impl SubagentManager {
    fn capture_current_state(&self) -> JumpyState {
        JumpyState {
            tags: self.store.current_tags(),
            memory: self.store.working_memory(),
            active_files: self.store.active_files(),
            context_summary: self.store.context_summary(),
            progress: self.store.progress(),
        }
    }
}
```

---

## 依赖清单

| 依赖 | 来源 | 状态 |
|------|------|------|
| `FlowNodeKind` | `clarity-core::agent::flow` | ✅ 已有 |
| `FlowRunner` | `clarity-core::agent::flow::runner` | ✅ 已有 |
| `SubagentManager` | `clarity-core::subagents` | ✅ 已有 |
| `OutcomePredictor` | `clarity-core::agent::jumpy::predictor` | ✅ 已有 |
| `devkit_skill_run` | devbase MCP | ✅ 已可用 |
| `HybridPredictor` | J6 实现 | ⏳ 依赖 J6 |

---

## 执行顺序

1. J6 `HybridPredictor` 实现完成
2. J7 新增 FlowNodeKind 变体 + FlowRunner 执行逻辑
3. J8 SubagentManager 集成 predictor + 路由策略
4. J7/J8 并行开发（接口已定义，无循环依赖）
