# Jumpy Workflow Orchestration — 实验报告

> **状态**: MVP 完成，通过单元测试与集成测试  
> **论文**: Farebrother et al., "Compositional Planning with Jumpy World Models", arXiv:2602.19634  
> **机构映射**: Meta FAIR / Mila / McGill → Clarity Agent Runtime

---

## 1. 实验目标

将跳跃世界模型（Jumpy World Model）的核心方法论适配到 **Clarity 的层级工作流编排**中：

- **时间抽象**：将预训练 Skill 视为可组合的"短跑专家"，直接预测其执行后的系统状态，而非逐消息模拟。
- **离线组合规划**：从历史执行日志中学习世界模型，零样本组合 Skills 解决长程任务。
- **视界一致性**：短视界预测作为长视界预测的锚点，消除复合误差。
- **MPC 重规划**：执行一步、观测偏差、触发重规划，而非一次性执行完整序列。

---

## 2. 核心映射（RL → 工作流）

| 论文概念 | Clarity 实现 | 文件 |
|---------|-------------|------|
| State `s` | `JumpyState` — 紧凑的工作流上下文快照（tags, memory, progress） | `state.rs` |
| Policy `π_z` | `Skill` — 参数化工作流（skill_id + params = z） | `planner.rs` |
| GHM `m^π_γ` | `HistoricalPredictor` — 基于历史观测的最近邻预测器 | `predictor.rs` |
| TD-HC 一致性 | `ConsistentPredictor` — 短视界引导长视界的引导层 | `predictor.rs` |
| CompPlan | `HierarchicalPlanner` — 随机射击 + 预测评估 | `planner.rs` |
| MPC 执行 | `SkillComposer` — 单步执行 + 偏差检测 + 重规划 | `composer.rs` |

---

## 3. 架构设计

```text
┌──────────────────────────────────────────────────────────────────────┐
│                         SkillComposer (Runtime)                       │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │              HierarchicalPlanner — 组合规划器                    │  │
│  │  - 随机射击生成候选 Skill 序列（SkillProposal）                  │  │
│  │  - 通过 OutcomePredictor 评估每个序列的终点状态                  │  │
│  │  - 选择最高 Goal.evaluate(state) 的序列                         │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│                              ▼                                       │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │              OutcomePredictor — 跳跃世界模型                     │  │
│  │  ┌──────────────────────────────────────────────────────────┐  │  │
│  │  │ HistoricalPredictor (基础层)                              │  │  │
│  │  │ - 内存中存储 (skill_id, params, before, after) 观测        │  │  │
│  │  │ - K-NN 查找最相似的初始状态，加权平均终点状态               │  │  │
│  │  └──────────────────────────────────────────────────────────┘  │  │
│  │  ┌──────────────────────────────────────────────────────────┐  │  │
│  │  │ ConsistentPredictor (引导层)                              │  │  │
│  │  │ - 强制长视界预测通过短视界引导（论文核心 insight）          │  │  │
│  │  │ - 提供 check_consistency() 用于诊断                        │  │  │
│  │  └──────────────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                              │                                       │
│                              ▼                                       │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │              JumpyState — 观测空间抽象                           │  │
│  │  - tags: 语义标签（如 ["build-failed", "tests-passing"]）       │  │
│  │  - memory: KV 工作记忆（如 {"refactored_file": "src/lib.rs"}）  │  │
│  │  - active_files: 当前活跃文件集合                               │  │
│  │  - progress: 目标完成度估计 [0.0, 1.0]                          │  │
│  │  - distance(): 状态间距离度量（加权 Jaccard + progress 差）     │  │
│  └────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 4. 关键算法

### 4.1 预测器（GHM 的简化实现）

论文使用流匹配（Flow Matching）学习后继测度。本 MVP 使用**统计最近邻**替代：

```rust
// 观测记录
observations: HashMap<(skill_id, params), Vec<(before_state, after_state)>>

// 预测
fn predict(skill, params, current) -> JumpyState {
    let candidates = observations[(skill, params)];
    let neighbors = candidates.sort_by(|a, b| {
        current.distance(a.before).cmp(current.distance(b.before))
    }).take(K);
    
    // 逆距离加权平均
    weighted_average(neighbors)
}
```

**为什么足够**：
- 工作流状态的维度远低于物理机器人状态（机器人是连续高维，工作流是离散标签+KV）。
- 最近邻在紧凑语义空间中的泛化能力已经很强。
- 避免了流匹配所需的大量训练数据和神经网络 infra。

### 4.2 视界一致性（Horizon Consistency）

论文核心创新。本实现简化为：

```rust
impl ConsistentPredictor {
    async fn predict(skill, params, state, long_commitment) {
        // 不直接预测长视界，而是先预测短视界，再从中预测长视界
        let short = inner.predict(skill, params, state, short_commitment).await?;
        inner.predict(skill, params, &short, long_commitment).await
    }
}
```

这利用了工作流的一个特性：**Skill 的效果通常是可叠加或可近似的**（不像物理动力学那样非线性）。

### 4.3 组合规划（CompPlan）

```rust
fn plan(goal, initial_state) -> SkillSequence {
    let mut best = None;
    let mut best_value = -inf;
    
    for _ in 0..num_candidates {
        let seq = random_sequence(initial_state);
        let value = evaluate_sequence(seq, goal, initial_state).await;
        if value > best_value {
            best = seq;
            best_value = value;
        }
    }
    best
}
```

**评估方式**：链式预测 + Goal.evaluate()。与论文 Lemma 1 的蒙特卡洛估值器等价。

### 4.4 MPC 执行

```rust
loop {
    if goal.satisfied(current_state) { break; }
    
    let (seq, _) = planner.plan(goal, &current_state).await?;
    let predicted = predictor.predict(seq[0], &current_state).await?;
    let actual   = execute_skill(seq[0]).await?;
    
    if predicted.distance(&actual) > threshold {
        // 偏差过大 → 下一轮自动重规划
        record_replan_event();
    }
    
    current_state = actual;
}
```

---

## 5. 测试结果

```bash
cargo test -p clarity-core --lib agent::jumpy

