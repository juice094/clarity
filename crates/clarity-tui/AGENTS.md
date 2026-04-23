# Agent 指引 — clarity-tui

## 构建

```bash
cargo build -p clarity-tui
```

## 测试

```bash
cargo test -p clarity-tui --lib
```

## 关键文件

- `src/main.rs` — 终端初始化、Agent 创建、`run_app` 入口
- `src/app.rs` — `App` 状态机与按键/命令处理
- `src/ui.rs` — `draw` 主界面渲染
- `src/events.rs` — `EventHandler` 终端事件捕获
- `src/widgets/` — ratatui 自定义组件（聊天区、输入框、状态栏等）
- `src/popups/` — 弹窗实现（Diff、帮助、工具结果）

## 约定

- 错误处理使用 `anyhow`
- 异步使用 `tokio`
- 终端原始模式在 `main` 中统一进入/恢复，避免 panic 后终端状态异常
- 命令以 `/` 开头，注册在 `CommandRegistry`
