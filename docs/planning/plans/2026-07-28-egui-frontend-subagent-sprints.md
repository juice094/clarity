# egui 前端美化 + 子代理编排 Sprint 交接文档

> **日期**：2026-07-28
> **状态**：Sprint 1 / Sprint 2 已落地并提交；N2（turn token 接线）/ N3（Gateway bg manager 挂 wire）已完成待提交
> **用途**：跨会话工作交接 + 任务告知。新会话读本文件即可接续，无需重新审计。
> **关联**：`AGENTS.md` §7、§7.3、§7.5；`docs/architecture/protocol-layer.md` §2.5

---

## 1. 目标与定位

对标 Kimi 客户端的美化水准与 Kimi Code 的子代理管理/编排能力，推进两件事：

- **Track A（编排）**：把 clarity-subagents 已具备的后端原语（并行执行、团队协调、Resume、Worktree 隔离）暴露给 LLM 与前端，打通"最后一公里"。
- **Track B（美化/功能密度）**：聊天区信息密度、i18n 一致性、右栏编排面板群。

定位边界（不可跑偏）：不做消费者级 polish、不做消息通道客户端、不做 Voice/Canvas、不引入 Docker/Electron/重型 RAG。美化服务开发者工作流。

## 2. 已落地（全部验证绿，已提交）

### Sprint 1 — `f4492910` + `ff69fef3`

| 项 | 内容 | 关键文件 |
|---|---|---|
| A1 | 修复 `task_create` 死信：task 工具绑定 `BackgroundTaskManager` 共享 store，有 executor 时创建即 spawn；egui 在 LLM 加载后 `set_agent_executor` 晚绑定 | `core/src/tools/task.rs`、`core/src/background/mod.rs`、`egui/src/app_state.rs` |
| A2 | 新增 `agent` / `agent_swarm` 内置工具（`{{item}}` 模板并行、默认超时 1800s）；`Agent::with_orchestrator` 自动绑定；修复 gateway orchestrator 未配 LLM | `core/src/tools/agent.rs`（新） |
| A3 | `CompletionInbox` + hook：后台任务完成/失败摘要注入父 Agent 下次 LLM 调用 | `core/src/agent/completion_inbox.rs`（新） |
| B1 | i18n 中英混杂清零：补 28 个 ZH_CN key，五处硬编码改 `t!()` | `clarity-ui/src/i18n.rs` 等 |
| B2 | 工具调用单击展开完整 args（JSON 高亮）+ output（200 行封顶）；高度缓存实测写回自愈 | `egui/src/render/turn_renderer.rs`、`components/agent_turn.rs` |
| B3 | AgentTurn header 极简 meta 行（耗时 · 工具数） | 同上 |

### Sprint 2 — `e158e162` + `63334c63`

| 项 | 内容 | 关键文件 |
|---|---|---|
| A4 | wire 新增 5 个 variant（`SubagentStage/Output/StatusChange/SubagentProgress/BackgroundTaskUpdate`，只增不改）；producer 复用既有发射点双发；egui 收敛 wire 单源（删除 mpsc 转发 task）；TUI 状态行；Gateway 零改动透传；mobile-core 显式忽略；集成测试 `tests/integration/src/wire_protocol.rs` 3 个端到端测试。附带修复 `finish_turn` 时序，wire Usage 现携带非空 turn_id | `clarity-wire/src/lib.rs:302-377`、`subagents/src/runner.rs`、`egui/src/services/wire_dispatcher.rs` |
| B4 | 右栏 Team/Task/Dashboard 三个 placeholder 填充为真实面板；`render_metrics` 抽为公共函数单源复用；dock take/restore 改 clone-then-write-back 消除 panic 隐患 | `egui/src/panels/right_ide_panel/{task,team,dashboard}_panel.rs`（新） |
| A5 | `subagents_panel.rs`（269 行）接线点亮：egui-local tab（无 `RightRailPanel` 对应物）+ Bot 栏按钮；运行/已完成分区、i18n 状态 badge | `right_ide_panel/mod.rs`、`panels/bot_bar.rs`、`app_logic.rs` |

## 3. 关键架构决策（后续改动须遵守）

1. **manager 绑定模式**：core 工具不直接持有 store，经 `with_task_manager()`/`with_cron_manager()` 注入，镜像现有 cron 模式。新工具需要运行时依赖时照此办理。
2. **wire 单源**：egui 子代理事件已从私有 mpsc 通道收敛到 wire → `wire_dispatcher` → `UiEvent` → `SubAgentStore`。不要再引入第二条私有通道。
3. **egui-local tab**：Subagents tab 没有 `RightRailPanel` 路由对应物（`mod.rs:32` 注释），开 Bot 栏按钮直接操作 dock。后续新增纯前端 tab 可照此模式。
4. **协议只增不改**：wire variant 新增必须带 `#[serde(default)] turn_id`，status 用 String 避免 wire→contract 依赖。
5. **完成通知走 inbox**：`CompletionInbox` 在下次 LLM input 前 drain 注入 system message（`ponytail:` 上限：空闲 Agent 不会被唤醒；升级路径 = wire 事件 + UI surfacing）。

## 4. 未完成任务（下一批候选，按价值/成本排序）

