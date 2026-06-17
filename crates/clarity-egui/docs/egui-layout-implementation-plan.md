# clarity-egui 新布局实施计划

> 基于与用户收敛后的目标拓扑：固定宽左栏导航树、极简标题栏、统一会话中栏、IDE 式压缩右栏。  
> 状态：已确认规格，进入实施计划阶段。  
> 版本：v0.1（2026-06-16）

---

## 1. 目标拓扑回顾

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│ [≡] 收起导航                                                —  □  ×        │  ← 标题栏
├────────────────┬──────────────────────────────┬─────────────────────────────┤
│                │  ○[Bot 名称]  [分享][控制台][文件/设置] │                  │  ← Bot 栏 + 右栏入口
│  新建会话       │                              │                             │
│  技能           │                              │                             │
│  定时任务       │       聊天内容画布            │      右栏面板              │
│  网页外链1      │       （统一会话容器）         │      ┌─────────────┐      │
│  网页外链2      │                              │      │ [×] 标题    │      │
│  功能1          │                              │      │             │      │
│  功能2          │                              │      │ 当前上下文  │      │
│  Claw          │                              │      │ 对应功能    │      │
│  ○ 设备1实例    │                              │      └─────────────┘      │
│  ○ 设备2实例    │                              │                             │
│  项目 ▼         │                              │                             │
│   项目1         │                              │                             │
│    项目1会话1   │                              │                             │
│   项目2         │                              │                             │
│    项目2会话1   │                              │                             │
│    项目2会话2   │                              │                             │
│  对话 ▼         │                              │                             │
│   无项目会话1   │                              │                             │
│   无项目会话2   │                              │                             │
│                │──────────────────────────────│                             │
│  ○ 用户头像 ▼  │  ⊕  [      输入框区域      ]  ↑                           │  ← 底部对齐
├────────────────┴──────────────────────────────┴─────────────────────────────┤
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. egui 布局结构草案

### 2.1 顶层 Panel 组织

```rust
// main.rs::render_layout_shell
fn render_layout_shell(&mut self, ctx: &egui::Context) {
    // 1. 标题栏：极简，只控制左栏折叠和窗口控制
    self.render_titlebar(ctx);

    // 2. 左栏：固定宽导航树
    if self.view_state.left_rail_expanded {
        self.render_left_navigation_tree(ctx);
    }

    // 3. 右栏：默认折叠的 IDE 式功能面板
    if self.view_state.right_rail_visible {
        self.render_right_ide_panel(ctx);
    }

    // 4. 中栏：CentralPanel，内部垂直分为 Bot 栏 / 画布 / 输入框
    self.render_central_stage(ctx);

    // 5. 模态框/浮层
    self.render_modals(ctx);
}
```

### 2.2 标题栏

```rust
egui::TopBottomPanel::top("titlebar")
    .exact_height(theme.size_titlebar)
    .frame(egui::Frame::new().fill(theme.bg).stroke(Stroke::NONE))
    .show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button("≡").clicked() {
                self.view_state.left_rail_expanded = !self.view_state.left_rail_expanded;
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                // 窗口控制按钮（仅 Windows 自定义；macOS/Linux 使用原生）
                window_controls(ui);
            });
        });
    });
```

> **平台策略**：Windows 保持自定义标题栏时使用上述结构；macOS/Linux 默认使用 `.with_decorations(true)`，标题栏内不渲染窗口控制按钮。

### 2.3 左栏导航树

```rust
egui::SidePanel::left("left_nav_tree")
    .exact_width(theme.size_sidebar)
    .resizable(false)
    .frame(egui::Frame::side_top_panel(&ctx.style()).fill(theme.bg))
    .show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            // 顶部操作入口
            render_quick_actions(ui); // 新建会话、技能、定时任务、网页外链、功能入口

            // Claw 设备分组
            render_claw_section(ui);

            // 项目树
            render_project_tree(ui);

            // 无项目对话组
            render_unprojected_chats(ui);
        });

        // 底部用户头像（使用 bottom_up 或 spacer 推到底）
        ui.with_layout(egui::Layout::bottom_up(Align::LEFT), |ui| {
            render_user_avatar(ui);
        });
    });
```

**关键实现点**：
- `size_sidebar = text_base * 17`，并在 `Theme::with_font_scale` 中同步缩放。
- 项目树使用 `collapsing_header` + 内部会话行；支持折叠/归档。
- 用户头像固定在左栏底部，与中央输入框底部对齐（因 SidePanel 与 CentralPanel 都从标题栏下延伸至窗口底）。

### 2.4 中栏（Central Stage）

