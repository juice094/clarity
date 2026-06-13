---
title: ADR-006: Protocol Layer Convergence (Triple → Single)
category: ADR
tags: [adr, protocol]
---

# ADR-006: Protocol Layer Convergence (Triple → Single)

> Status: Accepted
> Date: 2026-05-11
> Deciders: juice094 + Agent (architect layer)
> Affects: `clarity-wire`, `clarity-core`, `clarity-egui`, `clarity-tui`, `clarity-claw`, `clarity-gateway`
> Supersedes: 无
> Relates to: `docs/development/CODE-CHANGE-PRINCIPLES.md` (P3 单源真相, P7 协议不前瞻)

---

## 1. Context

`clarity-wire` 当前**并存三套协议**，按引入时间排序：

| 代际 | 类型 | 状态 | 生产消费者 |
|------|------|------|----------|
| Gen-1 | `WireMessage` (broadcast SPMC, 14 variants) | ✅ 生产路径在用 | core/tools 写入 + tui/egui/claw 读取 |
| Gen-2 | `EventMsg` + `EventBus` + `Event` (Codex-inspired) | ⚠️ 死路径 | `Agent::with_event_bus` 已挂载，**egui/gateway 均未 subscribe** |
| Gen-3 | `ViewCommand` + `UserAction` + `view_sender` (declarative UI) | ⚠️ 死路径 | `SettingsViewModel::sync_to_wire` 已 emit，**前端无 `ui_view_side()` 订阅** |

### 1.1 直接证据

```rust
// clarity-egui/src/stores/mod.rs:93
pub struct SettingsStore {
    pub settings_edit: GuiSettings,                       // ← 直接编辑
    #[allow(dead_code)]
    pub settings_vm: SettingsViewModel,                   // ← 死代码
    ...
}

// clarity-egui/src/services/agent_runner.rs:289
let wire = Arc::new(clarity_wire::Wire::new());           // ← 每次发送新建
let agent = state.agent.clone().with_wire(wire.clone());

// agent_runner.rs:334 (wire 事件翻译)
match msg {
    WireMessage::ToolCall { .. } => ...,
    WireMessage::ToolResult { .. } => ...,
    WireMessage::StepBegin { .. } => ...,
    // ...
    _ => None,                                            // ← ContentPart/TurnBegin/TurnEnd 被丢弃
}

// agent_runner.rs:346
let result = agent.run_streaming(&query, move |chunk| {
    tx_chunk.send(UiEvent::Chunk(chunk.to_string()))     // ← 流式正文走另一路 callback
})
```

### 1.2 痛点

1. **维护税**：新增一个语义事件需要在 `WireMessage` + `EventMsg` + `From<WireMessage> for EventMsg`
   + `UiEvent` + 翻译层 + 分发层 ≥ 6 处同步修改。
2. **双路径流式风险**：`ContentPart` 同时被 wire（合并 buffer）和 callback（逐 token）发送，
   合并粒度不同 → 持久化历史和实时显示可能不一致。
3. **Wire 实例错位**：`Wire::new()` 在每个 turn 重建，旧 turn 的 `wire_ui.recv()` task
   生命周期与新 Wire 不重叠 → 事件穿插无 turn_id 区分。
4. **幻象建筑**：`ViewCommand` / `EventBus` 提高了认知门槛（新人误以为参与生产路径），
   却无产出价值。
5. **测试虚高**：`view_channel_basic` / `view_channel_broadcast` 等 4 个测试覆盖的是
   零订阅的孤儿通道。

### 1.3 已知 ViewCommand 真实消费者（2026-05-11 复核新增）

初版本 ADR 草案声称 `ViewCommand` 完全无消费者。**事实校正**：
`clarity-tui` 含一个真实的 `protocol_renderer.rs` 把 `Vec<ViewCommand>` 渲染为
ratatui 元素：

```rust
// crates/clarity-tui/src/protocol_renderer.rs:1
use clarity_wire::{ButtonStyle, UserAction, ViewCommand};

pub fn render_view_commands(
    f: &mut Frame, area: Rect, commands: &[ViewCommand]
) -> Vec<UserAction> { ... }
```

`cached_view_commands` 在 `clarity-tui::App` 内被 `SettingsViewModel::commands()`
本地填充（**不经过** `Wire::view_sender`）。

**结论**：
- **传输通道**（`view_sender` / `ui_view_side` / `send_view` / `WireUIViewSide`）确实零订阅 → ADR-006 原决议成立
- **IR 类型**（`ViewCommand` / `UserAction` / `TextRole` / `ButtonStyle`）有 tui 这一个真实消费者
- **TUI 不依赖 wire 传输**，只依赖类型定义

这意味着 Phase D 要做的不是简单地把类型移入 `clarity-egui::view`，
而是要**抽出一个跨前端共享的 IR 层**（建议方案见 §3 Phase D 修订）。

---

## 2. Decision

