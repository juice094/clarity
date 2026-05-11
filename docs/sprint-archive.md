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

## 安全修复：彻底移除 openssl 依赖（Dependabot #22/#23）✅ 已完成

**背景：** `openssl 0.10.79` 仍存在 CVE-2026-42327（high）和 AES key-wrap-with-padding heap buffer overflow（moderate），`cargo update` 无法修复。

**措施：**
- 5 个 crate 的 `reqwest` 切换 `default-features = false` + `rustls-tls`：`clarity-core`、`clarity-gateway`、`clarity-mcp`、`clarity-claw`、`clarity-egui`
- `hf-hub` 禁用默认 features，启用 `tokio` + `rustls-tls`，移除 `ureq`/`native-tls`
- `local_gguf.rs` 的 tokenizer 下载从 `hf_hub::api::sync::Api`（阻塞）改为 `hf_hub::api::tokio::Api`（异步）
- `Cargo.lock` 移除：`openssl`、`native-tls`、`hyper-tls`、`tokio-native-tls`、`ureq` 共 17 个包

**结果：**
- `cargo tree -i openssl` → "did not match any packages" ✅
- `cargo test --workspace --lib` → 800+ passed / 0 failed ✅
- Dependabot alert #22、#23 消除 ✅
- 提交：`67b22912`

---

## 文档治理 Sprint（架构透明度 + 安全闭环）✅ 已完成

**交付：**
- `clarity-mcp` 零文档补全：新建 `README.md`（87 行）+ `AGENTS.md`（28 行）
- 3 个 crate 补 `AGENTS.md`：`clarity-contract`、`clarity-egui`、`clarity-headless`
- 10 个 crate README 新增 `## 边界与稳定性`（稳定性等级 + 反向依赖规则）
- `docs/adr/` 目录 + 4 条架构决策记录（Tauri→egui、parking_lot、contract 提取、rustls-tls）
- `docs/ARCHITECTURE.md` 补全 3 条数据流：MCP End-to-End、Memory Compaction、Plan-Parallel Execution
- `docs/OPERATIONS.md` 新建：二进制布局、资源需求、配置体系、日志观测、部署模式、故障排查
- `docs/API_CONTRACT.md` 新建：Gateway HTTP + WebSocket 端点、认证、错误码矩阵
- `docs/THREAT_MODEL.md` 新建：STRIDE 分析 16 条威胁、4 条攻击树、安全测试矩阵
- CI `doc-guard` job 新建：强制 README.md + AGENTS.md + rustdoc 编译 + cargo-modules 结构验证
- `scripts/verify.ps1` 升级：5 项检查（文档/编译/测试/Clippy/格式化）+ 可选 cargo-modules
- 死代码审计：`devkit_dead_code` 扫描 50 条，全部验证为误报（serde default / unwrap_or_else / Axum 路由 / 测试函数）
- 模块提取演习：`cargo check -p clarity-mcp` 独立编译通过
- 提交：`92b77781`

---

## 测试基线

```bash
cargo test --workspace --lib
# 预期：~800+ passed / 0 failed / 7 ignored
cargo check -p clarity-egui
# 预期：零 error，零 warning
```

---

## AGENTS.md §Current Phase 历史 Sprint 摘要（迁出存档，2026-05-11）

**Sprint 36.6 — Cron 迁移 + Markdown 表格渲染（已完成 ✅，2026-05-05）**

- A ✅: Cron Jobs 从右侧独立 `SidePanel` 迁移至左侧 sidebar 可折叠 section（与 Subagents/Teams 并列）
- B ✅: Markdown 表格渲染 — 自研轻量解析器：`RenderBlock::Table` + `scan_table()`/`parse_table_lines()` + `egui::Grid` 渲染；支持标准 `| header |` / `|---|---|` 语法

**Sprint 36.5 — UI 指示器迁移与死代码清理（已完成 ✅，2026-05-05）**

- A ✅: Agent/Gateway 状态指示器从 sidebar 迁移至 Workspace 面板标题栏右侧
- B ✅: Dead code 清理 — 5 个未使用图标常量、`UiEvent::TeamList/CronList` 死变体、`SubAgentProgress`/`AgentStatusEntry` 死字段；`cargo check -p clarity-egui` 0 警告
- C ✅: FIXME-WEEK1-RISK 止血 — rapid-Enter debounce、`stopping...` 视觉状态、session-delete draft race

**Sprint 36 — Cron/Team UI + 子代理状态持久化（已完成 ✅，2026-05-05）**

