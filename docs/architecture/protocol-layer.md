# Clarity 协议层架构文档

> **Status**: 草案 / 待评审（基于 v0.3.4-rc 代码审计）  
> **Scope**: `clarity-wire` → `clarity-core::ui` → 前端 (egui / tui / gateway) 的完整协议栈  
> **Related ADRs**: ADR-006, ADR-007, ADR-011, ADR-012, ADR-013  
> **Authors**: Agent (architect layer)  
> **Date**: 2026-06-16

---

## 1. 协议栈分层模型

Clarity 采用**三层协议栈**实现后端到前端的通信解耦：

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│  PRESENTATION  LAYER  (前端渲染)                                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ clarity-    │  │ clarity-    │  │ clarity-    │  │ Web IDE (browser)   │  │
│  │ egui        │  │ tui         │  │ claw        │  │ via Gateway WS      │  │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         │                │                │                    │            │
│  ┌──────┴────────────────┴────────────────┴────────────────────┴──────────┐  │
│  │  UI Internal Representation (UI IR)                                    │  │
│  │  - RenderLine (13 variants)   ← ADR-012                            │  │
│  │  - ViewState (AppView/TurnState/FocusScope/ModalType...)  ← ADR-011  │  │
│  │  - CommandItem (ShortcutRegistry)                    ← ADR-013     │  │
│  │  - Message { lines: Vec<RenderLine>, blocks: Vec<RenderBlock> }    │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────────────┤
│  SEMANTIC  LAYER  (语义翻译层)                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │  clarity-core::ui                                                   │  │
│  │  - UiEvent (egui 内部枚举)                                          │  │
│  │  - markdown_to_lines() → Vec<RenderLine>                          │  │
│  │  - view_state.rs (跨前端共享状态机)                                 │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────────────┤
│  TRANSPORT  LAYER  (传输契约)                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │  clarity-wire                                                       │  │
│  │  - WireMessage (20 variants)  ← SPMC broadcast channel (tokio)      │  │
│  │  - WireSoulSide (producer)  /  WireUISide (consumer)              │  │
│  │  - Dual channel: raw (逐条) + merged (ContentPart 合并)             │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────────────┤
│  PRODUCER  LAYER  (后端生成)                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │  clarity-core::agent                                                │  │
│  │  - Agent::run() → send_wire_message(WireMessage::TurnBegin)       │  │
│  │  - run_sync_loop() → ContentPart / ToolCall / ToolResult ...        │  │
│  │  - Agent::run_streaming() → callback (双路径，待消除)               │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

**设计原则**（按 P1–P7）：
- **P3 单源真相**：每层只有一个写入点。`WireMessage` 只在 `clarity-wire` 定义；`RenderLine` 只在 `clarity-core::ui` 定义；`UiEvent` 只在前端 crate 定义。
- **P7 协议不前瞻**：`clarity-wire` 只承载当前生产路径使用的变体，禁止为"未来可能"引入协议类型。
- **P1 单向迁移**：ADR-006 已删除 `EventBus`/`EventMsg`/`ViewCommand` 传输通道，不再双向桥接。

---

## 2. WireMessage 完整变体清单与语义

### 2.1 Turn 生命周期变体（核心路径）

| 变体 | 生产者 | 消费者 | 语义 | 当前接入状态 |
|------|--------|--------|------|-------------|
| `TurnBegin { turn_id, user_input }` | `Agent::run()` | egui (丢弃) | 新回合开始，携带用户输入 | ⚠️ **未接入** — 前端未消费 |
| `ContentPart { turn_id, text }` | `run_sync_loop` / `run_streaming` | `agent_runner.rs` → `UiEvent::Chunk` | LLM 输出文本块 | ✅ 已接入 — `run_streaming` callback 已移除 |
| `DraftEvent { turn_id, event }` | `run_sync_loop` / `run_streaming` | `agent_runner.rs` → `UiEvent::Draft*` | 流式草稿生命周期（Clear/Progress/Content） | ✅ 已接入 — 组件化 widget，视觉样式待设计 |
| `ToolCall { turn_id, id, name, arguments }` | `ToolParser` | `agent_runner.rs` → `UiEvent::ToolStart` | 模型请求调用工具 | ✅ 已接入 |
| `ToolResult { turn_id, id, result }` | `ToolExecutor` | `agent_runner.rs` → `UiEvent::ToolResult` | 工具执行结果 | ✅ 已接入 |
| `StepBegin { turn_id, tool_name }` | `Agent::run_sync_loop` | `agent_runner.rs` → `UiEvent::StepBegin` | 单步执行开始 | ✅ 已接入 |
| `TurnEnd { turn_id }` | `Agent::finalize_sync_turn` | egui (丢弃) | 回合结束 | ⚠️ **未接入** — 前端靠 `UiEvent::Done` 判断 |
| `Usage { turn_id, prompt_tokens, completion_tokens, total_tokens }` | `Agent::finalize_sync_turn` | `agent_runner.rs` → `UiEvent::Usage` | Token 用量报告 | ✅ 已接入 |

