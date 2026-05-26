---
title: 架构地图 · 影响层
category: Architecture
date: 2026-05-16
tags: [architecture]
---

# 架构地图 · 影响层

> 用途：改代码前快速确认"改了这里，哪里会炸"
> 使用方式：Ctrl+F 搜模块名，查看下游检查清单

---

## 影响矩阵格式

```
模块名
├── 直接下游（编译期依赖）
├── 运行时消费者（通过 Wire / HTTP / 事件）
├── 契约关联（共享 enum / trait）
└── 必须运行的验证命令
```

---

## Agent 运行时

### `agent/mod.rs` — Agent 构造与 API

- **直接下游**：`clarity-egui::app_logic`, `clarity-gateway::handlers`, `clarity-tui::main`, `tests/integration`
- **运行时消费者**：`background::agent_executor`（后台任务启动 Agent）
- **契约关联**：`LlmProvider`, `ApprovalRuntime`, `MemoryFactoryFn`
- **验证**：
  ```bash
  cargo test --workspace --lib
  cargo test --test integration
  ```

### `agent/controller.rs` — AgentController

- **直接下游**：`clarity-egui::app_logic`, `clarity-gateway::handlers`
- **契约关联**：`Wire`（ControllerEvent 序列化）
- **注意**：`interrupt()` / `stop()` 行为变更需手动测试 egui 的 "停止" 按钮

### `agent/driver.rs` — Streaming 事件驱动

- **直接下游**：无（被 controller 内部调用）
- **运行时消费者**：`clarity-egui::process_events`, `clarity-gateway::ws`, `clarity-tui::render`
- **契约关联**：`WireMessage`（每新增/删除一种事件，三端 UI 需同步）
- **验证**：
  ```bash
  cargo test --workspace --lib agent::
  # 手动：启动 egui，发送消息，观察 streaming 渲染是否正常
  ```

### `agent/execution.rs` — 工具执行 + 风险评级

- **直接下游**：`agent/driver.rs`
- **运行时消费者**：`approval::ModeAwareApprovalRuntime`（审批请求创建）
- **契约关联**：`Tool` trait, `ApprovalMode`, `RiskLevel`
- **注意**：改风险规则（如 file_write 从 Medium 改 High）→ Smart 模式行为变，需同步 `approval::rules.rs` 测试

### `agent/prompt.rs` — SystemPrompt 构建

- **直接下游**：`agent/driver.rs`
- **运行时消费者**：LLM（Prompt 内容变更属于语义变更，无编译检查）
- **注意**：改 Prompt → 可能影响 Agent 行为，需端到端验证；`ApprovalMode::Smart` 的 Prompt 描述需与 `approval` 模块逻辑一致

### `agent/plan.rs` — Plan 解析与执行

- **直接下游**：`tools::plan`（Plan 工具内部调用）
- **运行时消费者**：`clarity-egui::render`（PlanStepBegin/End 事件）
- **注意**：改 Plan JSON schema → `tools::plan` 和前端渲染需同步

### `agent/ops.rs` — Op 枚举

- **直接下游**：`agent/driver.rs`, `agent/execution.rs`
- **契约等级**：P1（内部契约）
- **注意**：增删 Op 变体 → driver 和 execution 必须同步处理新分支

---

## 审批系统

### `approval/mod.rs` — ApprovalRuntime Trait + 数据结构

- **直接下游**：`InMemoryApprovalRuntime`, `ModeAwareApprovalRuntime`, `MockApprovalRuntime`（测试中）
- **运行时消费者**：`clarity-egui::panels::approval`（UI 弹窗）, `clarity-gateway::handlers::admin`
- **契约关联**：`ApprovalMode` enum, `ApprovalResponse` enum
- **验证**：
  ```bash
  cargo test --workspace --lib approval::
  ```

### `approval/rules.rs` — 风险规则引擎

- **直接下游**：`agent/execution.rs`
- **注意**：改规则 → 影响所有工具调用的审批触发逻辑。必须跑 `approval::tests` 和 `agent::tests`（含 tool_call_approval_flow）

---

## LLM 层

### `llm/mod.rs` — LlmProvider Trait

- **直接下游**：6 个 Provider 实现 + `MockLlm`
- **运行时消费者**：`agent/mod.rs`（Agent 持有 `Arc<dyn LlmProvider>`）
- **契约等级**：P0
- **验证**：
  ```bash
  cargo test --workspace --lib llm::
  ```

### `llm/model_registry.rs` — ModelRegistry

- **直接下游**：`view_models::settings::get_available_models`, `app_state::ensure_llm`
- **运行时消费者**：`clarity-egui` Settings 面板模型下拉框
- **注意**：改 registry 数据格式 → `get_available_models()` 合并逻辑可能出错，需验证 Settings 面板模型列表是否非空

### `llm/local_gguf.rs` — 本地模型加载