```rust
egui::CentralPanel::default()
    .frame(egui::Frame::central_panel(&ctx.style()).fill(theme.bg))
    .show(ctx, |ui| {
        ui.vertical(|ui| {
            // 1. Bot 栏
            self.render_bot_bar(ui);

            // 2. 聊天画布（占用剩余空间）
            ui.with_layout(egui::Layout::top_down(Align::Center), |ui| {
                self.render_chat_canvas(ui);
            });

            // 3. 输入框（底部）
            self.render_composer(ui);
        });
    });
```

**Bot 栏结构**：

```rust
ui.horizontal(|ui| {
    // 头像 + 名称
    avatar_with_name(ui, bot_name);

    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
        // 三个右栏功能按钮，上下文相关
        let buttons = right_rail_buttons_for_context(self.current_session_context());
        for btn in buttons {
            if ui.button(btn.icon).clicked() {
                self.view_state.right_rail_visible = true;
                self.view_state.right_rail_panel = btn.panel;
            }
        }
    });
});
```

### 2.5 右栏（IDE 式压缩面板）

```rust
egui::SidePanel::right("right_ide_panel")
    .default_width(theme.size_panel_right)
    .min_width(220.0)
    .max_width(400.0)
    .resizable(true)
    .frame(egui::Frame::side_top_panel(&ctx.style()).fill(theme.surface))
    .show(ctx, |ui| {
        // Header：标题 + 关闭按钮
        ui.horizontal(|ui| {
            ui.label(panel_title);
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.button("×").clicked() {
                    self.view_state.right_rail_visible = false;
                }
            });
        });

        // 当前面板内容
        match self.view_state.right_rail_panel {
            RightRailPanel::Share => render_share_panel(self, ui),
            RightRailPanel::Console => render_console_panel(self, ui),
            RightRailPanel::Files => render_files_panel(self, ui),
            RightRailPanel::ClawSettings => render_claw_settings_panel(self, ui),
            RightRailPanel::KnowledgeBase => render_knowledge_panel(self, ui),
            RightRailPanel::Templates => render_template_panel(self, ui),
        }
    });
```

---

## 3. 需要新增/修改的模块清单

### 3.1 新增模块

| 文件 | 职责 |
|------|------|
| `src/panels/navigation_tree/mod.rs` | 左栏固定宽导航树：项目树、Claw 设备、对话组、快速入口 |
| `src/panels/navigation_tree/project_tree.rs` | 项目折叠树、归档、会话列表 |
| `src/panels/navigation_tree/claw_section.rs` | Claw 设备实例列表 |
| `src/panels/navigation_tree/quick_actions.rs` | 新建会话、技能、定时任务、网页外链等入口 |
| `src/panels/bot_bar.rs` | 中栏顶部 Bot 栏（头像、名称、右栏按钮） |
| `src/panels/right_ide_panel/mod.rs` | 右栏面板容器与 header |
| `src/panels/right_ide_panel/share_panel.rs` | 分享/导出面板 |
| `src/panels/right_ide_panel/console_panel.rs` | 控制台/任务日志面板 |
| `src/panels/right_ide_panel/files_panel.rs` | 文件资源管理器 |
| `src/panels/right_ide_panel/claw_settings_panel.rs` | Claw 终端/文件/设置 |
| `src/panels/right_ide_panel/knowledge_panel.rs` | 项目知识库 |
| `src/panels/right_ide_panel/template_panel.rs` | 模板注入/预设 sessions |
| `src/ui/session_context.rs` | 定义 `SessionContext` / `SessionType` 枚举，驱动右栏按钮 |
| `src/stores/project_store.rs` | 项目数据模型与持久化（或先 mock） |

### 3.2 修改模块

| 文件 | 修改内容 |
|------|----------|
| `src/main.rs` | 重写 `render_layout_shell`、`render_titlebar`；新增 `render_left_navigation_tree`、`render_central_stage`、`render_right_ide_panel` |
| `src/layout.rs` | 更新 `LayoutMetrics`：移除 36px rail 逻辑，左栏固定宽，右栏默认折叠；更新响应式断点 |
| `src/theme.rs` | 新增/调整 token：`size_sidebar = text_base * 17`、`size_titlebar`、`size_bot_bar`、`size_panel_right` |
| `src/design_system.rs` | 扩展 `Surface::BotBar`、`Text::BotName`、`ButtonStyle::Icon` 等 |
| `src/panels/sidebar/mod.rs` | 逐步替换为 `navigation_tree`，保留兼容直到迁移完成 |
| `src/panels/chat/header.rs` | 移除 titlebar session tabs，或改为 Bot 栏的一部分 |
| `src/panels/chat/input/tui_style.rs` | 调整底部 margin，确保与左栏头像底部对齐 |
| `src/panels/chat/message_list.rs` | 收敛渲染路径；聊天画布宽度随右栏折叠自动调整 |
| `src/components/chat/conversation.rs` | 统一为单一气泡样式；移除 Kimi/AgentTurn 等 flag 导致的分支 |
| `src/widgets/user_avatar.rs` | 支持下拉菜单扩展点 |
| `src/ui/types.rs` | 新增 `RightRailPanel`、`SessionContext`、`SessionLifecycle` 等类型 |
| `src/view_state.rs`（或 `clarity_core::ui::ViewState`） | 新增 `left_rail_expanded`、`right_rail_visible`、`right_rail_panel` 等字段 |

