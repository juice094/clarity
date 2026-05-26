---
title: Git 快照管理 UI 设计草案
category: UI Design
date: 2026-05-16
tags: [ui-design, ui]
---

# Git 快照管理 UI 设计草案

> 状态：草案 v0.1 | 关联 Sprint：39 (Side-Git Snapshot MVP UI) | 作者：AI Agent (juice094 工作区)

## 一、设计目标

Side-Git Snapshot 的 Core 功能（`SnapshotService` + `GitRestoreTool`）已完成并通过测试。本草案解决：**如何在 egui 桌面端提供一个不破坏现有 Glassmorphism 主题、低认知负担的快照浏览与回滚界面。**

用户原话："我自己作为开发者都还没用上成熟可用的"——因此 UI 必须**开箱即用、零配置、不阻挡主任务流**。

---

## 二、架构定位

### 2.1 集成位置：非侵入式 Chat Bubble 提示条 + 独立 Modal

> **更新说明 (Sprint 39 rev.2)**：原方案（Workspace Panel 底部折叠区）作废。Workspace Panel 底部预留给 Plan 功能面板。快照 UI 改为**零常驻空间占用**的轻量提示条模式。

```
┌─ Chat Area ──────────────────────────────────────────┐
│                                                      │
│  ┌─ AI Bubble ───────────────────────────────────┐  │
│  │  I'll update the Cargo.toml for you...         │  │
│  │                                                │  │
│  │  [Done]                                        │  │
│  │                                                │  │
│  │  ───────────────────────────────────────────── │  │
│  │  📸 Snapshot #5  ·  [↩ 回滚]  [📜 历史]        │  │  ← NEW
│  └────────────────────────────────────────────────┘  │
│                                                      │
│  ┌─ User Bubble ─────────────────────────────────┐  │
│  │  now fix the file browser                       │  │
│  └────────────────────────────────────────────────┘  │
│                                                      │
└──────────────────────────────────────────────────────┘
```

点击 "📜 历史" 弹出快照管理 Modal：

```
┌─ Snapshot History ───────────────────────────────────┐
│  Workspace Snapshots                          [✕]   │
│  ─────────────────────────────────────────────────── │
│  ● #5  post-turn-5   2 min ago    [↩] [👁]           │
│  ● #4  pre-turn-4    3 min ago    [↩] [👁]           │
│  ● #3  post-turn-3   5 min ago    [↩] [👁]           │
│  ● #2  pre-turn-2    8 min ago    [↩] [👁]           │
│  ○ #1  pre-turn-1    9 min ago    (pruned)           │
│                                                      │
│  [?] 回滚前会自动创建当前状态的备份快照              │
└──────────────────────────────────────────────────────┘
```

**决策理由：**
- 零常驻空间占用：不挤占 Sidebar / Workspace 任何面板区域
- 与 Plan 面板零冲突：Workspace 底部完全留给 Plan 功能
- 上下文感知：快照提示出现在它产生的地方（turn 结束后），用户自然知道"这次改动对应哪个快照"
- 低认知负担：平时看不见，需要时一键触达

### 2.2 主题兼容

完全复用现有 `Theme` design token：

| 元素 | Token | 值（dark） |
|------|-------|-----------|
| 区块标题栏背景 | `surface` | `rgba(38,38,52,0.60)` |
| 列表项背景 | `glass` / `glass_strong` | `rgba(255,255,255,0.06)` / `0.12` |
| 快照类型标签（pre-turn） | `status_busy` | `#D4A050` |
| 快照类型标签（post-turn） | `status_online` | `#6BCB8A` |
| 回滚按钮（hover） | `danger` | `#EF6B6B` |
| 回滚按钮（normal） | `text_dim` | `rgba(200,205,220,0.50)` |
| 预览按钮 | `accent` | `#5B8DEF` |
| 圆角 | `radius_sm` / `radius_md` | 8px / 16px |
| 字体 | `font_body` @ `text_sm` / `text_xs` | Inter 11px / 9px |

**注意**：不使用 backdrop-blur（egui 不支持），通过半透明层叠实现 glass 层次。

---

## 三、数据结构扩展

### 3.1 新增 `SnapshotStore`（Zustand-style slice）

```rust
// crates/clarity-egui/src/stores/mod.rs

pub struct SnapshotStore {
    /// 当前工作区的快照列表（从 SnapshotService::list() 加载）
    pub snapshots: Vec<clarity_core::agent::snapshot::SnapshotInfo>,
    /// 是否展开快照区块
    pub expanded: bool,
    /// 当前选中的快照 ID（用于 diff 预览）
    pub selected_id: Option<usize>,
    /// 最后一次刷新时间（控制轮询频率）
    pub last_refresh: Instant,
    /// 是否正在执行回滚（禁用按钮，防止重复点击）
    pub restoring: bool,
    /// 回滚确认对话框状态
    pub confirm_restore_id: Option<usize>,
    /// 快照预览内容（git diff --stat 或文件列表）
    pub preview: Option<SnapshotPreview>,
}

pub struct SnapshotPreview {
    pub snapshot_id: usize,
    pub diff_stat: String,      // git diff --stat 输出
    pub file_count: usize,
    pub changed_files: Vec<String>,
}
```