**保留 `WireMessage` 作为唯一的跨进程 / 跨 crate 传输契约，移除其他两套。**

具体决议：

| 类型 | 决议 | 处理方式 |
|------|------|--------|
| `WireMessage` (Gen-1) | **保留** | 加入 `turn_id: u64` 字段（破坏性变更，ADR-007 单独决议） |
| `EventMsg` / `Event` / `EventBus` (Gen-2) | **删除** | 整体下线（零外部消费者） |
| `Wire::view_sender` / `ui_view_side` / `send_view` / `WireUIViewSide` (Gen-3 transport) | **删除** | 零订阅 |
| `ViewCommand` / `UserAction` / `TextRole` / `ButtonStyle` (Gen-3 types) | **迁移** | 见 Phase D — `clarity-tui` 真实消费，先标记弃用、Phase D 抽出 `clarity-frontend-ir` |
| `DraftEvent`（在 `WireMessage::DraftEvent` 内部） | **保留** | 已被 producer 使用 |
| `WireMessage::ContentPart` callback 双路径 | **消除** | 流式正文统一走 wire，废弃 `run_streaming(query, callback)` 的 closure 参数 |

### 2.1 设计原则

按 `CODE-CHANGE-PRINCIPLES.md`：
- **P1 单向迁移**：删除即删除，不留 `From<EventMsg> for WireMessage` 桥接
- **P3 单源真相**：协议契约只在 `clarity-contract` + `clarity-wire`，UI IR 只在前端 crate
- **P7 协议不前瞻**：禁止再次引入"为了未来"的协议层

---

## 3. Phases

### Phase A — Deprecation Markers（本 ADR 同 PR 落地，零功能变更）

为待删除项添加 `#[deprecated(since = "0.3.1", note = "see ADR-006")]`：

| 目标 | 文件 | 标记类型 |
|------|------|---------|
| `clarity_wire::Event` | `crates/clarity-wire/src/event.rs:18` | type-level |
| `clarity_wire::EventMsg` | `crates/clarity-wire/src/event.rs:45` | type-level |
| `clarity_wire::EventBus` | `crates/clarity-wire/src/event.rs:140` | type-level |
| `clarity_wire::ViewCommand` | `crates/clarity-wire/src/lib.rs:664` | type-level |
| `clarity_wire::UserAction` | `crates/clarity-wire/src/lib.rs:706` | type-level |
| `clarity_wire::TextRole` | `crates/clarity-wire/src/lib.rs:645` | type-level |
| `clarity_wire::ButtonStyle` | `crates/clarity-wire/src/lib.rs:654` | type-level |
| `Wire::ui_view_side` | `crates/clarity-wire/src/lib.rs:280` | method-level |
| `Wire::view_receiver_count` | `crates/clarity-wire/src/lib.rs:308` | method-level |
| `WireSoulSide::send_view` | `crates/clarity-wire/src/lib.rs:444` | method-level |
| `Agent::with_event_bus` | `crates/clarity-core/src/agent/construct.rs:315` | method-level |

**输出**：构建产生 deprecation warnings（不破坏现有 build）。
**Exit criteria**: `cargo build --workspace` 通过；warning count = deprecation 标记总数。

### Phase B — Producer Removal（next sprint）

删除 producer 端调用：

1. `clarity-core::view_models::settings::SettingsViewModel::sync_to_wire()` → 删除
2. `clarity-core::agent::construct::Agent::with_event_bus()` → 删除
3. `clarity-core::agent::mod.rs::event_bus: Option<EventBus>` → 删除

**前置条件**：grep 确认无外部 caller。

### Phase C — Type Removal（next+1 sprint）

删除 `clarity-wire` 中的类型：

1. 整个 `event.rs` 文件删除
2. `lib.rs` 中 `view_sender: broadcast::Sender<Vec<ViewCommand>>` → 删除
3. `WireUIViewSide` struct → 删除
4. `TextRole` / `ButtonStyle` / `ViewCommand` / `UserAction` → 删除
5. `pub mod event; pub use event::*;` → 删除

**前置条件**：Phase B 已合并；workspace 编译通过。

### Phase D — Frontend IR Relocation（修订：跨前端共享 IR，next+2 sprint）

**修订原因**：§1.3 发现 `clarity-tui::protocol_renderer` 真实消费 `ViewCommand`。
不能简单地把类型移入 `clarity-egui::view`。

**新方案**：抽出 `clarity-frontend-ir` 独立 crate，由 egui / tui 共同依赖：

```text
crates/clarity-frontend-ir/             # 新 crate
├── src/lib.rs                          # ViewCommand / UserAction / TextRole / ButtonStyle
├── src/interpreter/egui.rs (feature)   # egui 翻译适配（可选 feature）
└── src/interpreter/ratatui.rs (feature) # ratatui 翻译适配（可选 feature）

依赖关系：
  clarity-egui ──┐
                 ├──→ clarity-frontend-ir   (no clarity-wire dep)
  clarity-tui ───┘

clarity-wire 不再含 IR 类型，回归"传输层"单一职责。
```

