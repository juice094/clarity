# UI Axis — Grid-vs-Cursor Classification & Dual-Track Rendering

> **Scope**: S3-S7 架构总览 | **Status**: S3-S7 Complete, S8 TUI pending | **Owner**: juice094

## 1. 核心问题：GUI 与 TUI 的范式差异

| 维度 | GUI (egui) | TUI (ratatui) |
|------|-----------|---------------|
| 布局模型 | 浮动/叠加/弹性（Y 轴自由） | 网格/固定行（Y 轴离散） |
| 输入设备 | 鼠标 + 键盘 | 纯键盘 |
| 滚动单位 | 像素（连续） | 行（离散） |
| 虚拟化 | 像素裁剪 + 行裁剪 | 行裁剪 |
| 信息密度 | 中等（间距、阴影、图标） | 高（字符画、ANSI 颜色） |

**统一策略**: 在 `clarity-core` 中定义前端无关的**信息架构层**（`ViewState` + `RenderLine`），两端各自做渲染适配。

## 2. 双轨渲染策略 (S6 Phase 2C)

```
Message::prepare()
    ├── msg.parsed  = parse_markdown() → Vec<RenderBlock>  【旧路径】
    └── msg.lines   = markdown_to_lines() → Vec<RenderLine> 【新路径】
```

### 2.1 控制开关

`clarity-egui/Cargo.toml`:
```toml
[features]
default = []
line-mode = []
```

- **默认（无 flag）**: `message_bubble()` → `agent_message()` / `user_bubble()` → `RenderBlock` 渲染
- **line-mode**: `message_bubble()` → `line_mode_agent()` / `line_mode_user()` → `render_lines()`

**设计理由**: 新路径未经过 10K 消息压力验证前，保留旧路径作为安全回退。

### 2.2 当前混合状态

| 区域 | line-mode OFF | line-mode ON |
|------|--------------|--------------|
| ChatArea 内容 | `RenderBlock` 气泡 | `RenderLine` 行列表 |
| ChatArea 滚动 | 消息级 `ScrollArea` | 消息级 `ScrollArea`（内容改为行级） |
| Workspace 预览 | `markdown::render_blocks` | `render_lines` |
| Sidebar | 未改动 | 未改动 |

**关键差距**: ChatArea 的滚动容器仍是**消息级**，而非完全行级。完全行级需要：
1. 操作按钮（编辑/复制/重新生成）行化
2. 时间戳行化
3. 统一固定行高（当前仅内容行固定 18px，操作栏不固定）

**决策**: 保持消息级滚动框架 + 行级内容渲染（当前状态），直到用户反馈证明需要完全行级滚动。

## 3. Grid-vs-Cursor 分类

| 组件 | 分类 | 说明 |
|------|------|------|
| 消息列表 | **Cursor 为主** | `LineCursor` + j/k 导航，但保留鼠标滚轮 |
| 会话侧边栏 | **Grid 为主** | 分类卡片、树形结构，点击驱动 |
| 文件浏览器 | **Grid 为主** | 树形节点，点击展开 |
| 命令面板 | **Cursor** | 上下箭头 + Enter 选择 |
| 设置面板 | **Grid** | 表单字段，Tab 切换 |
| Approval Modal | **Cursor** | 1/2/3 数字键选择 |
| Web Tabs | **Grid** | 标签列表，点击切换 |

**原则**: 文本流区域用 Cursor 模型（键盘优先），配置/管理区域用 Grid 模型（鼠标/Tab 优先）。

## 4. 前端共享层

```
clarity-core/src/ui/
├── view_state.rs    ──► ViewState, AppView, SidePanel, ModalType, TurnState, FocusScope
├── render_line.rs   ──► RenderLine, LineRole, Span, LineViewport, LineCursor
├── shortcut.rs      ──► KeyEvent, ShortcutBinding, ShortcutRegistry
├── commands.rs      ──► CommandItem, CommandScope, ids, built_in
└── ids.rs           ──► 全局命令 ID 常量
```

**约束**: `clarity-core` 不依赖任何前端 crate（egui/ratatui/crossterm），只提供**语义类型**和**纯逻辑**（如 `LineViewport::visible_range()` 是无副作用的数学计算）。

## 5. TUI 接入计划 (S8 Phase 3A)

```
clarity-tui
    ├── app.rs          ──► 状态容器（复用 ViewState）
    ├── ui.rs           ──► ratatui 布局
    ├── render_line.rs  ──► RenderLine → ANSI/字符画（新文件）
    └── shortcuts.rs    ──► crossterm KeyEvent → KeyEvent（复用 core）
```

**关键适配点**:
1. `RenderLine::Text` → 带 ANSI 颜色的文本行
2. `RenderLine::CodeLine` → 语法高亮（syntect 或 tree-sitter）
3. `RenderLine::Divider` → `─` 重复填充
4. `LineViewport::visible_range()` → 直接复用，TUI 行高 = 1
5. `ShortcutRegistry` → 复用 `resolve()`，只需将 crossterm 按键翻译为 `KeyEvent`

**Snapshot 测试策略**:
- 同一 markdown fixture → `markdown_to_lines()` → 两端渲染 → 纯文本 assert 匹配
- 不关心颜色/样式，只验证**文本内容一致性**

## 6. 性能基线（待 S9 验证）

| 指标 | 目标 | 当前状态 |
|------|------|----------|
| GUI 60fps @ 10K lines | 滚动不掉帧 | 骨架就绪，未压测 |
| TUI 60Hz @ 10K lines | 无闪烁 | 未实现 |
| 内存/消息 | < 1MB / 1K messages | `RenderLine` 使用 `SmolStr`，理论上满足 |
| 启动时间 | < 2s | 基准缺失 |

**压测方案** (S9):
1. 生成 10K 条消息的会话 JSON
2. `cargo run --features line-mode --release`
3. 使用 `egui::Context::run` 的 frame time 输出
4. 目标：p99 frame time < 16.6ms
