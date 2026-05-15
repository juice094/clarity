# Sprint 42 — 设计 Bug 修复计划

> 生成时间：2026-05-10  
> 来源：主会话设计审计（Session 接替澄清）  
> 基线：Sprint 41 完成，`main` @ `88ccb2e5`

---

## 一、问题总览

本 Sprint 聚焦四个**编译通过但设计层面存在断裂**的 bug。它们共同特征是：后端能力已就绪，前端呈现层未正确传递语义，导致用户认知负荷增加或系统状态被误判。

| ID | 问题 | 严重程度 | 核心断裂面 | 涉及 Crate |
|----|------|----------|-----------|-----------|
| D-01 | 系统错误伪装成 Agent 回复 | 🔴 P0 | Wire 协议：`SystemError` vs `AgentContent` 无区分 | `clarity-wire`, `clarity-core`, `clarity-egui` |
| D-02 | Error 记忆系统前端断层 | 🟡 P1 | 能力孤岛：`ToolExecutionMemory` 有数据，egui 无入口 | `clarity-core`, `clarity-egui` |
| D-03 | 三栏边界在特定硬件上消失 | 🟡 P1 | 主题系统：缺少最小对比度保证 | `clarity-egui` |
| D-04 | Plan 步骤可视化信息过载/不足 | 🟡 P1 | 信息架构：执行状态单一，依赖/预算/取消信号缺失 | `clarity-core`, `clarity-egui` |

---

## 二、逐案分析与修复路径

### D-01 — 系统错误伪装成 Agent 回复（SE-01）

**症状**
- LLM 加载失败、Provider 切换失败、工具执行失败等系统级错误被当作普通 `MessageRole::Agent` 文本渲染
- 用户视觉上无法区分"Agent 在说话"和"系统在报错"
- 原始审计报告将其归类为 P1，但主会话评估为 **P0（阻断体验）**

**根因分析**
1. `WireMessage` 中错误事件（如 `ToolExecutionFailed`、`LlmConfigError`）被转换为 `ContentBlock::Text` 后直接追加到 `session.messages`
2. `MessageRole::Agent` 承担了双重语义：真正的 Agent 输出 + 系统错误的fallback载体
3. 前端 `message_list.rs` 对所有 `Agent` 消息使用同一套气泡样式，无错误级别分支

**修复路径（分 Phase）**

- **Phase 1（前端止血，0.5d）**
  - `panels/chat/message_list.rs`：在渲染 `MessageRole::Agent` 消息时，检测内容前缀关键词（如 `"Error:"`, `"Failed to"`, `"Connection refused"`）
  - 匹配到的消息使用错误卡片样式：`theme.danger` 左边框 + `bg_elevated` 背景，替代标准 Agent 气泡
  - 不改动 Wire 协议，纯渲染层修复

- **Phase 2（协议层正本清源，1d）**
  - `clarity-wire`：扩展 `WireMessage` 新增 `SystemError { level: ErrorLevel, message: String, source: String }` 变体
  - `clarity-core`：`AgentController` / `run_streaming_turn()` 在捕获系统级错误时，emit `WireMessage::SystemError` 而非写入普通 Agent 消息
  - `clarity-egui`：`handlers/system.rs` 新增 `SystemError` 事件处理，可选择：
    - (a) 渲染为独立系统错误卡片（非对话流）
    - (b) 渲染为顶部 Toast / Banner（P0 阻断级错误）
  - 同步检查 TUI / Gateway / Headless 的 `WireMessage` match  exhaustiveness

- **Phase 3（错误分级契约，1d）**
  - 定义 `ErrorLevel` 枚举：`Blocking`（阻断继续）/ `Warning`（可继续但需知情）/ `Info`（仅日志）
  - egui 渲染层自动映射：`Blocking` = 红卡片 + 顶部 Banner；`Warning` = 黄边框卡片；`Info` = 折叠详情

**验收标准**
- [ ] `cargo test --workspace --lib` 全绿
- [ ] 模拟 LLM 加载失败场景，前端显示红色错误卡片而非普通 Agent 气泡
- [ ] TUI / Gateway 的 `WireMessage` match 无遗漏编译错误

---

### D-02 — Error 记忆系统前端断层

**症状**
- Sprint 19 已在 `clarity-core` 实现 `ToolExecutionMemory` + `ErrorMemoryStore`
- egui 端零可视化：用户无法查看历史错误模式，无法主动检索"过去同类错误的解决方案"
- 能力孤岛典型：backend 有矿脉，frontend 未铺运输带

