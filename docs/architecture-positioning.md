---
title: Clarity 架构定位文档（项目关系 + 边界）
category: Architecture
date: 2026-05-16
tags: [architecture]
---

# Clarity 架构定位文档（项目关系 + 边界）

> **目的**：固化 Clarity 在 Kimi CLI / ZeroClaw / OpenClaw / devbase 等周边项目中的层级、角色与禁用项。
> **维护规则**：任何"项目间关系"或"项目边界"变更后，必须同步更新此文件；纯代码/crate 拓扑变更请改 [`ARCHITECTURE.md`](ARCHITECTURE.md)。
> **生效范围**：`dev/third_party/clarity/` 及其子目录。
> **上次更新**：2026-05-11（按 §9 架构健康纪律重新整理）。

---

## 一、定位声明（不可变更，除非人类确认）

**Clarity 是 Layer 3 运行时基础设施**——全环境持久化下的跨角色认知协同引擎。

| Clarity 是 | Clarity 不是 |
|-----------|-------------|
| Agent/LLM 运行时内核 | 终端产品（不是 OpenClaw） |
| 联邦协调器（单机集群 → 分布式） | CLI 工具（不是 Kimi CLI） |
| 被嵌入的引擎（egui/Gateway/MCP） | 竞品代码库（不合并 ZeroClaw） |
| 守护进程（唯一生命周期） | oneshot 命令行工具 |

**核心原则**：Clarity 只有一个生命周期——守护进程。任何"单次调用"需求都通过外部瘦客户端（如 `clarity-cli`）向守护进程发请求解决，不允许 Clarity 内核支持 oneshot 模式。

---

## 二、五层架构

```
┌─────────────────────────────────────────────────────────────────┐
│ Layer 1: 开发工具层 (External)                                   │
│  Kimi CLI  ·  Claude Code  ·  编辑器                            │
│  关系：操作 Clarity 源码，不进入运行时                             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ 操作源码/编译
┌─────────────────────────────────────────────────────────────────┐
│ Layer 2: 产品应用层 (Product)                                    │
│  OpenClaw (TS/Node)  ·  egui GUI  ·  未来 Tauri/Web 前端         │
│  接口：Gateway HTTP / WebSocket / MCP                           │
│  职责：用户交互、多通道消息、业务编排                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ Gateway/MCP
┌─────────────────────────────────────────────────────────────────┐
│ Layer 3: Clarity 运行时（守护进程，唯一生命周期）                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ clarity-claw 联邦协调器                                  │   │
│  │  Coordinator  ·  Registry  ·  FederationRouter          │   │
│  │  CoreNode(LLM/Agent)  ·  MemoryNode  ·  GatewayNode      │   │
│  │  契约：clarity-contract (FederationMessage)              │   │
│  └─────────────────────────────────────────────────────────┘   │
│  状态：系统托盘常驻 / systemd 服务 / 后台进程                      │
│  记忆：PersistentMemoryStore（BM25 + Vector）持久化               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ MCP
┌─────────────────────────────────────────────────────────────────┐
│ Layer 4: 领域知识层 (Knowledge)                                  │
│  devbase (MCP Server)  ·  agri-paper  ·  其他 MCP Servers       │
│  职责：领域知识供给、长期记忆外部化、专业工具包                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ 只读参考
┌─────────────────────────────────────────────────────────────────┐
│ Layer 5: 参考/竞品层 (Reference)                                 │
│  ZeroClaw v0.7.4 — Rust 微内核 · 18 crates · 硬件/固件/30+通道   │
│  规则：可以抄思想，不能抄代码；可以问"ZeroClaw 怎么解决的"         │
│        不能把 zeroclaw-runtime 作为 Clarity 的依赖               │
└─────────────────────────────────────────────────────────────────┘
```

---

## 三、项目角色矩阵

