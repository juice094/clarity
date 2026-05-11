# Anthropic Managed Agents x Clarity 架构映射 — 2026-05-11

> Type: Comparative architecture analysis
> Source: Kimi share 对话 https://www.kimi.com/share/19e17737-7fa2-841d-8000-00006dd455ec
> Trigger: 用户视频 "比Openclaw更好！我发现了多Agent协作架构的版本答案" + Kimi 综合分析
> Status: 用于支撑 ADR-008（待立项）+ docs/architecture-positioning.md 章节更新

---

## 1. Anthropic Managed Agents 核心架构

### 1.1 三层解耦（Brain / Hands / Session）

```
Brain（推理）         Hands（执行）        Session（日志）
+----------+         +----------+         +----------+
| Harness  |--tool-->| Sandbox  |--event->| Session  |
| 无状态   |         | 容器隔离 |         | append-  |
| 编排器   |<--result| 可丢弃   |         | only log |
+----------+         +----------+         +----------+
     ^                                          ^
     +-------- wake(sessionId) + getSession(id) +
```

### 1.2 关键概念

| 概念 | 定义 |
|------|------|
| Harness | 无状态推理编排器，跑在 Anthropic 基础设施上。可被任意进程通过 wake(sessionId) 启动 |
| Sandbox | 每会话独立容器，工具在沙箱内执行；可随时销毁重建 |
| Session | append-only 事件日志，独立于 LLM context window |
| Vault | 凭证存储，工具调用通过 API proxy；凭证永不进入 sandbox |
| Memory Store | 跨会话持久化记忆 |
| Agent Skills | 可复用的能力定义（markdown + manifest） |
| MCP Servers | 工具协议，扩展 sandbox 能力 |

### 1.3 关键操作

```
createSession(agentId, env) -> sessionId
wake(sessionId) -> harness 从 sandbox + session log 恢复执行
getSession(sessionId) -> 读取事件日志
checkpointSession() -> 强制保存当前状态
suspendSession() -> 主动暂停（Harness 释放，sandbox 保留）
```

### 1.4 定价

- 0.08 USD/session-hour + token 费用 + 工具调用费用
- 三类计费同时：runtime / tokens / tool usage

### 1.5 权威来源

| 来源 | URL |
|------|-----|
| Anthropic Engineering Blog | https://www.anthropic.com/engineering/managed-agents |
| Claude Docs Overview | https://platform.claude.com/docs/en/managed-agents/overview |
| Claude Docs Sessions | https://platform.claude.com/docs/en/managed-agents/sessions |
| GitHub Skills 仓库 | https://github.com/anthropics/skills/blob/main/skills/claude-api/shared/managed-agents-overview.md |
| InfoQ 报道 | https://www.infoq.com/news/2026/04/anthropic-managed-agents/ |
| Finout 定价分析 | https://www.finout.io/blog/anthropic-just-launched-managed-agents |
| FDE 招聘 | http://job-boards.greenhouse.io/anthropic/jobs/4985877008 |

---

## 2. OpenClaw 对比（Kimi 引用学术分析）

### 2.1 OpenClaw 核心架构（七组件）

- Channel System（多渠道消息）
- Gateway（单一长生命周期）
- Plugins & Skills
- Agent Runtime
- Memory & Knowledge
- LLM Provider
- Local Execution

### 2.2 已知问题

来源：arxiv 2603.12644v1 "A Case Study of OpenClaw"
- Prompt Injection 漏洞
- RCE 风险（同进程工具执行）
- Section 4 提出 FASA 理论防御

### 2.3 OpenClaw vs Anthropic 对比

| 维度 | OpenClaw | Anthropic Managed Agents |
|------|----------|------|
| 执行模式 | 本地优先，同进程 | Cloud managed，容器隔离 |
| 安全模型 | 弱（同进程 RCE 风险） | 强（沙箱 + Vault） |
| 状态管理 | Gateway 长生命周期 | Harness 无状态 + Session log |
| 部署复杂度 | 低 | 高 |
| 定价 | 免费 / 自托管 | 0.08 USD/session-hour |
| 适用场景 | 个人/小团队 | 企业级长任务 |

---

## 3. Clarity x Anthropic 映射矩阵

| 概念 | Anthropic | Clarity 当前 | 一致度 | 差距 |
|------|-----------|--------------|--------|------|
| Agent 定义 | YAML/API | agent.yaml + Agent | OK | — |
| Brain（推理） | Harness 无状态 | Agent ReAct/Plan | WARN | Agent 有状态 |
| Hands（执行） | Sandbox 容器 | ToolRegistry 同进程 | FAIL | 无沙箱 |
| Session | append-only log | SessionStore + messages | WARN | messages 直接是 context |
| Session 恢复 | wake(sessionId) | 加载历史 messages | WARN | 无 wake 抽象 |
| Memory | Memory Store | clarity_memory | OK | 等价 |
| Vault | 凭证不进 sandbox | env var + TokenStore | OK | 等价 |
| 持久化 | 自动 Checkpoint | SQLite + JSON | OK | — |
| 通信契约 | Session Event Log | WireMessage（ADR-006） | OK | 收敛后一致 |
| 多 Agent | Sub-agents | SubagentOrchestrator | OK | — |
| MCP | MCP server skills | 三协议 | OK | — |
| Skills | Agent Skills | SkillRegistry | OK | — |
| 审批 | 权限范围 | ApprovalMode 4 种 | OK+ | Clarity 更精细 |

