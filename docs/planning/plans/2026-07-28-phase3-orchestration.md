# Phase 3 编排计划：子代理编排收尾 + Settings 重构 + 前端打磨

> **日期**：2026-07-28
> **前置状态**：Sprint 1/2 + 快赢批（N2–N5、N7、N9）已全部落地，main 干净，验证全绿。详见 [`2026-07-28-egui-frontend-subagent-sprints.md`](2026-07-28-egui-frontend-subagent-sprints.md)。
> **本文档用途**：交给执行代理（可多个并行）的任务卡。每张卡自包含：目标、背景指针、文件边界、验收标准。
> **执行纪律**：先读仓库根 `AGENTS.md`（§7 工程红线、§7.3 跨层检查单、§7.5 P1–P7）。所有改动遵循最小化原则；pub 项必须有 `///` 文档；新增 `unwrap/expect` 带 `// SAFE:`；非平凡逻辑配单元测试；提交信息中文、`<type>(<scope>): <summary>`、subject ≤72 字符、一个 commit 一个关注点、每个 commit 独立可编译。

---

## 0. 全局编排

```text
Wave 1（单 agent，独占）
  └── T1 = N1 工具路径子代理 wire 事件（contract 变更，横跨所有 crate）

Wave 2（T1 合并后，三 agent 并行，文件边界见各卡）
  ├── T2 = N8  team_run 工具 + Team 执行可视化
  ├── T3 = N10 Resume 暴露（REST + UI）
  └── T4 = N6  Settings 两栏布局（先 ADR 后实现，单 agent 内部顺序）

Wave 3（收口，单 agent 或人工）
  └── T5 = 整体验证 + 文档同步 + QA
```

**并行冲突规则**（沿用 Sprint playbook，已验证）：

- 同一文件同一时刻只能属于一个 agent。本计划已按此划分；执行中若发现需要越界改文件，停下来上报，不要越界。
- 新增 wire variant 会导致下游 match 不穷尽（E0004）：protocol 侧 agent 必须在同一改动里补齐所有前端的 match arm（至少显式忽略），下游 agent 不得自行加 `_ => None` 兜底。
- 验证范围：各 agent 跑 `-p` 受影响 crate；Wave 3 跑 workspace 级收口。

**明确不做**（冻结，等 P1d 迁移稳定）：`Message` 表示归一化（P2 #9）、大文件拆分（P2 #10）、ChatRenderer/take_* seam 消除、clarity-chrome 实质化、Pipeline DAG（T1-4，ROADMAP 冻结项）。

---

## T1（Wave 1，独占）：`agent`/`agent_swarm` 工具路径的子代理 wire 事件

**目标**：LLM 通过 `agent`/`agent_swarm` 工具派发的子代理，与 `/coder` `/explore` 路径一样发射 `SubagentStage/Output/StatusChange/SubagentProgress` wire 事件，egui Subagents 面板、TUI、Gateway WS 客户端全部可见。

**背景**：

- 缺口：`clarity-subagents/src/parallel.rs:154` 调 runner 时传 `None`（无 wire）。`/coder` 路径已激活的 `parent_wire` 双发机制（`subagents/src/runner.rs:47-91`、`emit_progress`、`OutputCollector::emit`）是现成参照。
- 阻塞点：工具侧拿不到 wire——`ToolContext`（`clarity-contract`）不含 wire 句柄，`SubagentOrchestrator` trait（contract）的 `run_parallel` 签名也没有。
- wire variant 已存在（`clarity-wire/src/lib.rs:302-377`），本任务**不需要新增 variant**。

**实施方案**（二选一，执行 agent 读完代码后判断，上报选择理由）：

- 方案 A（推荐）：`ToolContext` 增加可选 wire 句柄字段（`Option<Arc<Wire>>`，只增不改），`AgentTool`/`AgentSwarmTool` 执行时把它透传给 orchestrator；`SubagentOrchestrator::run_parallel` 加 `wire: Option<&Wire>` 参数（trait 签名变更，需同步所有实现与 mock）。
- 方案 B：orchestrator 构建期注入 wire（`with_wire` builder，镜像 `SubagentRunner::set_llm` 的晚绑定模式），工具路径经 `Agent::with_orchestrator` 时把 agent 的 wire 绑上。注意 tool 路径的 agent 与 UI 的 wire 是否同一个——查清楚 `clarity-core` Agent 的 wire 生命周期（per-turn 还是 app 级），如果是 per-turn，方案 B 绑定的可能是错的生命周期，那就回方案 A。

**文件边界**（独占 workspace，无并行冲突）：

- `clarity-contract`：ToolContext / SubagentOrchestrator trait
- `clarity-core`：`tools/agent.rs`、`agent/construct.rs`、registry
- `clarity-subagents`：`parallel.rs`、`runner.rs`（复用既有 `progress_event_to_wire` 映射，不要另起一套）
- 四前端：**预期零改动**（variant 已存在、消费端已接线）。若确需改动，按 §7.3 检查单逐个过并在报告中说明。
- `tests/integration`：在 `wire_protocol.rs` 补端到端测试——经 `AgentSwarmTool`（mock LLM）派发 2 个并行子代理，断言 wire 收到带各自 `agent_id` 的全生命周期事件。

