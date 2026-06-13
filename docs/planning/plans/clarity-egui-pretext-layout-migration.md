# clarity-egui → Pretext 单页面/三栏布局迁移规划

> 日期：2026-06-13  
> 依据：概念图 `C:/Users/22414/Desktop/屏幕截图 2026-06-13 152905.png` + 功能组件 `C:/Users/22414/dev/pretext-rust`  
> 范围：`crates/clarity-egui`  
> 目标：在保留现有功能的前提下，把当前"标题栏 + 侧边栏 + 主视图 + 浮动右侧面板"的架构，迁移为概念图所示的"左侧边栏 + 中主内容区 + 右工具栏 + 浮动操作轨"三栏单页面布局，并明确 pretext-rust 的接入点。

---

## 一、概念图结构解读

概念图整体为一个深色、圆角、无边框窗口，内部可拆为以下区域：

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│  自定义标题栏（品牌 / 会话标签 / 模型选择 / 窗口控制）                          │
├──────────┬───────────────────────────────────────────────┬──────────────────┤
│          │                                               │                  │
│  左侧边栏 │              中主内容区                        │   右工具面板      │
│  (rail)  │                                               │   (utility rail) │
│  - 头像  │  - 会话标题 / 工具图标                          │  - 系统状态      │
│  - 分类  │  - 聊天气泡（用户 / 助手）                       │  - Agent 状态    │
│  - 会话  │  - 思维/节点图（助手回复内嵌）                    │  - 工具列表      │
│  - 插件  │  - 底部输入栏                                   │  - 子代理进度    │
│  - 状态  │                                               │  - 记忆/文件预览 │
│          │                                               │                  │
├──────────┴───────────────────────────────────────────────┴──────────────────┤
│  底部流程/状态条（可选，概念图下方有流程图）                                   │
└─────────────────────────────────────────────────────────────────────────────┘
   最右侧浮动垂直工具轨（快捷操作）
```

关键设计特征：
- **三栏固定**：左、中、右三栏同时可见（宽屏），不再使用当前"右侧面板作为 overlay/modal"的临时弹出模式。
- **主内容区以聊天为主**：设置、Dashboard、Gantt、TaskBoard、Work 等视图作为中心区域的"页面模式"切换，而非独立面板。
- **右侧面板常驻**：系统状态、Agent 状态、工具、子代理、记忆等作为可折叠/可切换的 cards。
- **左侧边栏双层**：外层是 icon rail，内层是可展开的会话/分类列表。
- **聊天气泡富文本**：用户消息为普通文本；助手消息可包含普通文本、代码块、mention/chip、以及思维节点图。
- **pretext 适合点**：需要精确换行、CJK 混排、rich inline（chip/mention/code span）的文本区域。

---

## 二、当前 clarity-egui 布局资产盘点

| 当前模块/文件 | 当前职责 | 迁移后归属 |
|---------------|----------|------------|
| `main.rs::render_titlebar()` | 自定义标题栏 | 保留，顶部通栏 |
| `panels/sidebar/` | 左侧边栏 | 保留，拆分为 icon rail + 可展开列表 |
| `panels/workspace/` | 左中工作区（文件预览、plan） | 降级为左侧面板的一个 tab/section，或中主内容区的子面板 |
| `panels/chat/` | 聊天主视图 | 中主内容区默认视图，核心升级区 |
| `panels/system/dashboard.rs` | Dashboard 主视图 | 中主内容区切换视图 |
| `panels/work/` | Work 模式主视图 | 中主内容区切换视图 |
| `panels/legacy/{team,task,mcp,skill,task_board,gantt}.rs` | 右侧/独立业务面板 | 右工具面板内的 cards/tabs；Gantt/TaskBoard 可作为中主内容区视图 |
| `panels/settings/` | 设置主视图 | 中主内容区切换视图 |
| `panels/modals/` | 模态弹窗 | 保留，用于阻断式操作（审批、登录、创建任务等） |
| `widgets/` | 复用组件 | 保留，逐步迁移到 design_system |
| `design_system.rs` | 语义化 UI 原语 | 作为新布局的视觉基础 |
| `layout.rs` | 响应式几何计算 | 扩展为新版三栏布局 shell |

当前 `ViewState` 已经承载主视图、右侧面板、模态状态，可以直接扩展为新版布局状态机。

---

## 三、目标布局状态机（ViewState 扩展草案）

在现有 `clarity_core::ui::ViewState` 基础上增加：

```rust
pub struct ViewState {
    pub main: AppView,                 // 中主内容区当前页面
    pub left_rail: LeftRailSection,    // 左侧边栏展开的章节
    pub right_rail: RightRailSection,  // 右工具面板展开的章节
    pub right_rail_visible: bool,      // 右栏是否折叠
    pub left_rail_expanded: bool,      // 左侧列表是否展开
    pub modal: Option<ModalType>,
    pub turn: TurnState,
    pub expansions: PanelExpansion,
}