| 项目 | 路径 | 层级 | 角色 | 与 Clarity 的关系 |
|------|------|------|------|------------------|
| **Clarity** | `dev/third_party/clarity/` | Layer 3 | 主战场 | 自身 |
| **OpenClaw** | `Desktop/openclaw/` 或类似 | Layer 2 | 产品参考 | 未来可通过 Gateway 对接 |
| **ZeroClaw** | `Desktop/zeroclaw-master/` | Layer 5 | 只读参考 | 独立竞品，不合并 |
| **devbase** | `dev/third_party/devbase/` | Layer 4 | MCP 知识库 | Clarity 通过 MCP 连接 |
| **Kimi CLI** | `AppData/Roaming/uv/tools/kimi-cli/` | Layer 1 | 开发工具 | 操作 Clarity 源码，不进入运行时 |
| **agri-paper** | `dev/third_party/agri-paper/` | Layer 4 | 领域配置 | 可选 MCP 接入 |

---

## 四、Kimi CLI vs Clarity：非生命周期能力差异

用户的直觉正确：两者有生命周期以外的能力差异。以下差异必须被后续 Agent 知晓。

| 维度 | Kimi CLI | Clarity egui | 差距 | 优先级 |
|------|---------|-------------|------|--------|
| **工具结果回显** | 自动插入聊天记录 | 已修复：`on_tool_result` 插入 `session.messages` | ✅ 已补齐 | — |
| **并发 Approval** | 串行处理 | 已修复：`dispatch_tool_calls` 改为串行 `for` 循环 | ✅ 已对齐 | — |
| **上下文压缩** | 三级管道，精确 tokenizer | tiktoken-rs (cl100k_base) + tier1/tier2/budget 三级压缩 | ✅ 已补齐 | — |
| **Git 上下文** | 每次对话自动注入 | 已激活：`build_system_prompt()` 自动注入 `GitContext` + `ProjectMetadata` | ✅ 已补齐 | — |
| **子 Agent 路由** | `/coder` `/explore` `/plan` 成熟 | `/plan` 已支持；`/coder` `/explore` 待 egui 接入 | ⚠️ 中 | P2 |
| **错误恢复** | Circuit breaker + retry | 有 recoverable 判断 + Smart circuit breaker（3 次 fatal） | 🟡 接近 | P2 |
| **Skills 注入** | 动态激活，prompt 自动拼接 | `SystemPromptBuilder` 自动拼接 active skills | ✅ 已补齐 | — |
| **用户级 Skills** | `~/.config/kimi/skills` | 已支持：`~/.config/clarity/skills/` 全局扫描 | ✅ 已补齐 | — |
| **绝对路径读取** | 允许跨目录 | 已支持：绝对路径直接返回，不限制在 working_dir | ✅ 已补齐 | — |
| **扩展名优先 sniff** | `.txt` 信任扩展名 | 已支持：已知文本扩展名 bypass magic sniff | ✅ 已补齐 | — |
| **Provider 系统** | 内置多 provider | 完整 TOML 驱动 + OAuth + 本地模型 | ✅ 平 | — |
| **Approval 框架** | Interactive/Plan/Yolo | 四模式 + Smart batch grant toast + diff | ✅ 平 | — |
| **MCP 接入** | 有 | 自动连接 + 工具注册 + WebSocket/SSE/HTTP/stdio 四 transport | ✅ 强于 Kimi CLI | — |

> 表中"具体 Sprint 何时完成"已迁出此文件——查阅 `AGENTS.md §Current Phase` 与 `docs/sprint-archive.md`。

---

## 五、ZeroClaw 在这个架构中的具体用法

ZeroClaw 从"待融合对象"降级为"架构对照组"。

| 当你遇到这个问题 | 去 ZeroClaw 查什么 | 带回 Clarity 什么 |
|-----------------|-------------------|------------------|
| Agent 循环设计 | `zeroclaw-runtime/src/agent/` | 设计思想（状态机 vs 协程），不是代码 |
| 30+ 通道抽象 | `zeroclaw-channels/src/` | Channel trait 设计，评估 GatewayNode 是否需要 |
| 硬件/GPIO | `zeroclaw-hardware/` | **忽略**。Clarity 无硬件层 |
| 配置系统 | `zeroclaw-config/` | TOML 加密存储思路，不必兼容 |
| 微内核 RFC | 文档 | 模块加载机制，评估 Coordinator 是否需要动态加载 |

**规则**：每次从 ZeroClaw 借鉴，必须在 `docs/adr/` 写一条 ADR，说明"为什么 ZeroClaw 方案不适合直接复用，Clarity 的上下文需要什么变体"。

