# Agent 指引 — clarity-egui

## 构建

```bash
cargo build -p clarity-egui
```

## 测试

```bash
cargo test -p clarity-egui --bin clarity-egui
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
- `src/claw.rs` — Claw 设备发现、Bot 实例聚合、Claw 消息发送入口
- `src/panels/settings/claw_tab.rs` — Claw 设置面板（设备发现、配对 token、连接管理）

## 约定

- 错误处理使用自定义 `EguiError`（手动 `Display`，非 `thiserror`），错误以 Toast 形式展示
- 异步使用 `tokio::runtime::Runtime`，长时任务 spawn 到 runtime，结果通过 `std::sync::mpsc`（`UiEvent`）回传
- **热路径规则**：`update()` 中仅允许迭代、算术和 egui 调用；禁止字符串解析 / markdown / I/O / JSON
- UI 架构：即时模式 egui + Zustand 风格 stores
- `render_safe()` 提供 React 式 error boundary，按面板隔离崩溃
- Windows 平台通过 `raw-window-handle` + `windows` crate 实现圆角窗口
- **迁移期宽限**：Sprint S5 / Pretext 三栏外壳迁移完成前，`main.rs` 顶部保留 `#![allow(dead_code)]`；迁移结束后随 `render_layout_shell()` 落地一并移除

## Sprint 43 冻结声明（2026-05-10 起生效）

> **范围**：egui 布局架构重构期间（Phase 2 完成前）
> **目的**：防止在反模式代码上堆叠新债务，确保重构可控

### 禁止项

- ❌ 任何新的 UI 面板或组件
- ❌ 任何涉及 `painter.text()` / `painter.rect_filled()` / `painter.circle_filled()` 在 UI 交互元素中的新代码
- ❌ 任何新的 `ui.interact(rect, ...)` on raw rect
- ❌ 任何新的 `allocate_exact_size(..., Sense::click())` 用于交互组件
- ❌ 任何新的硬编码坐标值 > 8.0px（所有布局常量必须通过 `theme.rs`）
- ❌ 修改 `theme.rs` 中已定义 token 的语义（可补充新 token）

### 例外流程

如需解冻，需提交书面申请说明：
1. 为什么该需求不能在重构后的架构上实现
2. 预计增加的代码行数和涉及文件
3. 是否引入新的 painter / 硬编码坐标

由技术负责人（主会话）审批。

### 当前基线

- `cargo check -p clarity-egui`: 0 errors, 0 warnings（迁移期允许 dead_code）
- `cargo test -p clarity-egui --bin clarity-egui`: 249 passed, 0 failed, 2 ignored
- `cargo test --workspace --lib --exclude clarity-slint`: 全绿
- `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings`: 全绿
- 视觉基线：需手动截图保存（sidebar / titlebar / chat / workspace / settings）
