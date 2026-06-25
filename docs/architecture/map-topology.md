---
title: 架构地图 · 拓扑层
category: Architecture
date: 2026-06-25
tags: [architecture]
---

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
        ┌─────────────────────────┼─────────────────────────┐
        │                         │                         │
┌───────▼──────┐    ┌──────────────▼──────────────┐   ┌──────▼──────┐
│clarity-gateway│    │        clarity-egui         │   │ clarity-tui │
│   (HTTP API)  │    │        （主力 GUI）          │   │  (备用 TUI) │
└───────┬───────┘    └──────────────┬──────────────┘   └──────┬──────┘
        │                           │                        │
        │    ┌──────────────────────┘                        │
        │    │                                               │
        │    │              ┌────────────────────────────────┘
        │    │              │
        │    │         ┌────▼────┐
        │    │         │clarity-wire│
        │    │         │ (协议层)  │
        │    │         └────┬────┘
        │    │              │
        └────┼──────────────┘
             │
    ┌────────▼────────┐
    │  clarity-core   │
    │   (核心运行时)   │
    └────────┬────────┘
             │
    ┌────────┴────────┬───────────────┐
    ▼                 ▼               ▼
clarity-thread-store clarity-subagents clarity-telemetry
    │
    ▼
clarity-rollout
    │
    ▼
clarity-contract
    ▲
    ├── clarity-wire
    ├── clarity-memory
    ├── clarity-mcp
    ├── clarity-openclaw
    ├── clarity-llm
    ├── clarity-tools
    ├── clarity-channels
    └── clarity-secrets

旁路 / 独立入口 crate：
  clarity-claw            → 系统托盘监控（激活中）
  clarity-headless        → 无头 CLI（激活中）
  clarity-mobile-core     → 移动端 UniFFI FFI 核心
  clarity-anthropic-proxy → Anthropic Messages API → DeepSeek 代理
  clarity-slint           → 实验性 Slint GUI（不参与默认 CI）
  clarity-tauri           → 已归档，被 workspace 排除
```

### 1.1 依赖明细

| Crate | 内部依赖 | 外部关键依赖 | 说明 |
|-------|---------|-------------|------|
| `clarity-contract` | 无 | serde, uuid, chrono | 共享契约层；零内部依赖 |
| `clarity-wire` | contract | serde, tokio | 纯协议，无业务逻辑，可被任意前端引用 |
| `clarity-memory` | contract | rusqlite, ndarray, tantivy(opt) | 独立存储层；gateway 直接引用；core 通过 factory 注入 |
| `clarity-mcp` | contract, wire | serde_json, tokio | MCP client；被 `clarity-llm` 使用 |
| `clarity-openclaw` | contract | tokio-tungstenite, ed25519-dalek | OpenClaw Gateway 客户端与设备身份 |
| `clarity-llm` | contract, mcp, memory, secrets | reqwest, candle-core(opt) | Provider 绑定层 |
| `clarity-tools` | contract, memory | regex, glob | 内置工具库；从 `clarity-core` 拆出 |
| `clarity-channels` | contract | reqwest | 外部消息通道适配器 |
| `clarity-secrets` | contract | chacha20poly1305 | 加密 Secret 存储 |
| `clarity-rollout` | contract | serde_json, tokio | JSONL rollout 持久化 |
| `clarity-thread-store` | contract, rollout | rusqlite, tokio | Thread 持久化抽象；被 `clarity-core` 使用 |
| `clarity-subagents` | core | — | 消费 `clarity-core`；子代理/团队/并行执行 |
| `clarity-telemetry` | contract | chrono, serde | 当前由 `clarity-gateway` 使用 |
| `clarity-core` | contract, wire, memory, mcp, llm, tools, channels, secrets, thread-store | tokio, reqwest | **禁止任何内部 crate 反向依赖 core**，避免循环 |
| `clarity-egui` | core, wire | eframe 0.31, egui 0.31 | 主力 GUI。禁止直接依赖 memory |
| `clarity-gateway` | core, wire, memory, telemetry | axum, tokio | HTTP API + WebSocket |
| `clarity-tui` | core, wire | ratatui, crossterm | TUI 前端 |
| `clarity-claw` | core | notify, tray-icon | 系统托盘监控 |
| `clarity-headless` | core | clap | 无头 CLI |
| `clarity-mobile-core` | core, wire, memory, contract, llm | uniffi | 移动端 UniFFI FFI 核心 |
| `clarity-anthropic-proxy` | contract, core, llm | axum | Anthropic Messages API → DeepSeek 代理 |

---

## 2. clarity-core 模块拓扑

> 说明：以下仅描述 `crates/clarity-core/src/` 内部模块。大量能力已拆分为独立 crate，见 §1.1 依赖明细。

```
agent/                    ← 运行时核心（Agent, Controller, Op, Plan）
  ├── mod.rs              → Agent 主入口
  ├── controller.rs       → AgentController / Op / ControllerEvent
  ├── driver.rs           → streaming 事件分发
  ├── execution.rs        → 工具执行 + 风险评级 + 审批触发
  ├── prompt.rs           → SystemPrompt 构建
  ├── plan.rs             → Plan 解析 + 步骤执行
  ├── ops.rs              → Agent 内部操作原语
  ├── enhanced.rs         → 增强消息 + 上下文注入
  ├── compaction_service.rs → 对话压缩服务
  ├── construct.rs        → Agent 构造器
  └── config.rs           → AgentConfig