### 2.2 Plan 模式变体（计划执行路径）

| 变体 | 生产者 | 消费者 | 语义 | 当前接入状态 |
|------|--------|--------|------|-------------|
| `PlanStepBegin { turn_id, step_id, tool_name }` | `PlanExecutor` | `agent_runner.rs` → `UiEvent::PlanStepBegin` | 计划步骤开始 | ✅ 已接入 |
| `PlanStepEnd { turn_id, step_id, success }` | `PlanExecutor` | `agent_runner.rs` → `UiEvent::PlanStepEnd` | 计划步骤结束 | ✅ 已接入 |
| `PlanStepSkipped { turn_id, step_id }` | `PlanExecutor` | `agent_runner.rs` → `UiEvent::PlanStepSkipped` | 步骤被用户跳过 | ✅ 已接入 |

### 2.3 系统状态变体

| 变体 | 生产者 | 消费者 | 语义 | 当前接入状态 |
|------|--------|--------|------|-------------|
| `StatusUpdate { turn_id, message }` | `Agent::run_sync_loop` | egui (丢弃) | 通用状态文本 | ⚠️ **未接入** |
| `CompactionBegin { turn_id }` | `CompactionService` | `agent_runner.rs` → `UiEvent::CompactionBegin` | 上下文压缩开始 | ✅ 已接入 |
| `CompactionEnd { turn_id }` | `CompactionService` | `agent_runner.rs` → `UiEvent::CompactionEnd` | 上下文压缩结束 | ✅ 已接入 |

### 2.4 Thread 管理变体（S6 / Phase 7 新增）

| 变体 | 生产者 | 消费者 | 语义 | 当前接入状态 |
|------|--------|--------|------|-------------|
| `ThreadActive { thread_id, title }` | `ThreadManager` | egui (丢弃) | 当前活跃 Thread 切换 | ⚠️ **未接入** — 前端直接操作 `session_store` |
| `ThreadList { threads }` | `ThreadManager` | egui (丢弃) | Thread 列表刷新 | ⚠️ **未接入** |
| `ThreadCreated { thread_id, title }` | `ThreadManager` | egui (丢弃) | 新 Thread 创建 | ⚠️ **未接入** |
| `ThreadUpdated { thread_id, title, archived }` | `ThreadManager` | egui (丢弃) | Thread 元数据更新 | ⚠️ **未接入** |

---

## 3. WireMessage → UiEvent → RenderLine 映射表

当前前端 (`clarity-egui`) 通过 `agent_runner.rs` 将 `WireMessage` 翻译为 `UiEvent`，再由 `handlers/chat.rs` 更新 `Message` 结构。`Message::prepare()` 在回合结束时将 `content` 解析为 `Vec<RenderLine>`。

**当前映射（存在缺口）**：

```text
WireMessage::TurnBegin  ──►  (丢弃)  ──►  —
WireMessage::ContentPart  ──►  UiEvent::Chunk  ──►  追加到当前 Agent message content
WireMessage::DraftEvent   ──►  UiEvent::DraftProgress / DraftClear / DraftContent  ──►  `ChatStore.draft_status` → `widgets::draft_indicator`
WireMessage::ToolCall     ──►  UiEvent::ToolStart  ──►  Message.blocks.push(ToolCall)
WireMessage::ToolResult   ──►  UiEvent::ToolResult  ──►  ChatStore.tool_calls[idx].result = ...
WireMessage::StepBegin    ──►  UiEvent::StepBegin  ──►  (无 UI 反馈)
WireMessage::TurnEnd      ──►  (丢弃)  ──►  —
WireMessage::Usage        ──►  UiEvent::Usage  ──►  状态栏显示
WireMessage::Compaction*  ──►  UiEvent::Compaction*  ──►  view_state.turn = Compacting
WireMessage::PlanStep*    ──►  UiEvent::PlanStep*  ──►  ChatStore.plan_tracker
WireMessage::StatusUpdate ──►  (丢弃)  ──►  —
WireMessage::Thread*      ──►  (丢弃)  ──►  —
```

### 3.1 推荐完整映射（目标态）

