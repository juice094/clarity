---
title: Basidiocarp 生态参考笔记
category: Note
date: 2026-07-07
tags: [reference, architecture, knowledge, agent-memory]
---

# Basidiocarp 生态参考笔记

> 来源：[https://github.com/basidiocarp](https://github.com/basidiocarp)（GitHub Organization，创建于 2026-03）
> 状态：早期/小众生态，16 个公开仓库，总体 star 数极低，但架构设计高度对齐 Clarity 方向
> 用途：**架构参考与分层验证**，不建议作为依赖直接引入

---

## 1. 这是什么

Basidiocarp 是一套围绕 **“AI coding agent 的长期记忆与多 agent 协调”** 构建的本地优先 Rust 工具生态。名字取自真菌的担子果，旗下每个 crate 用菌类结构命名（hyphae、mycelium、canopy 等）。

它和 Clarity 要解决的是同一类问题：

> 会话压缩或结束后，Agent 会把架构决策、已修复 bug、项目约定全部忘掉；下一次会话重新读取、重新讨论、重新犯错。

---

## 2. 生态地图与 Clarity 映射

| Basidiocarp | 职责 | Clarity 对应 |
|-------------|------|--------------|
| **hyphae** | 持久化记忆 + RAG（episodic memory + semantic memoir graph） | `clarity-memory` + `clarity-knowledge` |
| **canopy** | 多 agent 协调运行时（task ownership、handoff、evidence ledger） | `clarity-core` 子代理 / Team / BackgroundTask |
| **hymenium** | 工作流编排引擎（phase gate、dispatch、escalation） | Plan Mode / Cron / Workflow |
| **cortina** | 生命周期信号捕获（hook events、session bridge） | 会话事件 / Watcher / Gateway events |
| **rhizome** | 代码智能 MCP（tree-sitter + LSP） | LSP Agent / MCP |
| **mycelium** | Token 压缩代理（命令输出过滤 60–90% token） | Compaction / 上下文压缩 |
| **volva** | 执行宿主运行时层 | Gateway / Headless / ExecutionHost |
| **lamella** | Skills / hooks / plugins | Skill 系统 |
| **cap** | Operator dashboard（React） | `clarity-egui` / Web UI |
| **spore / septa** | 共享传输、schema、primitives | `clarity-contract` / `clarity-wire` |
| **stipe** | 生态安装/管理器 | 安装包 / 更新器 |

结论：**分层思路几乎一致**——记忆、协调、工作流、代码智能、执行宿主、UI、共享契约各自独立成 crate/tool。

---

## 3. 对 Clarity 有直接借鉴价值的设计

### 3.1 两种记忆模型分离

Hyphae 明确区分：

- **Episodic memory**：日常事件、决策、错误，带衰减（decay）。
- **Semantic memoir**：持久的概念图、架构知识、领域模型，不衰减，只精炼。

Clarity 目前 `clarity-memory` 更偏 episodic，而 `clarity-knowledge` 的 `KnowledgeGraph` 正在承担 durable concept graph 的角色。可以进一步把“衰减”语义显式化，而不是默认所有节点同等重要。

### 3.2 Hybrid search 的评分比例

Hyphae 使用 **30% BM25 + 70% cosine similarity** 合并结果。Clarity 当前是 BM25/TF-IDF + cosine 两层，但权重和融合方式仍可调。可参考此比例做 A/B 对比。

### 3.3 本地优先、零依赖、MCP-native

Hyphae 走 SQLite + FTS5 + sqlite-vec + fastembed 本地嵌入，**不依赖云端向量库**。这与 Clarity 的主权可控目标一致，且比 Clarity 当前本地 TF-IDF + cosine 方案更先进（真正 embedding）。

> 启示：Clarity 的 `clarity-memory` 升级到 sqlite-vec + 本地 embedding 是可行路径，且不会引入外部服务。

### 3.4 Session / Recall / Outcome 反馈闭环

`feedback-loop-design.md` 提出：

- 记录每次 recall 返回了哪些 memory；
- 在 recall 后的 session 窗口内收集成功/失败信号（测试通过、无纠错、session 零错误等）；
- 根据结果动态提升或降低 memory weight。

这正是 Clarity 目前缺失的 **“记忆自我修正”** 机制。当前 `clarity-knowledge` 的激活图是单次查询的，没有跨会话的 recall effectiveness 学习。

### 3.5 Obsidian 单向导出设计

`hyphae export obsidian` 是**只读投影**：Hyphae 是真相源，Obsidian vault 是人类可读视图，Obsidian 的修改不写回。

这与我们之前讨论的“Clarity → Obsidian / 外部 vault”关系一致：Clarity 应该输出到 Obsidian，而不是被 Obsidian 反向同步。文档中的 folder layout、frontmatter、redaction 规则都可以作为 Clarity 记忆导出功能的参考。

### 3.6 Coordination ledger，不是 chat history

Canopy 的核心原则： durable work state 以 task / assignment / handoff / evidence 形式存在 ledger 中，而不是从自由对话里推断。Clarity 的 `clarity-core` 已经有 thread/session/task，但 handoff 和 evidence 引用还不够显式。

---

## 4. 与 Clarity 的差异 / 风险

| 维度 | Basidiocarp | Clarity |
|------|-------------|---------|
| **成熟度** | 2026-03 新建，star 极少，未经验证 | v0.4.0-rc，已有 1500+ 测试，Android 端到端验收 |
| **嵌入模型** | fastembed + bge-small-en-v1.5（384d） | 目前本地 TF-IDF / cosine，未用真正 embedding |
| **中文/CJK** | 未明确提及，bge-small-en 对中文弱 | 已有单字 CJK 支持，可升级 jieba |
| **协议** | MCP stdio 为主 | JSON-RPC / HTTP / WebSocket / Wire 总线 |
| **UI** | Cap 是 React Web | egui 桌面 + Android + TUI |
| **定位** | 编辑器插件/Agent 记忆层 | 通用 AI 协作运行时 |

**风险**：该生态可能是个人/小团队实验项目，更新虽活跃（2026-07-06 仍有 commit），但社区与长期维护未知。**不要引入为依赖，只吸收设计 rationale**。

---

## 5. 可落地的启示清单

按对 Clarity 的价值排序：

1. **记忆衰减与重要性分级**：给 `KnowledgeGraph` 节点引入 `Importance`/`weight`，支持时间衰减，而非所有节点同等激活。
2. **Recall effectiveness 反馈表**：在 `clarity-memory` 增加 recall_event → outcome_signal → effectiveness_score 的闭环，让常用记忆自动上浮。
3. **真正的本地 embedding**：评估 `sqlite-vec` + 小型本地 embedding 模型（如 bge-small）替代当前 TF-IDF cosine，提升语义召回。
4. **Obsidian 单向导出**：把 `clarity-knowledge` 的内容按 memory / memoir / session 等类型导出为 Markdown vault， frontmatter 标注 `clarity_id` 和 `type`。
5. **Coordination ledger 显式化**：把 task handoff、evidence 引用、agent heartbeat 等从隐式日志变为显式存储。

---

## 6. 后续动作建议

- 短期：将本笔记作为架构参考保留，不改动代码。
- 中期（下一个 Knowledge Field 迭代）：从第 5 条中选 1–2 项做 PoC，例如给 `KnowledgeGraph` 加 `weight`/`importance`。
- 长期：如果 Clarity 需要真正的本地 embedding，再评估 `sqlite-vec` + `fastembed` 的引入成本。

---

*本笔记用于跨会话继承，避免重复调研同一生态。*
