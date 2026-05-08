# Agent 指引 — clarity-egui

## 构建

```bash
cargo build -p clarity-egui
```

## 测试

```bash
cargo test -p clarity-egui --lib
```

## 关键文件

- `src/main.rs` — `eframe::App` 实现、`update()` 热路径、自定义标题栏、panic 隔离
- `src/app_state.rs` — `AppState`（包装 `clarity_core::Agent`、LLM 绑定、后台管理器、记忆存储）
- `src/app_logic.rs` — `App::new()`、事件处理、会话管理、MCP 热重载
- `src/stores/mod.rs` — Zustand 风格切片状态（`ChatStore`、`SessionStore`、`SettingsStore`…）
- `src/handlers/mod.rs` — `process_events()`：中心 `UiEvent` 分发器
- `src/error.rs` — `EguiError` 枚举（SettingsLoad、LlmLoad、NetworkUnavailable…）
- `src/llm_loader.rs` — 异步 LLM 加载器（云端 → registry → factory，本地 GGUF 回退）
- `src/theme.rs` — 设计系统：颜色、字号、圆角

## 约定

- 错误处理使用自定义 `EguiError`（手动 `Display`，非 `thiserror`），错误以 Toast 形式展示
- 异步使用 `tokio::runtime::Runtime`，长时任务 spawn 到 runtime，结果通过 `std::sync::mpsc`（`UiEvent`）回传
- **热路径规则**：`update()` 中仅允许迭代、算术和 egui 调用；禁止字符串解析 / markdown / I/O / JSON
- UI 架构：即时模式 egui + Zustand 风格 stores
- `render_safe()` 提供 React 式 error boundary，按面板隔离崩溃
- Windows 平台通过 `raw-window-handle` + `windows` crate 实现圆角窗口
