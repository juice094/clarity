# 架构地图 · 拓扑层

> 用途：确认模块边界、上下游关系、拆包可行性
> 更新触发：新增 crate、新增模块、Cargo.toml 依赖变更

---

## 1. Crate 依赖图

```
                    ┌─────────────────┐
                    │  tests/integration│
                    │   (集成测试)      │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
┌───────▼──────┐    ┌────────▼────────┐   ┌──────▼──────┐
│clarity-gateway│    │  clarity-egui   │   │ clarity-tui │
│   (HTTP API)  │    │   (主力 GUI)     │   │  (备用 TUI) │
└───────┬───────┘    └────────┬────────┘   └──────┬──────┘
        │                     │                   │
        │    ┌────────────────┘                   │
        │    │                                    │
        │    │         ┌──────────────────────────┘
        │    │         │
        │    │    ┌────▼────┐
        │    │    │clarity-wire│
        │    │    │ (协议层)  │
        │    │    └────┬────┘
        │    │         │
        └────┼─────────┘
             │
    ┌────────▼────────┐
    │  clarity-core   │
    │   (核心运行时)   │
    └────────┬────────┘
             │
    ┌────────▼────────┐
    │ clarity-memory  │
    │ (持久化记忆层)   │
    └─────────────────┘

旁路 crate（不活跃/归档）：
  clarity-claw     → CLI 入口，待激活
  clarity-headless → 无头模式，待激活
```

### 1.1 依赖明细

| Crate | 内部依赖 | 外部关键依赖 | 说明 |
|-------|---------|-------------|------|
| `clarity-core` | 无 | tokio, reqwest, serde, candle-core(opt) | **唯一真相源**。禁止其他 crate 被 core 依赖，避免循环 |
| `clarity-wire` | 无 | serde, tokio | 纯协议，无业务逻辑，可被任意前端引用 |
| `clarity-memory` | 无 | rusqlite, ndarray, tantivy(opt) | 独立存储层，gateway 直接引用；core/egui 通过 factory 间接使用 |
| `clarity-egui` | core, wire | eframe 0.31, egui 0.31 | 主力 GUI。禁止直接依赖 memory（通过 core 的 MemoryFactoryFn 注入） |
| `clarity-gateway` | core, wire, memory | axum, tokio | HTTP API + WebSocket。memory 直接引用用于 API 端点 |
| `clarity-tui` | core | ratatui, crossterm | TUI 前端。依赖少于 egui，作为 GUI 崩溃时的 fallback |

---

## 2. clarity-core 模块拓扑

```
agent/                    ← 运行时核心（Agent, Controller, Op, Plan）
  ├── controller.rs       → AgentController: 调度 Agent 生命周期
  ├── driver.rs           → 主驱动循环（streaming 事件分发）
  ├── execution.rs        → 工具执行 + 风险评级 + 审批触发
  ├── prompt.rs           → SystemPrompt 构建（8 个组件）
  ├── plan.rs             → Plan 解析 + 步骤执行
  ├── ops.rs              → Op 枚举（Agent 内部操作原语）
  ├── enhanced.rs         → 增强消息 + 上下文注入
  ├── compaction_service.rs→ 对话压缩服务
  ├── construct.rs        → Agent 构造器
  └── config.rs           → AgentConfig

approval/                 ← 审批运行时
  ├── mod.rs              → ApprovalRuntime trait + 数据结构
  └── rules.rs            → 风险规则引擎

llm/                      ← LLM 抽象层
  ├── mod.rs              → LlmProvider trait + 通用类型
  ├── model_registry.rs   → 模型注册表（Sprint 9 新增）
  ├── deepseek.rs         → DeepSeek Provider
  ├── ollama.rs           → Ollama Provider
  ├── local_gguf.rs       → 本地 GGUF Provider
  └── openai.rs           → OpenAI-compatible Provider

tools/                    ← 16 个内置工具
  ├── mod.rs              → Tool trait + 注册
  ├── file.rs             → file_read / file_write / file_edit
  ├── shell.rs            → shell / bash / powershell
  ├── web.rs              → web_search / web_fetch
  ├── web_browser.rs      → web_browser（需审批）
  ├── ask_user.rs         → ask_user
  ├── plan.rs             → plan_create / plan_list / plan_delete
  ├── todo.rs             → todo_add / todo_list / todo_complete / todo_delete
  ├── cron.rs             → schedule_cron / list_cron / cancel_cron
  ├── channel.rs          → notify_channel（钉钉/飞书/Slack/Webhook）
  ├── notify.rs           → notify（桌面通知）
  ├── search.rs           → grep / glob
  ├── think.rs            → think（思维链）
  └── team.rs             → team_create / team_list

subagents/                ← 子代理系统
  ├── mod.rs              → SubAgentManager
  ├── builder.rs          → SubAgent 构造
  ├── runner.rs           → 执行 + Git 上下文注入
  ├── parallel.rs         → 并行子代理批处理
  ├── team.rs             → 子代理团队 / Mailbox
  ├── token.rs            → 权限 Token（verify_allowed_tool 等）
  └── registry.rs         → 子代理类型注册表

background/               ← 背景任务
  ├── mod.rs              → BackgroundTaskManager
  ├── cron.rs             → Cron 调度器
  ├── worker.rs           → Worker Pool
  ├── store.rs            → 任务存储（SQLite）
  └── agent_executor.rs   → Agent 执行器（后台运行 Agent）

skills/                   ← Skill 系统
  ├── mod.rs
  ├── discovery.rs        → 扫描 .clarity/skills/
  ├── loader.rs           → SKILL.md 解析
  └── registry.rs         → Skill 注册与激活

mcp/                      ← MCP 客户端
  ├── mod.rs              → Manager
  ├── enhanced.rs         → 增强 MCP 能力
  └── config.rs           → mcp.json 解析

memory/                   ← 内存级记忆（clarity-core 内）
  ├── mod.rs              → Memory trait
  └── store.rs            → InMemoryStore

view_models/              ← UI 视图模型
  ├── mod.rs
  └── settings.rs         → GuiSettings / SettingsEdit / Model 选择

其他支撑模块：
  capability.rs           → 表面能力发现（Sprint 10）
  compaction.rs           → 压缩策略
  config.rs               → TOML 配置
  daemon.rs               → 守护进程锁
  diff.rs                 → Diff 计算与解析
  error.rs                → AgentError 枚举
  hooks.rs                → 钩子注册表
  model_download.rs       → HuggingFace 模型下载
  notifications.rs        → 通知广播
  personality.rs          → 人格/角色系统
  registry.rs             → 工具注册表
  server.rs               → stdio server（MCP 用）
  types.rs                → 共享类型（Message, ToolCall 等）
  activity.rs             → 活动日志
```

