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

---

## 八、Phase 1 交付审计（2026-04-28 执行后）

### 已缓解风险

| 风险 | 缓解措施 | 状态 |
|------|---------|------|
| R1.1 状态机错误 | `pending_approvals` 每帧刷新 + 全屏 blocker 拦截点击 + 主 UI 快捷键跳过 | ✅ 已缓解 |
| R1.2 Wire 事件死锁 | 未走 Wire 事件，直接通过 `InMemoryApprovalRuntime` 共享状态（`Arc` + `Mutex`） | ✅ 已缓解 |
| R1.4 diff 着色 | `parse_unified_diff` + `flatten_hunks` + egui `RichText::color` | ✅ 已缓解 |
| R5.1 零测试 | `diff.rs` 新增 5 个单元测试；approval 弹窗逻辑为纯 UI，暂无自动测试 | 🟡 部分缓解 |

### 新发现风险（Phase 1 执行后）

**R1.P1 `preview_file_edit_diff` 与 `FileEditTool::execute` 逻辑漂移**
- 两处独立实现了相同的 batch/legacy 替换逻辑
- 未来修改 `FileEditTool` 行为时，`preview` 不会自动同步，导致审批 diff 与实际 diff 不一致
- **缓解**: 将替换逻辑提取为 `clarity-core::tools::file_edit::simulate_replacement`，供 preview 和 execute 共用

**R1.P2 `execute_plan()` 绕过审批流程（🔴 阻塞 Plan 模式安全）**
- `execute_plan()` 直接调用 `registry.execute()`，不经过 `execute_tool_call()`
- 结果：Plan 模式执行步骤时完全跳过敏感文件检测、风险评估、审批弹窗
- **缓解**: Phase 2 必须将 `execute_plan()` 改为通过 `execute_tool_call()` 执行每步

**R1.P3 `parse_unified_diff` 未处理特殊标记**
- `\ No newline at end of file` 等标记被当作 context 行渲染
- **缓解**: 低影响， cosmetic 问题，延期处理

---

## 九、Phase 2 启动分析：Plan 步骤可视化

### 核心架构问题（L0）

`Agent::execute_plan()` 当前直接调用 `ToolRegistry::execute`，绕过了 `Agent::execute_tool_call()` 中完整的安全/审批管道。这意味着：
- Plan 模式 = YOLO 执行（无审批、无 diff、无风险检测）
- 即使 egui 做了漂亮的步骤可视化，底层执行仍然不安全

**Phase 2 必须先修复此问题，再谈 UI。**

### 执行方案

**Step A — 安全修复（L0，阻塞）**
1. `execute_plan()` 每步构造 `ToolCall`，调用 `execute_tool_call()` 而非 `registry.execute()`
2. 这样每步自动获得：敏感文件检测、风险评估、审批弹窗、diff 预览
3. 在 `execute_tool_call()` 中 emit `WireMessage::PlanStepBegin` / `PlanStepEnd`

**Step B — Wire 协议扩展**
1. `WireMessage` 新增：
   - `PlanStepBegin { step_id: String, tool_name: String }`
   - `PlanStepEnd { step_id: String, success: bool }`
2. `clarity-wire` 无需改 `ViewCommand`（走原生 WireMessage 事件）

**Step C — egui Plan 面板**
1. `UiEvent` 新增 `PlanStepBegin` / `PlanStepEnd`
2. 新增 `panels/plan.rs`：步骤列表 + 状态图标（⏳/✅/❌）
3. MVP 不做 DAG 图、不做步骤详情展开

**Step D — 取消支持**
1. `execute_plan()` 需要检查 `CancellationToken`，支持步骤间取消
2. 现有 `begin_turn()` 已返回 token，但 `execute_plan()` 未使用

### Phase 2 风险矩阵

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| `execute_plan` 改 `execute_tool_call` 引入行为变化 | 中 | 高 | 保留现有测试，新增 Plan 执行测试 |
| Plan 执行中审批弹窗与流式输出并发 | 低 | 高 | `execute_plan` 不走流式，风险低于 `run_streaming` |
| 步骤状态同步滞后 | 低 | 中 | wire 事件顺序保证（单生产者） |
| 大 Plan（>50 步）UI 性能 | 低 | 低 | 虚拟列表或 `ScrollArea` |


