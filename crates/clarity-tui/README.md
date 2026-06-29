# clarity-tui

Clarity 终端交互界面：基于 `ratatui` 的异步 TUI 前端，直接连接 clarity-core Agent。

## 职责

- **终端渲染** — 使用 ratatui 绘制状态栏、聊天区、输入框、命令栏与弹窗
- **事件循环** — 集成 crossterm 处理键盘、鼠标、窗口resize与异步事件
- **Agent 集成** — 通过 `AgentController` 发送操作，经由 `clarity-wire` 接收流式响应
- **命令系统** — 内置 `/help`、`/model`、`/skill`、`/task`、`/plan`、`/execute`、`/parallel` 等命令
- **弹窗交互** — 工具结果弹窗、Diff 预览弹窗、帮助弹窗
- **后台任务** — 支持与 Gateway 共享的 `BackgroundTaskManager`，可在 TUI 内管理后台任务

## 关键类型

- `App` — 应用状态机，持有消息历史、输入框、Agent 实例与事件发送器
- `run_app` — 主事件循环，驱动渲染与事件分发
- `ui::draw` — 主界面渲染函数
- `EventHandler` — 跨平台终端事件捕获与异步分发
- `InputPane` — 多行输入框组件，支持历史记录与光标移动

## 测试

```bash
cargo test -p clarity-tui --lib
```

## 边界与稳定性

- **Stability tier**: Experimental
  - Experimental: API may change before v0.4.0
- **MSRV**: 1.85（跟随 workspace）
- **反向依赖禁止** (No reverse dependencies):
  - 可依赖 clarity-core + clarity-wire
- **Library/binary classification**:
  - Library-only: has lib for parse module
