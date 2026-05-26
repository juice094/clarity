---
title: RenderLine Pipeline — ADR-012 Implementation Notes
category: Architecture
date: 2026-05-16
tags: [architecture, rendering]
---

# RenderLine Pipeline — ADR-012 Implementation Notes

> **Scope**: S4 Phase 2A → S6 Phase 2C | **Status**: Complete | **Owner**: juice094

## 1. 设计目标

将聊天内容的渲染从**块级**（`RenderBlock`：Paragraph / CodeBlock / Table ...）迁移到**行级**（`RenderLine`），实现：
- 10K 消息 @ 60fps 的虚拟滚动
- 前端无关的中间表示（GUI/TUI 共享同一套 `Vec<RenderLine>`）
- 键盘导航的原子单位（j/k 以行为粒度）

## 2. 类型体系

### 2.1 RenderLine — 13-variant 枚举

```rust
pub enum RenderLine {
    Text { spans: Vec<Span>, role: LineRole, indent: u8 },
    CodeLine { lang: SmolStr, content: SmolStr, line_no: Option<u32>, diff: DiffKind },
    ToolCallHeader { name: SmolStr, status: ToolStatus, expanded: bool },
    ToolCallArg { key: SmolStr, value: SmolStr },
    Thinking { content: SmolStr, collapsed: bool },
    ApprovalPrompt { options: Vec<ApprovalOption> },
    StatusLine { kind: StatusKind, content: SmolStr, transient: bool },
    ArtifactRef { artifact_id: ArtifactId, summary: SmolStr },
    CrossInstanceRef { target_instance: InstanceId, target_session: Option<SessionId>, message: SmolStr },
    SlashCompletion { command: SmolStr, description: SmolStr },
    StreamingCursor,
    Divider,
    Empty,
    BlockSlot { block_id: BlockId, line_count: u8 },
}
```

**变体分类**:
| 类别 | 变体 | 说明 |
|------|------|------|
| 文本流 | `Text`, `Empty`, `Divider` | 占主体 90%+ |
| 代码块 | `CodeLine` | 每行独立，支持行号 + diff 高亮 |
| 交互控件 | `ToolCallHeader`, `ToolCallArg`, `ApprovalPrompt`, `SlashCompletion` | 可点击/可折叠 |
| 元信息 | `Thinking`, `StatusLine`, `ArtifactRef`, `CrossInstanceRef` | 语义标注 |
| 占位符 | `BlockSlot` | 表格/图片等无法行原子化的逃生舱 |
| 光标 | `StreamingCursor` | 流式输出末尾闪烁符 |

### 2.2 LineRole — 语义角色着色

```rust
pub enum LineRole {
    UserMessage, AgentMessage, SystemMessage, ErrorMessage,
    Heading(u8), Quote,
    UnorderedListItem(u8), OrderedListItem { num: u32, indent: u8 },
    Mention, FileRef, Status, Warning, Note,
    TokenUsage, ContextCompaction, Sandbox,
}
```

**关键决策**: `markdown_to_lines()` 默认输出 `AgentMessage`；`Message::prepare()` 根据 `self.role == Role::User` 将段落级 `AgentMessage` 批量替换为 `UserMessage`，保证用户消息使用用户主题色。

## 3. 转换管道

```
Markdown 源文本
    │
    ▼
pulldown-cmark Event 流
    │
    ▼
markdown_to_lines() ──► Vec<RenderLine>
    │                      • role_stack 跟踪嵌套上下文
    │                      • BlockQuote → Quote
    │                      • ListItem → ListItem
    │                      • Table/Image → BlockSlot fallback
    │
    ▼
Message::prepare() ──► msg.lines ( alongside msg.parsed )
    │
    ▼
egui: line_renderer::render_lines() ──► 固定行高虚拟滚动
TUI: (待 S8 接入)
```

### 3.1 role_stack 模式