**总体匹配度 ~70%**

---

## 4. 三处实质差距详解

### 4.1 差距 A：Agent 有状态 vs Brain 无状态

Anthropic：Harness 完全无状态，状态在 Session Log。
Clarity：Agent 持有 registry / wire / approval_runtime / memory_ticker 等。

**影响**：
- Clarity 无法在不同进程间迁移 Agent 实例
- Time Travel debugging 困难
- 但 Clarity 是长进程，无状态不强求

**建议**：抽象 Wake/Suspend 接口（日常运行不强制无状态）。

### 4.2 差距 B：同进程执行 vs Sandbox 隔离

Anthropic：每会话独立容器。
Clarity：所有工具在同进程。

**影响**：
- 真实安全风险（与 OpenClaw 同类）
- Clarity 已部分缓解：path traversal / approval / MCP validate / XML 边界

**建议**：
- 不应强制容器化（违反本地优先定位）
- 应抽象 ToolExecutor trait，留沙箱后端扩展点
- 长期可选：wasm-sandbox / nsjail-rust

### 4.3 差距 C：messages = context vs Event Log 独立

Anthropic：Event Log 是 source of truth。
Clarity：messages 直接是 context，CompactionService 压缩。

**影响**：
- Clarity 压缩有损，事件无法重放
- Anthropic 事件可重放 → audit / time travel
- 但 CompactionService 已覆盖 80% 需求

**建议**：
- 拆分 events（append-only 事件流）+ compacted_context
- 与 ADR-007 Turn ID 注入合并设计

---

## 5. FDE = Forward Deployed Engineer

**澄清**：不是 Frontend-Driven Engineering，而是 Anthropic 招聘职位。

**职责**：
- 嵌入客户现场
- 交付 MCP servers / sub-agents / agent skills
- 类似 Palantir FDE 模式

**与 Clarity 关系**：
- 不是技术架构问题
- Clarity 可作为 FDE 工程师的本地 Agent 基础设施

---

## 6. 核心洞察

### 6.1 Anthropic 不是 Clarity 目标，是镜子

共享理念:
- Brain / Hands / Session 分层
- Session 作为 source of truth
- Vault 隔离凭证
- Sub-agents 并行
- Skills + MCP 扩展

差异:
- Anthropic: cloud-managed, 沙箱重, 计费 → Enterprise
- Clarity: local-first, 单二进制, 免费 → Individual / Small team

### 6.2 ADR-006 已部分命中 Anthropic 哲学

本会话 ADR-006 协议层收敛把 wire 协议变成单源真相 — 与 Anthropic Session 哲学异曲同工。
七条原则 P3 单源真相与 Anthropic Session 设计同源。

### 6.3 Hybrid UI = Anthropic 多前端解耦的本地版

后端统一，前端多态 = Brain 单一，Hands 可换

---

## 7. 不应照搬清单

| 项 | 拒绝理由 |
|----|------|
| Anthropic API 兼容 | Clarity 是 LLM 中立 |
| 完全无状态 Brain | 违反长进程单二进制定位 |
| Docker / 容器沙箱 | 违反无运行时依赖 |
| 0.08 USD/session-hour 计费 | Clarity 是本地免费 |
| FDE cloud runtime | 非技术架构问题 |

---

## 8. 应立即落实（短期文档）

| 项 | 内容 | 时间 |
|----|------|------|
| D1 | ADR-008 草案 | 30 min |
| D2 | architecture-positioning.md 新增章节 | 20 min |
| D3 | 本文件归档（已完成） | 30 min |

---

## 9. 应规划但暂不实施（中期）

| 项 | 内容 | 启动条件 |
|----|------|--------|
| M1 | 抽象 Wake/Suspend 接口 | S3 单源化完成 |
| M2 | 抽象 ToolExecutor trait | ADR-008 接受 |
| M3 | Event Log 模型拆分 | ADR-007 + ADR-008 都落地 |

---

## 10. Kimi 对话流程速查

1. 用户分享 B 站视频 + 要求工具深入解析
2. Kimi 调用 fetch_urls 获取视频元数据
3. Kimi 调用 web_search 三次：
   - Anthropic Managed Agents architecture 2026
   - Anthropic SessionStore managed agents
   - Anthropic FDE Frontend Driven Engineering
4. Kimi 调用 web_search 又三次：
   - Anthropic Managed Agents Martin 2026 paper arxiv
   - Anthropic Managed Agents session event log architecture
   - OpenClaw agent framework architecture
5. Kimi 综合分析输出

---

End of mapping analysis.