- A1 ✅: Cron 调度 UI（`panels/cron.rs` + `cron_create.rs`）本地 mock 状态
- A2 ✅: Team 协调 UI（`panels/team.rs` + `team_create.rs`）本地 mock 状态
- B ✅: `SubagentStore` 磁盘状态持久化（JSON save/load）
- C ✅: BACKLOG parity 矩阵同步

**Sprint 35 — 子代理预算条可视化与质量硬化（已完成 ✅，2026-05-05）**

- A ✅: 跨会话快照导出/导入（`session.rs` JSON serde + `rfd` 对话框 + sidebar 工具栏按钮）
- B ✅: Gateway 状态指示器（`gateway_poller.rs` 轮询 `/health` + sidebar 状态圆点）
- C ✅: 子代理进度预算条可视化
  - Core: 修复 `SubagentStore::progress()` 依赖空 `history.len()` 的根因；新增 `AgentInner.last_turn_message_count` + `AgentExecutor` trait 扩展；`SubagentProgressEvent::Progress` 事件；`SubagentStore.steps_taken` 字段
  - UI: `SingleSubagentProgress.steps/max_steps` + `subagent_progress.rs` 迷你 `ProgressBar` 渲染
- D ✅: 质量硬化 — `onboarding::render_onboarding` 与 `handle_window_resize` 接入 `render_safe` panic 隔离

**Sprint 20 — 工具可用性止血与 UI 零边框清理（已完成 ✅）**

- P0.1 ✅: `plan` 工具 `step_id` 柔性匹配 (`"1"` → `"step_1"`)
- P0.2 ✅: `devkit_status` Git 所有权错误处理 + 环境修复
- P0.3 ✅: `list_cron` BackgroundTaskManager 绑定（tui + egui）
- P0.4 ✅: `task_create` / `ChannelSendTool` / `TeamCreate/Delete/List` 补注册
- P1.1 ✅: Thinking Log 持久化（`session.rs` 恢复 `blocks` + `rebuild_tool_calls`）
- P1.2 ✅: 双框清理（settings Window、sidebar、provider_tab、markdown、task_panel、subagent_progress）
- P1.3 ✅: `ContentBlock::ToolCall` 增加 `id` 字段用于重建对应关系
- P2.1 ⏸️: `ask_user` Agent loop 暂停（移至 Sprint 21）
- P2.2 ⏸️: `computer_use` Windows `python3` 适配（移至 Sprint 21）
- P2.3 ⏸️: `web_browser` schema 精简（移至 Sprint 21）

**Sprint 19 — 设计原则封装与 UI 工程硬化（已完成 ✅）**

- P0.1 ✅: CLI→GUI 设计原则文档化 (`docs/design-principles.md`)
- P0.2 ✅: 错误记忆系统架构 (`ToolExecutionMemory` + `ErrorMemoryStore`)
- P0.3 ✅: AgentTurn 聚合层原型 (`agent_turn.rs` + `turn_renderer.rs`)
- P1.1 ✅: 输入框扁平化（消除三级边框）+ Agent 头像去重
- P1.2 ✅: Thinking Log 增强（Spinner/情绪点/错误展开/二次折叠）
- P1.3 ✅: 文件预览覆盖层 (`Area` + `Order::Foreground`)
- P1.4 ✅: 工具调用状态语义（`ToolCallStatus::Running/Success/Error/Warning`）
- P2.1 ✅: flaky test 修复 (`test_full_pipeline_with_replanning` 确定性 RNG)

**Sprint 18 — 架构解耦与 Claw 运行时集成（已完成 ✅）**

- P0.1 ✅: `TurnContext` 提取 — `AgentInner` 18 字段 → 14 字段
- P0.2 ✅: `run.rs` 拆分 — 1178 行 → 130 行 + 4 个子模块
- P0.3 ✅: `AgentLoop` trait + 管道模式 — `SyncLoop`/`StreamingLoop`
- P0.4 ✅: `ToolPayloadAdapter` 提取 — AnthropicLlm 解耦
- P1.1 ✅: `FederalAgentSession` 委托 `AgentExecutor`
- P1.2 ✅: `CoreNode` 桥接 Coordinator → Agent
- P1.3 ⏸️: Gateway → Claw 联邦化（推迟至后续 Sprint）

**Phase 2b — 跨前端 Settings 协议化（已完成 ✅）**