**根因分析**
1. `ErrorMemoryStore` 当前只被 `PersistingApprovalRuntime` 和 `AgentController` 内部读取
2. 无公共 API 暴露给前端查询（如 `query_similar_errors(tool_name, error_snippet)`）
3. egui 无专门面板或组件承载错误记忆

**修复路径**

- **Phase 1（暴露查询 API，0.5d）**
  - `clarity-core::error_memory`：新增 `pub fn query_similar_errors(&self, tool: &str, limit: usize) -> Vec<ErrorRecord>`
  - 确保 `ErrorRecord` 结构体包含：时间戳、工具名、错误摘要、解决状态、关联 session_id

- **Phase 2（前端最小面板，1.5d）**
  - `clarity-egui` 新增 `panels/error_memory.rs`：
    - 侧边栏可折叠 Section（与 Cron / Teams 并列）
    - 列表展示近期错误（时间倒序），每项显示：工具图标 + 错误摘要前 40 字符 + 解决状态圆点
    - 点击展开详情：完整错误信息 + "应用到当前会话" 按钮（将历史解决方案注入当前 context）
  - 样式遵循现有 Glassmorphism：半透明卡片、`theme.danger` / `theme.warning` 状态色

- **Phase 3（主动推送而非被动查询，2d，Backlog）**
  - 当当前 Agent turn 遇到与历史记录相似的错误时，自动在 Thinking Log 或 Chat 区提示："此错误与 2026-05-08 的 `file_edit` 错误相似，上次解决方案为：…"
  - 需 `clarity-core` 在 `dispatch_tool_calls` 失败时触发相似性检索

**验收标准**
- [ ] `cargo check -p clarity-egui` 无新错误
- [ ] Error Memory 面板可渲染历史错误列表（至少显示最近 5 条）
- [ ] 新增 API 有 doc comment 和至少 1 个单元测试

---

### D-03 — 三栏边界在特定硬件上消失（VL-05 衍生）

**症状**
- `theme.border = rgba(255,255,255,0.08)`（约 `#141414` 在 `#12121a` 背景上）
- 在 OLED 显示器、高环境光、或某些笔记本低亮度面板上，边界完全不可见
- 设计意图是"沉浸无边框"，但实际导致**布局边界认知丢失**

**根因分析**
1. 主题系统静态定义 border 颜色，无环境感知能力
2. 无"最小对比度保证"机制：WCAG 建议 UI 元素对比度至少 3:1，当前 border 与 bg 对比度约 1.05:1
3. 主题变体只有 Dark / OLED，无"高对比度"或"无障碍"模式

**修复路径**

- **Phase 1（提升默认对比度，0.5d）**
  - `theme.rs`：将 `border` 从 `rgba(255,255,255,0.08)` 提升至 `rgba(255,255,255,0.14)`（约 `#1e1e28`，对比度 ~1.3:1）
  - 同时检查 `separator` / `hairline` 等同类低对比度值，统一调整
  - 视觉验证：在 900×700 默认窗口中，三栏边界在普通 IPS 面板上应可见但不突兀

- **Phase 2（阴影替代方案，1d）**
  - `panels/chat/message_list.rs` 和 `sidebar.rs`：在 CentralPanel 与 Sidebar / Workspace 交界处增加 2px 内阴影（`theme.shadow`）
  - 阴影比线条更符合"无边框沉浸"设计范式，同时在所有显示器上提供边界暗示
  - egui 实现：`egui::Frame::new().shadow(egui::epaint::Shadow { ... })`

- **Phase 3（高对比度主题，1.5d，Backlog）**
  - `theme.rs` 新增 `HighContrast` 主题变体
  - `border` 使用纯色 `#555570`，`text` 使用纯白 `#FFFFFF`
  - Settings Panel 新增 "Accessibility → High Contrast" 开关

**验收标准**
- [ ] `cargo check -p clarity-egui` 无新错误
- [ ] 在默认 Dark 主题下，三栏边界在普通显示器上肉眼可见
- [ ] 阴影方案不引入性能回归（egui shadow 为单次 draw call）

---

### D-04 — Plan 步骤可视化信息过载/不足

**症状**
- `workspace_plan.rs` 仅显示执行状态图标（⏳/▶️/✅/❌）
- 步骤间的**数据依赖关系**、**CancellationToken 传播状态**、**预算消耗** 均未可视化
- 用户无法一眼看出"为什么步骤 B 在等待"或"哪个步骤耗尽了预算"

**根因分析**
1. `PlanStep` 结构体已有 `depends_on: Vec<String>` 字段（Sprint 13 Plan 解耦时引入），但 egui 端未读取
2. `CancellationToken` 的树级联状态（Sprint 24/38-D）只通过 Wire 事件广播，未在 Plan 面板聚合
3. 预算消耗（`cost_channel.rs`, Sprint 37-D）与 Plan 步骤无关联