approval/                 ← 审批运行时
  ├── mod.rs              → ApprovalRuntime trait + 数据结构
  └── rules.rs            → 风险规则引擎

background/               ← 后台任务
  ├── mod.rs              → BackgroundTaskManager
  ├── cron.rs             → Cron 调度器
  ├── worker.rs           → Worker Pool
  ├── store.rs            → 任务存储（SQLite）
  └── agent_executor.rs   → 后台运行 Agent

skills/                   ← Skill 系统
  ├── mod.rs
  ├── discovery.rs        → 扫描 .clarity/skills/
  ├── loader.rs           → SKILL.md 解析
  └── registry.rs         → Skill 注册与激活

thread/                   ← Thread / Session 生命周期集成
  └── manager.rs          → ThreadManager（消费 clarity-thread-store）

memory/                   ← 内存级记忆集成
  ├── mod.rs              → Memory trait / in-memory facade
  └── store.rs            → InMemoryStore

mcp/                      ← MCP 集成层（client 由 clarity-mcp 提供）
  ├── mod.rs              → Manager
  ├── enhanced.rs         → 增强 MCP 能力
  └── config.rs           → mcp.json 解析

ui/                       ← 跨前端共享 UI 状态机
  └── view_state.rs       → ViewState / SidePanel / ModalType / TurnState

view_models/              ← UI 视图模型
  ├── mod.rs
  └── settings.rs         → GuiSettings / SettingsEdit / Model 选择

实验性 / 演进中模块（未与主 ReAct/Plan 循环集成）：
  soul/                   → Soul / SoulManager（持久 Agent 身份）
  tier_bus/               → TierBus（层级消息总线）
  hub/                    → HubScheduler（Hub-Worker 调度器）

其他支撑模块：
  activity.rs             → 活动日志
  adaptive/               → 自适应模型路由与预测
  capability.rs           → 表面能力发现
  compaction.rs           → 压缩策略
  config.rs               → TOML 配置
  daemon.rs               → 守护进程锁
  diff.rs                 → Diff 计算与解析（re-export from clarity-tools）
  endpoint.rs             → 端点描述符抽象
  error.rs                → AgentError / ToolError
  hooks.rs                → 钩子注册表
  logging/                → 日志与脱敏
  model_download.rs       → HuggingFace 模型下载
  notifications.rs        → 通知广播
  personality.rs          → 人格/角色系统
  registry.rs             → 工具注册表（聚合 clarity-tools 与 MCP 工具）
  server.rs               → stdio server
  session/                → Session 模型
  types.rs                → 共享类型
```

### 2.1 已拆分为独立 crate 的能力

| 原 core 模块 | 现 crate | 集成方式 |
|-------------|---------|---------|
| `llm/` | `clarity-llm` | core 通过 `clarity_llm::LlmFactory` 创建 provider |
| `tools/` | `clarity-tools` | core 通过 `ToolRegistry` 注册内置工具 |
| `subagents/` | `clarity-subagents` | 消费 core；不被 core 依赖 |
| `memory/` 持久化实现 | `clarity-memory` | core 通过 factory / 注入使用 |
| MCP client transport | `clarity-mcp` | core 只做集成与配置管理 |
| Thread / Rollout | `clarity-thread-store` / `clarity-rollout` | core 通过 `thread::manager` 集成 |

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
