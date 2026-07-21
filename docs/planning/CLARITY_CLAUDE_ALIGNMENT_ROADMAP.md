---
title: Clarity → Claude Code 能力对齐优化路线图
category: Planning
date: 2026-06-27
tags: [optimization, roadmap, gap-analysis, claude-code-alignment]
---

# Clarity → Claude Code 能力对齐优化路线图

> 基于 2026-06-27 并行模块分析（8个子系统，210个文件），对 Clarity 各模块与 Claude Code 的等效能力差距进行了系统性评估。本文档输出优先级排序的优化路线图。

## 进度快照（2026-07-20 核实，Wave 1-3 后）

| 项 | 状态 | 证据 |
|----|------|------|
| CE-1 / T2-1 Tokenizer 贯通 | 🔄 部分落地 | `compaction::estimate_text_tokens` 已公开并被 `agent/run/loop_helpers.rs` 消费；`clarity-llm` 已引入 tiktoken；字节启发式替换未完成 |
| CE-2 / T2-2 JSON Mode | ✅ 已落地 | `ChatCompletionRequest.response_format` + `set_response_format` 穿透全部包装层（Kimi/DeepSeek/OAuth/LlamaServer）与组合器（Racing/Mesh/Router）；不支持的 provider 显式 warn（commit `bd841ff5`） |
| CE-3 / T2-3 Anthropic Messages | 🔄 部分落地 | `clarity-llm::anthropic` 适配器模块（adapter/tools/types/prompt）已交付，`clarity-gateway` `anthropic-api` feature 提供 `/v1/messages` 端点；cache_control / extended thinking 未实现 |
| T1-1 非阻塞 Spawn + Handle | ✅ 已落地 | `SubagentHandle`（poll/join/CompletionCallback）+ `SubagentCompletion::to_system_message`；`run_parallel` 旧 API 不变；向父 turn 注入待 core 侧接入（commit `0412f0f8`） |
| T1-2 Worktree 隔离 | ✅ 已落地 | `clarity-subagents/src/runner.rs`：`create_worktree_for_agent` + `WorktreeGuard`（`enable_worktree` token 开关） |
| T1-3 Structured Output Schema | ✅ 已落地 | `RunSpec::output_schema`（`clarity-contract/src/subagent.rs`）+ runner 注入 `response_format` |
| T1-4 Pipeline DAG | ⏸️ 未启动 | 无 `depends_on` / `pipeline.rs` |
| T2-5 流式 Tool-Call 增量 | 🔄 部分落地 | `StreamDelta.partial_tool_calls` 已贯通 6 crate，`ToolCallProgress` wire 事件 e2e 可用 |
| T2-4 / T2-6~T2-8 / Tier 3 | ⏸️ 未启动 | — |

## 差距评分总览

| 子系统 | 差距评分 | 状态 |
|--------|---------|------|
| **Subagent / 并行执行** | **3/10** | 🔴 关键差距 |
| Agent Loop | 6/10 | 🟡 显著差距 |
| LLM Provider Layer | 6/10 | 🟡 显著差距 |
| Context / Compaction | 6/10 | 🟡 显著差距 |
| Approval / Security | 7/10 | 🟢 接近等效 |
| Memory | 7/10 | 🟢 接近等效 |

## 跨模块关键使能器（Cross-cutting Enablers）

以下三个基础能力变动会同时解决多个子系统的差距，应优先投入：

### CE-1: Token 计数基础设施贯通
- **当前状态**: `tiktoken-rs` (cl100k_base) 仅在 `compaction.rs` 中使用
- **需要贯通**: Agent Loop → LLM Provider → Context Compaction 三层
- **影响范围**: 解决 5+ 个差距（精确上下文窗口管理、token 感知截断、成本预估、流式重试、预请求截断）
- **实现顺序**: 暴露 tokenizer → 替换 LLM 层字节启发式 → 替换 Tier-1 硬编码 120-char 阈值 → 接入主动压缩调度

### CE-2: Structured Output / JSON Mode
- **当前状态**: `ChatCompletionRequest` 无 `response_format` 字段
- **需要添加**: `response_format: Option<ResponseFormat>` → `LlmProvider` trait → Agent Loop → Subagent Runner
- **影响范围**: (a) Agent Loop 原生 JSON tool call（淘汰 text-parsing fallback）；(b) Subagent schema-validated output；(c) 工具的结构化数据提取
- **实现顺序**: ChatCompletionRequest → LlmProvider trait → Agent Loop → Subagent Runner

### CE-3: Anthropic Messages 协议完整实现
- **当前状态**: `AnthropicLlm` (`providers/anthropic.rs`) 仅处理 text content blocks
- **需要实现**: (a) `content_block_delta` 流式解析（tool_use incremental assembly）；(b) 原生 `tools[]` 序列化；(c) `cache_control: { type: 'ephemeral' }` breakpoints；(d) `thinking: { type: 'enabled' }` extended thinking
- **影响范围**: 流式 tool-call assembly、prompt caching、extended thinking — 均为 Claude Code 主执行路径
- **实现文件**: 单一文件 `crates/clarity-llm/src/providers/anthropic.rs`