pub enum LeftRailSection {
    Sessions,   // 会话/分类列表
    Plugins,    // 插件/扩展
    Workspace,  // 工作区/文件预览
    None,       // 仅 icon rail
}

pub enum RightRailSection {
    Status,     // 系统 + Agent 状态
    Tools,      // 工具列表
    Subagents,  // 子代理进度
    Memory,     // 记忆/上下文
    None,       // 折叠
}
```

旧 `SidePanel::Team/Task` 等概念不再作为右侧面板枚举，而是迁移到 `RightRailSection` 内的 cards。

---

## 四、分阶段迁移路线

### Phase A — 新版布局外壳（1~2 天）

1. 扩展 `layout.rs`
   - 新增 `ThreeColumnLayout` 计算：左 rail 宽度、左列表宽度、中内容区最小宽度、右 rail 宽度。
   - 定义响应式断点：宽屏三栏、中屏隐藏右栏/左列表、窄屏仅保留 icon rail + 中内容区。
2. 重写/扩展 `App::render_layout_shell()`
   - 左侧：`render_left_rail()` → icon rail + 可展开列表。
   - 中部：`render_main_stage()` → 根据 `view_state.main` 渲染 Chat / Dashboard / Settings / Gantt / TaskBoard / Work。
   - 右侧：`render_right_rail()` → 可折叠工具面板。
   - 顶部：标题栏。
   - 浮动：右侧快捷操作轨（`ActionRail`）。
3. 保留所有现有 render 方法作为内容实现，仅调整容器。

### Phase B — 右侧面板整合（1~2 天）

1. 新建 `panels/right_rail/mod.rs`
   - 包含 `StatusCard`、`ToolsCard`、`SubagentCard`、`MemoryCard`。
2. 把 `legacy/task.rs` 的内容迁入 `SubagentCard` 或 `MemoryCard` 的 task 子区。
3. 把 `legacy/team.rs` 的内容迁入 `MemoryCard` 或新建 `TeamCard`。
4. 把 `legacy/mcp.rs`、`legacy/skill.rs` 迁入 `ToolsCard`。
5. 删除 `legacy/` 目录（确认无调用后）。

### Phase C — 左侧边栏双层化（1 天）

1. 把当前 `panels/sidebar/mod.rs` 拆为：
   - `sidebar/rail.rs`：icon-only 垂直 rail。
   - `sidebar/panel.rs`：可展开的会话/分类/插件列表。
2. `panels/workspace/` 作为 `LeftRailSection::Workspace` 的内容，或保留为独立左中列（概念图中 workspace 更像左侧面板）。

### Phase D — 聊天区域 pretext 升级（2~3 天）

1. 新建 `widgets/message_bubble.rs` 或 `panels/chat/bubble.rs`
   - 使用 `pretext_core::rich_inline` 解析消息内容为 items（普通文本、mention、code span、图片占位）。
   - 用 `pretext_core::layout` 计算气泡内换行。
   - 渲染策略：**测量用 pretext，绘制用 egui** —— 每个 item 渲染为 egui label/rect，保留选择和交互；仅对复杂 CJK 混排借用 pretext 的宽度结果。
2. 可选路径：若 egui 原生文本无法满足需求，可像 `pretext-slint` 一样把整个气泡栅格化为 `egui::Image`，但会牺牲文本选择；建议作为兜底方案。
3. 思维节点图：先实现为静态图/简单自定义绘制；节点标签可用 pretext 测量。

### Phase E — 设计系统全面替换（2~3 天，可与 D 并行）

1. 用 `design_system::surface(Surface::Card, ...)` 替换现有的 card 类组件。
2. 用 `design_system::text()` / `gap()` 替换手写 `RichText` 和 `add_space`。
3. 用 `design_system::status_badge()` 替换 `status_capsule.rs`（若语义等价）。
4. 统一使用 `design_system::install_theme()`（已在 `main.rs::update()` 接入）。

### Phase F — 浮动操作轨与底部流程条（1 天）

1. 新增 `widgets/action_rail.rs`：最右侧垂直图标按钮列。
2. 可选新增底部 `StatusBar` / 流程指示器（概念图下方流程图）。

---

## 五、pretext-rust 接入点详单

| 接入点 | 当前实现 | pretext 能力 | 推荐策略 |
|--------|----------|--------------|----------|
| 聊天气泡文本换行 | `egui::Label` 自动换行 | `pretext_core::prepare` + `layout_with_lines` | 用 pretext 计算行范围，再用 egui 按行渲染 |
| mention / code chip | 无，纯文本 | `pretext_core::rich_inline` | 把 chip 作为 `RichInlineItem::new(..., break: Never)`，按结果手动定位渲染 |
| CJK 混排精确宽度 | egui 字体度量 | `pretext_core::FontMetrics` + `pretext-fontdb` | 用 `FontdbBackend` 测量中文宽度，指导 egui 的 `Galley` 或手动布局 |
| 节点图标签 | 暂无 | 同上行 | 用 pretext 测量节点文本尺寸后画节点框 |
| 标题栏标签/徽章 | 手写 | 普通 prepare/layout | 可作为 pretext 集成的第一个 PoC |

建议 PoC：先在 `widgets/theme_card.rs` 或 `widgets/sidebar_card.rs` 的标题/描述处接入 pretext 测量，验证 `pretext-fontdb` 与 egui 字体系统的对齐（字号、行高、颜色）。

---

## 六、依赖与仓库关系

- **方案 A（推荐）**：在 `pretext-rust` 仓库新增 `crates/pretext-egui`，提供 `PretextLabel` widget 和 `measure_text` 工具；`clarity-egui` 通过 git/path 依赖 `pretext-core`、`pretext-fontdb`、`pretext-egui`。
- **方案 B**：不新增 crate，直接在 `clarity-egui` 内调用 `pretext-core` / `pretext-fontdb`。适合快速验证，但不利于复用。

当前 clarity-egui 与 pretext-rust 是两个独立仓库，建议方案 A，保持 pretext-rust 的 crate 边界清晰。

---

## 七、风险与前置条件

| 风险 | 影响 | 缓解 |
|------|------|------|
| egui 的 `Galley` 无法完全复用 pretext 的换行结果 | 需要手动按行渲染，增加复杂度 | 先 PoC 验证；若不可行则改用 image 渲染 |
| pretext-fontdb 与 egui 使用不同字体源 | 宽度/字形不一致 | 统一字体描述（CSS shorthand），必要时让 egui 加载同一字体文件 |
| 三栏布局在窄屏下拥挤 | 可用性下降 | 定义明确的折叠断点；右栏默认折叠，左列表可隐藏 |
| 右侧面板整合可能丢失原有交互 | 功能回退 | 保留模态弹窗作为 fallback；逐步迁移 |
| 节点图渲染工作量大 | 延长时间 | 节点图放在 Phase D 后期，先用简单框图替代 |

---

## 八、验收标准

1. `cargo fmt --check`、`cargo clippy -p clarity-egui --bins --tests -- -D warnings`、`cargo test -p clarity-egui --bins` 全部通过。
2. 宽屏下三栏同时可见；中屏右栏可折叠；窄屏左列表可折叠。
3. 所有现有面板（设置、Dashboard、Gantt、TaskBoard、Work、审批、登录等）仍可正常渲染。
4. 至少一个组件（建议 `sidebar_card` 或聊天气泡）完成 pretext 测量/换行接入，并通过视觉检查。
5. 文档：更新 `AGENTS.md` Sprint S6 段落；新增/更新 ADR 记录三栏布局决策。

---

## 九、下一步建议

1. 确认本规划中的布局状态机扩展是否符合产品意图。
2. 选择 pretext 接入路径（方案 A 新增 `pretext-egui` crate，或方案 B 直接内嵌）。
3. 从 **Phase A 新版布局外壳** 开始实施，或先做 **pretext PoC** 验证测量精度。
