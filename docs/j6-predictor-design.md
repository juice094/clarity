# J6 Design: LLM-Augmented Predictor

## 目标

实现 J6「LLM-Augmented Predictor」—— 当 `HistoricalPredictor` 缺乏足够历史数据时，自动回退到 LLM 零样本预测，形成 Hybrid 预测体系。

当前状态：`HistoricalPredictor` + `ConsistentPredictor` 已实现（MVP）。缺失 LLM 回退链路。

---

## 架构

```
HybridPredictor
├── HistoricalPredictor (k-NN, 已有)
└── LlmAugmentedPredictor (新增)
    └── LlmProvider (clarity_contract::LlmProvider)
```

`HybridPredictor` 优先查询历史数据：
1. 若 `HistoricalPredictor` 返回 `Ok(state)` 且置信度 ≥ threshold → 直接返回
2. 若历史不足或置信度低 → 调用 `LlmAugmentedPredictor`
3. LLM 预测成功后，可选择将结果缓存为「合成观察」供未来历史查询

---

## 接口定义

### 1. LlmAugmentedPredictor

```rust
use crate::llm::LlmProvider;
use super::state::JumpyState;
use super::predictor::{OutcomePredictor, SkillObservation};

pub struct LlmAugmentedPredictor {
    llm: Arc<dyn LlmProvider>,
    system_prompt: String,
}

impl LlmAugmentedPredictor {
    pub fn new(llm: Arc<dyn LlmProvider>) -> Self {
        Self {
            llm,
            system_prompt: DEFAULT_LLM_PREDICTOR_PROMPT.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl OutcomePredictor for LlmAugmentedPredictor {
    async fn predict(
        &self,
        skill_id: &str,
        params: &str,
        current: &JumpyState,
        _commitment: f32,
    ) -> Result<JumpyState, String> {
        let prompt = build_prediction_prompt(skill_id, params, current);
        let response = self.llm.complete(&prompt, "")
            .await
            .map_err(|e| format!("LLM prediction failed: {}", e))?;
        parse_prediction_response(&response)
    }
}
```

### 2. HybridPredictor

```rust
pub struct HybridPredictor {
    historical: HistoricalPredictor,
    llm: LlmAugmentedPredictor,
    confidence_threshold: f32,
    cache_synthetic: bool,
}

#[async_trait::async_trait]
impl OutcomePredictor for HybridPredictor {
    async fn predict(
        &self,
        skill_id: &str,
        params: &str,
        current: &JumpyState,
        commitment: f32,
    ) -> Result<JumpyState, String> {
        // 1. Try historical first
        match self.historical.predict(skill_id, params, current, commitment).await {
            Ok(state) => Ok(state),
            Err(_) if self.llm_available() => {
                // 2. Fallback to LLM
                let predicted = self.llm.predict(skill_id, params, current, commitment).await?;
                if self.cache_synthetic {
                    // Optional: store as synthetic observation
                }
                Ok(predicted)
            }
            Err(e) => Err(e),
        }
    }
}
```

---

## LLM Prompt 设计

### System Prompt

```
You are a world-model predictor for an AI agent workflow system.
Given:
- Current workspace state (tags, memory, active files, progress)
- A skill to be executed (skill_id + parameters)

Predict the state AFTER the skill executes. Output valid JSON matching:
{
  "tags": ["tag1", "tag2"],
  "memory": {"key": "value"},
  "active_files": ["/path/to/file"],
  "context_summary": "what happened",
  "progress": 0.5
}

Rules:
- progress must be in [0.0, 1.0]
- tags should be concise semantic labels
- memory should capture key outputs of the skill
- active_files should list files created/modified by the skill
```

### User Prompt Template

```
Current State:
- Tags: {tags}
- Memory: {memory_json}
- Active Files: {active_files}
- Progress: {progress}
- Context: {context_summary}

Skill to Execute: {skill_id}
Parameters: {params}

Predict the resulting state after execution.
```

---

## 训练数据流

```
SessionStore (clarity-memory)
    ↓ session_to_observations()
Vec<SkillObservation>
    ↓ observe_batch()
HistoricalPredictor.observations
    ↓ predict()
JumpyState (or fallback to LLM)
```

J10 A/B 验证需要 ≥20 条轨迹：
- 来源 1：clarity-memory 的历史 session（已有）
- 来源 2：devbase `experiment_log`（tier 已调整为 Beta，可用）
- 来源 3：运行时自动记录（每次 predict → 实际执行 → 对比 → 写入 observation）

---

## 测试策略

1. **单元测试**：`LlmAugmentedPredictor` 使用 mock `LlmProvider`
2. **集成测试**：与 `HistoricalPredictor` 组成 `HybridPredictor`，验证回退逻辑
3. **一致性测试**：`ConsistentPredictor<HybridPredictor>` 验证 horizon consistency
4. **端到端测试**：使用真实 session 数据，对比历史预测 vs LLM 预测 vs 实际结果

---

## 依赖清单

| 依赖 | 来源 | 状态 |
|------|------|------|
| `LlmProvider` trait | `clarity_contract` | ✅ 已有 |
| `OutcomePredictor` trait | `clarity-core::agent::jumpy::predictor` | ✅ 已有 |
| `JumpyState` | `clarity-core::agent::jumpy::state` | ✅ 已有 |
| `SkillObservation` | `clarity-core::agent::jumpy::predictor` | ✅ 已有 |
| `session_store_adapter` | `clarity-core::agent::jumpy` | ✅ 已有 (J5) |
| `devkit_experiment_log` | devbase MCP | ✅ tier 已调为 Beta |
| `devkit_project_context` | devbase MCP | ✅ 已可用 |

---

## Blockers

1. **LLM Provider 注入**：`LlmAugmentedPredictor` 需要 `Arc<dyn LlmProvider>`，需确认 clarity 的 Agent 运行时如何传递 provider 到 jumpy 模块
2. **Prompt 调优**：零样本预测的准确率高度依赖 prompt 质量，需要迭代优化
3. **合成观察缓存**：LLM 预测结果是否应写入 `HistoricalPredictor` 作为合成数据，需要评估幻觉风险

---

## 执行顺序

1. 实现 `LlmAugmentedPredictor`（含 prompt builder + response parser）
2. 实现 `HybridPredictor`（history-first + LLM fallback）
3. 单元测试（mock LLM）
4. 集成到 clarity Agent 运行时（注入 LlmProvider）
5. J10 数据集收集（≥20 条轨迹）