---

## 3. 数据流图（核心路径）

### 3.1 用户发送消息 → AI 回复（主路径）

```
[egui 输入] → App::send()
                │
                ▼
        [Agent::run_streaming()]
                │
        ┌───────┴───────┐
        ▼               ▼
  [Prompt 构建]    [LLM 调用]
  (8 个组件)       (LlmProvider)
        │               │
        └───────┬───────┘
                ▼
        [ToolCall?] ──Yes──→ [审批?] ──Yes──→ [UI 弹窗]
                │                       No          │
                No                        │         ▼
                │                         ▼      [执行工具]
                ▼                    [自动执行]       │
        [返回文本]                         │         ▼
                │                         ▼      [结果回写]
                ▼                    [结果回写]       │
        [Wire::broadcast]                    │         ▼
                │                            ▼      [Agent 继续]
                ▼                       [Agent 继续]
        [egui 渲染消息]
```

### 3.2 审批流程（Interactive / Smart 模式）

```
[工具执行] → execution.rs 风险评级
                │
        ┌───────┴───────┐
        ▼               ▼
    [Low-risk]      [Medium/High-risk]
        │               │
        ▼               ▼
    [自动过]      [ModeAwareApprovalRuntime]
                        │
                ┌───────┴───────┐
                ▼               ▼
            [已 batch]      [首次请求]
            grant?                │
                │                 ▼
            Yes│          [创建 ApprovalRequest]
                │                 │
                ▼                 ▼
            [自动过]      [UI wait_for_response]
                                  │
                          [用户 Approve]
                                  │
                          ┌───────┴───────┐
                          ▼               ▼
                      [Approve]      [ApproveForSession]
                          │               │
                          ▼               ▼
                      [执行工具]      [写入 batch_grants]
                      [resolve]       [执行工具]
                                          [resolve]
```

### 3.3 ensure_llm 三层解耦（Sprint 13 Tech Debt）

```
[用户选择 provider] → [llm_policy.rs] resolve_provider()
                              │
                              ▼
                      [llm_loader.rs] load_llm()
                              │
                              ▼
                      [llm_binder.rs] bind_llm()
                              │
                              ▼
                      [Agent::set_llm()]
```

---

## 4. 边界与隔离

| 边界 | 规则 | 违反后果 |
|------|------|---------|
| core ← 不允许依赖任何其他 clarity crate | core 必须自包含 | 循环依赖，编译失败 |
| egui 不允许直接依赖 memory | 通过 core::MemoryFactoryFn 注入 | 破坏分层，gateway 和 egui 耦合 |
| wire 不允许有业务逻辑 | 纯消息协议 | 污染协议层，前端被迫理解业务 |
| tools/ 每个工具独立文件 | 禁止跨工具引用 | 工具间隐式耦合，单工具移除困难 |
| approval 不允许依赖 egui | 通过 Wire 异步解耦 | 审批逻辑被 UI 框架绑架 |

---

*本文件由 AI 会话维护，人类开发者可直接编辑。Crate/模块变更需同步更新。*
