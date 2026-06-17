# 协议层完善后续待办

> **状态**: A1 已完成，其余待办冻结，等待 egui 布局稳定后再推进。  
> **冻结原因**: egui 布局设计仍在调整，避免引入新 UI bug。  
> **最后更新**: 2026-06-16

---

## 当前进展

- **A1 已完成**: `Agent::run_streaming` 的 callback 已删除，流式 chunk 统一走 `clarity-wire::WireMessage::ContentPart`。
- 其余任务保持待办，不引入新的 UI 改动。

---

## 待办任务（按优先级）

| 任务 | 优先级 | 依赖 | 说明 |
|------|--------|------|------|
| A2. 接入 `DraftEvent` | **P0** | A1 | 骨架已完成：`UiEvent::Draft*`、`ChatStore.draft_status`、`widgets::draft_indicator` 已接入；视觉样式待设计 |
| B1. 接入 `StatusUpdate` / `TurnBegin` / `TurnEnd` | **P1** | A2 | 精确追踪 turn 边界，提供全局状态反馈 |
| B2. 引入 `WireMessage::ViewStateUpdate` | **P1** | A2 | 让后端驱动 `ViewState.turn` 等状态同步 |
| C. Thread 管理事件驱动化 | **P2** | B2 | `Thread*` 变体接入，导航树从直接 store 操作改为事件驱动 |
| D. Gateway WebSocket 协议升级 | **P2** | B2 + C | Web IDE 接收工具卡片 / RenderLine 流 / ViewState 更新 |

---

## WireMessage 接入状态与存储对照

`clarity-wire` 共 20 个变体：

| WireMessage 变体 | 是否已映射到 UiEvent | 是否有 Handler | 已有可查询存储 | 缺口说明 |
|---|---|---|---|---|
| `TurnBegin` | ❌ | ❌ | `SessionStore.sessions[].messages` | 用户消息由前端本地插入 |
| `ContentPart` | ✅ `UiEvent::Chunk` | ✅ `chat::on_chunk` | `Session.messages[].content` / `blocks` | **A1 已完成** |
| `DraftEvent` | ✅ `UiEvent::Draft*` | ✅（骨架） | `ChatStore.draft_status` | 组件已接入，`widgets::draft_indicator` 视觉样式待设计 |
| `ToolCall` | ✅ `UiEvent::ToolStart` | ✅ `chat::on_tool_start` | `ChatStore.tool_calls[]` / `Session.messages[].blocks` | 已接入 |
| `ToolResult` | ✅ `UiEvent::ToolResult` | ✅ `chat::on_tool_result` | 同上 | 已接入 |
| `StepBegin` | ✅ `UiEvent::StepBegin` | ⚠️ 仅 `tracing` | 无独立存储 | 有事件但无 UI 反馈 |
| `TurnEnd` | ❌（由返回值触发 `UiEvent::Done`） | ✅ `chat::on_done` | `ViewState.turn = Idle` | 未直接使用 wire 标志 |
| `Usage` | ✅ `UiEvent::Usage` | ✅ `chat::on_usage` | `ChatStore.last_usage` | 已接入 |
| `StatusUpdate` | ❌ | ❌ | 无 | 无全局状态通知/Toast |
| `CompactionBegin` | ✅ `UiEvent::CompactionBegin` | ✅ `chat::on_compaction_begin` | `ViewState.turn = Compacting` | 已接入 |
| `CompactionEnd` | ✅ `UiEvent::CompactionEnd` | ✅ `chat::on_compaction_end` | `ViewState.turn = Idle` | 已接入 |
| `PlanStepBegin` | ✅ `UiEvent::PlanStepBegin` | ✅ `chat::on_plan_step_begin` | `ChatStore.plan_tracker` | 已接入 |
| `PlanStepEnd` | ✅ `UiEvent::PlanStepEnd` | ✅ `chat::on_plan_step_end` | 同上 | 已接入 |
| `PlanStepSkipped` | ✅ `UiEvent::PlanStepSkipped` | ✅ `chat::on_plan_step_skipped` | 同上 | 已接入 |
| `ThreadActive` | ❌ | ❌ | `SessionStore.active_session_id` | 直接操作 store |
| `ThreadList` | ❌ | ❌ | `SessionStore.sessions` | 直接操作 store |
| `ThreadCreated` | ❌ | ❌ | `SessionStore.sessions` | 直接操作 store |
| `ThreadUpdated` | ❌ | ❌ | `SessionStore.sessions[].archived/title` | 直接操作 store |

