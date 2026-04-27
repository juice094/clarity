# Plan: clarity-egui 关键 Parity 差距修复路线图

> **目标**：将 clarity-core 已交付但 clarity-egui 未暴露的交互型功能，按优先级逐层补齐。
> **原则**：不新增外部依赖，不新增 crate，在现有 clarity-egui 边界内完成。
> **基线**：本 plan 依赖 Pretext 运维 plan Phase 1 完成（热路径清剿、Mutex 替换已完成）。

---

## 一、差距总览与优先级矩阵

| 优先级 | 差距项 | 核心影响 | 工作量 | 阻塞项 |
|--------|--------|---------|--------|--------|
| **P0** | 审批交互 UI | Interactive/Plan 模式在 GUI 中**完全无法使用** | 3-4 天 | 无 |
| **P1** | Token 用量显示 | 用户无法感知 Token 消耗 | ½ 天 | 无 |
| **P1** | 后台任务创建/取消 | Task Panel 只读，无操作入口 | 1-2 天 | 无 |
| **P2** | Plan 步骤可视化 | Plan 模式生成的步骤无展示界面 | 2-3 天 | 审批 UI 完成后更顺畅 |
| **P2** | 技能系统 UI | Skill 激活/切换无 GUI 入口 | 1-2 天 | 无 |
| **P3** | 子代理/并行执行可视化 | ParallelExecutor 结果无展示 | 2-3 天 | 后台任务创建完成后更顺畅 |
| **P3** | 模型下载 GUI | HuggingFace 直链下载 + 进度条 | 3-4 天 | 需评估 download crate |
| **P3** | 日志/Console 面板 | 前端错误与运行时日志可视化 | 2 天 | 无 |

**P0 理由**：审批系统是 core 的核心安全机制。当前 egui 只能选择 Yolo 模式，实质上放弃了三层审批中的两层，属于**功能退化**而非功能增强。

---

## 二、P0 — 审批交互 UI（Approval Runtime GUI）

### 2.1 问题诊断

core 审批系统：
- ApprovalMode::Interactive — 每个工具调用前暂停，等待用户 Approve/Reject
- ApprovalMode::Plan — 先生成结构化计划，用户 Review 后批量执行
- InMemoryApprovalRuntime — 内存中的审批队列，支持 approve/reject/approve_for_session

egui 当前无监听审批请求的事件通道，无渲染 Approve/Reject 按钮的 UI 组件。

### 2.2 技术方案（Wire 事件转发）

利用已有的 clarity-wire 事件总线，在 core 侧 emit 审批请求事件，egui 侧监听并渲染模态弹窗。

```rust
// clarity-wire 新增（若缺失）
pub enum WireMessage {
    // ... existing
    ApprovalRequest { tool_name: String, arguments: String, request_id: String },
    ApprovalResolved { request_id: String, approved: bool, approve_for_session: bool },
}
```

egui 侧改动：
1. process_events() 新增 UiEvent::ApprovalRequest 分支
2. 新增 render_approval_modal() — 居中模态弹窗，显示工具名、参数摘要
3. 三个按钮：Approve / Reject / Approve for Session

core 侧改动：检查 InMemoryApprovalRuntime 是否已集成 Wire 事件发射；若无，增加 tx.send() 调用。预计 < 20 行。

### 2.3 验收标准

- [ ] Interactive 模式下，工具调用前弹出审批弹窗，阻塞 Agent 执行直至用户选择
- [ ] Plan 模式下，计划生成后弹出 Review 弹窗，显示步骤列表 + 批量 Approve/Reject
- [ ] "Approve for Session" 按钮生效，当前 session 不再弹出审批
- [ ] Reject 后 Agent 收到错误响应，继续后续对话（不 panic）
- [ ] 弹窗样式与 Theme 系统一致（dark/light 自适应）
- [ ] cargo run -p clarity-egui 手动验证通过

### 2.4 风险

| 风险 | 概率 | 缓解 |
|------|------|------|
| 审批弹窗与流式输出冲突 | 中 | 弹窗使用最高 z-index Layer，流式输出在后台暂停等待 |
| core 侧 Wire 事件缺失 | 低 | 预先审计 InMemoryApprovalRuntime；若缺失，core 侧改动 < 20 行 |

---

## 三、P1 — Token 用量显示

### 3.1 方案

改动量 ~30 行：
1. App 新增 session_usage: Option<Usage>
2. process_events() 匹配 UiEvent::Usage(u) → self.session_usage = Some(u)
3. Chat 区域底部增加微型标签："1,240 tokens"（使用 text_dim，字号 10-11px）
4. 切换 session 时重置 session_usage

### 3.2 验收标准

- [ ] 每次 AI 回复后底部显示 prompt/completion/total tokens
- [ ] 数字随新消息追加更新（session 级累计）

---

## 四、P1 — 后台任务创建/取消

### 4.1 方案

Phase 1（1 天）：
1. Task Panel 顶部增加工具栏：+ Spawn 按钮 + Cancel 按钮
2. App 新增 spawn_task()/cancel_task() 方法，通过 state.task_store 直接调用 core API
3. 操作后列表即时刷新

Phase 2（1 天，可选）：Cron 表达式输入 + 下次执行时间预览

### 4.2 验收标准

