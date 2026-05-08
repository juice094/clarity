# Plan 功能面板 — 设计草案

> 基于子代理只读分析生成 | 状态：草案 v0.1 | 关联 Sprint：39

---

## 一、现有架构梳理

### 1.1 Core 层 Plan 结构
- **`Plan`**：`title: String` + `steps: Vec<PlanStep>`
- **`PlanStep`**：`id`, `description`, `tool_name`, `tool_params`
- **`Agent::execute_plan()`** 逐步骤执行，通过 `WireMessage::PlanStepBegin` / `PlanStepEnd` 发送生命周期事件
- 执行结果收集为 `Vec<PlanResult>`，不中断后续步骤

### 1.2 现有 egui Plan UI（位于 Chat 区域）
- 当前 Plan UI 渲染在 **message_list ScrollArea 内**、输入栏上方
- **Review Card**：展示 `ChatStore::pending_plan`，提供 Execute / Cancel
- **Tracker Card**：展示 `ChatStore::plan_tracker`，使用 `PlanStepStatus` → (ICON_HOURGLASS, ICON_PLAY, ICON_CHECK, ICON_X) × (text_dim, accent, ok, danger)
- 卡片样式：`Frame::group` + `fill(surface)` + `stroke(accent)` + `radius_md`

### 1.3 Workspace 面板现状
- `SidePanel::right("workspace_panel")`，默认 320px，范围 240–480px
- 垂直布局：**Title/Status** → **File Tree (ScrollArea)** → **File Preview (inline)**
- File Tree 在有 preview 时 `max_height = available_height * 0.45`

### 1.4 ChatStore Plan 状态
```rust
pub pending_plan: Option<clarity_core::agent::Plan>;
pub plan_tracker: Option<PlanExecutionTracker>;  // { title, steps: Vec<PlanStepTracker> }
```
`PlanStepTracker` 含 `id, description, tool_name, status: PlanStepStatus(Pending|Running|Success|Failed)`

---

## 二、Workspace Plan 面板集成方案

### 2.1 布局 ASCII 图

```
┌─────────────────────────────┐  ← SidePanel::right (240–480px)
│  Workspace           ● Online│
│                    ○ Gateway │
├─────────────────────────────┤
│                             │
│  📁 src/                    │
│  📁 crates/                 │
│  📄 Cargo.toml     ← active │
│  ...                        │   ← File Tree ScrollArea
│                             │      (动态高度，受下方挤压)
├─────────────────────────────┤
│ 🌐 Cargo.toml          [×]  │   ← File Preview (可选)
│ ┌─────────────────────────┐ │
│ │ [package]               │ │
│ │ name = "clarity"        │ │
│ └─────────────────────────┘ │
├─────────────────────────────┤
│ ▼ 📋 Plan: Refactor auth  │   ← Plan 折叠区 (CollapsingHeader)
│ ─────────────────────────── │
│ ⏳ 1. Rename module        │   ← Step list
│    → bash({ "cmd": "..." }) │
│    [⋯]                     │   ← 展开后显示 tool_params
│ ✅ 2. Update imports       │
│ ▶  3. Run tests            │   ← Running (accent 色)
│ ─────────────────────────── │
│         [Execute] [Cancel] │   ← Pending 态操作栏
│         [Skip] [Retry]     │   ← Failed 态操作栏
└─────────────────────────────┘
```

### 2.2 空间分配策略

| 面板内容 | 高度策略 |
|---------|---------|
| File Tree | 弹性高度，根据下方内容自适应 |
| File Preview | 有选中文件时占据 `available_height * 0.35` |
| Plan 折叠区 | 折叠时 ≈ header 行高 (28px)；展开时 `min(240px, 剩余高度)`，内部 ScrollArea |

关键改动：将现有 File Tree 的固定 `max_height * 0.45` 改为动态计算——先测量 Plan 区实际占用，再分配剩余空间给 Tree。

### 2.3 零常驻空间原则实现

- **默认折叠**：`workspace_plan_expanded` 初始为 `false`
- **自动展开触发条件**（任一满足）：
  1. `chat_store.pending_plan.is_some()` — 有待审核计划
  2. `chat_store.plan_tracker.is_some()` — 有执行中/未关闭的 tracker
- **自动折叠**：当 `pending_plan` 和 `plan_tracker` 均为 `None` 时，下一帧自动折叠（或保留用户手动折叠状态）
- 实现：在 `render_workspace_panel` 顶部添加状态检测：
  ```rust
  let plan_active = app.chat_store.pending_plan.is_some() 
                 || app.chat_store.plan_tracker.is_some();
  if plan_active && !app.ui_store.workspace_plan_manually_collapsed {
      app.ui_store.workspace_plan_expanded = true;
  }
  ```