| # | 任务 | 上下文与入口 | 预估 |
|---|---|---|---|
| N1 | `agent`/`agent_swarm` 工具路径的子代理 wire 事件 | 工具路径走 orchestrator→`ParallelExecutor`（`subagents/src/parallel.rs:154` 传 `None`）。需把 wire 穿透 `ToolContext` 或 `SubagentOrchestrator` trait——**contract 变更**，按 §7.3 检查单过四前端 | 中 |
| ~~N2~~ | ~~egui turn header 的 token 接线~~ | ✅ `Message` 新增 `turn_id`，`Session` 新增 `turn_usage`；`Usage`/`TurnStart` 透传 turn_id；`agent_turn.rs` 按 turn_id 查表填充 `token_count`；修复 `tests/session_roundtrip.rs`、`tests/memory_profile.rs` 构造。关键文件：`egui/src/ui/types.rs`、`egui/src/handlers/chat.rs`、`egui/src/components/agent_turn.rs`、`egui/src/panels/chat/message_list.rs`、`egui/tests/*.rs` | ~~小~~ |
| ~~N3~~ | ~~Gateway 长期 bg manager 挂 wire~~ | ✅ `BackgroundTaskManager::wire` 改为 `Arc<Mutex<Option<Arc<Wire>>>>` + `set_wire`；gateway 创建 `event_wire` 后绑定；egui `BackgroundTaskUpdate` 映射为 `UiEvent::BackgroundTaskUpdate` 并刷新 `TaskStore`。关键文件：`core/src/background/mod.rs`、`gateway/src/server.rs`、`egui/src/services/wire_dispatcher.rs`、`egui/src/ui/types.rs`、`egui/src/handlers/task.rs` | ~~小~~ |
| ~~N4~~ | ~~`POST /v1/tasks/:id/cancel`~~ | ✅ Gateway 端点实际已注册为 `DELETE /v1/tasks/:id`（`gateway/src/handlers/tasks.rs`）；补 `GatewayTaskClient::cancel_task`；Task 面板取消先调 Gateway、失败回落本地 `BackgroundTaskManager`。更新 ponytail 注释。关键文件：`egui/src/services/gateway_task_client.rs`、`egui/src/panels/right_ide_panel/task_panel.rs` | ~~小~~ |
| ~~N5~~ | ~~B5 工具调用 Running 态 spinner/活感~~ | ✅ `turn_renderer.rs` Running 工具行改用 `egui::Spinner` 替代静态 hourglass 图标。关键文件：`egui/src/render/turn_renderer.rs` | ~~小~~ |
| N6 | B6 Settings 两栏布局（VSCode/Obsidian 式左 icon rail + 右内容区） | 规划文档 P2 #8，需先出 mock/ADR；`clarity-apps/src/settings.rs:107-160` | 大 |
| N7 | TUI/headless 的 task 工具绑 manager | 目前只有 egui/gateway 调了 `with_task_manager`，其余宿主走 legacy `~/.clarity/tasks` 无消费者路径 | 小 |
| N8 | Team 执行可视化 + `team_run` 工具 | `TeamCoordinator` 后端现成，`team_create` 工具只写配置不执行；Team 面板已留 ponytail 注释 | 中 |
| N9 | Cron 持久化 + 常驻 ticker | `CronScheduler` 内存态重启丢失（`core/src/background/cron.rs:75`） | 小 |
| N10 | Resume 暴露（REST/工具/UI"继续此子代理"） | `RunSpec.resume` 后端完整，无暴露面 | 小 |

**明确不做**（架构债，等 P1d 迁移稳定）：`Message` 表示归一化（P2 #9）、大文件拆分（P2 #10）、ChatRenderer/take_* seam 消除、clarity-chrome 实质化。

## 5. 并行编排 playbook（已验证有效）

多 coder agent 并行时的文件边界划分原则：

- **后端线 vs 前端线**天然可并行：core/subagents/gateway ↔ egui panels/render/apps/ui。
- 共享文件要显式指定归属：`app_state.rs`、`handlers/`、`wire_dispatcher.rs`、右栏 `mod.rs` 每次只能属于一条线。
- 协议变更（wire variant 新增）会让下游 match 不穷尽（E0004），下游线需加 `_ => None` 兜底或等协议线补 arm——本批实际发生过一次，靠显式忽略分支解决。
- 并行 cargo 构建等锁正常；agent 验证范围限定 `-p` 受影响 crate，主 agent 最后跑 workspace 级 check/clippy/fmt 收口。

## 6. 验证命令（收口必跑）

```bash
cargo fmt --all -- --check
cargo check --workspace --lib --bins
cargo clippy --workspace --lib --bins --tests -- -D warnings   # 至少覆盖受影响 crate
cargo test -p clarity-core --lib          # 703+
cargo test -p clarity-egui --bins         # 293+（面板测试在 bin target）
cargo test -p clarity-integration-tests --lib   # 40+（含 wire_protocol）
```

提交规范：`<type>(<scope>): <imperative summary>`，subject ≤72 字符（pre-commit 钩子强制），钩子还含 fmt+check，提交前无需手动重跑。

## 7. 手动 QA 清单（无头环境验不了，需人工）

- [ ] 右栏四个新 tab（Team/Task/Dashboard/Subagents）打开/切换/关闭
- [ ] `/coder` 或 `agent` 工具调用时 Subagents 面板实时进度
- [ ] Task 面板取消运行中任务、查看已完成任务结果
- [ ] 工具调用行单击展开/折叠（含长输出截断提示）
- [ ] turn 头 meta 行（耗时 · 工具数）显示与省略逻辑
- [ ] 中英文切换后五处原硬编码区域（审批模态/Dashboard/工具渲染/输入框 tooltip/hover 文案）

## 8. 跨会话恢复指引

新会话接续流程：

1. 读本文件 §4 选任务；`git log --oneline -6` 确认 4 个 commit 在。
2. 涉及 wire/core 类型改动 → 先读 `AGENTS.md` §7.3 跨层检查单。
3. 并行多 agent → 按 §5 划文件边界，prompt 里写死各自活动范围。
4. 收口按 §6 跑验证，提交信息中文、一个 commit 一个关注点。