**选择依据**：
- 让 wire 回归"传输契约"单一职责（P3 单源真相）
- 让 frontend IR 由前端 crate 共享，遵循"领域驱动"
- `clarity-core::view_models::settings` 改为 `SettingsViewModel::commands() -> Vec<frontend_ir::ViewCommand>`

**备选方案**（若 tui 即将退出维护）：
直接把 `ViewCommand` 移入 `clarity-egui::view`，
让 tui 改写 `protocol_renderer` 为直接 ratatui 调用。

最终决策推迟到 Phase D 启动时（基于届时 tui 的存活预期）。

**前置条件**：Phase B + Phase C 已合并；workspace 编译通过。

### Phase E — Turn ID Injection（独立 ADR-007）

为 `WireMessage` 引入 `turn_id`，消除事件穿插。**不在本 ADR 范围**，将作为 ADR-007 单独决议。

---

## 4. Consequences

### Positive

- **维护税**：新增语义事件从 ≥ 6 处同步降至 ≤ 3 处（WireMessage + UiEvent + 处理器）
- **认知门槛**：新人无需理解三套协议的关系
- **测试基线**：`clarity-wire` 测试从 37 减到约 25（删除 12 个孤儿通道测试），但**所有测试覆盖生产路径**
- **二进制大小**：删除 ~ 400 行 wire 代码 + 200 行死代码 → ~ 50KB 减少

### Negative

- **回退成本**：如未来真的需要声明式 UI 协议，需重新设计（但有 ADR-006 决议作为参考）
- **下游影响**：无（grep 确认 EventBus / ViewCommand 无外部下游消费者）

### Neutral

- `WireMessage` 的 broadcast capacity (1024) 不变
- merge_buffer 逻辑不变

---

## 5. Migration Guide for Maintainers

### 如果你看到 deprecation warning

```text
warning: use of deprecated struct `clarity_wire::EventBus`:
         see ADR-006: removed in favor of WireMessage broadcast
```

→ 不需要立即处理。Phase B 会统一删除 producer。

### 如果你需要新增一个语义事件

**之前**（错误）：
```rust
// clarity-wire/src/lib.rs
pub enum WireMessage { ... NewEvent { ... } }
// clarity-wire/src/event.rs
pub enum EventMsg { ... NewEvent { ... } }
// + 5 处同步修改
```

**之后**（ADR-006 后）：
```rust
// clarity-wire/src/lib.rs (唯一源)
pub enum WireMessage { ... NewEvent { ... } }
// + 在 producer 处 send()
// + 在 consumer 处 match (egui::services::agent_runner)
// 共 3 处修改
```

### 如果你需要前端 declarative panel

不要使用 `clarity-wire::ViewCommand`（已废弃）。

参考 Phase D：在 `clarity-egui::view` 创建前端内部 IR。

---

## 6. References

- `docs/development/CODE-CHANGE-PRINCIPLES.md` § P1, P3, P7
- `crates/clarity-egui/docs/layout-audit-architecture-crisis.md`
- `crates/clarity-wire/src/lib.rs` § Protocol-Driven UI Layer (Phase 2 Pilot) — 本 ADR 否决该 Pilot
- Codex repo `protocol.rs::Event` — Gen-2 的设计灵感来源（不再引用）
- `docs/adr/RFC-2026-04-30-ensure-llm-decoupling.md` — 同类"三层解耦"成功范例

---

## 7. Verification Criteria

ADR 落地完成的判据：

- [x] **Phase A 完成**：本 PR 合并 + `cargo build --workspace` 通过 + deprecation warnings 可见 (2026-05-11, commit `c8e71fdb`)
- [x] **Phase B 完成**：grep 确认 EventBus / sync_to_wire 在 producer 端零调用 → 已删除 (2026-05-11, commit `dd07b42f`)
- [x] **Phase C 完成**：`clarity-wire` 测试数下降到 14（远低于 ~25 目标），且 100% 覆盖生产路径 (2026-05-11, commits `1c15621a` Phase C.1 + `2867c0a5` Phase C.2)
  - 总删除 +12 / -802 = **-790 行死代码**
  - 测试基线 955 → 932（删除 23 个覆盖死代码的测试）
  - `cargo clippy --workspace --lib --bins --tests -- -D warnings`: PASS
- [ ] **Phase D 完成**：（按需，无硬截止）— 启动条件：egui 内 SettingsViewModel 激活时
- [ ] **Phase E 启动**：ADR-007 草案立项（Turn ID 注入 WireMessage）

---

## 8. Revision Log

| 日期 | 变更 | 提议者 |
|------|------|--------|
| 2026-05-11 | 1.0 Accepted; Phase A 同 PR 落地 | 主会话 |
