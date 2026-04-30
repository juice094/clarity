# Sprint 12 风险点与优化点分析

> 状态: Plan 模式
> 日期: 2026-04-28
> 前置: Sprint 12 规划已确认，V2 验证通过

---

## 一、审批弹窗 UI（🔴 P0，阻塞 Interactive/Plan 模式可用性）

### 技术风险

**R1.1 egui 无原生模态阻塞机制**
- egui 的 `App::update()` 是每帧回调，无法像 TUI 那样用 `loop { popup.draw(); popup.handle_event(); }` 阻塞主事件循环
- 必须用状态机方案：`pending_approval: Option<ApprovalRequest>` 暂停主界面交互，强制用户处理弹窗
- **影响**: 状态机设计错误会导致弹窗无法关闭或主界面卡死

**R1.2 Wire 事件总线单向性**
- core emit `ApprovalRequest` → egui 接收 → 用户选择 → egui emit `ApprovalResponse`
- 若 egui 未正确订阅 ApprovalRequest 事件，或 ApprovalResponse 丢失，Agent 的 `wait_for_response()` 将永久阻塞
- **影响**: 死锁，Agent 无响应，用户只能强退

**R1.3 流式输出与审批弹窗并发**
- Agent 可能处于 `run_streaming` 状态（流式输出中）时触发 tool_call → 需要审批
- 需同时处理：暂停流式回调 + 显示弹窗 + 用户确认后恢复流式
- **影响**: 状态复杂度倍增，容易出现 race condition

**R1.4 `_diff_patch` 字符串着色渲染**
- egui 的 `ui.label()` / `RichText` 支持逐行着色，但不如 ratatui 的 `Line::from(vec![Span::styled(...)])` 灵活
- 需逐行解析 patch 字符串，为 `+`/`-`/`@@` 前缀分配不同颜色
- **影响**: 渲染代码冗长，性能在超大 diff 时可能下降

### 架构风险

**R1.5 `clarity-wire` 协议扩展**
- 审批弹窗需要新增 `WireMessage::ApprovalRequest` / `ApprovalResponse` 变体
- 根据 AGENTS.md 跨层变更检查单：需同步修改 egui `protocol_renderer.rs`、TUI `protocol_renderer.rs`、Gateway `ws.rs`
- **影响**: 牵一发而动全身，TUI 和 Gateway 可能被迫同步修改

**R1.6 `ViewCommand` 协议扩展 vs Wire 原生事件**
- 方案 A: 扩展 `ViewCommand` 枚举（新增 `Modal`/`ApprovalRequest`），通过现有 wire 通道传输
- 方案 B: 直接扩展 `WireMessage`，绕过 `ViewCommand` 层
- 方案 A 更符合 Phase 2b 的协议化方向，但延迟更高；方案 B 更直接，但破坏分层

### 优化点

**O1.1 复用 TUI `from_patch` 解析逻辑**
- 将 `diff_popup.rs` 中的 patch 解析下沉到 `clarity-core` 作为 `format_diff_patch(patch: &str) -> Vec<DiffLine>`
- TUI 和 egui 共用同一套解析结果，避免重复实现

**O1.2 审批弹窗最小可用原型**
- MVP 只显示文件名 + 三行 diff 摘要 + "确认/取消" 按钮
- 不追求完整 diff 滚动，先解决"有没有"的问题

**O1.3 快捷键统一**
- Enter = 确认, Esc = 取消 — 与 TUI DiffPopup 保持一致

---

## 二、Plan 步骤可视化（🔴 P0，阻塞 Plan 模式可用性）

### 技术风险

**R2.1 Plan 执行顺序与可视化状态同步**
- `execute_plan()` 当前是顺序 for 循环，步骤状态变化（Pending → Running → Done/Failed）需要实时 emit 事件
- 但 Plan 执行在 Agent 的 async 任务中，egui 在主线程，状态同步需要跨线程
- **影响**: 步骤状态可能滞后或不同步

**R2.2 Plan 数据结构跨 crate 重复**
- `Plan` 和 `PlanStep` 定义在 `clarity-core/src/agent/plan.rs`
- egui 需要展示步骤列表，可能需要在前端重新定义类似的序列化结构
- **影响**: 数据结构变更时需要多处同步修改

**R2.3 步骤间数据依赖的隐式性**
- Plan 步骤 B 可能依赖步骤 A 的输出（如文件路径）
- 当前这种依赖是隐式的（靠顺序保证），可视化时用户无法理解"为什么步骤 B 失败了"
- **影响**: 需要额外显示步骤输入/输出，增加复杂度

### 架构风险

**R2.4 Plan 执行与 UI 渲染耦合**
- 如果 Plan 执行被暂停（等待用户单步确认），`execute_plan()` 需要支持"断点"机制
- 当前 `execute_plan()` 是连续执行的，没有中断点
- **影响**: 需要重构 Plan 执行流程，增加 `StepBreakpoint` 或类似机制

### 优化点