```text
WireMessage::TurnBegin { user_input }
  └──► UiEvent::TurnStart { user_input }
       └──► Message { role: User, content: user_input, lines: markdown_to_lines(user_input) }

WireMessage::ContentPart { text }  ──►  UiEvent::Chunk { text }
  └──► 如果最后一条 Message 是 Agent：
       - content.push_str(text)
       - 实时增量解析：逐行更新 lines（避免回合结束时全量 re-parse）

WireMessage::DraftEvent { Clear }  ──►  UiEvent::DraftClear
  └──► 清除 `ChatStore.draft_status`，恢复 `typing_indicator` 或等待 ContentPart

WireMessage::DraftEvent { Progress { text } }  ──►  UiEvent::DraftProgress { text }
  └──► 更新 `ChatStore.draft_status = Progress { text }`，由 `widgets::draft_indicator` 渲染

WireMessage::DraftEvent { Content { text } }  ──►  UiEvent::DraftContent { text }
  └──► 更新 `ChatStore.draft_status = Content { text }`（可选渲染，如 <think> 折叠块）

WireMessage::ToolCall { id, name, arguments }
  └──► UiEvent::ToolStart { id, name, arguments }
       └──► RenderLine::ToolCallHeader { name, status: Running, expanded: false }
       └──► 后续 ToolCallArg 行（参数展开）

WireMessage::ToolResult { id, result }
  └──► UiEvent::ToolResult { id, result }
       └──► 更新 ToolCallHeader status → Success/Warning/Error
       └──► 追加 RenderLine::CodeLine（格式化结果）或 RenderLine::Text

WireMessage::StepBegin { tool_name }
  └──► UiEvent::StepBegin { tool_name }
       └──► RenderLine::StatusLine { kind: Spinner, content: tool_name, transient: true }

WireMessage::TurnEnd { }
  └──► UiEvent::Done
       └──► Message::prepare()（最终解析）
       └──► view_state.turn = Idle

WireMessage::StatusUpdate { message }
  └──► UiEvent::StatusUpdate { message }
       └──► RenderLine::StatusLine { kind: Spinner, content: message, transient: true }
       └──► 或 Toast 通知（非持久）

WireMessage::ThreadActive { thread_id, title }
  └──► UiEvent::ThreadSwitched { thread_id, title }
       └──► session_store.active_session_id = thread_id
       └──► 导航树高亮更新

WireMessage::ThreadList { threads }
  └──► UiEvent::ThreadListUpdated { threads }
       └──► 导航树重新渲染（按 project_id 分组）

WireMessage::ThreadCreated { thread_id, title }
  └──► UiEvent::ThreadCreated { thread_id, title }
       └──► 导航树插入新节点，自动展开所在 project 分组

WireMessage::ThreadUpdated { thread_id, title, archived }
  └──► UiEvent::ThreadUpdated { thread_id, title, archived }
       └──► 导航树更新节点文本 / 移入/移出 "Archived" 分组
```

---

## 4. ViewState 同步协议

`ViewState`（`clarity-core::ui::view_state.rs`）当前**未通过 `clarity-wire` 传输**，仅由 `clarity-core` 和 `clarity-egui` 通过内存共享（同进程）。

**现状**：
- `clarity-egui` 直接修改 `app.view_state`（`TurnState`、面板开关、focus 等）。
- `clarity-tui` 同进程共享同一 `ViewState` 定义（通过 `clarity-core::ui` 引用）。
- `clarity-gateway` 的 WebSocket 不传输 `ViewState`；Web IDE 前端无法获知后端状态变化。

**建议：引入 `WireMessage::ViewStateUpdate`**

```rust
// clarity-wire/src/lib.rs — 新增变体
pub enum WireMessage {
    // ... existing variants ...

    /// ViewState delta update — only carries changed fields to minimize payload.
    ViewStateUpdate {
        turn: Option<TurnState>,
        left: Option<Option<SidePanel>>,
        right: Option<Option<SidePanel>>,
        modal: Option<Option<ModalType>>,
        focus: Option<FocusScope>,
        // ... other fields as needed
    },
}
```

**使用场景**：
1. **Gateway WebSocket**：当后端 Agent 状态变化（Loading → Compacting → Idle），通过 `WireMessage::ViewStateUpdate { turn: Some(Compacting) }` 推送给 Web IDE。
2. **claw（系统托盘）**：监听 `ViewStateUpdate` 更新托盘图标状态（忙碌/空闲/错误）。
3. **多前端一致性**：egui 和 tui 同时运行时，通过 wire 同步 view state（而非当前直接内存共享）。