### 3.2 `App` 结构体追加字段

```rust
// crates/clarity-egui/src/main.rs
pub(crate) snapshot_store: stores::SnapshotStore,
```

### 3.3 `ChatStore` 追加字段

```rust
pub struct ChatStore {
    // ... existing fields ...
    /// Snapshot info for the most recently completed turn.
    pub last_snapshot: Option<clarity_core::agent::snapshot::SnapshotInfo>,
}
```

### 3.4 `UiStore` 追加字段

```rust
pub struct UiStore {
    // ... existing fields ...
    /// Snapshot history modal open state.
    pub snapshot_modal_open: bool,
}
```

---

## 四、交互流程

### 4.1 快照信息传递流程

**策略：事件驱动，零轮询**

1. `Agent::run_sync_loop()` 中，`maybe_snapshot_post_turn()` 完成后：
   - 将 `SnapshotId` 通过 `mpsc` 发送到 UI 层
   - UI 层接收后写入 `chat_store.last_snapshot`

2. `render_chat_area()` 在渲染 AI response bubble 时：
   - 若 `last_snapshot` 存在且属于当前 turn，在 bubble 底部渲染提示条

3. 点击 "📜 历史" → `ui_store.snapshot_modal_open = true`
   - Modal 打开时从 `Agent::snapshot_list()` 一次性加载完整列表

4. **Core API 需新增**（当前未暴露给 egui）：
   ```rust
   // clarity-core/src/agent/mod.rs
   impl Agent {
       /// 返回快照列表的克隆（同步，仅读 index.json）
       pub fn snapshot_list(&self) -> Vec<SnapshotInfo> { ... }
       /// 触发异步回滚
       pub async fn restore_snapshot(&self, id: usize) -> Result<(), AgentError> { ... }
       /// 回滚前自动创建当前状态备份
       pub async fn restore_with_backup(&self, id: usize) -> Result<(), AgentError> { ... }
   }
   ```

### 4.2 回滚确认对话框（在 Modal 内部完成）

```
┌─ Snapshot History ───────────────────────────────────┐
│  Workspace Snapshots                          [✕]   │
│  ─────────────────────────────────────────────────── │
│  ● #5  post-turn-5   2 min ago                     │
│    [👁 预览]  [↩ 回滚]                               │
│                                                      │
│  ┌─ Confirm Restore ──────────────────────────────┐ │
│  │  ⚠️  回滚到快照 #5 (post-turn-5)               │ │
│  │  当前状态将自动备份为 #6 (pre-restore)。       │ │
│  │              [取消]    [确认回滚]                │ │
│  └────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

- 回滚前**自动创建当前状态备份**（`pre-restore` 快照），防误操作
- 确认按钮使用 `theme.danger` 红色
- 回滚期间显示进度指示（`theme.status_busy` 旋转点）

### 4.3 快照 Diff 预览

**交互**：点击 👁 预览按钮 → 在 Modal 内联展开 diff 摘要

```
┌─ Snapshot History ───────────────────────────────────┐
│  ● #5  post-turn-5   2 min ago    [👁] [↩]           │
│  ┌─ Diff Preview ────────────────────────────────┐  │
│  │  Cargo.toml           |  3 +++                │  │
│  │  src/main.rs          | 12 ++++++----         │  │
│  │  src/config.rs        |  1 -                  │  │
│  │  3 files changed, 12 insertions(+), 4 deletions│ │
│  └────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

- Diff 在 Modal 内联展开，不跳转其他面板
- 点击 👁 再次折叠
- 调用 `GitSnapshot::diff_stat(hash)` 获取

---

## 五、视觉细节规范

### 5.1 快照列表项（单条）

```rust
// 伪代码 — 实际用 egui API
ui.horizontal(|ui| {
    // 左侧：编号 + 类型标签
    ui.label(RichText::new(format!("#{}", info.id)).font(theme.font_mono(theme.text_sm)).color(theme.text_muted));
    
    let tag_color = if info.label.starts_with("pre-turn") { theme.status_busy } else { theme.status_online };
    ui.label(RichText::new(tag_text).size(theme.text_xs).color(tag_color));
    
    // 中部：时间（相对或绝对）
    ui.label(RichText::new(format_time(&info.timestamp)).size(theme.text_xs).color(theme.text_dim));
    
    // 右侧：操作按钮
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if ui.add(theme.ghost_button("↩")).on_hover_text("回滚到此快照").clicked() {
            store.confirm_restore_id = Some(info.id);
        }
        if ui.add(theme.ghost_button("👁")).on_hover_text("预览变更").clicked() {
            store.selected_id = Some(info.id);
        }
    });
});
```