**验收标准**：

1. `agent_swarm` 工具派发的每个子代理在 wire 上可见（Running→Completed + agent_id 命名空间）。
2. egui Subagents 面板能显示工具派发的子代理（可写集成/单元测试证明 store 更新，GUI 人工 QA 列入 Wave 3）。
3. `cargo fmt --all -- --check`、`cargo check --workspace --lib --bins`、`cargo clippy --workspace --lib --bins --tests -- -D warnings`、`cargo test -p clarity-contract -p clarity-core -p clarity-subagents -p clarity-integration-tests --lib` 全绿。

**提交**：`feat(contract): agent/agent_swarm 工具路径子代理接入 wire 事件`（若跨 crate 一笔太大可按 contract→core/subagents→测试 拆 2-3 笔，每笔独立可编译）。

---

## T2（Wave 2 并行）：`team_run` 工具 + Team 执行可视化

**目标**：团队从"只写配置"变为可执行、可视化。

**背景**：

- `TeamCoordinator`（`clarity-subagents/src/team.rs:49-104`）：N 成员共享 Mailbox、执行后收集 `TeamResult`，后端现成。
- `Agent::run_team`（`clarity-core/src/agent/plan.rs:243`）存在但无工具/REST 入口。
- `team_create`/`team_list`/`team_delete`（`clarity-tools/src/team.rs:117,260`）只做 `~/.clarity/teams/*.json` CRUD。
- egui Team 面板（`clarity-egui/src/panels/right_ide_panel/team_panel.rs`）是配置列表，头部有 ponytail 注释标明"执行可视化待 wire 团队事件"。

**范围**：

1. **team_run 工具**：参照 `core/src/tools/agent.rs` 的 AgentSwarmTool 模式新增（参数：team 名称或内联成员定义、task 描述；返回 TeamResult 聚合摘要）。注册进 `register_common_tools`。
2. **执行可视化**：团队成员本质是并发子代理——若 T1 已合并，复用 wire 子代理事件（成员以 `team:<name>/<member>` 形式的 agent_id 命名空间发射），egui 侧 Team 面板或 Subagents 面板按命名空间分组展示。**不新增 wire variant**，除非命名空间方案走不通（走不通就停下来上报，不要私自加 variant）。
3. **Team 面板升级**：加"运行"入口（触发 team_run 等价物，经 services/ 层）、运行中状态展示；保持 300 行上限，超出拆子组件。

**文件边界**：

- 拥有：`clarity-core/src/tools/`（新文件）、`clarity-core/src/registry.rs`、`clarity-subagents/src/team.rs`、`clarity-egui/src/panels/right_ide_panel/team_panel.rs`、`clarity-egui/src/services/`（新增文件）、`clarity-ui/src/i18n.rs`（与 T3/T4 按 key 段协调：各自追加，冲突时后提交者 rebase）
- 不碰：`gateway/`（T3  territory）、`subagents_panel.rs`（T3）、`clarity-apps/src/settings*`（T4）、`clarity-contract`、`clarity-wire`

**验收标准**：LLM 调 `team_run` 能执行一个真实团队并返回聚合结果（mock LLM 测试）；面板运行入口可用；`cargo test -p clarity-core -p clarity-subagents --lib`、`cargo test -p clarity-egui --bins`、`cargo clippy -p clarity-core -p clarity-subagents -p clarity-egui --lib --bins --tests -- -D warnings` 绿。

**提交**：`feat(core): team_run 工具打通团队执行链路` + `feat(egui): team 面板运行入口与执行状态`。

---

## T3（Wave 2 并行）：Resume 暴露（REST + UI）

**目标**：子代理可从历史实例续跑，暴露在 REST 与 egui UI。

**背景**：

- 后端完整：`RunSpec.resume: Option<String>`（`clarity-contract/src/subagent.rs:234`），`prepare_instance` 从 store 恢复实例、续用 agent_id 和历史（`clarity-subagents/src/runner.rs:692-712,801`）。
- `agent` 工具的 LLM 侧已有 `resume` 参数（Sprint 1 A2 已带）——**剩余只有 REST 与 UI 两面**。

**范围**：

1. **REST**：`gateway/src/handlers/subagents.rs:23-42` 的 `RunSubagentRequest` 加 `resume: Option<String>`（serde default，向后兼容），透传到 RunSpec；parallel 端点如结构允许一并支持，不允许就只做单 run 并注释。
2. **UI**：egui Subagents 面板（`panels/right_ide_panel/subagents_panel.rs`）已完成分区加"继续"按钮——以该 agent_id resume 重新派发（走 services/ 层既有 run 路径）；i18n key 补齐。

**文件边界**：