**O2.1 Plan 执行事件化**
- `execute_plan()` 每执行一步 emit `WireMessage::PlanStepBegin { step_id, tool_name }` 和 `WireMessage::PlanStepEnd { step_id, result }`
- egui 只订阅事件，不直接调用 Plan 逻辑

**O2.2 MVP 只做步骤列表**
- 第一版只显示：步骤编号 + 工具名 + 状态图标（⏳/✅/❌/⏭️）
- 不做 DAG 图、不做步骤详情展开

---

## 三、Skill 面板（🟡 P1）

### 技术风险

**R3.1 `SkillRegistry` API 暴露不足**
- 当前 `SkillRegistry` 只有 `active_ids()`、`discover_for_path()`、`get(id)`
- egui 需要列出所有可用 Skill（包括未激活的），但 `skills: HashMap` 是私有的
- **影响**: 需要新增 `list_all()` 或 `iter()` 等遍历 API

**R3.2 Skill 元数据展示**
- `SkillMeta` 包含 `name`、`description`、`paths`、`tools`
- `paths` 是 glob 模式字符串，`tools` 是工具白名单
- egui 需要友好展示这些信息，但当前没有专门的 UI 组件

### 优化点

**O3.1 自动发现 + 手动覆盖**
- 面板显示两部分：① 自动发现的 Skill（基于当前文件路径）② 手动激活的 Skill
- 用户可手动激活/停用，不受自动发现限制

---

## 四、Token 用量显示（🟡 P1）

### 技术风险

**R4.1 `Usage` WireMessage 已存在但可能未处理**
- `agent/run.rs` 在 turn 结束时 emit `WireMessage::Usage { prompt_tokens, completion_tokens, total_tokens }`
- egui 当前可能没有处理这个事件类型
- **影响**: 需要检查 egui 的事件分发逻辑，确认 `Usage` 事件被消费

**R4.2 累计用量 vs 单轮用量**
- 显示当前轮次的用量简单，但用户更关心会话累计用量
- 累计用量需要持久化，否则切换会话后丢失
- **影响**: 需要新增会话级用量累加逻辑

### 优化点

**O4.1 先做单轮用量**
- MVP 只显示最近一轮的 `prompt / completion / total`
- 累计用量延期到 Sprint 13

---

## 五、跨交付项架构风险

### R5.1 egui 零单元测试
- `clarity-egui` 当前 0 个单元测试
- 审批弹窗、Plan 面板、Skill UI 都是交互密集型功能，纯 UI 测试困难
- **缓解**: 新增纯逻辑测试（审批状态机、Plan 步骤解析、Skill 激活逻辑），UI 渲染通过人工验证

### R5.2 事件总线负载
- Sprint 12 同时增加 ApprovalRequest、PlanStepUpdate、Usage 三类事件
- `clarity-wire` 的通道容量（默认 100）可能在快速事件爆发时溢出
- **缓解**: 增加通道容量或添加背压机制

### R5.3 编译时间回归
- egui 代码量增加 → 编译时间增加
- 当前 `cargo build -p clarity-egui` 已需要较长时间
- **缓解**: 保持模块化，避免 monolithic `update()` 回归

---

## 六、关键决策点（L1，需人类裁定）

| # | 决策 | 选项 A | 选项 B | 推荐 |
|---|------|--------|--------|------|
| D1 | 审批弹窗架构 | 扩展 `ViewCommand` 协议（Modal 变体） | 扩展 `WireMessage` 原生事件 | A（符合 Phase 2b 协议化方向） |
| D2 | Plan 可视化深度 | MVP 步骤列表 + 状态图标 | 一步到位 DAG 图 + 详情展开 | A（先闭环再抛光） |
| D3 | diff 解析复用 | TUI `from_patch` 下沉到 core | TUI/egui 各自实现 | A（消除重复） |
| D4 | Skill 列表 API | `SkillRegistry` 新增 `list_all()` | 前端通过 wire 请求列表 | A（简单直接） |

---

## 七、风险矩阵总览

| 风险 | 交付项 | 概率 | 影响 | 缓解 |
|------|--------|------|------|------|
| 审批弹窗状态机错误 | 审批 UI | 中 | 高 | 先写纯状态机测试，再写 UI |
| Wire 事件死锁 | 审批 UI | 低 | 高 | 增加超时机制（ApprovalRequest 30s 未响应自动拒绝） |
| Plan 执行无法中断 | Plan 可视化 | 中 | 高 | 重构 `execute_plan()` 为事件驱动，非连续循环 |
| egui 编译时间回归 | 全部 | 中 | 低 | 保持模块化，新增功能独立文件 |
| 零测试导致回归 | 全部 | 高 | 中 | 每个交付项至少 1 个纯逻辑测试 |
| TUI/Gateway 被迫同步改 | 审批 UI | 中 | 中 | 协议变更前与 TUI/Gateway 调用方对齐 |

---

> 本分析受能力汇流审计协议 v1.0 统辖。风险评级为工程启发式，非定量模型。
