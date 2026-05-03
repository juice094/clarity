# Clarity 架构锚定文档

> **目的**：防止上下文压缩后对 Kimi CLI / ZeroClaw / OpenClaw / devbase 的架构关系产生混乱。
> **维护规则**：任何架构决策变更后，必须同步更新此文件。
> **生效范围**：`dev/third_party/clarity/` 及其子目录。

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
| **上下文压缩** | 三级管道，精确 tokenizer | 加权估算（ASCII ÷4 / 非 ASCII ÷2）+ tier1/tier2 压缩 | 🟡 接近 | P2 |
| **Git 上下文** | 每次对话自动注入 | 已激活：`build_system_prompt()` 自动注入 `GitContext` + `ProjectMetadata` | ✅ 已补齐 | — |
| **子 Agent 路由** | `/coder` `/explore` `/plan` 成熟 | `/plan` 已支持；`/coder` `/explore` 待 egui 接入 | ⚠️ 中 | P2 |
| **错误恢复** | Circuit breaker + retry | 有 recoverable 判断 + Smart circuit breaker（3 次 fatal） | 🟡 接近 | P2 |
| **Skills 注入** | 动态激活，prompt 自动拼接 | `SystemPromptBuilder` 自动拼接 active skills | ✅ 已补齐 | — |
| **用户级 Skills** | `~/.config/kimi/skills` | 已支持：`~/.config/clarity/skills/` 全局扫描 | ✅ 已补齐 | — |
| **绝对路径读取** | 允许跨目录 | 已支持：绝对路径直接返回，不限制在 working_dir | ✅ 已补齐 | — |
| **扩展名优先 sniff** | `.txt` 信任扩展名 | 已支持：已知文本扩展名 bypass magic sniff | ✅ 已补齐 | — |
| **Provider 系统** | 内置多 provider | 完整 TOML 驱动 + OAuth + 本地模型 | ✅ 平 | — |
| **Approval 框架** | Interactive/Plan/Yolo | 四模式 + Smart batch grant toast + diff | ✅ 平 | — |
| **MCP 接入** | 有 | 自动连接 + 工具注册 | ✅ 平 | — |

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

**规则**：每次从 ZeroClaw 借鉴，必须在 `docs/arch-decisions/` 写一条 ADR，说明"为什么 ZeroClaw 方案不适合直接复用，Clarity 的上下文需要什么变体"。

---

## 六、当前状态（截至 2026-05-02）

### 已完成 ✅

| 模块 | 说明 |
|------|------|
| Phase 1 Claw 编译修复 | 607 tests passed |
| Agent Flow 移植 | `agent/flow/` — types + mermaid parser + runner |
| Skill 系统扩展 | `skill_type` + `flow` 字段，loader 解析 mermaid/d2 blocks |
| Agent Flow 集成 | `Agent::run_flow()` + `FlowExecutor` trait（已接入完整 `Agent::run` turn） |
| egui 单体模式 | 直接嵌入 `clarity_core::Agent`，功能完整（chat/tools/plan/skills/mcp） |
| Provider 系统 | 6 内置 provider + TOML 自定义 + OAuth |
| **工具结果回显** | `handlers/chat.rs` — `on_tool_result` 插入 `Message` 到 `session.messages` |
| **Approval 串行化** | `agent/run.rs` — `dispatch_tool_calls` 从 `join_all` 改为串行 `for` 循环 |
| **Git 上下文自动注入** | `agent/prompt.rs` — `build_system_prompt()` 消费 `refresh_context()` 收集的数据 |
| **ProjectMetadata 自动注入** | `agent/prompt.rs` — `Cargo.toml` / `package.json` 自动读取并注入 |
| **工具结果截断** | `handlers/chat.rs` — 超过 2000 字符自动截断，带 "..." 提示 |
| **Smart batch grant toast** | `main.rs` — 每帧轮询 `drain_auto_approval_notifications()` 并 toast |
| **加权 token 估算** | `compaction.rs` — ASCII ÷4 vs 非 ASCII ÷2，修复 CJK 严重低估 |
| **用户级 skill 目录** | `skills/discovery.rs` — 扫描 `~/.config/clarity/skills/` |
| **扩展名优先 sniff** | `tools/file.rs` — `.txt/.md/.rs` 等 bypass magic 检测 |
| **绝对路径读取** | `tools/helpers.rs` — 绝对路径直接返回，不限制在 working_dir |
| **Windows 不注册 bash** | `tools/mod.rs` + `registry.rs` — 条件编译，仅 PowerShell |
| **shell timeout 60s** | `tool.rs` — `ToolContext::new()` 默认 timeout 从 30s 改为 60s |