---

## 五-A、与 Anthropic Managed Agents 的关系

> **更新背景（2026-05-11）**：用户分享了 Anthropic Managed Agents 架构剖析（Kimi share + 视频）。
> 详细映射分析见 [`notes/2026-05-11-anthropic-managed-agents-mapping.md`](notes/2026-05-11-anthropic-managed-agents-mapping.md)。

### 5A.1 定位关系

| 维度 | Anthropic Managed Agents | Clarity |
|------|--------------------------|---------|
| 部署形态 | Cloud-managed 托管运行时 | Local-first 单二进制守护进程 |
| 商业模型 | $0.08/session-hour + tokens | 本地免费 |
| LLM 绑定 | Anthropic Claude（managed-agents-2026-04-01 beta） | LLM 中立（OpenAI / Claude / Kimi / DeepSeek / Local GGUF） |
| 沙箱模型 | 每会话独立容器，凭证 Vault 代理 | 同进程 + Approval mode + path validation |
| 适用场景 | 企业级长任务，FDE 工程师交付现场 | 个人 / 小团队本地 Agent 基础设施 |
| 共享哲学 | Brain / Hands / Session 三层解耦 + Session 单源真相 | ✅ ADR-006 已对齐 wire 协议单源真相 |

**Anthropic Managed Agents 不是 Clarity 的竞品**，而是**架构镜子**：在不同部署约束（cloud-managed vs local-first）下，两者走出**相似但具体实现差异巨大**的路径。

### 5A.2 三层解耦理念映射

```
Anthropic 模型              Clarity 当前              一致度
────────────────            ──────────────            ──────
Harness（Brain，无状态） ←→ Agent（含状态）           ⚠️ 部分一致
Sandbox（Hands，容器隔离）←→ ToolRegistry（同进程）   ❌ 物理隔离差距大
Session（事件日志单源）   ←→ SessionStore + messages  ⚠️ messages = context
```

**Clarity 已对齐**：
- Session 持久化（SQLite + FTS5）
- Vault 等价物（`${env:VAR}` + TokenStore + RedactingWriter）
- Memory Store（clarity_memory 4 级 compaction）
- Sub-agents（SubagentOrchestrator + Team coordinator）
- MCP 三协议
- Agent Skills (markdown + YAML)
- 审批系统（ApprovalMode 4 种 — 实际比 Anthropic 更精细）

**Clarity 实质差距**：
- ❌ 无容器沙箱（与 OpenClaw 同类安全风险）
- ⚠️ Agent 有状态（无 wake/suspend 抽象）
- ⚠️ messages 直接是 context（未分离事件日志和 context window）

### 5A.3 借鉴决议（不照搬清单）

**不应该借鉴**（违反 Clarity 定位）：

| 项 | 拒绝理由 |
|----|----------|
| Anthropic API 兼容（managed-agents-2026-04-01） | LLM 中立原则 |
| 完全无状态 Brain（Harness 风格） | 违反长进程单二进制定位 |
| Docker / 容器沙箱 | 违反"无运行时依赖"定位 |
| Session-hour 计费 | 本地免费定位 |
| FDE cloud runtime | 非技术架构问题 |

**应该借鉴**（哲学层面 — 已 / 待落实）：

| 项 | 状态 | 实施路径 |
|----|------|---------|
| Session 作为单源真相 | ✅ ADR-006 已对齐 wire 协议 | ADR-006 Phase A/B/C 完成 |
| Brain / Hands 物理解耦概念 | 🟡 部分（Agent 与 ToolRegistry 已分层但同进程） | M2: 抽象 `ToolExecutor` trait，留沙箱后端扩展点 |
| Wake/Suspend 抽象 | ⏸ 待立项 | M1: 抽象 `Wake/Suspend` 接口，允许 Agent 从 SessionStore 完全重建 |
| Event Log 独立于 context window | ⏸ 待立项 | M3: Session 拆分 `events` + `compacted_context` |

### 5A.4 与 OpenClaw 的对照启示

Kimi 引用学术分析（arxiv 2603.12644v1 "A Case Study of OpenClaw"）指出 OpenClaw 存在 **Prompt Injection** 和 **RCE** 风险。