**接入条件**：
- `clarity-core::Agent` 在 `turn` 状态变化时调用 `send_wire_message(ViewStateUpdate)`。
- `clarity-gateway::ws` 将 `ViewStateUpdate` 透传给 WebSocket 客户端。
- Web IDE 前端（JavaScript/TypeScript）定义对应的 `ViewStateUpdate` 类型并更新 React/Vue 状态。

---

## 5. Gateway WebSocket 协议扩展

当前 `clarity-gateway::ws` 的 `WsResponse` 只暴露了 5 个变体：`Welcome`、`Chat`、`Pong`、`History`、`Error`。这不足以支撑 Web IDE 的完整功能。

### 5.1 建议的 WsResponse 扩展

```rust
// crates/clarity-gateway/src/ws.rs
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum WsResponse {
    // --- 现有变体 ---
    Welcome { session_id: String, message: String },
    Chat { message: String, tool_calls: Option<Vec<ToolCall>> },
    Pong,
    History { messages: Vec<ChatMessage> },
    Error { error: String },

    // --- 新增：WireMessage 透传层 ---
    WireMessage {
        /// 原始 WireMessage JSON payload.
        payload: serde_json::Value,
    },

    // --- 新增：ViewState 同步 ---
    ViewState {
        /// 完整或增量 ViewState JSON.
        delta: serde_json::Value,
        /// true = 完整状态替换; false = 增量合并.
        is_full: bool,
    },

    // --- 新增：RenderLine 流（替代纯文本 Chunk）---
    RenderLines {
        /// 新增/替换的 RenderLine 数组.
        lines: Vec<serde_json::Value>,
        /// 目标 message index in the session.
        message_index: usize,
        /// true = 最终渲染; false = 增量追加.
        is_final: bool,
    },
}
```

### 5.2 客户端接入建议（Web IDE）

```typescript
// 前端 TypeScript 类型定义（建议放入 web-ide/src/types/protocol.ts）

interface WsResponse {
  type: 'welcome' | 'chat' | 'pong' | 'history' | 'error'
       | 'wire_message' | 'view_state' | 'render_lines';
}

interface WireMessagePayload {
  type: 'turn_begin' | 'content_part' | 'draft_event' | 'tool_call' | 'tool_result'
      | 'turn_end' | 'usage' | 'status_update' | 'compaction_begin' | 'compaction_end'
      | 'plan_step_begin' | 'plan_step_end' | 'plan_step_skipped'
      | 'thread_active' | 'thread_list' | 'thread_created' | 'thread_updated';
  turn_id?: string;
  // ... variant-specific fields
}

interface ViewStateDelta {
  turn?: 'idle' | 'loading' | 'compacting' | 'stopping' | 'restoring';
  left?: string | null;
  right?: string | null;
  modal?: string | null;
  focus?: { kind: string; value?: string };
}

// 消息处理器路由
function handleWsMessage(msg: WsResponse) {
  switch (msg.type) {
    case 'wire_message':
      const wire = msg.payload as WireMessagePayload;
      dispatchWireMessage(wire);   // 映射到 React store
      break;
    case 'view_state':
      const delta = msg.delta as ViewStateDelta;
      updateViewState(delta, msg.is_full);
      break;
    case 'render_lines':
      appendRenderLines(msg.message_index, msg.lines, msg.is_final);
      break;
    // ... existing cases
  }
}
```

---

## 6. 前端功能接入检查清单

基于 `crates/clarity-egui/src/services/agent_runner.rs` 和 `handlers/chat.rs` 的代码审计：

| WireMessage 变体 | 前端对应功能 | 当前是否接入 | 缺口说明 | 优先级 |
|-----------------|-------------|------------|---------|--------|
| `TurnBegin` | 用户消息渲染 | ❌ 未接入 | 用户输入已直接插入 `Message`，不依赖 wire | P2 |
| `ContentPart` | Agent 流式输出 | ✅ 已接入 | `agent_runner.rs` 映射为 `UiEvent::Chunk` | — |
| `DraftEvent` | 思考过程/进度指示 | ✅ 已接入 | `widgets::draft_indicator` 已接入，视觉样式待设计 | — |
| `ToolCall` | 工具调用卡片 | ✅ 已接入 | `UiEvent::ToolStart` → `ContentBlock::ToolCall` | — |
| `ToolResult` | 工具结果展示 | ✅ 已接入 | `UiEvent::ToolResult` → `ChatStore.tool_calls` | — |
| `StepBegin` | 单步状态栏 | ⚠️ 部分接入 | 只发 `UiEvent`，无 RenderLine 映射 | P1 |
| `TurnEnd` | 回合结束标志 | ❌ 未接入 | 靠 `UiEvent::Done` 间接判断 | P2 |
| `Usage` | Token 用量显示 | ✅ 已接入 | 状态栏显示 | — |
| `StatusUpdate` | 全局状态通知 | ❌ 未接入 | 无 Toast/StatusLine 映射 | P1 |
| `CompactionBegin/End` | 压缩动画 | ✅ 已接入 | `view_state.turn` 切换 | — |
| `PlanStep*` | 计划执行跟踪 | ✅ 已接入 | `ChatStore.plan_tracker` | — |
| `ThreadActive` | 会话切换高亮 | ❌ 未接入 | 前端直接操作 `session_store` | P2 |
| `ThreadList` | 导航树刷新 | ❌ 未接入 | 前端直接操作 `session_store` | P2 |
| `ThreadCreated` | 新会话插入 | ❌ 未接入 | 前端直接操作 `session_store` | P2 |
| `ThreadUpdated` | 会话归档/重命名 | ❌ 未接入 | 前端直接操作 `session_store` | P2 |