- [ ] 点击 + Spawn 可创建新任务，任务出现在列表中
- [ ] 点击 Cancel 可取消 Running 任务，状态变为 Cancelled

---

## 五、P2 — Plan 步骤可视化

### 5.1 方案

与审批 UI 协同：Plan 模式的 Review 弹窗与 Interactive 审批弹窗共用底层模态组件。

```rust
enum ApprovalContent {
    SingleTool { name: String, args: String },
    PlanReview { steps: Vec<PlanStep> },
}
```

UI 设计：
- 模态弹窗标题："Review Plan — 3 steps"
- 步骤列表：序号、工具名、描述、状态图标
- 底部按钮：Approve All / Reject

### 5.2 验收标准

- [ ] Plan 模式下计划生成后弹出 Review 弹窗
- [ ] 步骤列表可滚动，状态实时更新
- [ ] Approve All 后按顺序执行步骤

---

## 六、P2 — 技能系统 UI

### 6.1 方案

最小实现（1 天）：
1. Settings Panel 新增 "Skills" 标签页
2. 扫描 ~/.clarity/skills/ 目录，列出所有 .md 技能文件
3. 每个技能显示：名称、描述、工具白名单、启用/禁用开关
4. 启用时将 skill_name 写入 GuiSettings.active_skill

### 6.2 验收标准

- [ ] Skills 标签页可查看所有本地技能
- [ ] 启用/禁用开关即时生效（无需重启）
- [ ] 当前激活技能在 sidebar 底部显示小标签

---

## 七、P3 — 子代理/并行执行可视化

### 7.1 方案

与后台任务面板复用：在 Task Panel 中增加过滤标签（All / Single / Parallel / Team）。

并行任务展开后显示子代理列表和各自进度。

core 侧检查：确认 BackgroundTaskManager 是否已为并行执行创建独立的 TaskRecord；若无，需补充 task_type 标识字段。

### 7.2 验收标准

- [ ] Task Panel 可过滤并行任务
- [ ] 并行任务展开显示子代理列表（名称、状态、结果摘要）

---

## 八、P3 — 模型下载 GUI

### 8.1 方案

依赖：reqwest 已存在于 workspace（通过 clarity-core），无需新增 crate。

UI 设计：
1. Settings Panel 中 Local Model Path 旁边增加 "Download Model" 按钮
2. 点击后弹出模型选择对话框（内置常用模型列表）
3. 下载过程中显示 egui 原生 ProgressBar
4. 下载完成后自动刷新 Local Model Path

冷路径处理：下载放到 tokio::spawn 后台任务，不阻塞 UI 主线程。

### 8.2 验收标准

- [ ] 可选择内置模型或粘贴 HuggingFace URL
- [ ] 下载进度条实时更新
- [ ] 断网时优雅失败（Toast 提示）

---

## 九、P3 — 日志/Console 面板

### 9.1 方案

tracing-subscriber 自定义 Layer：

```rust
struct GuiLogLayer {
    buffer: Arc<Mutex<Vec<LogEntry>>>,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for GuiLogLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        // 格式化并写入 buffer
    }
}
```

UI 设计：
- Sidebar 底部增加 "Logs" 按钮，点击后展开底部面板
- 日志列表：时间戳、级别、消息
- 过滤：级别筛选、关键词搜索
- 缓冲区限制 1000 条

### 9.2 验收标准

- [ ] 所有 tracing 输出实时显示在日志面板
- [ ] 支持级别过滤和关键词搜索
- [ ] 性能：缓冲区限制 1000 条，超出时丢弃最旧记录

---

## 十、执行时间表

| 周次 | 任务 | 产出 | 验收 |
|------|------|------|------|
| **W1** | P0 审批 UI | render_approval_modal() + Wire 事件集成 | 手动测试 Interactive/Plan 模式 |
| **W2 上** | P1 Token 用量 + P1 任务创建/取消 | session_usage 标签 + Task Panel 工具栏 | 单元测试 + 手动测试 |
| **W2 下** | P2 Plan 可视化 + P2 技能 UI | Plan Review 弹窗 + Skills 标签页 | 手动测试 Plan 模式 |
| **W3** | P3 并行可视化 + P3 日志面板 | Task Panel 过滤 + 日志 Layer | 手动测试 |
| **W4** | P3 模型下载 GUI | 下载对话框 + 进度条 | 手动测试断网/成功场景 |

**硬截止**：若 P0 审批 UI 无法在 1 周内完成，冻结所有 P2/P3 任务，全力攻坚 P0。

---

## 十一、与 Pretext 运维 plan 的衔接

| 运维 plan Phase | 本 plan 的依赖 |
|----------------|---------------|
| Phase 1（热路径清剿）| **已完成**。Mutex 替换、settings 修复已交付。App::update() 拆分为本 plan 隐含前提。 |
| Phase 2（确定性硬化）| 本 plan P1/P2 完成后执行。新增审批/任务/Plan 的单元测试。 |
| Phase 3（维护契约）| 本 plan 全部完成后执行。新增 AGENTS.md 规则：新增交互型功能必须配套单元测试。 |

---

*Plan created by agent on 2026-04-27*
*生效条件：人类开发者确认后执行*
*与 docs/plans/2026-04-27-egui-pretext-health-plan.md 并行维护，交叉引用*
