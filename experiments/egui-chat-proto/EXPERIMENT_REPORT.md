# egui Chat Proto — 实验报告

## 目标

验证问题 B：**egui 能否在 2 周内跑通可对话的聊天窗口？**

> ⚠️ 本实验仅回答"基础聊天 UI 可行性"，不支撑"egui 可完整替代 Tauri"的广义结论。

## 实验环境

- **egui**: 0.31.1
- **eframe**: 0.31.1 (glow backend)
- **OS**: Windows 11
- **构建时间**: ~1.3s (增量 0.16s)
- **依赖树**: 277 crates (含 winit/glow/image 等)

## 已实现功能

| 功能 | 状态 | 说明 |
|------|------|------|
| 消息历史列表 | ✅ | `ScrollArea::vertical()` + `stick_to_bottom` |
| 用户/AI 气泡 | ✅ | `Frame::group()` + 颜色区分 + 左右对齐 |
| 多行输入框 | ✅ | `TextEdit::multiline()` |
| 发送按钮 | ✅ | 基础 `ui.button()` |
| 流式响应模拟 | ✅ | 逐字追加（50ms/批次，3 chars/batch） |
| 打字指示器 | ✅ | "..." 占位气泡 |
| FPS 计数器 | ✅ | 用于性能监控 |
| 退出日志 | ✅ | 保存交互记录到 `chat_proto_log.txt` |

## 关键代码结构

```
App {
    messages: Vec<Message>,      // 历史消息
    input: String,               // 当前输入
    pending_ai: bool,            // 是否等待 AI 响应
    ai_text: String,             // 待流式输出的完整文本
    ai_timer: Option<Instant>,   // 流式定时器
}
```

流式逻辑：`update()` → `tick_ai()` → 每 50ms 追加 3 个字符到末尾消息 → `request_repaint_after(50ms)`。

## 验证结果

### 构建可行性：✅ 通过

- 从 `cargo new` 到可运行窗口：**< 10 分钟**
- Workspace 隔离配置正确（`[workspace]` 空表）
- API 变动已适配：`Rounding::same(f32)` → `CornerRadius::same(u8)`

### 基础 UI 组件：✅ 可用

- `ScrollArea` 自动滚动到底部稳定工作
- `TextEdit::multiline` 支持 Shift+Enter 换行
- `Frame::group().fill().corner_radius()` 气泡样式足够表达
- 左右对齐通过 `Layout::top_down(Align::LEFT/RIGHT)` 实现

### 流式效果：✅ 可用（有边界）

- 逐字追加在视觉上是连续的
- **已知边界**：每次追加触发 `ScrollArea` 重新布局 + 重绘，长文本时可能掉帧（未量化）
- **优化空间**：两阶段分离思想（见 `../egui-streaming-text-proto/`）可应用于此

## 未验证边界（明确声明）

| 边界 | 风险等级 | 说明 |
|------|---------|------|
| Markdown / 代码块渲染 | 🔴 高 | egui 无原生 markdown，需第三方库或自研 parser + `ui.code()` |
| 长消息列表（>1000条）性能 | 🟡 中 | `ScrollArea` 默认渲染全部项，需 `Table` 或虚拟滚动 |
| 真实 LLM 后端连接 | 🟡 中 | 需 `reqwest`/`tokio` 集成，Gateway 已有 HTTP API |
| 富文本（链接、按钮内嵌） | 🟡 中 | `RichText` 有限，复杂排版需 `ui.horizontal_wrapped` |
| CJK/Emoji 字体回退 | 🟡 中 | `default_fonts` feature 覆盖基础，复杂回退待验证 |
| 多面板/侧边栏 | 🟢 低 | `SidePanel`/`TopPanel` 原生支持，布局简单 |
| 主题/深色模式 | 🟢 低 | `ctx.set_visuals()` 一行切换 |
| 文件拖拽/附件 | 🟡 中 | `ctx.input(|i| i.raw.dropped_files)` 原生支持，UI 需设计 |

## 结论

### 问题 B 回答：**是，2 周内可跑通可对话的聊天窗口**

依据：
1. **基础 UI 一天内已验证** — 本原型从空项目到可交互窗口耗时 < 2 小时
2. **架构路径清晰** — 输入→发送→后端→流式响应→追加显示的循环无结构性障碍
3. **Gateway 已存在** — 真实后端连接是"集成"问题而非"攻关"问题

### 但："成为主力栈" ≠ "2 周内可聊天"

egui 要成为 Clarity 的**主力**前端栈，仍需验证：
- Markdown 渲染方案选型（`egui-markdown`? `pulldown-cmark` + 自定义 widget?）
- 长消息列表虚拟滚动性能基准
- 与 `clarity-gateway` 的真实集成（WebSocket / HTTP SSE）
- 跨平台打包一致性（Windows .exe / 未来 Linux）

### 建议路径

```
Week 1: 集成 Gateway HTTP API + 真实流式响应
Week 2: Markdown 渲染 POC + 长列表性能基准
```

若两周内上述三项均达标 → **启动 `clarity-egui` crate 作为 Tauri 替代方案**
若任一失败 → **维持 Tauri 冻结状态，egui 降级为 side project**

## 关联实验

- `../egui-streaming-text-proto/` — 文本布局子系统性能验证（40.7× 加速）