---

## 三、Plan 状态流转可视化

### 3.1 状态机

```
        ┌─────────────┐
        │   (idle)    │  ← 无 plan，折叠区隐藏或收折
        └──────┬──────┘
               │ 用户触发 Plan 模式 / LLM 返回 plan
               ▼
        ┌─────────────┐
        │   Pending   │  ← pending_plan 存在，等待审核
        │  [Execute]  │
        │  [Cancel]   │
        └──────┬──────┘
               │ 点击 Execute
               ▼
        ┌─────────────┐
        │  Executing  │  ← plan_tracker 激活，步骤逐条 Running
        │  ▶ Step N   │
        └──────┬──────┘
          ┌────┴────┐
          ▼         ▼
   ┌──────────┐ ┌──────────┐
   │ Success  │ │  Failed  │  ← 单步骤结果
   │  ✅      │ │  ❌      │
   └──────────┘ └────┬─────┘
                     │ 用户点击 [Retry] / [Skip]
                     ▼
              ┌──────────┐
              │ Skipped  │  ← 新增状态（见 §5）
              │  ⏭       │
              └──────────┘
```

### 3.2 视觉编码（复用 Theme Token）

| 状态 | 图标 | 颜色 Token | 说明 |
|------|------|-----------|------|
| Pending | `ICON_HOURGLASS` ⏳ | `text_dim` | 等待执行 |
| Running | `ICON_PLAY` ▶ | `accent` | 当前执行中，可配旋转动画 |
| Success | `ICON_CHECK` ✅ | `ok` | 成功完成 |
| Failed | `ICON_X` ❌ | `danger` | 执行失败 |
| Skipped | `ICON_PROHIBIT` ⛔ | `warn` | 用户手动跳过 |

### 3.3 进度摘要
Header 行右侧显示 compact 进度：`2/5 ✅  1/5 ❌  1/5 ⏳` 或文本 `2 of 5 done`

---

## 四、步骤级别交互设计

### 4.1 展开/折叠单个步骤

- 使用 `egui::CollapsingHeader` 或 `ui.collapsing` 包装每个步骤
- 默认：**当前 Running 步骤自动展开**，其他折叠
- 展开后显示内容：
  - `tool_name` (monospace, `text_xs`, `text_dim`)
  - `tool_params` (JSON，渲染在 `code_block_bg` 的 frame 中，可滚动)

### 4.2 跳过/重试操作

| 场景 | 可用操作 | 位置 |
|------|---------|------|
| Pending 步骤 | [Skip] 按钮 | 步骤右侧（hover 时显示或常驻） |
| Failed 步骤 | [Retry] [Skip] | 步骤下方操作栏 |
| Running 步骤 | [Cancel]（整个 plan） | Plan Header 全局操作 |

**交互细节**：
- Skip：将该步骤状态设为 `Skipped`，Core 层 `execute_plan` 中跳过该步骤（需 Core 支持）
- Retry：重新执行该步骤，状态回退到 `Running`，完成后更新结果

### 4.3 步骤详情查看
```
▶ 3. Run tests          [Skip]
   ─────────────────────────
   Tool: cargo_test
   Params:
   ┌─────────────────────────┐
   │ {                       │
   │   "filter": "agent::"   │
   │ }                       │
   └─────────────────────────┘
   Output:
   ┌─────────────────────────┐
   │ running 3 tests         │
   │ test plan::tests ... ok │  ← 执行完成后显示
   └─────────────────────────┘
```

---

## 五、需要追加的数据结构字段

### 5.1 `UiStore` 新增

```rust
pub struct UiStore {
    // ... existing fields ...
    
    /// Workspace 面板 Plan 折叠区是否展开。
    /// 由自动逻辑驱动，但用户手动折叠后变为手动模式。
    pub workspace_plan_expanded: bool,
    
    /// 用户是否手动折叠了 Plan 区（阻止自动展开）。
    pub workspace_plan_manually_collapsed: bool,
}
```

### 5.2 `PlanStepTracker` 扩展

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlanStepStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,   // ← NEW
}

#[derive(Clone, Debug)]
pub struct PlanStepTracker {
    pub id: String,
    pub description: String,
    pub tool_name: String,
    pub status: PlanStepStatus,
    
    // --- 新增字段 ---
    /// 该步骤是否被用户展开查看详情。
    pub expanded: bool,
    /// 步骤执行输出（PlanStepEnd 时回填）。
    pub output: Option<String>,
    /// 重试次数。
    pub retry_count: u8,
}
```

### 5.3 `PlanExecutionTracker` 扩展

```rust
#[derive(Clone, Debug)]
pub struct PlanExecutionTracker {
    pub title: String,
    pub steps: Vec<PlanStepTracker>,
    