---

## 十、Phase 2 交付审计（2026-04-28 执行后）

### 已缓解风险

| 风险 | 缓解措施 | 状态 |
|------|---------|------|
| `execute_plan` 绕过审批 | 改走 `execute_tool_call()`，每步获得完整安全管道 | ✅ 已缓解 |
| `execute_plan` 无法取消 | `CancellationToken` 步骤间检查 | ✅ 已缓解 |
| 步骤状态无法可视化 | `PlanStepBegin/End` wire 事件 + `PlanExecutionTracker` | ✅ 已缓解 |

### 新发现风险（Phase 2 执行后）

**R2.P1 `ToolCall` 手动构造的 JSON 往返序列化**
- `execute_plan()` 中 `serde_json::to_string(&step.tool_params)` 再 `from_str`
- 数值类型可能在往返中变化（如 `Value::Number` 的精度）
- **缓解**: 当前工具参数均使用 `serde_json::Value`，往返一致；已观察通过测试

**R2.P2 `plan_tracker` 需手动关闭**
- 执行完成后面板一直保留，需用户点击 ✕
- **缓解**: 低影响，可后续增加自动超时清除

**R2.P3 Plan 执行中审批弹窗的 UX 未验证**
- 理论：Plan 执行时如果某步触发审批弹窗，egui 会显示弹窗，用户确认后继续
- 实际：未做端到端人工验证
- **缓解**: 高优先级在后续迭代中验证

---

## 十一、Phase 3 启动分析：Skill 面板

### 核心架构问题（L0）

`SkillRegistry` **没有 `deactivate` 方法**。当前 API 只有：
- `activate_by_path(paths)` — 按路径自动激活
- `is_active(id)` / `active_ids()` — 查询激活状态

这意味着：
1. 用户无法手动停用已自动激活的 Skill
2. egui 面板只能显示"激活"状态，无法切换

**Phase 3 必须先补全 `SkillRegistry` 的激活控制 API，再做 UI。**

### 执行方案

**Step A — `SkillRegistry` API 补全**
1. 新增 `deactivate(id: &str) -> bool`
2. 新增 `toggle_active(id: &str) -> bool`
3. 新增 `list_skills() -> Vec<Skill>`（返回完整 Skill 列表，不只是 summary）

**Step B — `Agent` 代理方法**
1. `Agent::list_skills() -> Vec<Skill>` — 代理到 `skill_registry`
2. `Agent::skill_active_ids() -> HashSet<String>`
3. `Agent::set_skill_active(id: &str, active: bool)` — 代理到 `SkillRegistry`
4. `Agent::discover_skills() -> Vec<String>` — 触发路径扫描

**Step C — egui Skill 面板**
1. 新增 `panels/skill.rs`：Skill 列表 + 激活开关 + 元数据展示
2. 面板入口放在 sidebar 底部或设置面板中
3. 显示：id、name、description、tools、激活状态
4. 手动刷新按钮（触发 `discover_skills`）

**Step D — 持久化**
1. 手动激活/停用状态是否需要跨会话持久化？
2. 如果 `working_dir` 变化，自动发现的 Skill 会变化，手动状态可能失效
3. **MVP 不做持久化**，仅当前会话有效

### Phase 3 风险矩阵

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| `SkillRegistry` 并发写冲突 | 低 | 中 | `RwLock` 已保护；toggle 操作原子 |
| `Agent` 代理方法暴露过多内部状态 | 低 | 低 | 仅返回克隆数据，不暴露内部引用 |
| egui 面板与 `active_skill` 字段不一致 | 中 | 中 | `active_skill` 是单 skill，`active_ids` 是多 skill；需明确语义 |
| Skill 列表为空时的空状态 UI | 低 | 低 | 显示引导文字 "No skills found in .clarity/skills/" |