---

## Tier 1: 关键差距（Subagent 子系统）

> Claude Code 的 Agent + Workflow 工具是 Clarity 与 Claude Code 之间最大的能力差距。Clarity 的子代理系统评分 3/10。

### T1-1: 非阻塞子代理 Spawn + Poll/Join Handle ✅ 计划中
- **文件**: `crates/clarity-subagents/src/parallel.rs`, `lib.rs`, `crates/clarity-contract/src/subagent.rs`
- **描述**: `run_parallel` 当前阻塞等待全部子代理完成。改为返回 `SubagentHandle`，暴露 `poll()` / `join()`。
- **产出**: 父代理可在子代理后台运行时继续自己的 turn；完成时注入 system message。
- **工作量**: M

### T1-2: Worktree 隔离
- **文件**: `crates/clarity-subagents/src/runner.rs`, `sandbox.rs` (new)
- **描述**: 对需要写权限的子代理，使用 `git worktree add` 在 `.clarity/worktrees/<agent_id>/` 创建隔离工作树。
- **产出**: 子代理文件系统操作不污染主仓库；跨代理文件冲突防护。
- **工作量**: L

### T1-3: Structured Output Schema（依赖 CE-2）
- **文件**: `crates/clarity-contract/src/subagent.rs`, `crates/clarity-subagents/src/runner.rs`
- **描述**: `AgentTypeDefinition` 添加 `output_schema: Option<Value>`；注入 `response_format` 到 LLM 请求；完成后验证。
- **产出**: 子代理输出为 schema 验证的结构化 JSON。
- **工作量**: M

### T1-4: Pipeline DAG 模式
- **文件**: `crates/clarity-subagents/src/parallel.rs`, `pipeline.rs` (new)
- **描述**: `RunSpec` 添加 `depends_on: Vec<AgentId>`；拓扑排序 + 分阶段执行（fan-out per stage, fan-in between stages）。
- **产出**: 多阶段工作流（research → analyze → report）声明式表达。
- **工作量**: L

---

## Tier 2: 重要增强

### T2-1: 客户端 Tokenizer 贯通（CE-1 实现）✅ 计划中
- **文件**: `crates/clarity-core/src/compaction.rs`, `loop_helpers.rs`, `crates/clarity-llm/src/providers/openai_compatible.rs`
- **描述**: 将 `tiktoken-rs` 从 compaction 模块暴露为 public utility，替换三个位置的字节启发式。
- **产出**: 精确客户端 token 计数；预请求截断真正适配上下文窗口；计费预估与 provider 端对齐。
- **工作量**: M

### T2-2: `response_format` / JSON Mode（CE-2 实现）
- **文件**: `crates/clarity-llm/src/api.rs`, `providers/openai_compatible.rs`, `providers/kimi.rs`
- **描述**: 添加 `ResponseFormat::JsonSchema { name, schema, strict }` / `JsonObject` 到 `ChatCompletionRequest`。不支持的 provider 返回错误，caller 回退到 text parsing。
- **产出**: DeepSeek/Kimi/OpenAI 等 provider 使用原生 JSON 模式。
- **工作量**: M

### T2-3: Anthropic Messages 协议完整实现（CE-3）
- **文件**: `crates/clarity-llm/src/providers/anthropic.rs`
- **描述**: 重写 Anthropic adapter 支持原生 content_block 流式、tool_use、cache_control、extended thinking。
- **产出**: Anthropic 成为一等 Provider，消除 OpenAI-compatible 代理路径。
- **工作量**: L

### T2-4: ReliableProvider 流式重试统一
- **文件**: `crates/clarity-llm/src/providers/openai_compatible.rs`, `reliable.rs`
- **描述**: `stream()` 当前无重试/截断/重投逻辑。重构为共享 helper，同时服务 `complete()` 和 `stream()`。
- **产出**: 流式 LLM 调用具备与非流式相同的韧性。
- **工作量**: L

### T2-5: 流式 Tool-Call 增量组装与发射
- **文件**: `crates/clarity-core/src/agent/run/loop_streaming.rs`, `clarity-wire/src/lib.rs`
- **描述**: 当前所有 tool_calls 在流结束后一次性发射。改为维护 in-progress tool call map，增量发射 `ToolCallDelta` / `ToolCallComplete` wire events。
- **产出**: UI 可实时展示 tool call 构建过程。
- **工作量**: M

### T2-6: DefaultHasher → Blake3 替换
- **文件**: `crates/clarity-core/src/agent/loop_detector.rs`, `tool_prompt_manager.rs`
- **描述**: 将 SipHash-1-3 (64-bit, 非碰撞抵抗) 替换为 Blake3 (256-bit, 加密哈希) 用于工具输出与参数哈希。
- **产出**: 消除循环检测的碰撞风险。
- **工作量**: S