- `SettingsViewModel` 下沉至 `clarity-core`，provider→model 联动统一 ✅
- `clarity-wire` 扩展独立 `ViewCommand` 广播通道（`WireUIViewSide` / `send_view`）✅
- egui Settings 面板协议化（`ViewCommand` + `UserAction`）✅
- TUI 新增 `protocol_renderer`（ratatui 翻译层）+ `/settings` 命令 + `settings_mode` 覆盖层 ✅
- Gateway `WsResponse` 扩展 `ViewCommands` variant；WebSocket 并发转发 wire + view 双通道 ✅

**Sprint 9 — 服务商支持硬化（Phase 1/2 完成 ✅, Phase 3 冻结 ⏸️→ 已解锁 🔓）**

- Phase 1 ✅: API Key `${env:VAR}` 语法注入 + Settings 增量保存（`merge_json`）
- Phase 2 ✅: `ModelRegistry` 接入 egui — `get_available_models()` 动态读取 registry + 硬编码 fallback 合并；`ensure_llm` registry 优先创建（支持自定义 provider from `models.toml`）；`build_provider_from_registry_with_key()` 支持 UI 传入 api key 覆盖环境变量
- Phase 3 🔓 解锁: Kimi 交叉审计识别出"协议先行，架构后置"路径 —— AgentProfile TOML Schema 无需等待 `agent/mod.rs` 拆分即可实现 Agent 级 Provider 覆盖

**Sprint 10 — 协议先行解锁（已完成 ✅）**

- D1 ✅: AgentProfile TOML Schema + GuiSettings 扩展（`profiles.toml`）
- D2 ✅: LlmFactory 功能冻结（`#[deprecated]` + 路由表更新）
- D3 ✅: 能力发现协议（`CapabilityRegistry::supported_approval_modes`）
- D4 ✅: egui 冒烟测试基线（`apply_profile_overlay` 纯函数 + 11 个新增测试）

> 详见 [`docs/plans/2026-04-29-sprint10-protocol-first.md`](./docs/plans/2026-04-29-sprint10-protocol-first.md)

**Sprint 11 — 超越 Kimi CLI（✅ Complete）**

- Phase A ✅: 上下文注入 — `SystemPromptBuilder` 自动汇流 `GitContext` + `ActiveFiles` + `ProjectMetadata`
- Phase B ✅: 编辑精度升级 — `file_edit` 批量替换 + unified diff 预览
- Phase C ✅: 终端体验补齐 — TUI `/yolo`/`/interactive` + Headless stdin 管道
- V1 ✅: 风险点清偿 — 批量替换原子性验证 + 文件路径目录结构保留
- V2 ✅: 端到端验证 — 上下文注入 + approval_mode 切换 + legacy 兼容全部通过

> 详见 [`docs/plans/2026-04-28-sprint11-validation-and-sprint12-plan.md`](./docs/plans/2026-04-28-sprint11-validation-and-sprint12-plan.md)

**Sprint 12 — egui 功能补齐（✅ 已完成）**

- 目标: 将 `clarity-core` 中已完备的能力完整暴露到 `clarity-egui`
- 关键交付:
  - ✅ Phase 1: 审批弹窗 UI — diff 预览 + 键盘快捷键 (Enter/Esc/Shift+Enter) + 交互拦截
- ✅ Phase 2: Plan 步骤可视化 — execute_plan 安全修复（接入审批管道）+ 实时状态图标 (⏳/▶️/✅/❌) + CancellationToken 步骤间检查
- ✅ Phase 3: Skill 面板 — 手动激活/停用开关 + 自动发现状态 + 元数据展示 + 🔄 刷新按钮
- ✅ Phase 4: Token 用量显示 — Session 累计用量 + 千位分隔符 + sidebar 摘要 + plan() token 记录
- ✅ Polish: `parse_unified_diff` 跳过 `\ No newline at end of file` + `send()` 自动清除旧 `plan_tracker` + Diff 下沉至 `clarity-core::diff`
- 周期: 2 周

> 详见 [`docs/plans/2026-04-28-sprint12-egui-feature-parity.md`](./docs/plans/2026-04-28-sprint12-egui-feature-parity.md)
> 风险与优化分析: [`docs/plans/2026-04-28-sprint12-risk-analysis.md`](./docs/plans/2026-04-28-sprint12-risk-analysis.md)（7 项风险 + 4 决策点 + 5 优化）

**Sprint 13 — 稳定性硬化 + 架构解耦（✅ 已完成，2026-04-27 ~ 2026-05-03）**