### 5.2 空状态

当工作区不是 git repo 或快照未启用时：

```
📸 Snapshots                          [▼]
─────────────────────────────────────────────────────
  快照未启用。当前工作区不是 Git 仓库，
  或在 agent.yaml 中设置了 snapshot.enabled = false。
  
  [了解如何启用 →]          ← 链接到文档
```

- 使用 `theme.text_dim` 小号字体
- 不显示为错误（非阻断），仅信息提示

---

## 六、实现步骤（建议顺序）

### Phase 1：Core API 暴露（1 小时）
- [ ] `Agent::snapshot_list()` — 同步读取
- [ ] `Agent::restore_snapshot(id)` — 异步回滚
- [ ] `clarity-core` 单元测试 2 个

### Phase 2：Store + 基础渲染（2 小时）
- [ ] 新增 `SnapshotStore`
- [ ] `App` 初始化 `snapshot_store`
- [ ] Workspace panel 底部追加 `render_snapshot_section()`
- [ ] 列表渲染 + 展开/折叠

### Phase 3：交互完善（2 小时）
- [ ] 回滚确认 Modal
- [ ] 回滚进度/结果 Toast 通知
- [ ] Diff 预览（调用 `git diff` 并展示）
- [ ] 空状态处理

### Phase 4：主题微调（1 小时）
- [ ] 暗色/亮色/OLED 三主题颜色校验
- [ ] 高 DPI 字体缩放测试
- [ ] 手动端到端验证

**预估总工时：6 小时（单人工）**

---

## 七、风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| `SnapshotService` 在 `Agent` 内部为 `Option`，egui 侧频繁访问需加锁 | 中 | 性能 | `snapshot_list()` 返回克隆 `Vec`，不持锁跨帧 |
| `GitSnapshot::checkout` 耗时较长，阻塞 UI 线程 | 高 | UX | 回滚必须在 `tokio::spawn` 异步执行，UI 显示 loading 状态 |
| 大仓库 diff --stat 输出过长，列表项膨胀 | 低 | 布局 | diff 预览截断至 20 行，提供 "查看更多" |
| 用户误回滚导致工作丢失 | 中 | 数据 | 强制确认对话框 + 回滚前自动创建当前状态快照（可选） |

---

## 八、与现有模式的对比

| 特性 | 当前 CLI (`git_restore` 工具) | 本 UI 草案 |
|------|------------------------------|-----------|
| 发现性 | 低（需 Agent 自己调用工具） | 高（可视化列表） |
| 操作粒度 | 按索引号 | 点击回滚 |
| 预览能力 | 无 | diff --stat |
| 确认机制 | 依赖 Agent 提示词 | 强制 Modal |
| 适用场景 | 自动化回滚 | 人工审核后回滚 |

两者互补：CLI 工具保留给 Agent 自动决策，UI 提供给人类监督和干预。

---

## 九、待决策事项

1. **时间格式**：绝对时间（`2026-05-05 21:03`）还是相对时间（`2 分钟前`）？
   - 推荐：相对时间 + tooltip 显示绝对时间
2. **最大显示条数**：默认展开显示最近 5 条，还是全部？
   - 推荐：最近 5 条 + "显示全部" 展开
3. **回滚前自动快照**：是否在当前状态未知会用户的情况下先自动 snapshot？
   - 推荐：是，防止误操作丢失当前进度
4. **Diff 预览实现方式**：调用系统 `git diff` 命令，还是通过 `GitSnapshot` 新增 `diff()` 方法？
   - 推荐：后者（Core 层封装，UI 层不直接调系统命令）

---

## 十、附录：与 Core API 的对接点

```rust
// 当前 Core 已提供的 API（无需修改）
clarity_core::agent::snapshot::SnapshotService::list(&self) -> Vec<SnapshotInfo>
clarity_core::agent::snapshot::SnapshotService::restore(&self, id: usize) -> Result<(), AgentError>

// 需要 Core 新增的 API（Phase 1）
clarity_core::Agent::snapshot_list(&self) -> Vec<SnapshotInfo>
clarity_core::Agent::restore_snapshot(&self, id: usize) -> impl Future<Output = Result<(), AgentError>>
clarity_core::agent::snapshot::GitSnapshot::diff_stat(&self, hash: &str) -> Result<String, AgentError>
```