```rust
let mut role_stack: Vec<LineRole> = Vec::new();
let mut current_role = LineRole::AgentMessage;

// BlockQuote Start
role_stack.push(current_role);
current_role = LineRole::Quote;

// BlockQuote End
current_role = role_stack.pop().unwrap_or(LineRole::AgentMessage);
```

**解决的问题**: 段落结束时的 `flush` 必须使用正确的嵌套角色，而非默认 `AgentMessage`。

### 3.2 BlockSlot Fallback (S6 P2C.5)

| 元素 | 触发事件 | 生成的 RenderLine |
|------|----------|-------------------|
| Markdown Table | `Start(Tag::Table)` / `End(TagEnd::Table)` | `BlockSlot { block_id: "table", line_count: rows }` |
| Markdown Image | `Start(Tag::Image)` / `End(TagEnd::Image)` | `BlockSlot { block_id: "image", line_count: 3 }` |

**设计理由**: `RenderLine` 体系当前不支持原生表格/图片渲染。`BlockSlot` 作为占位符，UI 层显示为 "⤢ Block table (N lines) — click to expand"，为 S8+ 的专用渲染器预留扩展点。

## 4. 虚拟滚动

### 4.1 前端无关层 (clarity-core)

```rust
pub struct LineViewport {
    pub line_height: f32,      // 主题 token，egui 当前为 18.0
    pub scroll_offset: f32,    // 像素偏移
    pub viewport_height: f32,  // 可视区高度
}

impl LineViewport {
    pub fn visible_range(&self, total_lines: usize) -> (usize, usize) {
        let start = (scroll_offset / line_height).floor() as usize;
        let visible_count = (viewport_height / line_height).ceil() as usize;
        let end = (start + visible_count + 2).min(total_lines); // +2 overscan
        (start.min(total_lines), end)
    }
}
```

### 4.2 egui 实现 (clarity-egui)

```rust
pub fn render_lines(
    ui: &mut egui::Ui,
    lines: &[RenderLine],
    theme: &Theme,
    scroll_offset: f32,
    viewport_height: f32,
    selected_idx: Option<usize>, // S7: 选中行高亮
) {
    let (start_idx, end_idx) = compute_visible_range(lines.len(), scroll_offset, viewport_height, LINE_HEIGHT);
    for (idx, line) in lines.iter().enumerate().take(end_idx).skip(start_idx) {
        let is_selected = selected_idx == Some(idx);
        // ... render_line(ui, line, theme, is_selected)
    }
}
```

**当前限制**: ChatArea 的滚动容器仍是消息级 `ScrollArea`（每个消息气泡内部调用 `render_lines`，`scroll_offset=0`）。完全行级滚动需要将所有消息的 `lines` 扁平化为单一列表，涉及操作按钮行化，计划在 S8 评估。

## 5. 行级导航 (S7 Phase 2D)

```rust
pub struct LineCursor {
    pub selected: Option<usize>, // None = 导航休眠
    pub total: usize,
}

impl LineCursor {
    pub fn move_down(&mut self) { /* j */ }
    pub fn move_up(&mut self)   { /* k */ }
    pub fn move_top(&mut self)  { /* g */ }
    pub fn move_bottom(&mut self) { /* G */ }
    pub fn clear(&mut self)     { /* Esc */ }
}
```

**全局 ↔ 局部索引映射**: `message_list.rs` 每帧计算 `msg_line_offsets: Vec<usize>`，将全局 `line_cursor_selected` 映射为每个消息气泡内的局部 `selected_idx` 传给 `render_lines`。

## 6. 冻结项与后续

| 项 | 状态 | 后续计划 |
|----|------|----------|
| 原生表格渲染 | `BlockSlot` 占位 | S8+ 评估是否接入 `RenderBlock::Table` 回退 |
| 原生图片渲染 | `BlockSlot` 占位 | 同上 |
| 完全行级滚动 | 消息级 `ScrollArea` + 行级内容 | S8 评估操作按钮行化可行性 |
| TUI 接入 | 待实现 | S8 Phase 3A |