- Week 1（安全止血）:
  - ✅ A1: 智能断路器 — 同一工具连续 3 次 recoverable 失败升级为 fatal
  - ✅ A2-A5: 代码风险/优化点标注（approval_mode 热路径、path sanitization、ToolExecutionFailed 脱敏、SystemPrompt 防泄露指令）
- Week 2（Approval 状态一致性）:
  - ✅ B2: Approval Request ID 一致性校验（AgentController 层防御 stale/forged ID）
  - ✅ B1: Approval 持久化 — `PersistingApprovalRuntime` 委托 `clarity-memory` 存储 JSON 记录
  - ✅ B3: Plan 类型解耦 — `Plan`/`PlanStep`/`PlanResult` 上提 `types.rs`
- Week 3（Provider 抽象 + 循环依赖打破）:
  - ✅ C1: `ProviderSelectionPolicy` trait + `DefaultProviderSelectionPolicy`（纯同步、可插拔）
  - ✅ C2: 网络探测设计确认 — probe 只驱动 UI banner，永不自动切换 provider
  - ✅ P1-1: `background↔subagents` 循环打破 — `AgentTypeDefinition` + `LaborMarket` 上提 `types.rs`
- Week 4（PoC 提取 + Trait 抽象）:
  - ✅ P1-2: `AgentExecutor` trait — `subagents::runner::execute_agent` 接收 `&dyn AgentExecutor`
  - ✅ P2: `clarity-contract` crate PoC — `ToolCall` + `FunctionCall` 提取，core 重新导出保持兼容
  - ✅ P3: MCP 提取评估 — 当前被 `clarity-contract` 成熟度阻塞，暂缓

> 详细执行计划见 [`docs/plans/BACKLOG.md`](./docs/plans/BACKLOG.md)
> 长程计划见 [`docs/plans/black-widow-stature-john-stewart.md`](./docs/plans/black-widow-stature-john-stewart.md)

> 详见 [`docs/plans/2026-04-30-sprint11-surpass-kimicli.md`](./docs/plans/2026-04-30-sprint11-surpass-kimicli.md)

**Sprint 13.5 — 前端架构重构（已完成，2026-05-01）**

- 对比分析 `openhanako-main` 前端架构，提取成熟前端理论映射
- 错误边界：`render_safe()` + `std::panic::catch_unwind` — 单 panel panic 不崩溃整应用
- 事件处理分离：`process_events` 从 200+ 行 monolith 拆分为 15 个独立 handler（Flux 分发器模式）
- Zustand-style Store 提取：`App` 从 50+ 平铺字段 → 8 个嵌套 domain store
  - `SessionStore` / `ChatStore` / `SettingsStore` / `TaskStore`
  - `UiStore` / `SubAgentStore` / `McpStore` / `OnboardingStore`
- Services 拆分：`send()` / `poll_parallel_batches()` / `refresh_tasks()` 提取至 `services/` 模块
  - `services/agent_runner.rs` / `gateway_poller.rs` / `task_service.rs`
- 验证：`cargo test --workspace --lib` 584 passed / 0 failed / 6 ignored

> 详细对比报告见 [`docs/frontend-architecture-comparison.md`](./docs/frontend-architecture-comparison.md)

**Sprint 14 — Glassmorphism 视觉精调（✅ 已完成，2026-05-01 ~ 2026-05-03）**

- 参考 Kimi 网页版 Swiss International Style + Agent-Native UI 进行批评式设计审查
- 美学评分 4.6/10，识别 20+ 设计缺陷，撰写完整审查报告
- **Phase 1 — 基础设施重写 ✅**
  - `theme.rs` 语义化字号 token（text_xs→text_2xl）+ OLED Black 主题变体
  - Glassmorphism 配色：冰蓝 accent `#5B8DEF`、半透明 surface `rgba(28,28,38,0.72)`、圆角系统 6/12/20/999
  - 字体注册：CJK 优先 `msyhl.ttc` + Phosphor 图标字体嵌入
- **Phase 2 — 布局重构 ✅**
  - Settings 拆分：`panels/settings.rs` 426 行 → `components/settings/{mod,provider,interface,about}.rs`
  - 三栏架构定型：左侧 Sidebar（分类导航）+ Central Chat + 右侧 Tools（Tasks + Files）
  - Sessions 从 Sidebar 迁移至 Header tabs（浏览器式），支持双击重命名
  - Files 从 Sidebar 迁移至右侧 Tools 面板底部
  - 响应式最大宽度：`content_max_width` 600/720/900 可配置，持久化到 `gui-settings.json`
