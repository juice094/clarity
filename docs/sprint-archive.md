# Clarity Sprint 归档（Sprint 22–32）

> 存档日期：2026-05-02
> 当前基线提交：待 Sprint 32 结束前固化

---

## Sprint 22–27（已归档）

| Sprint | 核心交付 |
|--------|---------|
| 22 | MCP 错误检测与优雅降级 |
| 23 | Provider 重试机制 |
| 24 | EventBus 重构 |
| 25 | Prompt Reorder 优化 |
| 26 | 子代理类型注册表（LaborMarket） |
| 27 | 并行批处理执行（ParallelExecutor） |

---

## Sprint 28

**交付：**
- `LocalGgufProvider` LCP-based KV cache 跨 turn 持久化
- static prompt hash 基础设施
- Codex CLI / OpenClaw 功能对标分析
- README + ROADMAP 更新推送 → `26be67c1`

---

## Sprint 29

**交付：**
- `run.rs` 从 217 行压缩到 117 行（零行为变化，纯重构）
- 提取 `prepare_turn()` / `build_messages_with_cache()` / `finalize_turn()`
- `build_system_prompt_split` 从 `loop_helpers.rs` 迁移至 `prompt.rs`
- `execute_plan_mode` 从 `run.rs` 迁移至 `plan.rs`

---

## Sprint 30（IS-1：子代理 UI 接入）

**后端改动（clarity-core）：**
- `SubagentProgressEvent` enum：Stage / Output / StatusChange（含 agent_type）
- `OutputCollector` 集成 `mpsc::Sender<SubagentProgressEvent>`
- `SubagentRunner` 新增 `with_progress_tx()` builder
- `SubagentManager::run()` 签名扩展为接收可选 `progress_tx`
- `memory/extraction.rs` 适配新签名

**前端改动（clarity-egui）：**
- `UiEvent` 新增 4 个变体：`SubagentStage` / `SubagentOutput` / `SubagentStatus` / `SubagentComplete`
- `SingleSubagentProgress` 类型 + `SubAgentStore.running_agents: HashMap`
- `handlers/subagent.rs` 4 个新 handler + `ensure_agent()` 动态创建
- `handlers/mod.rs` 路由新事件
- `panels/subagent_progress.rs` 渲染单代理卡片（类型图标、状态徽章、阶段日志、输出预览、耗时）+ 保留并行批处理面板
- `services/agent_runner.rs` `/coder` `/explore` 传入 progress channel
- `theme.rs` 新增 4 个 Phosphor 图标常量

**状态：** ✅ 完成

---

## Sprint 31（零配置首次启动）

**后端改动（clarity-core）：**
- `model_download.rs`：`download_model` → `download_model_files`
- `PreconfiguredModel` 新增 `tokenizer_repo_id` / `tokenizer_filename`
- `CancellationToken` 支持：每 chunk 检查 `is_cancelled()`，取消时删除部分文件
- `ModelDownloadProgress::Cancelled` 新变体
- 新增 `test_cancellation_token_early_exit`

**前端改动（clarity-egui）：**
- `onboarding.rs`：`ChooseProvider` 首次渲染自动触发下载
- 下载完成自动配置 local provider + reload LLM + 隐藏 onboarding
- "Cancel and Skip" 按钮真正终止 HTTP stream
- `OnboardingStore` 新增 `downloading_auto` + `cancel_token`
- `Cargo.toml` 新增 `tokio-util` 依赖

**状态：** ✅ 完成

---

## Sprint 32（修复 + 审计 + 硬化）✅ 已完成

### 已修复

| 问题 | 文件 | 修复方式 |
|------|------|---------|
| 滚动错位（msg_idx desync） | `panels/chat/message_list.rs` | 渲染循环前预推进 `msg_idx` 到 `start_idx` 对应的消息位置 |
| 滚动跳变（嵌套 ScrollArea） | `panels/chat/mod.rs` + `message_list.rs` | 移除外层 ScrollArea，`render_plan` 移入内层 ScrollArea 底部 |
| dead code warnings | 5 个文件 | 移除未使用代码 / 添加 `#[allow(dead_code)]` |

### 功能完整性审计结果

**完全工作：**
- 消息流式渲染（legacy + AgentTurn 聚合）
- 工具调用生命周期 + Thinking Log 侧边栏
- 文件附件 / 拖拽 / 预览浮层
- Web 获取 + Web Tabs
- 会话 CRUD + 分类 + 持久化
- Slash 命令（/plan /coder /explore）
- 设置面板（provider / interface / about）+ 自定义 provider + 连接测试
- Kimi Code OAuth 设备流
- 首次启动自动下载 + 自动配置
- 计划评审 + 步骤追踪
- 审批弹窗（diff 预览 + 快捷键）
- 记忆注入、压缩横幅、token 用量、toast 通知
- 自定义标题栏 + 窗口缩放

