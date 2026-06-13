---
title: Clarity 设计原则 — CLI→GUI 转译与 Agent 工作流可视化
category: Guide
date: 2026-05-16
tags: [guide, agent, ui]
---

# Clarity 设计原则 — CLI→GUI 转译与 Agent 工作流可视化

> 本文档封装从 Kimi CLI/Claude Code CLI 等终端交互中提取的设计原则，指导 Clarity egui 前端的工程决策。
> 版本：2026-05-05
> 关联：Sprint 19 规划、`docs/architecture/ai-protocol.md`

---

## 一、核心原则：信息本身即界面

CLI 的优雅来自于 **「信息本身即界面」（The content is the interface）**。Clarity 的 egui 也应追求 **「工具输出本身即卡片」**，而不是给输出套一个华丽的容器。

### 1.1 层级区分公式

```
层级区分 = 背景色差异(70%) + 圆角(20%) + 阴影/边框(10%)
```

- 能用背景色区分的地方，绝不用边框
- 必须用边框时，只用 1px，颜色取 `ui.visuals().widgets.noninteractive.bg_stroke`
- 圆角统一：外层大圆角（8-12px），内层小圆角（4-6px）

### 1.2 边框嵌套禁令

| 嵌套层级 | 状态 | 示例 |
|---------|------|------|
| 单层背景色 | ✅ 推荐 | 工具卡片外层背景 + 内容直接渲染 |
| 单层 1px 边框 | ⚠️ 可用 | 输入框、模态弹窗 |
| 双层 Frame | ❌ 禁止 | 外层 group + 内层 group |
| 三层边框 | ❌ 严重 | 弹窗外壳 → 中间 Frame → 输入框 |

---

## 二、CLI→GUI 转译映射

### 2.1 身份头：一轮推理 = 一个身份标识

CLI 做法：
```
Thought for 11s · 971 tokens  ← 一行搞定身份、耗时、消耗
```

GUI 转译：
- 单轮单头：一轮推理只画一次头像，`[A] Agent · 11s · 971 tokens`
- 所有子工具挂在它下面，不再重复画头像
- 违反此原则 = 头像刷屏（见 Issue #16）

### 2.2 工具调用行：最小信息单元

CLI 做法：
```
• Used Shell (cd ...; git status)
  M crates/clarity-core/src/agent/mod.rs
```

GUI 转译：
- 左侧缩进 24px（与头像右边缘对齐）
- 2px 状态色条（琥珀=进行中，绿=成功，红=失败）
- 工具名 + 参数摘要 + 结果预览（右对齐）
- 无边框：直接躺在父级背景上

### 2.3 内容呈现：直接打印，无「卡片」概念

CLI 做法：
```
• Used Shell (git status --short)
M crates/clarity-core/src/agent/mod.rs
```

GUI 转译：
- 代码/文本直接躺在背景色上，最多一层微弱背景区分
- 默认折叠：只显示前 3 行 + 「展开 ▼」
- 长输出（>200 字符）自动截断

### 2.4 状态指示：单字符语义

CLI 做法：
```
⠙ Using Agent      ← 旋转 Braille 点 = 进行中
• Used SetTodoList  ← 静态圆点 = 已完成
✗ Grep failed      ← 叉号 = 失败
```

GUI 转译：
- 进行中：`egui::Spinner::new().size(12.0)` 或旋转圆点
- 已完成：静态圆点 `•`
- 失败：`✗` 或红色圆点
- 颜色编码：蓝/琥珀（进行中），绿（成功），红（失败），黄（警告）

---

## 三、AgentTurn 聚合原则

### 3.1 渲染单元定义

一轮 ReAct 循环 = 一个 `AgentTurn` 渲染单元，内部包含：

```rust
enum RenderBlock {
    User(UserMessage),                          // 用户消息，永远独立
    AgentTurn {
        thinking: Option<ThinkingSummary>,      // 思考过程，默认折叠
        tool_calls: Vec<ToolCallBlock>,         // 聚合的工具调用
        final_response: Option<AgentMessage>,   // 最终回复
        expanded: bool,
    },
}

struct ToolCallBlock {
    tool_name: String,
    status: ToolStatus,        // Running / Success / Error
    result_preview: String,    // 摘要（如 "23 files matched"）
    full_output: String,       // 完整内容，默认折叠
    expanded: bool,
}
```

### 3.2 垂直空间控制

| 场景 | 当前（错误） | 修复后（正确） |
|------|-------------|---------------|
| 3 次 file_read | 3 个 `[A] Agent` 头 + 3 个卡片 = ~360px | 1 个头 + 3 行摘要 = ~120px |
| think 内容 | 原始 JSON 墙 | 折叠为「思考过程 ▼ · 856 tokens」 |
| 10+ 工具调用 | 10 个独立消息 | 单头 + 前 3 个摘要 + 「还有 7 个 ▼」 |