---

## 已有存储字段速查

| 存储 | 文件 | 关键字段 | 适用场景 |
|---|---|---|---|
| `SessionStore` | `crates/clarity-egui/src/stores/session.rs` | `sessions`、`active_session_id`、`drafts`、`active_category` | 会话列表、当前会话、草稿、按项目分组 |
| `ChatStore` | `crates/clarity-egui/src/stores/chat.rs` | `input`、`tool_calls`、`last_usage`、`plan_tracker`、`pending_send`、`agent_status` | 聊天输入、工具调用、Token 用量、计划跟踪 |
| `ProjectStore` | `crates/clarity-egui/src/stores/project.rs` | `projects`、`archived_projects`、`selected_project_id` | 项目分组、归档项目 |
| `ViewState` | `crates/clarity-core/src/ui/view_state.rs` | `turn`、`left`、`right`、`modal`、`focus`、`app_view` | 后端状态同步目标 |
| `UiStore` | `crates/clarity-egui/src/stores/ui.rs` | `toasts`、`preview_item`、`locale` | Toast 通知、预览、国际化 |
| `SubagentStore` | `crates/clarity-egui/src/stores/subagent.rs` | `batches`、`single_progress` | 子代理进度 |
| `TaskStore` / `CronStore` / `SettingsStore` | 同名文件 | 各领域状态 | 任务、定时任务、设置 |

持久化：
- **Session**: `clarity-egui/src/session.rs` 提供 `load_sessions()` / `save_session_internal()`，存 `.clarity/sessions/` JSON。
- **Project**: `ProjectStore` 当前为 UI 层 mock，待与 `clarity-core` 同步。

---

## 恢复执行时的建议

### A2. DraftEvent（骨架已完成）
- `UiEvent::DraftProgress / DraftClear / DraftContent` 已新增。
- `ChatStore.draft_status: DraftStatus` 已新增。
- `widgets::draft_indicator` 已创建，使用 theme tokens，视觉样式待设计。
- `agent_runner.rs` 已映射 `WireMessage::DraftEvent`。
- `handlers/chat.rs` 已处理三类 `UiEvent::Draft*`。
- `panels/chat/message_list.rs` 在 loading 时优先显示 draft indicator，回退到 legacy typing indicator。
- 不需要新增持久化存储。

### B1. StatusUpdate / TurnBegin / TurnEnd
- `StatusUpdate` → 复用 `UiStore.toasts` 队列。
- `TurnEnd` → 新增 `UiEvent::TurnEnd`，handler 调用现有 `chat::on_done` 逻辑。
- `TurnBegin` → 仅用于确认/埋点，不重复插入用户消息。

### B2. ViewStateUpdate
- 仅同步后端权威字段（如 `turn`）。
- 面板开关等前端本地状态不上传，避免循环。

### C. Thread 事件驱动化
- 复用 `SessionStore`，将 UI 层直接 store 操作改为事件处理函数。
- 注意与当前导航树渲染的绑定关系。

### D. Gateway WebSocket 升级
- 保留现有 `WsResponse` 变体，仅新增变体，确保旧客户端兼容。
- 提供 TypeScript 类型定义：`crates/clarity-gateway/static/types/protocol.ts`。

---

## 风险提示

- 所有改动必须等 egui 布局设计稳定后推进，避免新 UI bug。
- 每个任务独立 PR，确保 `cargo clippy` / `cargo test` / `cargo fmt` 通过。
- 新增协议变体必须同步更新 `docs/architecture/protocol-layer.md` 和 `docs/architecture/lifecycle-diagrams.md`。
