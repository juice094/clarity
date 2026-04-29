# egui 架构演进计划

> 基于 2026-04-27 深夜技术对话的各方议题整理。
> Phase 1 W5-W6 已交付（Token/Task/Plan），egui 代码膨胀问题加剧，需分阶段治理。

---

## 一、现状诊断

| 指标 | 数值 | 评估 |
|---|---|---|
| `main.rs` 行数 | ~1,550 | 单文件承载 6 个面板 + 状态机 + 事件处理 |
| 总 crate 数 | 8 | clarity-egui 是唯一二进制前端 |
| 前端测试 | 18 | 覆盖 settings/theme/app_state/error，UI 渲染 0 覆盖 |
| 业务逻辑位置 | 80% 在 egui 层 | 聊天/任务/审批/计划的流程控制沉在前端 |
| 跨前端复用 | 0 | egui 代码无法驱动 TUI/Web/Headless |

根因：IMGUI 范式下，**业务逻辑 + 状态管理 + 绘制** 三合一，每加一个功能 `main.rs` 膨胀 50-100 行。

---

## 二、三阶段路线图

### 阶段 1：止血（发布 v0.3.1 + 物理拆分）
**时间：2 天**

目标：停止 `main.rs` 膨胀，建立文件边界，不改运行时行为。

| 任务 | 文件 | 说明 |
|---|---|---|
| 打 tag `v0.3.1` | `CHANGELOG.md` | 汇总 Phase 1 W1-W6 |
| 提取 `panels/` | `chat_panel.rs` / `sidebar_panel.rs` / `task_panel.rs` / `settings_panel.rs` / `mcp_panel.rs` | 每个面板只暴露 `render_x(&mut self, ctx, &state)` |
| 提取 `widgets/` | `message_bubble.rs` / `code_block.rs` / `status_badge.rs` / `input_box.rs` | 纯绘制函数，禁止调 core API |
| 集中状态 | `app_state.rs` | 所有状态从 `main.rs` 迁移，替代 `ui.memory()` 碎片化 |
| 保留入口 | `main.rs` | 只剩 `eframe::App::update()` 路由调度 |

硬性规则：
- `panels/` 禁止直接调 `ui.label()`/`ui.button()`，只能调 `widgets/`
- `widgets/` 禁止调 `clarity-core` API，只接收数据 + 返回事件
- `app_state.rs` 是唯一可变状态源

**验收：clippy 0 警告 + 18 测试通过 + `main.rs` < 300 行。**

---

### 阶段 2：协议驱动试点（1 个面板验证）
**时间：1 周**

目标：验证"后端输出 RenderCommand，前端只翻译"的可行性，选一个简单面板做端到端验证。

#### 2.1 协议定义（`clarity-wire` 扩展）

```rust
// crates/clarity-wire/src/protocol.rs
pub enum ViewCommand {
    VStack { children: Vec<ViewCommand> },
    HStack { children: Vec<ViewCommand> },
    Text  { content: String, role: TextRole },
    Button{ id: String, label: String, enabled: bool },
    Input { id: String, value: String, placeholder: String },
    // ... 约 15-20 种原子控件
}

pub enum UserAction {
    ButtonClick { id: String },
    InputChange { id: String, value: String },
}

pub enum TextRole { Title, Body, User, Agent, Error, Code }
```

#### 2.2 试点面板选择：`settings_panel`

理由：
- 无实时流，无复杂交互
- 状态简单（表单字段 + 保存/取消）
- 失败不影响核心聊天体验

#### 2.3 后端 ViewModel

在 `clarity-core` 新增 `view_models/settings_vm.rs`：
- 接收 `UserAction::InputChange`
- 输出 `Vec<ViewCommand>`
- 通过 `clarity-wire` SPMC 总线投递

#### 2.4 前端翻译层

`clarity-egui/src/protocol_renderer.rs`：
- `ViewCommand::Text` → `ui.label(RichText::new(...))`
- `ViewCommand::Button` → `ui.add_enabled(..., Button::new(...))`
- 样式由本地 `theme.rs` 根据 `TextRole` 映射，后端只发语义