---

## 4. 数据模型变更（建议）

### 4.1 Session 增加上下文字段

```rust
pub struct Session {
    pub id: String,
    pub title: String,
    pub project_id: Option<String>, // None 表示默认/无项目对话组
    pub context: SessionContext,
    pub lifecycle: SessionLifecycle,
    pub archived: bool,
    // ... 现有字段
}

pub enum SessionContext {
    Chat,          // 普通对话
    Project,       // 项目会话
    Claw { device_id: String }, // 远程设备会话
}

pub enum SessionLifecycle {
    Temporary,     // Chat：临时问题导向
    ProjectBound,  // Work/项目：与项目生命周期绑定
    UserBound,     // Claw：长期，与用户绑定
}
```

### 4.2 Project 模型（新增）

```rust
pub struct Project {
    pub id: String,
    pub name: String,
    pub archived: bool,
    pub has_workspace: bool, // 是否有工作区：决定算力来源与可用工具
    pub session_ids: Vec<String>,
}
```

> **实施策略**：UI 层先用 mock 数据实现布局骨架；项目/session 模型变更可与 `clarity-core` 团队同步，或在 `clarity-egui` 内部先用本地 store 实验，稳定后再下沉到 core。

---

## 5. 分阶段迁移路线

### Phase 0：准备与清理（1–2 天）

1. 在 `Theme` 中新增/调整 layout tokens：`size_sidebar`、`size_titlebar`、`size_bot_bar`、`size_panel_right`。
2. 在 `ViewState` 中新增字段：`right_rail_panel: RightRailPanel`。
3. 定义 `SessionContext`、`RightRailPanel` 枚举。
4. 删除 `panels/legacy/` 中确认不再使用的文件（如 `task.rs`、`team.rs` 已删除则跳过）。
5. 添加布局调试 instrumentation 到新的 panel 入口。

**验收标准**：
- `cargo check -p clarity-egui` 通过。
- 新增类型有基本单元测试。

### Phase 1：标题栏简化（2–3 天）

1. 重写 `render_titlebar`：
   - 保留 `[≡]` 按钮和窗口控制；
   - 移除 brand、session tabs、persona、model、status capsules；
   - 在 Windows 上保持自定义标题栏，macOS/Linux 默认使用原生装饰。
2. 将 model/persona 显示暂时移到 Bot 栏或右栏（后续 Phase 3 细化）。
3. 移除 `ui_store.titlebar_right_width` 运行时测量逻辑。

**验收标准**：
- 标题栏高度 36px 或更小；
- 窗口控制、拖拽、最大化/最小化行为正常；
- 无布局反馈循环。

### Phase 2：左栏导航树（3–5 天）

1. 新建 `panels/navigation_tree/` 模块。
2. 实现固定宽左栏：
   - 顶部快速入口（新建会话、技能、定时任务、网页外链、功能入口）；
   - Claw 设备分组；
   - 项目树（支持折叠、归档）；
   - 无项目对话组；
   - 底部用户头像下拉菜单。
3. 用 `ViewState.left_rail_expanded` 控制显隐。
4. 替换旧 `panels/sidebar/mod.rs` 调用。

**验收标准**：
- 左栏宽度随字体缩放；
- 项目树可折叠/归档；
- 用户头像固定在底部；
- 在 768px 以下自动折叠。

### Phase 3：中栏 Bot 栏 + Composer 对齐（3–4 天）

1. 新建 `panels/bot_bar.rs`：
   - 头像 + Bot 名称；
   - 右侧三个动态按钮，根据 `SessionContext` 切换。
2. 重写 `render_central_stage`：
   - 顶部 Bot 栏；
   - 中部聊天画布；
   - 底部 Composer。
3. 调整 `input/tui_style.rs` 底部 padding，确保与左栏头像底部对齐。
4. 将原 `chat/header.rs` 中的 session tabs 移除或整合到 Bot 栏/左栏。

**验收标准**：
- Bot 栏按钮在不同会话类型下显示正确；
- 点击按钮展开右栏并切换面板；
- 输入框与左栏头像底部基线对齐。

### Phase 4：右栏 IDE 式面板（4–6 天）

1. 新建 `panels/right_ide_panel/` 模块。
2. 实现右栏容器：header + `[×]` + 当前面板。
3. 实现各面板占位版：
   - Share / Console / Files / ClawSettings / KnowledgeBase / Templates。