---

## 7. 实现建议

### 7.1 短期（当前 Sprint）

1. **消除 `ContentPart` 双路径**（ADR-006 遗留）：
   - 删除 `Agent::run_streaming` 的 `callback` 参数。
   - 将 `run_sync_loop` 的 `ContentPart` 发送改为走 `WireMessage::ContentPart`。
   - `agent_runner.rs` 将 `ContentPart` 映射到 `UiEvent::Chunk`。

2. **接入 `DraftEvent`**：
   - 在 `Agent::run()` 开始时发送 `WireMessage::DraftEvent { event: Progress { text: "thinking...".into() } }`。
   - `agent_runner.rs` 映射为 `UiEvent::DraftProgress` / `UiEvent::DraftClear`。
   - egui 在 `MessageBubble` 中显示进度指示器（`RenderLine::StatusLine { kind: Spinner }`）。

3. **接入 `StatusUpdate`**：
   - 将 `run_sync_loop` 中的状态文本发送改为 `WireMessage::StatusUpdate`。
   - `agent_runner.rs` 映射为 `UiEvent::StatusUpdate` → `RenderLine::StatusLine` 或 Toast。

### 7.2 中期（下一 Sprint）

4. **引入 `WireMessage::ViewStateUpdate`**：
   - 在 `Agent` 的 `turn` 状态切换点（`Loading` → `Compacting` → `Idle`）发送 `ViewStateUpdate`。
   - Gateway 透传给 WebSocket。
   - Web IDE 前端接入 `ViewState` 同步。

5. **Thread 管理变体接入**：
   - `ThreadManager` 在创建/切换/更新时发送对应 `WireMessage`。
   - `agent_runner.rs` 映射为 `UiEvent`。
   - egui 导航树监听这些事件（替代直接内存操作）。

### 7.3 长期（v0.4.0+）

6. **Gateway 协议升级**：
   - 将 `WsResponse` 扩展为支持 `WireMessage` 透传、`ViewState` 同步、`RenderLine` 流。
   - Web IDE 前端实现完整的 `RenderLine` 渲染器（HTML/CSS 对应 egui 的 `MessageBubble`）。

7. **跨前端 IR 提取**（ADR-006 Phase D）：
   - 若 `clarity-tui` 继续维护，提取 `clarity-frontend-ir` crate 共享 `ViewCommand`/`UserAction`。
   - 若 tui 退役，将 `protocol_renderer.rs` 迁移为直接 `ratatui` 调用，无需 IR crate。

---

## 8. 参考文献

- `docs/adr/ADR-006-protocol-layer-convergence.md` — 协议层收敛决策
- `docs/adr/ADR-007-turn-id-in-wiremessage.md` — Turn ID 注入
- `docs/adr/ADR-011-workspace-architecture.md` — 工作区架构
- `docs/adr/ADR-012-renderline-enum-design.md` — RenderLine 设计
- `docs/adr/ADR-013-keyboard-shortcuts-claudecode-inspired.md` — FocusScope
- `docs/development/CODE-CHANGE-PRINCIPLES.md` — P1–P7 工程原则
- `crates/clarity-wire/src/lib.rs` — WireMessage 定义
- `crates/clarity-core/src/ui/render_line.rs` — RenderLine 定义
- `crates/clarity-core/src/ui/view_state.rs` — ViewState 定义
- `crates/clarity-egui/src/services/agent_runner.rs` — 前端 WireMessage 消费
- `crates/clarity-egui/src/handlers/chat.rs` — UiEvent 处理
- `crates/clarity-gateway/src/ws.rs` — WebSocket 协议

---

*最后更新：2026-06-16*