#### 2.5 高频交互本地缓存策略

| 交互 | 协议参与 | 本地处理 |
|---|---|---|
| Hover / 鼠标移动 | ❌ 不上报 | 前端本地计算 |
| 拖拽 | 仅起止点 | 本地跟踪 `drag_delta` |
| 文本输入 | 防抖 200ms | 本地缓冲，防抖后发 `InputChange` |
| 按钮点击 | ✅ 实时 | 立即发 `UserAction::ButtonClick` |
| 滚动 | 停止后 | 本地处理，停止后发 `ScrollPosition` |

**验收：settings_panel 完全由协议驱动，代码量减少 ≥ 50%，clippy 0 警告。**

---

### 阶段 3：全面协议化 + 跨前端复用
**时间：2-3 周**

目标：所有面板走协议，`clarity-egui` / `clarity-tui` / `clarity-gateway` 消费同一协议。

#### 3.1 迁移顺序

按复杂度从低到高：
1. `settings_panel`（已完成试点）
2. `task_panel`（列表 + 按钮，交互简单）
3. `approval_modal`（弹窗，生命周期短）
4. `chat_panel`（核心，最复杂，最后迁移）

#### 3.2 `clarity-tui` 接入

`clarity-tui` 新增 `protocol_renderer_tui.rs`：
- `ViewCommand::Text` → `ratatui::Paragraph`
- `ViewCommand::Button` → `ratatui::widgets::Button`（或模拟）
- 同一套 `clarity-wire` 协议，零业务逻辑重复

#### 3.3 `clarity-gateway` WebSocket 接入

- Axum WebSocket endpoint 消费 `ViewCommand` 流
- Web IDE（React/Vue）渲染，无需调 REST API 拼状态

#### 3.4 预期效果

| 维度 | 协议化前 | 协议化后 |
|---|---|---|
| `clarity-egui` 代码量 | ~1,550 行 | ~400-500 行（纯翻译层） |
| 跨前端复用 | 0% | 业务逻辑 100% 复用 |
| 测试方式 | 必须启 GUI | 直接对协议流断言 |
| 新增面板成本 | 100+ 行 egui | 后端 ViewModel + 协议定义 |

---

## 三、并行事项

### 3.1 Frame + trait 组件封装
**与阶段 1 并行，不阻塞**

在 `widgets/` 层引入 `BubbleStyle` / `StyledWidget` trait：
- 不是架构升级，是代码整洁
- 先 B（物理拆分），观察重复模式，再上 C（trait 封装）

### 3.2 本地模型首体验（T_KALOSM_REAL）
**阻塞条件：agri-paper 7B 数据到达**

数据到达后：
- `.gguf` 文件拖拽选择器
- 加载进度条 + 显存/内存占用显示
- 加载失败友好提示（替代 panic）

### 3.3 unwrap() 削减
**20% 债务时间**

目标：workspace ≤ 150 → 当前 ~150（踩线），冻结新增，逐步降级高频路径。

---

## 四、风险与反叙事

| 风险 | 说明 | 缓解 |
|---|---|---|
| 协议膨胀 | 后端发太多细粒度指令 | RenderTree diff：只发变更节点 |
| 延迟感知 | 输入 → 后端 → 渲染 往返延迟 | 本地乐观 UI：前端先假设，异步校正 |
| 调试复杂度 | 逻辑在后端，无法单步前端 | 协议流日志化 + 可回放 |
| egui 生态不兼容 | egui_dock / egui_table 不消费协议 | 第三方库保持直接调用逃生舱 |
| IMGUI 本质冲突 | egui 每帧主动查询 vs 协议被动接收 | 高频交互本地缓存，低频走协议 |

---

## 五、一句话结论

**先 B 止痛（今晚拆文件），后 A 根治（本周协议试点）。C（trait 封装）是锦上添花，不阻塞主线。**
