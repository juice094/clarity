# 前端架构对比：Clarity-egui vs OpenHanako

> 范围：代码组织结构、状态管理、关注点分离、错误处理
> 日期：2026-05-01
> 基准：openhanaka `desktop/src/react` vs clarity-egui `crates/clarity-egui/src`

---

## 一、架构对比矩阵

| 维度 | OpenHanako (React/Zustand) | Clarity-egui (Rust/egui) | 严重程度 |
|------|---------------------------|-------------------------|---------|
| **状态管理** | Zustand slice 模式：20+ 个自治 slice，按领域切分 | God Object `App`：50+ 字段全部堆在一个 struct | 🔴 高 |
| **组件边界** | 组件接收 props + hooks，纯渲染职责 | Panels 直接接收 `&mut App`，无数据边界 | 🔴 高 |
| **关注点分离** | `stores/` `components/` `hooks/` `utils/` `services/` 五层分离 | `app_logic.rs` 800+ 行混合业务逻辑、事件处理、异步任务 | 🔴 高 |
| **错误边界** | `ErrorBoundary` + `RegionalErrorBoundary` 隔离组件 panic | 无隔离：单个 panel panic 导致整个进程崩溃 | 🔴 高 |
| **事件分发** | 纯分发器，action handler 分布在各 slice | 200+ 行 `process_events` match，所有事件挤在一个方法 | 🟡 中 |
| **性能优化** | `memo` + alive list + `visibility:hidden` 保持状态 | 虚拟列表已存在，但无组件级缓存策略 | 🟡 中 |
| **样式系统** | CSS Modules + CSS Variables，局部作用域 | `Theme` struct 全局应用，无局部覆盖 | 🟢 低 |

---

## 二、违反的成熟前端理论

### 1. SOLID — Single Responsibility Principle
`App` struct 同时承担：会话管理、聊天状态、设置持久化、任务调度、MCP 配置、子代理进度追踪、UI 动画帧计数等 8+ 个职责。

**修复路径**：按领域提取嵌套 store（SessionStore / ChatStore / SettingsStore / TaskStore / UiStore）。

### 2. Flux / Redux — 单向数据流
Clarity 没有明确的数据流方向：
- Panels 直接 mutate `App` 字段
- `process_events` 直接修改任意字段
- 没有 action → reducer → state 的规范路径

**修复路径**：将 `process_events` 改造为纯分发器，事件 handler 按领域归属到对应 store。

### 3. 关注点分离 (Separation of Concerns)
`app_logic.rs` 同时包含：
- Tokio runtime 初始化
- 网络探针循环
- LLM 预加载
- 消息发送（`send()`）
- 事件处理（`process_events`）
- 会话 CRUD
- 设置保存

**修复路径**：拆分为 `services/`（副作用）+ `stores/`（纯状态）+ `utils/`（纯函数）。

### 4. 错误边界 (Error Boundaries)
React 的 `componentDidCatch` 允许子树 panic 不摧毁整棵树。egui 无此机制，但可用 `std::panic::catch_unwind` 模拟。

**修复路径**：在 `update()` 的每个 panel render 外层包裹 `catch_unwind`。

---

## 三、本次修复内容（Sprint 13.5 热修复）

### ✅ 已执行

#### 1. 领域分组注释（`main.rs`）
将 `App` 的 50+ 字段按 8 个领域分组，为后续提取嵌套 store 做准备：
```
Core Runtime / Session Domain / Chat Domain / UI Domain /
Settings Domain / Task Domain / SubAgent Domain / MCP Domain / Onboarding Domain
```

#### 2. 错误边界（`main.rs`）
新增 `render_safe()` 方法，用 `std::panic::catch_unwind` 隔离 panel panic：
- 单个 panel panic → Toast 通知 + 日志记录，应用不崩溃
- 已覆盖：titlebar / sidebar / chat / settings / task / skill / mcp / toast / approval / task_create

#### 3. 事件处理分离（`app_logic.rs`）
将 200+ 行的 `process_events` match 拆分为 15 个独立方法：
- `on_chunk` / `on_tool_start` / `on_tool_result` / `on_step_begin`
- `on_compaction_begin` / `on_compaction_end`
- `on_done` / `on_error` / `on_fallback`
- `on_task_list` / `on_subagent_batch` / `on_usage`
- `on_plan_ready` / `on_plan_step_begin` / `on_plan_step_end`
- `on_provider_test_result` / `on_provider_model_list`

`process_events` 现在是一个**纯分发器**（Pure Dispatcher），只负责 match → 转发。

### 📊 验证结果
- `cargo check -p clarity-egui`：✅ 通过（2 pre-existing warnings）
- `cargo test --workspace --lib`：✅ 584 passed, 0 failed, 6 ignored

---

## 四、剩余重构债务（需后续 Sprint）

### 🔴 高优先级
1. **提取嵌套 Store structs**：将字段组真正提取为独立 struct（而非仅注释），减少 `App` 的字段数量
2. **Panel 签名改造**：`render_chat_area(app: &mut App, ctx: &egui::Context)` → `render_chat_area(store: &mut ChatStore, ctx: &egui::Context)`
3. **Services 拆分**：将 `send()` / `poll_parallel_batches()` / `refresh_tasks()` 提取到 `services/` 目录

### 🟡 中优先级
4. **虚拟列表缓存优化**：为消息气泡添加 `egui::Id` + `ui.memory` 缓存，减少每帧重排
5. **Settings store 持久化解耦**：Settings 保存不应由 UI 直接触发，应通过事件队列异步写入

### 🟢 低优先级
6. **Theme 局部覆盖**：允许 panel 级别覆盖 theme token（模仿 CSS Modules）
7. **i18n 完整化**：当前仅有 English，需补充 `locales/` 目录

---

## 五、参考模式映射（OpenHanako → Clarity-egui）

| OpenHanako 模式 | Clarity-egui 对应 | 状态 |
|----------------|------------------|------|
| `stores/index.ts` (Zustand slice 组合) | `App` struct 领域分组注释 | 🟡 过渡中 |
| `stores/chat-slice.ts` (per-session LRU) | `ChatStore` (待提取) | 🔴 未开始 |
| `components/ErrorBoundary.tsx` | `render_safe()` + `catch_unwind` | ✅ 已完成 |
| `App.tsx` 纯布局编排 | `update()` 纯 render 调度 | 🟡 过渡中 |
| `services/websocket.ts` | `app_logic.rs` 中的 async spawn | 🔴 未开始 |
| `hooks/use-sidebar-resize.ts` | `SidebarLayout` inline logic | 🔴 未开始 |

---

*本报告由代码健康维护会话生成，修复实施已验证编译与测试通过。*