    // --- 新增字段 ---
    /// 全局计划状态（派生或显式维护）。
    pub overall_status: PlanOverallStatus,
    /// 计划开始时间。
    pub started_at: Option<std::time::Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlanOverallStatus {
    Reviewing,   // pending_plan 阶段
    Executing,
    Completed,
    Partial,     // 部分成功（有 skipped/failed）
}
```

### 5.4 Core 层最小改动（如需 Skip/Retry）

`crates/clarity-core/src/agent/plan.rs` 中 `execute_plan` 需支持：
- 接收一个 `step_id_filter: Option<HashSet<String>>` 或步骤级别的控制信号
- 最简单方案：将 `execute_plan` 改为逐步骤可控的流式 API，或让 UI 层通过 cancellation + 重建 plan（去掉 skipped steps）来模拟

> **建议 MVP 阶段**：UI 层先只支持 [Cancel 整个 Plan] 和 [Dismiss Tracker]，Skip/Retry 作为 Phase 2。

---

## 六、主题兼容性

全部复用现有 Theme token，不新增颜色：

| UI 元素 | Token | 备注 |
|--------|-------|------|
| Plan 区背景 | `surface` 或 `bg_accent` | 与 File Preview 区分层级 |
| Plan 卡片 | `card_frame()` | 复用现有方法 |
| 步骤列表分隔线 | `border` | 1px |
| 选中/悬停步骤 | `bg_hover` | hover 高亮 |
| 代码块背景 | `code_block_bg` | tool_params JSON |
| 图标颜色 | `accent` / `ok` / `danger` / `warn` / `text_dim` | 按状态映射 |

---

## 七、实现步骤估算

| # | 步骤 | 文件 | 工作量 |
|---|------|------|--------|
| 1 | **新建 `panels/workspace/plan.rs`** | 新文件 | 新增 Plan 渲染逻辑（从 `chat/plan.rs` 迁移并扩展） |
| 2 | **扩展 `UiStore`** | `stores/mod.rs` | +2 字段 (`workspace_plan_expanded`, `workspace_plan_manually_collapsed`) |
| 3 | **扩展 `PlanStepTracker` / `PlanExecutionTracker`** | `ui/types.rs` | + `Skipped` 状态、`expanded`, `output`, `retry_count` |
| 4 | **修改 `workspace.rs`** | `panels/workspace.rs` | 底部插入 Plan 折叠区；调整 File Tree 高度计算 |
| 5 | **修改 `chat/plan.rs`（可选）** | `panels/chat/plan.rs` | 保留或移除：用户要求迁移到右侧面板，建议保留 chat 中轻量提示（如 "Plan moved to Workspace"）或完全移除 |
| 6 | **Wire 事件处理更新** | `handlers/chat.rs` | `PlanStepBegin`/`PlanStepEnd` 事件继续更新 `plan_tracker`，但 UI 渲染从 chat 迁移到 workspace |
| 7 | **Skip/Retry 支持（Phase 2）** | `plan.rs` (core) + `ui/types.rs` | Core `execute_plan` 支持跳过指定步骤 |

**预估**：Phase 1（迁移 + 折叠区 + 自动展开）约 **1.5–2 天**；Phase 2（Skip/Retry + 步骤详情持久化）约 **1 天**。

---

## 八、风险与待决策项

1. **Chat 区域 Plan UI 是否保留？**  
   建议完全迁移到 Workspace 面板，避免双渲染造成状态同步问题。若用户希望在 chat 中也看到进度，可在 chat 中保留一个不可交互的只读进度条。

2. **Plan 区与 File Preview 的空间竞争**  
   当两者同时存在时，file tree 需要被压缩。是否允许 Plan 区超过 `min(240px, 剩余)` 挤压 Preview？建议 Plan 区 `max_height` 限制为 240px，超出内部滚动。

3. **Skip/Retry 的 Core 层语义**  
   当前 `execute_plan` 是顺序执行 Vec。Skip 可以在 UI 层过滤 steps 后重建 Plan 传入；Retry 需要重新调用单步 execute。最轻量方案是 UI 层维护一个 `Vec<PlanStepTracker>`，由 UI 层编排下一步调用哪个工具，而非改造 Core `execute_plan`。

4. **右侧面板宽度 240px 时的密度**  
   步骤描述可能被截断。建议步骤行使用 `ui.horizontal_wrapped` 或 tooltip 显示完整描述。
