---
title: ViewState Migration — From Boolean Hell to Typed State Machine
category: Architecture
date: 2026-05-16
tags: [architecture]
---

# ViewState Migration — From Boolean Hell to Typed State Machine

> **Scope**: S3 Phase 1.5 | **Status**: Complete | **Owner**: juice094

## 1. 问题背景

S3 前的 UI 状态由 **50+ 独立布尔标志** 控制：
- `team_panel_open`, `task_panel_open`, `dashboard_panel_open`
- `skill_panel_open`, `mcp_panel_open`
- `is_loading`, `compacting`, `stopping`, `restoring`
- `sidebar_collapsed`, `tools_expanded`, `thinking_log_expanded` ...

**导致的 bug**: 
- 右侧面板可同时打开 Team + Task（实际 UI 只能显示一个）
- `is_loading && compacting` 同时 true（语义互斥但代码未强制）
- 新增面板需要新增布尔标志，复制粘贴 boilerplate

## 2. 迁移策略

### 2.1 三阶段桥接（Bridge Pattern）

```
Phase 1 (P1.5.4a-c): ViewState 写 ──►  legacy bool 读
Phase 2 (P1.5.4d):   ViewState 写 ──►  legacy bool 只读 mirror
Phase 3 (P1.5.2):    删除 legacy bool（计划中）
```

**当前状态**: Phase 2 完成。`view_state.right: Option<SidePanel>` 是右侧面板的唯一权威写入口，旧 `team_panel_open` 等字段作为只读镜像供尚未迁移的渲染方法使用。

### 2.2 聚合类型替换映射

| 旧标志群 | 新类型 | 规则 |
|----------|--------|------|
| `team_panel_open` / `task_panel_open` / `dashboard_panel_open` | `view_state.right: Option<SidePanel>` | **互斥** — Tab D 只能开一个 |
| `skill_panel_open` / `mcp_panel_open` | `view_state.modal: Option<ModalType>` | ADR-014 Decision 2：Skill/Mcp 是模态而非面板 |
| `is_loading` / `compacting` / `stopping` / `restoring` | `view_state.turn: TurnState` | **互斥** — 优先级：`Stopping > Compacting > Loading > Restoring > Idle` |
| 7 个 `*_expanded` 布尔 | `view_state.expansions: PanelExpansion` | **独立** — struct 内 7 个 bool |

## 3. ADR-014 关键决策

### Decision 1: 右侧面板 = 单 Tab D

**问题**: Team / Task / Dashboard 三个抽屉在宽屏下同时打开，挤压中央聊天区至 <300px。

**决策**: 三者变为 `SidePanel` 枚举的互斥变体，共享同一个右侧面板物理区域。切换即替换。

```rust
pub enum SidePanel {
    Sidebar, Workspace, Team, Task, Dashboard,
    PreviewDrawer, SubAgentProgress,
}
```

### Decision 2: Skill / Mcp 重分类为 Modal

**问题**: Skill 面板和 MCP 面板被错误归类为"浮动面板"，实际行为是遮罩层（scrim）+ 中央卡片，阻塞底层交互。

**决策**: 迁移到 `ModalType` 枚举，由 `view_state.modal` 控制。

```rust
pub enum ModalType {
    Approval, Snapshot, Login, TaskCreate, TaskView,
    TeamCreate, CronCreate, SubAgentView, AddProvider,
    KimiCodeLogin, Skill, Mcp,
}
```

### Decision 3: 响应式折叠顺序

当窗口宽度 < 1280px 时，按优先级折叠：
1. `right` 面板（最不重要的业务面板）
2. `left` sidebar（保留会话列表）
3. 中央聊天区保留最小 360px

## 4. 非法状态测试 (P1.5.7)

```rust
#[test]
fn turn_state_from_legacy_exhaustive() {
    // 2⁴ = 16 种组合全遍历
    for loading in [false, true] {
        for compacting in [false, true] {
            for stopping in [false, true] {
                for restoring in [false, true] {
                    let state = TurnState::from_legacy(loading, compacting, stopping, restoring);
                    // 断言非法组合被映射到高优先级状态
                }
            }
        }
    }
}
```

**覆盖**:
- `TurnState` 16 种输入组合的优先级解析
- `TurnState` 类型级互斥（无法同时是 Loading 和 Compacting）
- `ViewState` 结构不变量（`right` 与 `modal` 的物理排他性）
- `FocusScope` 兼容性规则（`Modal` 阻塞 `Panel` 焦点）

## 5. 当前状态快照

```rust
pub struct ViewState {
    pub main: AppView,              // Chat (default) | Settings | Dashboard | Gantt | TaskBoard
    pub left: Option<SidePanel>,    // None | Sidebar
    pub right: Option<SidePanel>,   // None | Workspace | Team | Task | Dashboard | ...
    pub modal: Option<ModalType>,   // None | Approval | Skill | Mcp | ...
    pub turn: TurnState,            // Idle | Loading | Compacting | Stopping | Restoring
    pub expansions: PanelExpansion, // cron | web_tabs | thinking_log | tools | subagents | workspace_plan
    pub focus: FocusScope,          // Os | App | Panel | Modal | Widget
}
```

**API**:
- `view_state.toggle_right(panel)` — 互斥切换
- `view_state.open_modal(modal)` — 自动设置焦点为 `Modal(modal)`
- `view_state.close_modal()` — 恢复焦点为 `App`
- `view_state.focus_panel(kind)` — Modal 打开时拒绝覆盖（安全门）

## 6. 遗留债务

| 字段 | 位置 | 计划移除时间 |
|------|------|-------------|
| `team_panel_open` | `UiStore` | v0.4.0 |
| `task_panel_open` | `TaskStore` | v0.4.0 |
| `dashboard_panel_open` | `UiStore` | v0.4.0 |
| `skill_panel_open` | `UiStore` | v0.4.0（已只读） |
| `mcp_panel_open` | `McpStore` | v0.4.0 |

**移除条件**: 所有读取这些字段的渲染方法完成迁移，改用 `view_state.right` / `view_state.modal`。