### 进行中 🔄

| 模块 | 说明 |
|------|------|
| d2.rs 解析器 | 仅 mermaid 已完整，D2 语法子集待实现 |
| 子 Agent 快捷入口 | `/coder` `/explore` 前缀路由待 egui 接入（已有 `/plan`） |
| 三级压缩管道 | 已有 tier1（截断）+ tier2（LLM 总结），budget 级待评估 |

### 已知 Bug 🔴

| Bug | 位置 | 影响 | 状态 |
|-----|------|------|------|
| ~~工具结果不回显到聊天流~~ | ~~`handlers/chat.rs`~~ | ~~用户看不到 tool 输出~~ | ✅ 已修复（commit `2f526399`） |
| ~~并发 Approval timeout~~ | ~~`agent/run.rs`~~ | ~~多工具 approval 时后面的 300s timeout~~ | ✅ 已修复（commit `2f526399`） |
| ~~Tool-calling round content 丢失~~ | ~~`agent/run.rs`~~ | ~~reasoning 不可见~~ | ✅ 已修复（Sprint 14.5） |
| Gateway 路径不调用 `refresh_context()` | `run_streaming_turn()` | Git 上下文在 Gateway 路径 stale | 🔄 遗留至 Sprint 15 |

---

## 七、演进路线图

### Phase 0：止血（已完成 ✅）
- [x] 修复工具结果不回显（Bug 1）— commit `2f526399`
- [x] 修复并发 Approval timeout（Bug 2）— commit `2f526399`
- [x] 扩展名优先 sniff — commit `64c239e5`
- [x] 允许绝对路径跨目录读取 — commit `64c239e5`
- [x] Windows 不注册 bash — commit `64c239e5`
- [x] shell timeout 60s — commit `64c239e5`

### Phase 1：UX 补齐（部分完成 🟡）
- [x] Git 上下文自动注入到每次对话 — commit `62664b0d`
- [x] ProjectMetadata 自动注入 — commit `62664b0d`
- [x] 工具结果截断（>2000 字符）— commit `62664b0d`
- [x] Smart 模式 batch grant UI 提示 — 已预存在 `main.rs`
- [ ] 子 Agent 快捷入口（`/coder` `/explore`）— 待 egui 接入子 Agent 系统

### Phase 2：上下文压缩升级（部分完成 🟡）
- [x] 加权 token 估算（ASCII ÷4 / 非 ASCII ÷2）— commit `353fccfc`
- [x] 统一 `CompactionService::estimate_tokens` 委托 — commit `353fccfc`
- [ ] 精确 tokenizer（tiktoken/cl100k_base）— 待模型感知动态选择
- [x] 三级压缩管道基础（tier1 截断 + tier2 LLM 总结）— 已有实现

### Phase 3：基础设施（部分完成 🟡）
- [x] 用户级 skill 目录 `~/.config/clarity/skills/` — commit `16f92445`
- [ ] MemoryNode 接入 egui
- [ ] MCP 配置热重载

### Phase 4：联邦化（1 个月后）
- [ ] CoreNode 拆为独立进程
- [ ] egui 通过本地 IPC/WebSocket 连接
- [ ] + MemoryNode + GatewayNode 形成单机集群

---

## 八、Hard Veto（价值观硬约束）

以下约束适用于所有在 Clarity 上的工作：

- 禁止闭源 / 云端强制 / 数据外泄
- 禁止 Docker / RAG(Qdrant) / GUI(Electron)
- 禁止项目广度 > 5 核心工具
- 本地 LLM 优先
- Rust 核心模块不可外包给子 Agent

任何方案触碰以上约束 → 立即 HALT，转交人类。