### T2-7: 主动压缩调度（token 使用量预测）
- **文件**: `crates/clarity-core/src/agent/compaction_service.rs`, `compaction.rs`
- **描述**: 将 `BehaviorPredictor::predict_token_usage` 接入 `needs_compaction`，在阈值被突破前 2 条消息触发压缩。
- **产出**: 压缩在自然对话间隙运行，LLM 永不见 context-overflow 错误。
- **工作量**: M

### T2-8: 缓存感知消息保留
- **文件**: `crates/clarity-core/src/agent/compaction_service.rs`, `loop_helpers.rs`, `anthropic.rs`
- **描述**: 记录每次 LLM 调用后被缓存的消息索引范围；压缩时优先丢弃未缓存消息；截断后显式 invalidate cache。
- **产出**: 更高 cache-hit rate，降低延迟与成本。
- **工作量**: M

---

## Tier 3: 优化完善

### T3-1: 审批规则配置化
- **文件**: `crates/clarity-core/src/approval/rules.rs`, `approval-rules.toml` (new)
- **描述**: 将硬编码 `with_defaults()` 迁移到 TOML 配置，支持 `.clarity/approval-rules.toml` 项目级覆盖。
- **工作量**: M

### T3-2: 审批 batch_grants 扩展到 `(tool_name, path_prefix)` 元组
- **文件**: `crates/clarity-core/src/approval/mod.rs`, `file_read.rs`, `file_write.rs`, `file_edit.rs`
- **描述**: 路径规范化 + workspace 边界校验；按路径前缀授权。
- **工作量**: M

### T3-3: 危险命令检测
- **文件**: `crates/clarity-tools/src/shell.rs`, `powershell.rs`, `bash.rs`
- **描述**: 添加 `RISKY_PATTERNS` 列表（rm -rf, curl | sh, sudo, chmod 777 等），匹配时升级到 Critical 风险等级。
- **工作量**: S

### T3-4: 记忆稠密嵌入搜索
- **文件**: `crates/clarity-memory/src/semantic_index.rs`, `embedding.rs` (new)
- **描述**: 添加 ONNX/Candle 嵌入模型支持；混合搜索 score = α * BM25 + (1-α) * cosine_similarity。
- **工作量**: L

### T3-5: SessionStore V2 FTS5 全文索引
- **文件**: `crates/clarity-memory/src/session_store_v2.rs`
- **描述**: 添加虚拟 FTS5 表索引跨 session 消息内容。
- **工作量**: M

### T3-6: 四级编译输出回流到可搜索 fact store
- **文件**: `crates/clarity-memory/src/compilation.rs`, `semantic_index.rs`
- **描述**: 编译后的摘要作为新 fact 插入（source='compilation'），可被同一搜索 API 检索。
- **工作量**: M

---

## 已完成的优化（本会话）

| 任务 | 描述 | 状态 |
|------|------|------|
| Tool Result Truncation | `dispatch.rs` 添加 `truncate_for_context()`；`AgentConfig.max_tool_result_chars` 默认 30K | ✅ 已完成 |
| Loop Detector Semantic Similarity | `loop_detector.rs` 添加 Jaccard 行集相似度检测（三层策略：hash → pattern → semantic） | ✅ 已完成 |

---

## 依赖关系

```text
CE-1 (Tokenizer) ──┬── T2-1 (Client Tokenizer) ──┬── T2-4 (Stream Retry)
                   │                              ├── T2-7 (Proactive Compaction)
                   │                              └── T2-8 (Cache-aware Retention)
                   │
CE-2 (JSON Mode) ──┼── T2-2 (response_format) ────── T1-3 (Subagent Structured Output)
                   │
CE-3 (Anthropic) ──┼── T2-3 (Anthropic Messages) ─── T2-5 (Streaming Tool-Call Assembly)
                   │
T1 (Subagent) ─────┼── T1-1 (Non-blocking Spawn) ── T1-4 (Pipeline DAG)
                   └── T1-2 (Worktree Isolation)
```

## 实施建议

1. **Phase A (1-2 周)**: CE-1 (Tokenizer 贯通) → T2-6 (Blake3) → T2-2 (JSON Mode)
2. **Phase B (2-4 周)**: CE-3 (Anthropic Messages) → T2-5 (Streaming Tool-Call) → T2-4 (Stream Retry)
3. **Phase C (4-6 周)**: T1-1 (Non-blocking Spawn) → T1-3 (Structured Output) → T1-4 (Pipeline DAG)
4. **Phase D (6-8 周)**: T1-2 (Worktree Isolation) → T2-7/T2-8 (Cache-aware)
5. **Phase E (持续)**: Tier 3 各项按需推进

---

*最后更新：2026-07-20（增加进度快照）*
*数据来源：并行模块分析 Workflow (21 agents, 526s), 实机验证*