- 拥有：`clarity-gateway/src/handlers/subagents.rs`（+ 其测试）、`clarity-egui/src/panels/right_ide_panel/subagents_panel.rs`、`clarity-egui/src/services/agent_runner.rs`、`clarity-ui/src/i18n.rs`
- 不碰：`clarity-contract`、`clarity-subagents`、`core/`、`team_panel.rs`（T2）、`settings*`（T4）、gateway 其他 handler

**验收标准**：REST 带 `resume` 的请求恢复既有实例（handler 测试证明 RunSpec 透传）；UI 按钮触发 resume 派发（纯逻辑可测部分有测试）；`cargo test -p clarity-gateway -p clarity-egui --lib --bins`、`cargo clippy -p clarity-gateway -p clarity-egui --lib --bins --tests -- -D warnings` 绿。

**提交**：`feat(gateway): 子代理 REST 端点支持 resume` + `feat(egui): subagents 面板支持继续历史子代理`。

---

## T4（Wave 2 并行，单 agent 内部两阶段）：Settings 两栏布局（P2 #8）

**目标**：Settings 从 640px 顶部 tab + Overlay 模态改为 VSCode/Obsidian 式左 icon rail + 右内容区两栏布局。

**阶段 1（先 ADR，不码代码）**：

- 产出 `docs/adr/ADR-XXX-settings-two-column-layout.md`：现状问题、目标布局（左 rail 宽度/icon+label/选中态、右内容区滚动与最小宽度、响应式收窄策略）、与 design token 的关系、迁移步骤、回退策略。参照 `docs/adr/` 现有 ADR 格式。
- ADR 里附 ASCII mock。**ADR 完成后停下来等评审**（由协调方确认后再进阶段 2）。

**阶段 2（实现）**：

- 实施 ADR。入口：`clarity-apps/src/settings.rs:107-160`（当前 tab bar 结构）；五个 tab 内容在 `clarity-apps/src/settings_panels/`（provider_detail.rs 722 行等——内容面板本身**不重写**，只换外壳导航）。
- 约束：全部走 `t!()` i18n 与 theme token；外壳渲染函数 ≤300 行；左 rail 选中态用 `animation.rs` 的 `animate_bool_normal` 过渡；保持既有 SettingsSnapshot/SettingsViewModel 数据流不动。

**文件边界**：

- 拥有：`docs/adr/`（新文件）、`clarity-apps/src/settings.rs`、`clarity-apps/src/settings_panels/`（仅外壳相关）、`clarity-ui/src/i18n.rs`、`clarity-ui/src/theme.rs`（如需新 token，上报后加）
- 不碰：`clarity-egui/src/panels/`、`clarity-shell`、其他一切

**验收标准**：ADR 入库；实现后 `cargo test -p clarity-apps --lib`、`cargo clippy -p clarity-apps -p clarity-ui --lib --tests -- -D warnings` 绿；布局逻辑（rail 选中/收窄）有纯函数测试；人工 QA 列入 Wave 3。

**提交**：`docs(adr): settings 两栏布局 ADR` + `feat(apps): settings 改左 rail 右内容两栏布局`。

---

## T5（Wave 3，收口）

1. **整体验证**：`cargo fmt --all -- --check`、`cargo check --workspace --lib --bins`、`cargo clippy --workspace --lib --bins --tests -- -D warnings`、`cargo test --workspace --lib --bins`、`cargo test --workspace --doc`、`cargo test -p clarity-integration-tests --lib`。
2. **文档同步**：
   - 更新 `2026-07-28-egui-frontend-subagent-sprints.md` §4 各任务状态（N1/N6/N8/N10 划线归档）。
   - `AGENTS.md` §11.3 移除已解决的已知限制条目；§5.2/§5.4 如有结构变化同步。
   - `docs/architecture/protocol-layer.md` §2.5 更新"agent/agent_swarm 工具路径不发 wire 事件"的限制说明。
   - `CHANGELOG.md` Unreleased 段补本批条目。
   - 若 T4 落地，`docs/planning/optimization-plan-2026-07-06.md` P2 #8 标记完成。
3. **人工 QA**（按 `AGENTS.md` §8.3 + 交接文档 §7 增补）：`agent_swarm` 工具调用时 Subagents 面板实时进度；Team 面板运行入口；Subagents 面板"继续"按钮；Settings 两栏布局交互与中英文切换。
4. **遗留登记**：本计划执行中产生的 ponytail 注释/未接项，汇总进交接文档 §4 新条目。

---

## 附：执行代理通用 prompt 骨架

```text
你在 Rust 项目 C:/Users/22414/dev/clarity 工作。先读：
1. 仓库根 AGENTS.md（§7 红线、§7.3 跨层检查单）
2. docs/planning/plans/2026-07-28-phase3-orchestration.md 中你的任务卡 T<N>
3. docs/planning/plans/2026-07-28-egui-frontend-subagent-sprints.md §3 架构决策、§5 并行 playbook

严格执行任务卡 T<N> 的范围与文件边界；越界需求停下来上报，不要自行扩大。
禁止 git push；commit 遵循任务卡要求。完成后输出：改动文件清单、关键决策、
验证结果、遗留问题。
```