**修复路径**

- **Phase 1（依赖图可视化，1.5d）**
  - `clarity-core`：确保 `PlanStep` 的 `depends_on` 在 `execute_plan` 时正确填充
  - `clarity-egui`：`workspace_plan.rs` 每个步骤左侧增加缩进导轨线：
    - 无依赖 = 0 缩进
    - 有依赖 = 缩进 + 虚线连接箭头指向父步骤
  - 步骤卡片内新增一行小字：`"Depends on: step_3, step_4"`（若存在）

- **Phase 2（取消状态传播可视化，1d）**
  - `workspace_plan.rs`：步骤图标旁新增 `"⊘"` 取消标记（当 `CancellationToken` 被触发时）
  - 已取消步骤的文字颜色降为 `theme.text_dim`，与成功/失败状态区分
  - 用户点击取消标记可展开详情："Cancelled by user at 14:32" 或 "Cancelled due to parent step failure"

- **Phase 3（预算消耗条，1.5d）**
  - `clarity-core`：`PlanStep` 新增 `cost_usd: Option<f64>` 字段，在步骤完成后由 `cost_channel` 回填
  - `clarity-egui`：每个步骤底部增加细粒度预算条（类似子代理进度条）
    - 宽度 = 步骤卡片宽度
    - 颜色：`theme.accent`（正常）/ `theme.warning`（超过预估 50%）/ `theme.danger`（超过预估 100%）
  - Plan 总览底部增加汇总行：`"Total: $0.34 / $1.00 estimated"`

**验收标准**
- [ ] `cargo test --workspace --lib` 全绿
- [ ] 一个 5 步骤 Plan，其中 2 个有依赖，面板正确显示缩进和依赖文字
- [ ] 手动取消 Plan 后，已执行步骤保留 ✅，未执行步骤显示 ⊘，颜色降级
- [ ] 预算条数值与 `cost_channel` 上报值一致（允许 ±$0.01 浮点误差）

---

## 三、依赖关系与执行顺序

```text
Week 1
├── D-01 Phase 1（前端止血）        [0.5d]  可立即启动，无依赖
├── D-03 Phase 1（提升对比度）      [0.5d]  可立即启动，无依赖
└── D-02 Phase 1（暴露查询 API）    [0.5d]  可立即启动，无依赖

Week 2
├── D-01 Phase 2（协议层正本清源）  [1d]    依赖 Phase 1
├── D-03 Phase 2（阴影替代）        [1d]    依赖 Phase 1
└── D-02 Phase 2（前端最小面板）    [1.5d]  依赖 Phase 1

Week 3
├── D-04 Phase 1（依赖图可视化）    [1.5d]  依赖 Sprint 38-D CancellationToken 级联
├── D-01 Phase 3（错误分级契约）    [1d]    依赖 Phase 2
└── D-04 Phase 2（取消状态可视化）  [1d]    依赖 Phase 1

Week 4
└── D-04 Phase 3（预算消耗条）      [1.5d]  依赖 Phase 2 + cost_channel 稳定
```

**并行策略**：
- D-01 / D-02 / D-03 的 Phase 1 可并行启动，互不干扰
- D-04 建议等 D-01 Phase 2 完成后再启动，因为 Plan 面板的错误状态展示需要 `SystemError` 协议支持

---

## 四、风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| `WireMessage` 新增变体导致 TUI/Gateway 编译失败 | 高 | 每次新增变体后立即跑 `cargo check --workspace --lib`，确保所有 match arm exhaustive |
| 主题对比度提升破坏现有视觉平衡 | 中 | Phase 1 只做轻微提升（0.08→0.14），不颠覆设计范式；保留回滚空间 |
| Plan 预算条引入浮点精度问题 | 低 | 使用 `f64` 累加，显示时格式化为 `"$%.2f"`，允许 ±$0.01 误差 |
| Error Memory 面板与现有 sidebar 空间竞争 | 中 | 使用可折叠 Section，默认折叠；记忆条目限制最近 10 条 |

---

## 五、验收命令（任何变更后必执行）

```bash
cd C:\Users\22414\dev\third_party\clarity
cargo test --workspace --lib -- --test-threads=1
cargo clippy --workspace --lib --tests -- -D warnings
cargo fmt --all -- --check
```

---

*本计划遵循 AGENTS.md §9 提取性纪律：每个 Phase 均可在半天内验证并写出 50 字 README。*