- **直接下游**：无（被 `llm_loader.rs` 动态调用）
- **注意**：改 chat template 解析 → 影响 deepseek-r1 / qwen2 格式，需跑 `local_gguf::tests`

---

## Wire 协议

### `clarity-wire/src/lib.rs` — WireMessage / Wire

- **直接下游**：`clarity-core::agent::driver`（发送）, `clarity-egui::process_events`（接收）, `clarity-gateway::ws`（接收/转发）, `clarity-tui::render`（接收）
- **契约等级**：P0
- **注意**：
  - 新增变体 → 三端 UI 需处理（否则事件被静默丢弃）
  - 删除变体 → breaking change
  - 改字段 → breaking change
- **验证**：
  ```bash
  cargo test --workspace --lib
  # 手动：启动 egui，触发对应事件，观察渲染
  ```

---

## 工具层

### `tools/mod.rs` — Tool Trait + 注册表

- **直接下游**：16 个内置工具 + MCP 动态工具
- **运行时消费者**：`agent/execution.rs`
- **契约等级**：P0
- **验证**：
  ```bash
  cargo test --workspace --lib tools::
  ```

### `tools/file.rs` — 文件工具

- **直接下游**：无（通过注册表动态调用）
- **运行时消费者**：`approval::rules.rs`（敏感文件检测）, `diff.rs`（file_edit diff 预览）
- **注意**：改路径解析逻辑 → 影响 `resolve_path` 测试 + 安全边界；改 diff 格式 → 影响 `diff.rs` 解析

### `tools/shell.rs` — Shell 工具

- **注意**：Windows 下是 PowerShell，Linux/mac 下是 bash。改命令执行逻辑 → 需双平台验证。

### `tools/web_browser.rs` — 浏览器工具

- **注意**：需要审批（`requires_approval: true`）。改行为 → 需同步 `approval::rules.rs`。

---

## 子代理层

### `subagents/mod.rs` — SubAgentManager

- **直接下游**：`subagents::builder`, `subagents::runner`, `subagents::parallel`
- **运行时消费者**：`tools::task`（task 工具内部 spawn 子代理）
- **注意**：改 spawn 接口 → `tools::task.rs` 和 `background::agent_executor` 需同步

### `subagents/token.rs` — 权限 Token

- **直接下游**：`subagents::runner`（执行前校验）
- **注意**：改权限规则 → 影响所有子代理的执行能力，需跑 `token::tests`

---

## 背景任务层

### `background/mod.rs` — BackgroundTaskManager

- **直接下游**：`background::cron`, `background::worker`, `background::store`
- **运行时消费者**：`tools::cron`（schedule_cron / list_cron / cancel_cron）
- **注意**：改任务存储 schema → 需 migration（SQLite `PRAGMA user_version`）

---

## egui 前端

### `clarity-egui/src/main.rs` — App 定义 + eframe 入口

- **直接下游**：`eframe::run_native`
- **运行时消费者**：用户输入、Window 事件
- **注意**：改 `App` struct 字段 → 所有 `app.xxx` 引用点需同步；改 `eframe` 版本 → 全 UI 可能编译错误

### `clarity-egui/src/app_logic.rs` — 核心逻辑

- **直接下游**：`main.rs`（被 `update()` 调用）
- **运行时消费者**：`clarity-core::Agent`（发送消息）, `Wire`（接收事件）
- **注意**：改 `send()` 逻辑 → 影响 draft persistence、steer mode、附件处理；改 `new_session()` → 影响 draft save/restore

### `clarity-egui/src/app_state.rs` — AppState + ensure_llm

- **直接下游**：`app_logic.rs`, `panels::settings`, `panels::chat`
- **注意**：改 `ensure_llm` 三层结构 → 需同步 `llm_policy.rs` / `llm_loader.rs` / `llm_binder.rs` 测试

### `clarity-egui/src/settings.rs` — Settings 逻辑

- **直接下游**：`app_state.rs`, `view_models::settings`
- **注意**：改保存格式 → 可能破坏旧版 `gui-settings.json`，需验证增量 merge 逻辑

---

## gateway

### `clarity-gateway/src/handlers/*.rs` — HTTP handlers

- **直接下游**：`axum` 路由
- **运行时消费者**：HTTP 客户端（外部）
- **注意**：改 API 响应格式 → 外部客户端可能崩溃；改 admin 审批接口 → 需同步 `approval` 模块

---

## 通用安全规则

1. **改 P0 契约** → 全 workspace 编译检查 `cargo test --workspace --lib`
2. **改工具** → `cargo test --workspace --lib tools::`
3. **改审批** → `cargo test --workspace --lib approval::`
4. **改 Agent 流程** → `cargo test --workspace --lib agent::`
5. **改 UI** → 无自动 UI 测试，必须手动启动验证
6. **改配置/设置** → 验证旧配置文件的向后兼容

---

*本文件由 AI 会话维护。新增模块时需追加影响条目。*