running 3 tests
test agent::jumpy::planner::tests::test_planner_finds_sequence ... ok
test agent::jumpy::tests::test_consistency_wrapper_bootstraps_long_horizon ... ok
test agent::jumpy::tests::test_full_pipeline_with_replanning ... ok

test result: ok. 3 passed; 0 failed; 0 ignored
```

**集成测试验证的场景**：
1. 从历史观测学习两个 Skill（explore → coder）的效果分布。
2. 为目标 `"coded"` 规划序列。
3. 执行 explore（符合预测）。
4. 执行 coder（严重偏离预测：缺少目标标签 + progress 不足）。
5. 偏差检测触发重规划。
6. 第二次执行 coder 成功到达目标状态。

---

## 6. 与现有 Clarity 系统的集成点

### 6.1 已连接

- `agent/mod.rs` 导出了 `jumpy` 模块。
- `JumpyState` 的设计兼容 `AgentInner` 中的 `active_file_paths`、`git_context` 等字段。
- `SkillComposer::compose()` 通过回调解耦，可接入 `Agent::run_flow()` 或 `SkillRegistry::run_flow()`。

### 6.2 待连接（未来 Sprint）

| 集成点 | 方案 |
|--------|------|
| **历史观测来源** | 从 `clarity-memory::session_store` 提取 (skill_id, initial_context, final_context) 记录 |
| **Skill 参数化** | 扩展 `SkillMeta` 的 `parameters` 字段，允许运行时传入子目标 |
| **Flow 节点扩展** | 新增 `FlowNodeKind::InvokeSkill` 和 `FlowNodeKind::PredictCheckpoint` |
| **子 Agent 委托** | `SkillComposer` 的回调可绑定到 `SubagentManager::spawn()` |
| **后台任务** | 长序列可通过 `BackgroundTaskManager` 分片执行，跨会话持久化 |

---

## 7. 局限与下一步

### 7.1 当前局限

1. **预测器无神经网络**：仅适用于技能效果高度可重复的场景。对于创造性/开放性任务，需要 LLM 增强预测。
2. **状态空间手工设计**：`JumpyState` 的 tags 和 memory keys 需要人工定义。未来可通过 LLM 自动提取。
3. **无嵌套 Skill**：当前序列是平面的。论文支持策略的递归组合（GSP 嵌套）。
4. **随机射击效率低**：高维参数空间下需要更聪明的提案分布（如使用无条件 GHM 生成 waypoints）。

### 7.2 下一步实验

1. **LLM-Augmented Predictor**：当历史记录不足时，调用 LLM 做零样本状态转移预测。
2. **从 Session Store 自动提取观测**：编写 pipeline 将现有聊天记录转换为 `SkillObservation`。
3. **与 `Agent::run_flow()` 打通**：实现一个 `JumpyFlowExecutor`，让 Flow 的 Task 节点支持 `InvokeSkill`。
4. **A/B 测试**：在真实任务上对比 "Jumpy 规划 + 重规划" vs "传统 Plan 执行" 的成功率和 token 消耗。

---

## 8. 文件清单

```
crates/clarity-core/src/agent/jumpy/
├── mod.rs        — 模块入口与架构文档
├── state.rs      — JumpyState 与距离度量
├── predictor.rs  — HistoricalPredictor + ConsistentPredictor
├── planner.rs    — HierarchicalPlanner + Goal + SkillProposal
├── composer.rs   — SkillComposer + ComposerBuilder + MPC 执行
├── tests.rs      — 集成测试（离线学习 → 规划 → 执行 → 重规划）
└── README.md     — 本实验报告
```

---

## 9. 参考

- Farebrother, J., Pirotta, M., Tirinzoni, A., Bellemare, M. G., Lazaric, A., & Touati, A. (2026). *Compositional Planning with Jumpy World Models*. arXiv:2602.19634.
- Farebrother, J., et al. (2025). *Temporal Difference Flows*. arXiv:2503.09817.
- Thakoor, S., et al. (2022). *Geometric Policy Composition*. arXiv:2206.08736.