- **Phase 3 — 消息渲染策略 ✅**
  - AB 混合布局：Agent 纯文本 = 无气泡 + 底边框分隔；Agent 结构化 = 玻璃卡片；用户 = 右对齐玻璃气泡
  - 用户气泡宽度 0.72×、错误背景不透明度 0.28、移除多余 separator
  - Markdown 字号 token 化（硬编码 15.0/18.0/16.0 → theme.text_base/md/lg），随 Settings 同步缩放
- **Phase 4 — 细节精调 ✅**
  - 呼吸间距：CentralPanel 16→20、Input inner_margin 10→14、Task panel 12px
  - 输入框安全宽度计算（防溢出）、底部 margin 12→16px
  - Attachments 空状态隐藏、Session 默认标题加序号（New Engineering 2…）
  - 任务按钮 ghost 化、移除 FPS 显示、侧边栏分隔线
- **Phase 5 — 布局精调 ✅**
  - 聊天区精确居中：`allocate_new_ui` + `UiBuilder::max_rect` 创建严格 `content_max_width` 宽的居中渲染区域，全屏下 Swiss Style 留白对称
  - 三栏定型：左侧 Sidebar（分类导航 + 可折叠 Tools）+ 居中 Chat + 右侧 Workspace（常驻）
  - Tools/Tasks 从右侧迁移至左侧 sidebar，移除冗余 `toolbar.rs` 面板
  - Tools 改为可折叠组件：默认折叠（标题栏 + 活跃任务数 + ▶/▼ 按钮），展开后复刻旧 toolbar.rs 视觉风格（状态圆点、分隔线、accent 按钮）
  - Workspace 面板常驻右侧：default_width 320px（原 260px），标题 "Workspace"
  - 文件树独占右侧全部垂直空间，无常驻 Preview 挤压
  - 文件点击后在中间 Chat 区域以 glass card 全宽预览（4000 字符截断，可关闭）
  - Agent 消息底边框取消，改用 `space_12` 留白分隔
- **Bug 修复（本轮）**
  - Header tabs 与右侧状态栏重叠 — `allocate_ui_with_layout` 限制 tab 最大宽度，恢复正确渲染顺序（`01990446`）
  - 窗口控制按钮 tofu — Unicode `□/❐/─` 替换为 epaint 向量笔画（`365eba56`）
  - 文件预览可编辑 — `PreviewItem::File` 新增 `path` 字段，TextEdit + Save 按钮写回磁盘（`365eba56`）
  - Provider Tab 可编辑 — API Key / Base URL 改为 `TextEdit`，自动保存 via `ProviderRegistry::update_provider()`（`d0ae04b5`）
  - Provider list 高度塌陷 — `min_scrolled_height(200.0)` 防止折叠到 2 项（`d0ae04b5`）
  - 新-tab (+) 无效 — 移除 `app_logic.rs` 中 lazy-creation 逻辑，确保每次点击创建新 session
  - 拖拽/resize 不可靠 — `button_down` → `button_pressed` + 标题栏排除 + 边缘阈值 8→10px + 最大化跳过（`48851ad1`）
- **遗留问题**
  - 响应式自动收缩：无 Hanako 式 `CHAT_MIN_WIDTH` 自动折叠逻辑
  - `toolbar.rs` 残留：`panels/toolbar.rs`、`render_toolbar`、`toolbar_open` 未完全清理（编译 warning 但不影响功能）
- **已完成（本轮）**
  - P0 ✅ — 输入框固定在底部：`TopBottomPanel::bottom` 固定输入栏，`CentralPanel` 内 `ScrollArea` 滚动 message_list + preview + plan；输入栏宽度跟随 `content_max_width` 居中
  - Sidebar Web Tabs ✅ — 左侧 Sidebar 新增网页标签面板：URL 列表管理（添加/删除/持久化），点击后异步抓取网页纯文本并在 Chat 区以 glass card 预览（复用文件预览 UI）
  - Sidebar Thinking Log ✅ — 左侧 Sidebar 新增思考日志面板：显示当前 Agent turn 的工具调用链（状态图标 + 工具名 + 参数），从 message_list 底部迁移至此
  - 窗口圆角 ✅ — Win11 `DwmSetWindowAttribute` + `DWMWA_WINDOW_CORNER_PREFERENCE`（`platform/windows.rs:apply_rounded_corners`）