---

## 十二、Phase 3 交付审计（2026-04-28 执行后）

### 已缓解风险

| 风险 | 缓解措施 | 状态 |
|------|---------|------|
| `SkillRegistry` 无 `deactivate` | 新增 `deactivate`/`toggle_active`/`list_skills` | ✅ 已缓解 |
| egui 无法切换 Skill 激活状态 | ON/OFF 按钮调用 `Agent::set_skill_active` | ✅ 已缓解 |
| Skill 列表无法获取 | `list_skills()` 返回完整 `Vec<Skill>` | ✅ 已缓解 |

### 新发现风险（Phase 3 执行后）

**R3.P1 `set_skill_active` 非原子读-改-写**
- `is_active`（读锁）→ `toggle_active`（写锁）之间存在窗口
- egui 单线程操作，实际不会并发，但 API 层面不保证
- **缓解**: 低影响，单用户桌面场景；如需强化可改为 `compare_and_set` 语义

**R3.P2 `list_skills()` 克隆所有 Skill body**
- 返回 `Vec<Skill>` 包含完整 Markdown body，大 skill 集时内存开销高
- **缓解**: 当前 skill 数量通常 < 50，body < 10KB，可接受；未来可改为 `Vec<SkillMeta>`

**R3.P3 Skill 面板无刷新/发现按钮**
- 用户放置新 skill 文件后需重启才能看到
- **缓解**: MVP 设计如此；`discover_skills()` API 已就绪，UI 按钮可后续添加

**R3.P4 `active_skill`（单 skill）与 `active_ids`（多 skill）语义差异**
- `set_active_skill` 控制 `AgentInner.active_skill`（单选，影响 prompt 注入顺序）
- Skill 面板控制 `SkillRegistry.active_ids`（多选，影响 prompt 注入集合）
- 两者独立，用户可能困惑为什么面板激活了 skill 但 `active_skill` 为空
- **缓解**: 当前 `build_system_prompt` 会将 `active_ids` 中所有 skill 注入，面板操作确实有效；`active_skill` 主要用于确定 tool whitelist

---

## 十三、Phase 4 启动分析：Token 用量显示

### 现状审计（L0）

Phase 4 **核心功能已在前期实现**，现状如下：

1. `WireMessage::Usage` 定义 ✅
2. `agent/run.rs` 在 `run()`/`run_streaming()`/`run_with_messages_sync()` 结束时 emit Usage ✅
3. `clarity-egui/src/app_logic.rs` wire → `UiEvent::Usage` 映射 ✅
4. `process_events` 更新 `App::last_usage` ✅
5. `panels/chat.rs` 顶部栏渲染：`Tokens: {prompt}↑ {completion}↓ {total}∑` ✅

**结论：Phase 4 功能交付已完成，本次仅需审计优化点。**

### 优化点

**O4.P1 `plan()` 无 Token 用量**
- `plan()` 调用 `llm.complete()` 但不报告用量（`LlmResponse` 无 token 字段）
- **缓解**: `plan()` 为单次 LLM 调用，用量较小；非阻塞问题

**O4.P2 `execute_plan()` 不更新 `last_usage`**
- `execute_plan()` 发送 `TurnBegin`/`TurnEnd` 但不发送 `Usage`
- 执行完成后 `last_usage` 保持旧值或 `None`
- **缓解**: `execute_plan()` 不经过 LLM（只执行工具），无新增 token；行为正确

**O4.P3 无累计/会话级用量**
- 仅显示最近一轮用量，用户无法看到整个会话的累计
- `AgentInner.session_usage` 已累加，但未通过 wire 暴露
- **缓解**: Sprint 13 规划；MVP 当前显示足够

**O4.P4 新会话清除 `last_usage`**
- `new_session()` 设置 `last_usage = None`
- 用户切换会话后用量显示消失
- **缓解**: 设计决策，每会话独立统计

---

> 本分析受能力汇流审计协议 v1.0 统辖。风险评级为工程启发式，非定量模型。