### 3.3 父子层级隐喻

```
[A] Agent · 3 tools used              ← 父节点（身份头）
│  • glob → 23 files                  ← 子节点（缩进 + 色条）
│  • read → deploy-rust-service.md    ← 子节点
│  • read → Cargo.toml                ← 子节点
└─ 最终回复内容                        ← 子节点（左边框或背景色区分）
```

---

## 四、状态语义化：结果情绪点

### 4.1 三层语义模型

| 层级 | 符号 | 语义 | 颜色 |
|------|------|------|------|
| 动作标记 | `⠙` / `•` | 「Agent 做了某事」 | 蓝（进行中）/ 灰暗（已完成） |
| 工具类型 | Shell / WriteFile | 「用哪类能力做」 | 中性色（可配置） |
| 结果判定 | 隐式（下一行） | 「做得怎么样」 | 绿（成功）/ 红（失败）/ 黄（警告） |

### 4.2 egui 实现

```rust
fn result_emotion_color(outcome: &Outcome) -> Color32 {
    match outcome {
        Outcome::Success => Color32::from_rgb(34, 197, 94),      // 绿
        Outcome::Partial { .. } => Color32::from_rgb(245, 158, 11), // 琥珀
        Outcome::Failure { .. } => Color32::from_rgb(239, 68, 68),  // 红
    }
}

// 6px 结果情绪点
ui.painter().circle_filled(rect.center(), 3.0, emotion_color);
```

---

## 五、错误记忆闭环：环境认知 → Agent 自我强化

### 5.1 存储结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolExecutionMemory {
    timestamp: DateTime<Utc>,
    tool_name: String,
    tool_args: serde_json::Value,
    // 环境上下文（本机认知）
    working_dir: PathBuf,
    shell: String,
    os_info: String,
    // 结果
    outcome: Outcome,
    stdout: Option<String>,
    stderr: Option<String>,
    exit_code: Option<i32>,
    // 错误分类
    error_category: ErrorCategory,
}

enum ErrorCategory {
    NotFound,
    PermissionDenied,
    EncodingError,
    Timeout,
    EnvironmentMismatch,
    Unknown,
}
```

### 5.2 认知增强回路

```
Agent 执行工具
    │
    ▼
结果判定（成功/失败/警告）
    │
    ├── 成功 → 轻量记录（统计/优化）
    │
    └── 失败 → 详细捕获（错误类型 + 环境上下文 + 堆栈）
                │
                ▼
        本地存储：~/.clarity/error-memory/YYYY-MM/
                │
                ▼
        实时注入 Agent「世界模型」
                │
                ▼
        下次同类型任务：
        「上次 Grep 因编码问题失败，本次改用 Glob」
```

### 5.3 时间尺度

| 尺度 | 机制 | 用途 |
|------|------|------|
| 短期 | 会话内上下文注入 | 当前 turn 的 system prompt 附加最近 5 条错误 |
| 长期 | 跨会话模式学习 | 启动时加载生成 `EnvironmentCognition` |

---

## 六、具体组件规范

### 6.1 输入框：扁平化

```rust
// 单层背景容器 + 无边框输入区
Frame::none()
    .fill(ui.visuals().extreme_bg_color)
    .rounding(16.0)
    .inner_margin(12.0)
    .show(ui, |ui| {
        let text_edit = TextEdit::multiline(&mut self.input_text)
            .desired_width(ui.available_width() - 48.0)
            .margin(egui::vec2(12.0, 10.0))
            .background_color(ui.visuals().extreme_bg_color) // 与外层同色
            .hint_text("输入消息...");
        ui.add(text_edit);
    });
```

### 6.2 文件预览：覆盖层

```rust
// Area 脱离 Panel 布局系统，独立控制宽度
let screen_rect = ctx.screen_rect();
egui::Area::new(Id::new("file_preview_overlay"))
    .order(egui::Order::Foreground)
    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
    .show(ctx, |ui| {
        let preview_width = 800.0_f32.min(screen_rect.width() * 0.85);
        ui.set_min_width(preview_width);
        // 渲染内容...
    });
```

### 6.3 Thinking Log 侧边栏

```rust
CollapsingHeader::new("Thinking Log")
    .default_open(true)
    .show(ui, |ui| {
        ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                for call in active_turn.tool_calls.iter() {
                    tool_log_row(ui, call);
                }
            });
    });
```

---

## 七、参考

- CLI 风格来源：Kimi CLI、Claude Code CLI、Sprint 18 子代理执行记录
- 相关 Issue：egui 双框线、内容不折叠、头像重复、think JSON 暴露
- 关联文档：`docs/architecture/ai-protocol.md`、`AGENTS.md`