- **下一周期规划（待决策）**
  - P1 — 文件预览可折叠/钉住：预览卡片不常驻占用 message_list 空间
  - P2 — 左侧 Activity Bar 视图切换：窄条图标切换对话/文件/网页/设置（VS Code 模式）
  - P3 — Cursor 式内联对话：文件预览本身成为 Agent message 的一种，支持选中文段直接提问

> 设计审查报告见 [`docs/plans/frontend-design-critique-2026-05-01.md`](./docs/plans/frontend-design-critique-2026-05-01.md)

**Sprint 14.5 — 架构解耦与代码健康（✅ 已完成，2026-05-02）**

> 详见 [`docs/plans/nightcrawler-drax-atom.md`](./docs/plans/nightcrawler-drax-atom.md)

- **Phase A ✅**: 统一 Agent Streaming Loop — 提取 `run_streaming_turn()` 共享编排逻辑（setup → loop → teardown），`run_streaming()` 与 `run_streaming_with_messages()` 缩减为纯消息构建包装器
- **Phase B ✅**: 复活 `ChatDriver` trait + 解耦 `Op` 枚举 — Gateway 通过 `ConversationChatDriver` 注入 OpenAI 风格消息历史；移除 `Op::ConversationTurn` / `Op::ConversationTurnSync`；`Op` 恢复为 5 个纯生命周期变体
- **Phase C ✅**: 清理 AppState — 移除 `initialized`（egui）、`active_connections`（gateway）死字段；统一 `approval_runtime` 为 `mode_aware_approval_runtime.inner()`；去除 Gateway `Agent` 外层 `RwLock`（Agent 内部已是 `std::sync::RwLock<AgentInner>`）
- **验证**: `cargo test --workspace --lib` = 438 passed / 0 failed / 6 ignored；`cargo check --workspace --lib` 0 warnings

**Bug 修复（本轮）**
- **P0 — Agent 空响应**（`fix/agent-empty-response` 分支，commit `b74bc79f`）
  - 根因 1：`llm.stream()` 中途报错后，旧代码清空积累内容并 break，但仍设置 `turn_response = Some(empty)`，导致跳过 `complete()` fallback，返回空字符串。修复：引入 `stream_ok` 标志，仅 stream 完整成功时才设置 `turn_response`。
  - 根因 2：`run_streaming_turn()` 直接调用 `registry.get_tool_schemas()`，绕过 `filter_tools_value()`。当 skill 激活时发送全量工具描述，可能导致 LLM 忽略指令。修复：恢复 `filter_tools_value()` 调用。
  - 根因 3：`run_streaming_loop()` 返回 Err 时，`?` 提前返回，`finish_turn()` 未执行，Agent 状态卡在 `Running`，后续输入被阻塞。修复：将 `finish_turn()` 移到结果检查之前。

**遗留问题**
- `run_streaming_with_messages()`（Gateway/ChatDriver 路径）不调用 `refresh_context()`，导致 Git 上下文和项目元数据可能 stale。修复方案：将 `refresh_context()` 移入 `run_streaming_turn()`（而非仅在 `run_streaming()` 中调用）。影响：Gateway 驱动的 turn 也能感知最新 Git 状态和项目文件变更。→ 纳入 Sprint 15 / Context Convergence Phase 1。
- `task_store` 孤儿问题未处理，保留至后续 Sprint 决策

---

**Sprint 15 — 多方面强化（✅ 已完成，2026-05-02 ~ 2026-05-03）**

> 详见 plan file: `~/.kimi/plans/beast-boy-batgirl-spectre.md`

**Phase 0: 工具层止血（✅ 已完成）**
- commit `64c239e5` — 4 项工具层修复：
  - 扩展名优先 sniff：`.txt/.md/.rs` 等 bypass magic 检测（防 MP3 误报）
  - 允许绝对路径跨目录读取（`C:/Windows/.../hosts`）
  - Windows 仅注册 PowerShell，不注册 BashTool
  - shell timeout 默认 30s → 60s（与 Kimi CLI 对齐）

**Phase 1: UX 补齐（✅ 已完成）**
- commit `62664b0d` — Git 上下文 + ProjectMetadata 自动注入 `SystemPromptBuilder`
- commit `62664b0d` — 工具结果 >2000 字符自动截断
- Smart batch grant toast — 已预存在 `main.rs`（每帧轮询 `drain_auto_approval_notifications`）
- commit `53f6fb05` — 子 Agent 快捷入口 `/coder` `/explore`：输入框前缀检测 → `SubagentRunner` 异步执行 → 结果回显聊天区
- commit `53f6fb05` — Gateway 路径上下文刷新：将 `refresh_context()` 从 `run()`/`run_streaming()` 移入 `run_streaming_turn()`，Gateway/ChatDriver 路径现在也能感知最新 Git 状态