4. 实现右栏展开时压缩中栏（egui SidePanel 天然支持）。
5. 将旧 `panels/right_rail/` 的 Progress/Context 卡片内容迁移到新的 IDE 面板中（或决定废弃）。

**验收标准**：
- 右栏默认折叠；
- 按钮切换面板，互斥显示；
- 右栏关闭后中栏恢复全宽；
- 聊天和输入在右栏展开时仍可正常操作。

### Phase 5：统一会话容器（3–5 天）

1. 在 `message_list.rs` 中收敛渲染路径：
   - 移除 `agent_turn_style`、`agent_turn_glass` 导致的多分支，统一走 AgentTurn 聚合路径；
   - 保留 `kimi_conversation_style` 用于 approval dock，`line-mode` 保持为 feature-gated 能力；
   - 保留统一的 AgentTurn 渲染路径，视觉样式通过 theme 微调。
2. 统一空状态：大 Logo + 居中 composer，移除「Configure Settings」按钮。
3. 动作栏改为 hover 显示。
4. 确保虚拟列表在右栏折叠/展开时高度缓存正确失效。

**验收标准**：
- 所有消息使用同一条渲染路径；
- 空状态符合目标；
- 虚拟列表无闪烁、stick-to-bottom 正常。

### Phase 6：项目模型与上下文驱动（5–7 天）

1. 在 `clarity-egui` 中新增 `ProjectStore`（或先 mock）。
2. 为 Session 增加 `project_id`、`context`、`lifecycle` 字段。
3. 根据上下文驱动 Bot 栏按钮和右栏面板。
4. 实现项目创建、归档、折叠交互。
5. 实现「无工作区」时的网页/本地算力提示 UI。

**验收标准**：
- 项目树真实反映 session 分组；
- 切换 session 时 Bot 栏和右栏自动更新；
- 项目归档/折叠状态持久化。

### Phase 7：测试与打磨（3–5 天）

1. 接入 `egui_kittest` 或类似 snapshot 测试：
   - 三栏切换；
   - 右栏展开/关闭；
   - 新建会话/切换会话；
   - 窗口缩放响应式折叠。
2. 布局诊断 overlay 覆盖所有新 panel。
3. 性能测试：虚拟列表在右栏压缩后的高度估算稳定性。
4. i18n：所有新 UI 字符串走 `t!()`。

**验收标准**：
- `cargo clippy -p clarity-egui --bins --tests -- -D warnings` 通过；
- 新增 snapshot/集成测试通过；
- 无硬编码像素值（>8.0 必须走 theme token）。

---

## 6. 风险与应对

| 风险 | 影响 | 应对 |
|------|------|------|
| 左栏从 36px rail 改为 240px 树，内容区宽度骤减 | 聊天画布变窄 | 默认窗口保持 1280px；右栏默认折叠；内容区 max_width 自适应 |
| `SessionContext` / Project 模型变更影响 `clarity-core` | 跨 crate 改动大 | UI 层先用 mock/store 实验，稳定后再下沉；保持 core 接口兼容 |
| 旧 `AgentTurn` / `kimi_conversation_style` 路径移除导致回归 | 消息渲染异常 | Phase 5 先保留 flag 作为编译开关，验证稳定后再删除 |
| macOS/Linux 原生标题栏与当前自定义逻辑冲突 | 跨平台行为不一致 | Phase 1 用 `cfg(target_os)` 区分，分别实现并测试 |
| 右栏压缩中栏时输入框和头像底部对齐出现像素偏差 | 视觉不齐 | 使用 `SidePanel` + `CentralPanel` 全高布局，底部对齐自然达成；必要时用 debug overlay 校准 |

---

## 7. 建议的启动顺序

**推荐从 Phase 0 + Phase 1 同时启动**：
1. 先调整 theme tokens 和 ViewState，建立新类型基础。
2. 同步重写 titlebar，这是用户感知最明显的变化，也能立即减少标题栏职责。
3. 然后按 Phase 2 → Phase 3 → Phase 4 逐步搭建新外壳。
4. Phase 5 收敛渲染路径可与 Phase 3/4 并行，但需避免冲突。
5. Phase 6 项目模型建议在 UI 骨架稳定后再深入。

---

## 8. 下一步动作

如需开始实施，建议：
1. 创建 tracking issue 或 TODO list，按 Phase 拆分任务；
2. 先开一个 **Phase 0 + Phase 1 的原型 PR**，验证 titlebar + theme tokens + 新 panel 入口；
3. 每完成一个 Phase 都跑 `cargo check/clippy/test` 并做手动截图对比。

本计划文件位于 `crates/clarity-egui/docs/egui-layout-implementation-plan.md`，可在实施过程中持续更新。