**部分工作 / 有缺口：**
- 消息编辑 / 重新生成 / 复制 — 仅代码块复制，无消息级操作
- 本地模型路径选择 — onboarding 自动设置，但设置面板无手动浏览按钮
- 会话列表 — 分类在侧边栏，实例在聊天标签页（无扁平侧边栏列表）

**Stubbed / 缺失：**
- 文件浏览器 — `ui/file_browser.rs` 已 stub，未接入任何面板

---

---

## Sprint 33（Cron/Team UI + 子代理状态持久化）✅ 已完成

**交付：**
- Cron 调度 UI（`panels/cron.rs` + `cron_create.rs`）本地 mock 状态
- Team 协调 UI（`panels/team.rs` + `team_create.rs`）本地 mock 状态
- `SubagentStore` 磁盘状态持久化（JSON save/load）
- BACKLOG parity 矩阵同步

---

## Sprint 34（UI 指示器迁移与死代码清理）✅ 已完成

**交付：**
- Agent/Gateway 状态指示器从 sidebar 迁移至 Workspace 面板标题栏右侧
- Dead code 清理 — 5 个未使用图标常量、`UiEvent::TeamList/CronList` 死变体、`SubAgentProgress`/`AgentStatusEntry` 死字段
- FIXME-WEEK1-RISK 止血 — rapid-Enter debounce、`stopping...` 视觉状态、session-delete draft race

---

## Sprint 35（Cron 迁移 + Markdown 表格渲染）✅ 已完成

**交付：**
- Cron Jobs 从右侧独立 `SidePanel` 迁移至左侧 sidebar 可折叠 section（与 Subagents/Teams 并列）
- Markdown 表格渲染 — 自研轻量解析器：`RenderBlock::Table` + `scan_table()`/`parse_table_lines()` + `egui::Grid` 渲染

---

## Sprint 36（Plan Skip/Retry 后端）✅ 已完成

**交付：**
- `PlanExecutionController` — `new`/`states`/`results`/`has_next`/`skip_step`/`retry_step`/`execute_next`
- Agent 内部状态扩展 `plan_controller: Arc<tokio::sync::Mutex<Option<PlanExecutionController>>>`
- `Agent::skip_plan_step` / `retry_plan_step` 公共 async 控制 API
- 10 个单元测试：controller + agent-level async integration

---

## Sprint 37（Plan Skip/Retry 前端交互）✅ 已完成

**交付：**
- Wire 协议扩展：`PlanStepSkipped` 变体
- egui UI：Tracker 每步添加条件按钮（Pending→Skip，Failed→Retry）
- 事件分发：`UiEvent::PlanSkip`/`PlanRetry` → `agent.skip_plan_step()` / `retry_plan_step()`
- `chat.rs` handler：更新 tracker 状态为 Skipped

---

## Sprint 38（Cron/Team 后端接线 + 工具健壮性）✅ 已完成

**交付：**
- Cron UI 6 个 TODO 全部替换为真实后端调用（`bg_manager.schedule_cron`/`cancel_cron`/`set_cron_enabled`）
- Team UI 6 个 TODO 全部替换为真实后端调用（`TeamCreateTool`/`TeamDeleteTool`/`Agent::run_team`）
- `AppState` 新增 `bg_manager` 字段持久化 `BackgroundTaskManager`
- 工具健壮性：plan/cron/task/notify/team/todo + computer_use 屏蔽

---

## Sprint 39（运行时稳定性 + 工程纪律 + Backlog）✅ 已完成

**交付：**
- `TaskStore::get_result_opt()`：文件缺失 → `None`，不再抛 OS error 2/3
- `TaskOutputTool` 返回 `{"exists": false}` 而非 raw error
- `StdioMcpClient` 进程健康检测：`alive: Arc<AtomicBool>`，stdout reader 结束后返回 `ConnectionFailed`
- TODO/FIXME 清理：6→0，迁移至 `docs/notes/todo-migration-2026-05-07.md`
- `ParallelExecutor::execute` 新增 `cancel: Option<CancellationToken>` 参数
- `TeamCoordinator` 将团队级 cancel token 级联到并行执行器
- 提交：`072ad6ab`

---

## Sprint 40（运行时健壮性深化 + 集成测试补全）✅ 已完成

**交付：**
- `parking_lot` 迁移：6 个 crate 的 `std::sync::RwLock`/`Mutex` → `parking_lot`，消除 ~154 个锁 unwrap（100% 清零）
- 保留 `approval/mod.rs` + `tools/web_browser.rs` 的 `std::sync::Mutex`（依赖 `LockResult` poison 语义）
- 新增 MCP 端到端集成测试：`tests/integration/tests/mcp_end_to_end.rs`，mock HTTP server → `HttpMcpClient` → `ToolRegistry.execute`，2 个测试
- `gateway_http.rs` 补全 `WireMessage::PlanStepSkipped` match arm
- 提交：`5e827983`

---

## 测试基线

```bash
cargo test --workspace --lib
# 预期：~800+ passed / 0 failed / 7 ignored
cargo check -p clarity-egui
# 预期：零 error，零 warning
```