**Phase 1.5: UI 视觉精调（✅ 已完成）**
- commit `9a81ce51` — Pretext 结构化消息 Phase 1：
  - `ContentBlock` enum（7 变体：Text/Code/ToolCall/ToolResult/Think/Plan/FilePreview）
  - `Message.blocks` 替代纯 content 字符串，按类型分策略渲染
  - 序列化兼容：旧 session `blocks: None` 回退为 Text 块
- commit `9a81ce51` — 三栏边界分隔线系统：左/右/顶/底 1px hairline `theme.border`
- commit `9a81ce51` — Sidebar 角色状态指示器：活跃会话数绿点 + 最近实例名
- commit `9a81ce51` — 选中态统一：背景填充 `theme.bg_hover` 替代 accent 描边
- commit `9a81ce51` — 主题色板修复：暗色 `bg` #050507→#12121a，border 显化，亮色文字加深

**Phase 2: 上下文压缩升级（部分完成 🟡）**
- commit `353fccfc` — 加权 token 估算（ASCII ÷4 vs 非 ASCII ÷2），修复 CJK 严重低估
- `CompactionService::estimate_tokens` 统一委托 `crate::compaction::estimate_text_tokens`
- commit `9a81ce51` — Pretext 结构化消息 Phase 1（为精确 token 计数和上下文压缩打基础）
- 三级压缩基础已存在：tier1（本地截断）+ tier2（LLM 总结）
- ❌ 精确 tokenizer（tiktoken-rs）待评估
- ❌ d2.rs 解析器 待实现

**Phase 3: 基础设施（部分完成 🟡）**
- commit `16f92445` — 用户级 skill 目录 `~/.config/clarity/skills/` 自动扫描
- commit `53f6fb05` — MCP 配置热重载：每 5 秒轮询 mcp.json mtime，后台 async 重新加载 + UI toast
- ❌ MemoryNode 接入 egui 待实现

**健康检查（Sprint 15.5）**
- commit `08e0e678` — Workspace 健康检查：
  - 全部 9 crates 独立编译通过
  - `cargo clippy --workspace --lib --bins` = 0 warnings（修复 24 个）
  - `cargo test -p clarity-contract` 新增 41 个契约层测试
  - `background/` 模块可见性收紧：`cron`/`worker` → `pub(crate) mod`，`TaskScheduler`/`TaskHandle` → `pub(crate)`
  - cron tools 绑定到 `BackgroundTaskManager`（gateway）

**验证**: `cargo test --workspace --lib --test-threads=1` = 665 passed / 0 failed / 6 ignored
- `cargo clippy --workspace --lib --bins` = 0 warnings（允许少量 background dead_code）

---

**Sprint 16 — 内核升级 + 基础设施（✅ 已完成，2026-05-03）**

> 承接 Sprint 15，按用户指定优先级推进：精确 tokenizer → d2.rs 解析器 → 三级压缩 budget 级 → MemoryNode 接入 → 测试去 Registry 化。
>
> 详见 plan file: `~/.kimi/plans/beast-boy-batgirl-spectre.md`

| 优先级 | 任务 | 状态 |
|--------|------|------|
| P1 | **精确 tokenizer** — tiktoken-rs (cl100k_base) 替换加权估算，`estimate_text_tokens` 精确到 ±5% | ✅ 已完成 (`a1900eaf`) |
| P2 | **d2.rs 解析器** — 与 mermaid.rs 同构的 D2 语法子集，6 个测试通过 | ✅ 已完成 (`876c47b3`) |
| P2 | **三级压缩 budget 级** — `BudgetRoles` 1:3:6 配额 + `budget_compact()` 逆序丢弃，14 个测试通过 | ✅ 已完成 (`5688abfd`) |
| P3 | **MemoryNode 接入 egui** — 长期记忆检索 enrich query + turn 摘要保存，`cargo check/test` 全绿 | ✅ 已完成 (`7333aa27`) |
| P3.5 | **sidebar UTF-8 安全截断** — `chars().take()` 替代 byte-index 切片，修复 CJK panic | ✅ 已完成 (`c325482f`) |
| — | **测试去 Registry 化 Phase 1** — `mock_registry_with_tools()` 基础设施 + 3 个测试替换 | ✅ 已完成 (`e5fb1a7d`) |
| — | **zeroclaw 吸收 — 凭证脱敏** — `scrub_credentials()` 清洗工具输出中的敏感信息 | ✅ 已完成 (`438f2e7e`) |
| — | **zeroclaw 吸收 — 上下文溢出恢复** — `fast_trim_tool_results()` + 重试机制 | ✅ 已完成 (`a090e5b6`) |