**Clarity 架构上同样暴露这类风险**，但已部分缓解：
- Path traversal protection
- MCP command validation (`validate_mcp_command`)
- Approval mode 4 种（含 Interactive 强制人工确认）
- `<tool_result>` XML 边界符（Prompt Injection 防御）
- TLS 纯 Rust（消除 OpenSSL 攻击面）
- Log credential redaction（`RedactingWriter`）

**安全模型差距评估**：Clarity 在**软件层防御**上已超过 OpenClaw 基线，但在**物理隔离**上仍是同进程模式。

**长期方向**：M2 抽象 `ToolExecutor` trait 可为未来可选的 wasm-sandbox / nsjail-rust 后端铺路，但**不强制要求**。

### 5A.5 FDE（Forward Deployed Engineer）澄清

**FDE = Anthropic 招聘的工程师职位**（不是 Frontend-Driven Engineering）：
- 嵌入客户现场推动 AI 落地
- 交付 MCP servers / sub-agents / agent skills
- 类似 Palantir 的 FDE 模式

**与 Clarity 的潜在关系**：
- Clarity 可作为 FDE 工程师的**本地 Agent 基础设施**（不依赖 Anthropic cloud）
- 提供 MCP / Skills / Sub-agents 等同等能力，但 LLM 提供方可自选
- 适合**对接 Anthropic 生态但不被绑定**的企业场景

### 5A.6 Hybrid UI = Anthropic 解耦哲学的本地实现

Clarity 的 **Hybrid UI（egui GUI + tui TUI 共享后端）** 与 Anthropic 的 **Harness 无状态可被任意进程 wake** 哲学异曲同工：

- Anthropic: Brain 单一可重启 / Hands 可换沙箱 / Session 持久
- Clarity: 后端单一 / 前端多态（GUI/TUI） / Session 持久

**后端统一，前端多态 ≈ Brain 单一，Hands 可换**

---

## 六、演进路线图（阶段性，非 Sprint 维度）

> 与 `docs/ROADMAP.md` 的关系：本节给出 **运行时形态** 的阶段演化；ROADMAP 给出 **产品交付** 的版本节奏。

### Phase 0：止血（已完成 ✅）
- [x] 修复工具结果不回显、并发 Approval timeout、扩展名优先 sniff、绝对路径跨目录读取、Windows 仅注册 PowerShell、shell timeout 60s

### Phase 1：UX 补齐（已完成 ✅）
- [x] Git 上下文 + ProjectMetadata 自动注入 `SystemPromptBuilder`
- [x] 工具结果 >2000 字符自动截断
- [x] Smart 模式 batch grant UI 提示
- [ ] 子 Agent 快捷入口（`/coder` `/explore`）— 待 egui 接入子 Agent 系统

### Phase 2：上下文压缩升级（已完成 ✅）
- [x] tiktoken-rs (cl100k_base) 精确 tokenizer
- [x] 三级压缩管道（tier1 截断 / tier2 LLM 总结 / budget 角色权重）

### Phase 3：基础设施（部分完成 🟡）
- [x] 用户级 skill 目录 `~/.config/clarity/skills/`
- [x] MemoryNode 接入 egui
- [ ] MCP 配置热重载

### Phase 4：联邦化（中长期，1+ 月）
- [ ] CoreNode 拆为独立进程
- [ ] egui 通过本地 IPC/WebSocket 连接守护进程
- [ ] CoreNode + MemoryNode + GatewayNode 形成单机集群
- [ ] FederationRouter 上升到主线

> 具体 commit hash 与 sprint 完成日期请查阅 `CHANGELOG.md` 与 `docs/sprint-archive.md`，本节只描述阶段意图。

---

## 七、Hard Veto（价值观硬约束）

以下约束适用于所有在 Clarity 上的工作：

- 禁止闭源 / 云端强制 / 数据外泄
- 禁止 Docker / RAG(Qdrant) / GUI(Electron)
- 禁止项目广度 > 5 核心工具
- 本地 LLM 优先
- Rust 核心模块不可外包给子 Agent

任何方案触碰以上约束 → 立即 HALT，转交人类。