**验证**: `cargo test --workspace --lib -- --test-threads=1` = 484 passed / 0 failed / 6 ignored

---

**Sprint 17 — ZeroClaw 吸收与工程深化（✅ 已完成，2026-05-03 ~ 2026-05-04）**

> 详见 plan file: `~/.kimi/plans/sprint-17-zeroclaw-absorption.md`

| 优先级 | 任务 | 来源 | 状态 |
|--------|------|------|------|
| P0 | **LoopDetector** — 输出哈希比对检测重复调用 | `zeroclaw/src/agent/loop_detector.rs` | ✅ `cae045bd` |
| P0 | **ProviderCapabilities** — provider 自声明 + prompt-guided fallback | `zeroclaw/src/providers/traits.rs:272-305` | ✅ `67f24d42` |
| P0 | **测试去 Registry 化 Phase 2** — 剩余 ~25 个测试 | — | ✅ 完成；`registry.rs::test_get_schemas` 和 `tool_map.rs::test_filter_registry` 保留 `with_builtin_tools()` 为合理设计决策 |
| P1 | **Memory time decay** — 检索结果时间半衰期降权 | `zeroclaw/src/memory/decay.rs` | ✅ `81db30d2` |
| P1 | **运行时预算控制** — USD/turn 上限拦截 | `zeroclaw/src/agent/loop_.rs:2508` | ✅ `d4a54281` |
| P1 | **DraftEvent 三态流** — Clear/Progress/Content 区分 | `zeroclaw/src/agent/loop_.rs:267-276` | ✅ `f055d82a` |
| P2 | **Vision provider routing** — 自动切换 vision provider | `zeroclaw/src/agent/loop_.rs:2425` | ✅ `949dc395` |
| P2 | **Hooks 系统** — before/after tool_call + llm_input | `zeroclaw/src/agent/loop_.rs:2948` | ✅ 混入 `d7eac154` |
| P2 | **多格式工具解析器** — XML/MiniMax/Perl fallback | `zeroclaw/src/agent/loop_.rs` | ✅ `d7eac154` |
| — | **`~` 路径展开修复** | 用户报告 | ✅ `73dd66e2` |
| — | **测试稳定性修复** | `test_budget_day_limit` tiktoken 不稳定 | ✅ `ac4881e3` |
| — | **`execute_flags` 声明修复** | P2.3 提交遗漏 | ✅ `71ebe441` |

**验证**: `cargo test --workspace --lib -- --test-threads=1` = 709 passed / 0 failed / 6 ignored

**Sprint 18 前瞻**: 架构解耦（`TurnContext` 提取、`run.rs` 拆分、`AgentLoop` trait）+ Claw runtime 集成（`FederalAgentSession` 接入 `clarity_core::Agent`）

**Phase 3 — v0.3.0 每日使用体验硬化（已完成）**

- `LocalGgufProvider` 完善（Candle 原生 GGUF 推理）✅
- Settings-Runtime 打通（`ensure_llm` 读取 `GuiSettings`）✅
- 启动时后台预加载 + 网络探测离线 fallback + Provider 热切换 ✅
- CUDA 编译验证通过（可选 feature，不默认启用）✅
- UI/UX 全面重构（Header/Chat Input/Welcome/Sidebar Tools）✅
- Tauri 自动更新（updater plugin + Release workflow 签名）✅
- v0.3.0 四阶段硬化 ✅
  - 阶段一：工具调用可视化（`ToolCallIndicator` + Wire 事件转发）
  - 阶段二：Compaction 状态提示（`CompactionBegin/End` WireMessage + banner）
  - 阶段三：模型下载 GUI（HuggingFace 直链下载 + SettingsPanel 进度条）
  - 阶段四：前端日志面板（Console 劫持 + 可折叠面板）
- 零依赖发行准备（单二进制 + 嵌入式模型）✅

> AI 关键决策见 [`docs/ai-protocol.md`](./docs/ai-protocol.md)。
> 架构定位声明：Clarity 是集群协作原语的单机验证运行时（非本地聊天工具）。

完整路线图见 [`docs/ROADMAP.md`](./docs/ROADMAP.md)。
