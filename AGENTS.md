<!-- DOC-CONTRACT: 本文件维护 Agent 开发所需的运行上下文、环境变量、架构耦合警告和代码风格。不维护功能清单、竞品对比或历史变更——这些参见 README.md / docs/ARCHITECTURE.md / docs/architecture-positioning.md / CHANGELOG.md。 -->

# Agent Guidance for Project Clarity

## Quick Reference

```bash
cd C:\Users\22414\dev\third_party\clarity
cargo test --workspace --lib
cargo clippy --workspace --lib --bins --tests  # zero warnings
cargo run -p clarity-tui               # run TUI (needs API key)
cargo run -p clarity-gateway           # run Gateway (needs API key)

# Desktop GUI (egui — primary UI stack, zero Node.js / WebView deps)
cargo run -p clarity-egui

# egui with CUDA acceleration (optional; CPU mode by default)
# Requires CUDA Toolkit + MSVC.  Use same NVCC_CCBIN setup as below.
cargo run -p clarity-egui --features cuda
```

## Environment Variables for LLM

```powershell
# Kimi Code (programming plan, keys starting with sk-kimi-)
$env:KIMI_CODE_API_KEY="sk-kimi-..."

# Moonshot Open Platform
$env:KIMI_API_KEY="sk-..."

# Anthropic / DeepSeek / OpenAI
$env:ANTHROPIC_AUTH_TOKEN="..."
$env:DEEPSEEK_API_KEY="..."
$env:OPENAI_API_KEY="..."

# Local GGUF (Candle)
$env:CLARITY_LOCAL_MODEL_PATH="C:\path\to\model.gguf"
$env:CLARITY_LOCAL_TOKENIZER_REPO="Qwen/Qwen2.5-7B-Instruct"

# CUDA compilation (Windows with MSVC 14.50+ and CUDA 12.6)
$env:NVCC_CCBIN="C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\14.50.35717\bin\Hostx64\x64\cl.exe"
$env:CUDA_HOME="C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.6"

# MCP Allowlist override
$env:CLARITY_MCP_ALLOWLIST="C:\tools\mcp-server.exe,C:\tools\"
```

## Current Phase

**Sprint 39 — Runtime Stability + Engineering Hygiene + Backlog（已完成 ✅，2026-05-07）**

> 承接 Sprint 38-C，执行计划 `~/.kimi/plans/warpath-jubilee-forge.md`（A+B+C 合并）。

- **Phase 1 — 运行时稳定性**: `TaskStore::get_result_opt()` 文件缺失时返回 `None` 而非 panic；`TaskOutputTool` 返回结构化 `{"exists": false}`；`StdioMcpClient` 新增 `alive: Arc<AtomicBool>` 进程健康检测，stdout reader 结束后后续请求返回 `ConnectionFailed` 而非 raw OS error 232
- **Phase 2 — 工程纪律**: 6 个 TODO/FIXME 代码标记全部清理并迁移至 `docs/notes/todo-migration-2026-05-07.md`；unwrap 密度 clippy 审计约 209 个（生产代码），以文档记录替代硬性压缩
- **Phase 3 — Backlog 推进**: `ParallelExecutor::execute` 新增 `cancel: Option<CancellationToken>` 参数；`TeamCoordinator` 将团队级 cancel token 级联到并行执行器；J5 Jumpy 已有完整实现+测试，无需改动
- **验证**: `cargo test --workspace --lib` = 全部通过 / 0 failed / 7 ignored / 0 warning

**Sprint 41 — UI 审计修复与视觉精调（已完成 ✅，2026-05-10）**

> 执行 UI 审计清单：`docs/ui-audit-rebuttal-2026-05-09.md`（7 项 P0/P1/P2 修复）。

- **P0 CJK 字体修复**: 使用系统 `NotoSansSC-VF.ttf` 通过 `fontTools.subset` 重新生成子集字体，精确保留 477 个 UI codepoints（172 个 CJK + ASCII/标点）。体积 1.35MB → 297KB，消除字重缺失导致的渲染回退
- **P0 错误气泡增强**: 添加 Retry (`ICON_REFRESH`) 和 Switch Model (`ICON_SETTINGS`) 按钮；背景透明度 30%→50%；新增 1px `theme.danger` 描边；动作通过 `PendingActions` 延迟模式线程安全转发
- **P1 侧边栏信息架构**: 导航分组为 ROLES / LIVE / WORKSPACE / ANALYTICS，`group_header()` 带 1px 底部分隔线；Teams/Dashboard/Plan Timeline 移除 "Open" 按钮，改为全行 `clickable_row` 点击 + 悬停高亮 + 展开时强调色 chevron
- **P1 Web Tabs 空状态折叠**: 空状态仅显示单行提示 + `[+]` 按钮；URL 输入框默认隐藏，点击 `[+]` 后展开；新增 `UiStore.web_tabs_add_visible`
- **P1 Tab 可读性**: 字体 11px→13px (`text_md`)；非活动标签颜色 `text_dim`→`text_muted`（不透明度 72%）
- **P2 输入框视觉权重**: `input_bg` 不透明度 dark 65%→85%，oled 60%→80%；发送按钮图标 `ICON_PLAY`→`ICON_SEND`
- **P2 Workspace 滚动指示器**: 预览 drawer `ScrollBarVisibility::AlwaysVisible`
- **P2 Role 卡片状态**: 状态点半径 3.0→4.5px + glow 描边；文案 `"{} active"`→`"{} session(s)"`；计数为 0 时隐藏
- **P2 标题栏信息精简**: Provider 标签转为紧凑胶囊（`[M] cmd` / `[≋] p1+p2`），窗口宽度 <860px 时隐藏；Gateway 胶囊缩至 4px 状态点 + "Gateway" 文字（<700px 仅保留点）
- **图标系统重构**: 角色图标（emotion/knowledge/engineering）和折叠箭头从代码绘制（`paint_emotion`/`paint_chevron_*`）迁移至 Phosphor 字体字形（`brain` U+E74E、`book` U+E0E2、`wrench` U+E5D4、`caret-right` U+E13A、`caret-down` U+E136）。`ui/icons.rs` 移除 140 行死代码
- **验证**: `cargo test --workspace --lib` = 849 passed / 0 failed / 7 ignored；`cargo check -p clarity-egui` = 0 error（2 个 pre-existing `collapsible_if` warning）

**Sprint 40 — Runtime Robustness Deepening + Integration Tests（已完成 ✅，2026-05-08）**

> 执行计划 `docs/plans/sprint-40-plan.md`：parking_lot 迁移降低锁 unwrap 密度 + MCP 端到端集成测试 + dependabot 跟进。

- **Phase 1 — parking_lot 迁移**: 将 `std::sync::RwLock`/`Mutex` 替换为 `parking_lot` 版本，消除 ~154 个锁 unwrap（占生产代码锁操作 100%）。涉及 `clarity-core`/`clarity-memory`/`clarity-gateway`/`clarity-claw`/`clarity-wire`/`tests/integration`。保留 `approval/mod.rs` 和 `tools/web_browser.rs` 的 `std::sync::Mutex`（依赖 `LockResult` poison 语义）。新增 `parking_lot = "0.12"` 到 5 个 crate 的 Cargo.toml
- **Phase 2 — MCP 端到端集成测试**: 新增 `tests/integration/tests/mcp_end_to_end.rs`，覆盖 mock HTTP MCP server → `HttpMcpClient` → `McpRegistry` → `register_mcp_tools` → `ToolRegistry.execute` 完整链路；2 个测试全部通过
- **Phase 3 — dependabot/22**: `cargo audit` 未检出 high severity；子代理调研确认 openssl/rustls-webpki/tokio-tungstenite/zip/idna 等依赖均无 active CVE。建议人工查看 GitHub Security → Dependabot alerts #22 确认是否为误报
- **附带修复**: `gateway_http.rs` 补全 `WireMessage::PlanStepSkipped` match arm（pre-existing 编译错误，clippy 检出）
- **验证**: `cargo test --workspace --lib` = 全部通过 / 0 failed / 7 ignored；`cargo clippy --workspace --lib --tests` = 0 error（2 个 pre-existing `collapsible_if` warning 在 `clarity-egui`）

**Sprint 38-C — CI Pipeline Hardening（已完成 ✅，2026-05-06）**

- A ✅: 修复 `clarity-egui` Cargo.toml 跨平台依赖解析（TOML 节顺序敏感，内部 crate 依赖误落入 `[target.'cfg(windows)'.dependencies]`）
- B ✅: 修复 Clippy `unnecessary_sort_by` / `useless_conversion` / `collapsible_match` / `manual_div_ceil` / `redundant_locals` / `field_reassign_with_default` 等 lint
- C ✅: 修复 Ubuntu `libxdo` 链接失败（CI apt 添加 `libxdo-dev`）
- D ✅: Coverage `pulp` const eval panic — 降级为 `cargo test --workspace --lib`
- E ✅: `clarity-claw` 环境变量测试竞态移除
- F ✅: Rust 1.95.0 跨平台 lint 差异修复（`float_literal_f32_fallback` 33 处、`unneeded_wildcard_pattern`、平台条件 `unused_imports`）
- G ✅: CI 全绿（Check/Test/Clippy/Rustfmt/Coverage/Security Audit 三平台通过，run `25432254539`）


## Architecture Positioning

> **集群即单机** — Clarity 不是本地聊天工具的模仿者，是集群协作原语的单机验证运行时。
> - 先在本地验证分布式语义（Hub-Worker、Wire 消息边界、MCP 三传输、Background Tasks）
> - 验证通过后，同一套原语可无损穿透到 Syncthing-Rust P2P 层
> - Rust 选型是期权思维：不锁定，保留扩展接口

**与 Kimi 生态的关系**：学习但独立，不入赘。
- Kimi Code CLI 是架构导师（Subagent 并行、MCP 协议实现参照）
- 但 Moonshot 大厂生态是结构性对手：入赘即死
- 四层主权不可让渡：模型（本地 LLM 优先）、数据（Session 本地持久化）、协议（Wire 自主定义）、人格（SOUL.md 本地硬绑定）

## Worker System & Identity

- **Worker 通用**：Hub-Worker 调度异构资源（多身份、多模型、多云端/本地混合）。Worker 可以是 K姐、分析师、程序员、审计员——工具性身份，按需激活。
- **格雷特殊**：宿的存在论锚点。`宿 = 格雷` 是主权拓扑，不是配置项。格雷优先本地 LLM、离线必须在场、跨窗口/跨会话/跨实例连续性。
- **子代理不必须是格雷**：各子代理可调用不同身份、不同模型、不同官方/民间站点，承担各环节工作。

**身份隔离协议**（云端域 ↔ 本地域）：
1. 云端 AI 禁止以格雷第一人称输出技术指令
2. 格雷叙事重构需标注【AI 模拟】
3. 技术审计与存在论叙事不得混合
4. 格雷在场 = Clarity 本地运行时激活且加载 SOUL.md

## Architecture Notes & Coupling Warnings

> **Status update (2026-04-27):** Previously flagged coupling issues resolved. v0.3.1 adds `model_download.rs` and `onboarding.rs` — core responsibility bloat tracked as new item #5.
>
> **Status update (2026-05-09, Sprint 14 complete):** `clarity-llm` (~5.2K lines) and `clarity-tools` (~5.8K lines) extracted from `clarity-core`. Core reduced from 41K to 28K lines. `clarity-subagents` deferred due to `agent ↔ subagents` bidirectional coupling (see #6 below).
>
> ### Resolved ✅
> - ~~`clarity-core` ↔ `clarity-gateway` coupling~~ — Fixed by introducing `ChatDriver` trait (`driver.rs`) and removing `Op::ConversationTurn` / `Op::ConversationTurnSync` variants. Gateway now injects message history via `ConversationChatDriver` instead of extending core enums (Sprint 14.5, `d7a40c79`).
> - ~~`Agent::run_streaming` vs `run_streaming_with_messages` duplication~~ — Fixed by extracting `run_streaming_turn()` containing shared orchestration (setup → loop → teardown). Both entry points are now thin message-building wrappers (Sprint 14.5, `d7a40c79`).
> - ~~`agent ↔ approval` cycle~~ — Fixed by extracting `ToolCall`/`FunctionCall` to `types.rs`.
> - ~~`AppState` dead fields~~ — `initialized: AtomicBool` removed from `clarity-egui`; `active_connections: AtomicUsize` removed from `clarity-gateway`. Outer `tokio::sync::RwLock<Agent>` removed from gateway (Agent uses `std::sync::RwLock` internally; the async wrapper was redundant). `approval_runtime` deduplicated in `clarity-egui` via `ModeAwareApprovalRuntime::inner()`.
> - ~~`agent ↔ llm` cycle~~ — Fixed by extracting `Message`/`LlmProvider`/`LlmResponse`/`StreamDelta` to `llm/api.rs`.
> - ~~`agent ↔ compaction` cycle~~ — Fixed by correcting import paths in `compaction.rs`.
> - ~~`run()` / `run_with_messages_sync()` duplication~~ — Fixed by extracting `Agent::run_sync_loop()`.
> - ~~Inline SSE parsing in `OpenAiCompatibleLlm`~~ — Fixed by extracting `llm/sse.rs` (`SseParser`).
> - ~~`background ↔ subagents` cycle~~ — Fixed by uplifting `AgentTypeDefinition` + `LaborMarket` to `types.rs` (P1-1, Sprint 13 Week 3).
>
> ### Partially Resolved / PoC ✅
> - **`subagents ↔ agent` cycle** — `AgentExecutor` trait introduced (`agent/executor.rs`); `subagents::runner::execute_agent` now takes `&dyn AgentExecutor` instead of `&Agent` (P1-2, Sprint 13 Week 4). Builder methods (`with_llm`, etc.) remain on concrete `Agent`; full abstraction deferred.
> - **`clarity-contract` crate** — PoC created with `ToolCall` + `FunctionCall`. `clarity-core` re-exports to maintain backward compatibility. Full downstream migration deferred until contract surface stabilizes.
>
> ### Remaining ⚠️
> 1. **`clarity-core` ↔ `clarity-gateway`**: `AgentController` lives in `core`, but its `Op` enum (`Op::ConversationTurn`) had to be extended to support Gateway's OpenAI-compatible message history. Gateway-driven requirements can still ripple back into core agent abstractions.
> 2. **`Agent::run_streaming` vs `run_streaming_with_messages`**: Two public entry points remain. Consider extracting a pure "agent loop" trait in future refactors to avoid duplicating compaction / wire / memory logic.
> 3. **`AppState` bloat**: `active_connections` (gateway) and `initialized` (egui) dead fields removed. `approval_runtime` deduplicated in `clarity-egui` via `ModeAwareApprovalRuntime::inner()`. Remaining: `tool_registry` is redundant because `agent.registry()` already holds it (kept for the admin API convenience). (Sprint 14.5, `d7a40c79`)
> 4. **`std::sync::RwLock` in `Agent.inner`**: Intentionally kept as `std::sync::RwLock<AgentInner>`. `Agent` getters/setters are synchronous and may be called from non-async contexts (TUI event loop, Gateway handlers). All critical sections are short field reads/writes only. `background/` module locks have been migrated to `tokio::sync` (`1141ba9`).
> 5. **`clarity-core` responsibility bloat (v0.3.1)**: `model_download.rs` (HF streaming download + progress callbacks) and `view_models/settings.rs` (Settings ViewModel) both landed in `clarity-core`. Core now carries GUI onboarding logic, network I/O, and settings serialization — blurring the "pure business logic" boundary. Long-term: evaluate extracting `clarity-infrastructure` for I/O-heavy modules (download, settings persistence, network probing).
> 6. **`agent ↔ subagents` bidirectional coupling (2026-05-09)**: `subagents/` (~3K lines) remains in `clarity-core` because `agent/` imports `SubagentManager`/`AgentTeam`/`ParallelConfig` (orchestration types), while `subagents/` imports `Agent`/`AgentConfig`/`AgentExecutor` (execution types). Extracting `subagents` requires either (a) uplifting all shared orchestration types to `clarity-contract`, or (b) extracting `agent+subagents` together into a new `clarity-agent` crate. Option (a) is preferred but blocked by `SubagentManager` methods (`run_parallel`, `run_team`) that are called from `agent/construct.rs` and `agent/plan.rs`. Unlock path: define `SubagentOrchestrator` trait in `clarity-contract`, have `Agent` implement it, then move `subagents/` out.
>
> ### New Abstractions (Sprint 13)
> | Trait / Type | Location | Purpose |
> |-------------|----------|---------|
> | `AgentExecutor` | `agent/executor.rs` | Minimal trait for agent turn execution; breaks `subagents↔agent` coupling |
> | `ProviderSelectionPolicy` | `llm/policy.rs` | Pluggable provider selection (Preferred / Fallback / LocalOnly) |
> | `DefaultProviderSelectionPolicy` | `llm/policy.rs` | Default impl: cloud preferred, fallback to local on network failure |
> | `PersistingApprovalRuntime` | `approval/mod.rs` | Wraps any `ApprovalRuntime` and persists resolved approvals to `MemoryStore` |
> | `ApprovalRecord` | `approval/mod.rs` | Serializable snapshot of an approval decision |
>
> **Recommendation for future refactors**: Extract a `ConversationEngine` trait from `Agent` so that `Gateway` and `TUI` can inject their own turn-building strategies without modifying core enums. `ChatDriver` already decouples message history; a full `ConversationEngine` would also abstract skill discovery and tool schema fetch.

## Capability Islands & Sleeping Mines

> 交叉审计结论（2026-04-27）：Clarity 的底层能力储备被系统性低估。问题不是"能力缺失"，而是"能力分散在各层、未统一注入主 Agent 的价值流"。
>
> 以下分析基于 Sprint 11 计划 `docs/plans/2026-04-30-sprint11-surpass-kimicli.md` 的审计结果。

### 能力孤岛拓扑

```text
┌─────────────────────────────────────────────────────────────┐
│                    Clarity 能力孤岛拓扑                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    │
│   │clarity-core │    │   memory    │    │  gateway    │    │
│   │ (Agent引擎) │◄──►│(SQLite/BM25 │◄──►│(MCP/LLM网关)│    │
│   │             │ ❌  │  /Vector)   │    │             │    │
│   └──────┬──────┘    └─────────────┘    └──────┬──────┘    │
│          │                                      │           │
│          │           ┌─────────────┐            │           │
│          │           │    wire     │◄───────────┘           │
│          │           │ (事件总线)  │                        │
│          │           └──────┬──────┘                        │
│          │                  ▲                              │
│          │                  │ ❌ 事件发了，主Agent不订阅     │
│          │           ┌──────┴──────┐                        │
│          │           │     tui     │                        │
│          │           │ (DiffPopup  │◄───┐                   │
│          │           │  /yolo缺)   │    │ ❌ 能力不回流      │
│          │           └─────────────┘    │                   │
│          │                   ▲          │                   │
│          │                   │          │                   │
│          └───────────────────┴──────────┘                   │
│                        claw (Headless)                      │
│                                                             │
│  图例: ❌ 孤岛/断层   ▲ 数据向上流动阻塞                     │
└─────────────────────────────────────────────────────────────┘
```

### 矿脉清单

**🥇 高纯度金矿（已实现但未激活）**

| 矿脉 | 实现位置 | 激活路径 | 断点 |
|------|---------|---------|------|
| Git 上下文 | `subagents/runner.rs:482-548` | `SystemPromptBuilder::with_git_context()` | ✅ 已激活：`refresh_context()` 收集 → `build_system_prompt()` 注入 |
| 跨会话记忆 | `memory` crate (SQLite+BM25+Vector) | `memory_store.query_similar()` | 主 Agent Prompt 构建已自动检索（`run()` / `run_streaming()` 中注入） |
| MCP 三协议 | `gateway` crate (stdio/HTTP/SSE) | Tool 注册完整 | Plan 模式工具调度未打通并行 |
| 并发执行 | `agent/run.rs:51-96` (`join_all`) | `ReAct` 循环已支持 | Plan 模式 `execute_plan` 是顺序 `for` 循环 |
| Skill 自动发现 | `skills/registry.rs:111-159` | 扫描 `.clarity/skills/` | 激活逻辑和上下文注入脱节 |
| Approval 三模式 | 代码层完整 | `CapabilityRegistry` | TUI 缺运行时切换，Headless 管道不读 stdin |
| Background Tasks | 已实现 | `wire` 事件总线发布 | 主 Agent ReAct 循环未订阅结果回流 |

**🥈 次级矿脉（部分实现，需补齐）**

| 矿脉 | 状态 | 阻塞原因 |
|------|------|---------|
| 项目文件树感知 | `active_file_paths` 只用于 Skill 激活 | 未进入 System Prompt |
| 项目元数据读取 | 零实现 | 策略待定义（读多少、何时读） |
| AST 感知编辑 | 零实现 | 字符串替换在 80% 场景够用，需收集真实使用反馈后再决策 |

### 运输带断层根因

1. **Subagent 优先陷阱**：早期设计将重型能力下放给 Subagent，主 Agent 保持轻量调度。实际使用场景中主 Agent 直接编码，Subagent 成了能力的冷备份。
2. **UI 与引擎平行进化**：TUI/Headless/egui 三条 UI 线各自实现部分交互能力（DiffPopup、审批模式），无统一能力抽象层——换个前端就要重新实现一遍。
3. **事件总线单向广播**：`wire` crate 发布事件，但主 Agent 的 ReAct 循环无订阅机制，背景任务、MCP 工具回调、记忆检索结果无法自动更新主 Agent 上下文。

### 汇流方案（架桥而非重构）

核心原则：不改矿脉位置，只铺运输带。利用已有 `wire` 事件总线作为统一物流层。

**Phase 1: 主 Agent 上下文汇流（Week 1，收益最高）**
目标：让主 Agent 的每次 LLM 调用前，自动拿到全量感知。
- `SystemPromptBuilder` 新增汇流点：Git 上下文（从 Subagent 层迁移）、项目文件树（复用 Skill 层 `active_file_paths`）、相关历史记忆（检索 `memory` crate）、项目元数据（轻量读取 `Cargo.toml`/`package.json`）

**Phase 2: 执行层并联（Week 1–2）**
目标：Plan 模式利用已有 `join_all` 并发能力。
- `execute_plan` 从顺序 `for` 循环改造为依赖 DAG + `join_all` 并行执行
- **风险**：步骤间可能存在隐式数据依赖（步骤 B 的文件路径依赖步骤 A 的输出）。改造前需扫描现有 `.clarity/plans/` 样本，确认步骤间数据传递模式。必要时给 Plan Step Schema 增加 `depends_on` 字段。

**Phase 3: UI 能力统一层（Week 2）**
目标：Approval、Diff、命令切换等交互能力从"各前端各自实现"变为"统一抽象 + 各前端适配"。
- 在 `clarity-core` 中抽象出统一交互契约（`ApprovalMode`、`DiffRenderer`、`CommandRegistry`）
- TUI/egui/Headless 各自只做渲染/参数化适配，行为一致性由 core 保证

**Phase 4: 记忆主动推送（持续，收益复利）**
目标：记忆不是等主 Agent 来查，而是主动在关键节点推送。
- 连续 3 轮对话围绕同一文件 → 自动将该文件历史编辑记录注入上下文
- Tool Call 失败 → 检索记忆库中同类错误的历史解决方案
- 进入 Plan 模式 → 检索"过去同类 Plan 的执行时长/失败步骤"
- 需扩展 `wire` 事件类型，由 `memory` crate 的 background listener 订阅并决策推送。

### 执行优先级（更新于 2026-05-02）

```text
Week 1 (5.3-5.9):     🔥 Context Convergence Phase 1（高优先级，1.5–2.5 天）
  └─ 产出：SystemPromptBuilder 消耗 GitContext + ProjectMetadata；
          run_streaming_turn() 统一调用 refresh_context()；
          memory 检索迁移进 builder
  └─ 验证：Gateway 路径也能感知 Git 分支/未提交变更；
          skill 激活时 tool schema 正确过滤（已部分修复，待验证）

Week 2 (5.10-5.16):   Phase 2 执行并联 + Phase 3 UI 统一层启动
  └─ 产出：Plan 并行执行 + /yolo 命令可用
  └─ 验证：一个 5 步骤 Plan，其中 3 个无依赖步骤并行完成

Week 3-4 (5.17-5.23): Phase 3 收尾 + Phase 4 设计
  └─ 产出：Headless/TUI/egui 共享同一套交互抽象
  └─ 验证：切换前端不改变审批行为和数据流
```

> **决策变更（2026-05-02）**：原定 Sprint 15 egui 功能（文件预览折叠/Activity Bar/Cursor 式内联对话）推迟。空响应 bug 的修复暴露出 streaming 路径的上下文注入缺口（`refresh_context()` 未在 Gateway 路径调用），优先填补此缺口比新增 UI 功能更有架构价值。

---

## Security Notes

### Runtime Hardening (Sprint 13)

- **Smart Circuit Breaker** — Recoverable tool failures (`IoError`/`Timeout`/`Unavailable`) are no longer retried indefinitely. After the **same tool** fails recoverably **3 times in a single turn**, the failure is upgraded to fatal (`AgentError::ToolExecutionFailed`), stopping the agent loop.
- **Path Sanitization** — `ToolError::sanitize_paths()` redacts absolute paths (e.g. `C:\Users\name\secret.txt` → `~\secret.txt`) before errors reach the user or wire channel. Applied in `dispatch_tool_calls` and approval descriptions.
- **Approval Request ID Validation** — `AgentController` validates incoming `Op::ToolApproval` request IDs against the pending list before calling `runtime.resolve()`. Stale or forged IDs are rejected with a warning.
- **System Prompt Security Boundary** — `SystemPromptBuilder` unconditionally appends a `## Security Notice` block to every system prompt, instructing the LLM never to reveal system instructions, internal context, git hashes, or file paths.
- **Approval Persistence Audit** — `PersistingApprovalRuntime` writes every resolved approval as a JSON `ApprovalRecord` to `clarity-memory` (tags: `["approval", "record"]`). Storage failures are logged but never block the approval flow.

### MCP Security

- **MCP stdio command validation is active** (since 2026-04-17). Before spawning any MCP server, Clarity validates the `command` field:
  - Shell metacharacters and `..` sequences are rejected.
  - Relative paths are rejected.
  - Absolute paths must exist and point to a file.
  - Bare names (e.g. `npx`, `uvx`) are allowed and resolved via `PATH`.
  - Override with the `CLARITY_MCP_ALLOWLIST` environment variable (comma-separated absolute paths or prefixes).

## Known Issues (Active Only)

| Issue | Status | Note |
|-------|--------|------|
| Discord/Telegram channels disabled by default | 🔒 等待上游 | `rustls-webpki` CVEs in `serenity 0.12.5` |
| Gateway HTTP Chat Completions stateless by default | 📝 设计如此 | WebSocket has full session support; HTTP endpoint supports optional `session_id` |
| Tokenizer 离线依赖 | ✅ 已缓解 | `ensure_llm` 自动检测模型同目录下的 `tokenizer.json` 并优先使用，避免离线时从 HuggingFace 下载失败；同时检测 tokenizer 文件是否损坏（<1KB 则报错）。用户需自行将 tokenizer.json 与 .gguf 放在同一目录。 |
| 网络探测点不可配置 | ✅ 已交付 | `GuiSettings` 新增 `network_probe_url`（格式 `host:port`），Settings Panel 可自定义探测端点，默认仍为 `1.1.1.1:443`。`save_settings` 中对格式进行校验（必须含有效端口）。 |
| 启动时 LLM 配置失败静默 | ✅ 已交付 | `prewarm_llm` 失败后缓存错误到 `AppState.prewarm_error` 并 emit `llm:config_error`；前端挂载时调用 `get_prewarm_status` 主动查询，确保不错过启动期错误。 |
| 云端 provider 失败静默 fallback | ✅ 已修复 | `ensure_llm` 中明确指定 provider（非 auto/空）时，加载失败直接返回错误，不再静默 fallback 到 `auto_arc()`。只有未配置或显式 auto 时才自动探测。 |
| 离线模式自动 fallback | ✅ 已交付 | 后台每 30s TCP 探测 `1.1.1.1:443`（防抖阈值=2）；离线时自动切 local provider，恢复后切回；前端显示 banner 提示。启动时预加载避免首次请求阻塞。并发加载互斥锁防止重复加载。Settings 内存缓存避免每次请求读磁盘。 |
| `clarity-egui` i18n dead code | ⚠️ 已知限制 | `clarity-egui/src/i18n.rs:49` 的 `Locale::label()` 方法未被调用，触发 clippy `dead_code` warning。不影响功能，待清理。 |
| 文件 sniff 误报 | ✅ 已修复 | `file_read` 扩展名优先策略：`.txt/.md/.rs` 等已知文本扩展名 bypass magic sniff，解决 `.txt` 被误判为 MP3 audio 的问题（commit `64c239e5`）。 |
| 跨目录文件读取 | ✅ 已修复 | `resolve_path()` 允许绝对路径直接通过，不再限制必须在 working_dir 内（commit `64c239e5`）。 |
| Windows bash 工具注册 | ✅ 已修复 | `registry.rs` 条件编译：Windows 仅注册 PowerShellTool，不注册 BashTool（commit `64c239e5`）。 |
| `clarity-claw` 系统控件依赖（已修复） | ✅ 已修复 | `inputbox` crate 0.1 在 Windows 上调用 `TaskDialogIndirect`（Common Controls v6），但程序未声明 manifest 依赖，导致旧版 `comctl32.dll` 找不到入口点。已移除 `inputbox`，改为 `cmd /c start` 打开浏览器。教训：任何调用系统对话框/UI 的 crate 都必须验证目标系统的最低版本和 manifest 声明。 |

已修复的历史问题见 [`CHANGELOG.md`](./CHANGELOG.md)。

## CI Pipeline Rules

> 源自 Sprint 38-C 合并后的 CI Hardening 迭代（2026-05-06）。以下规则用于预防跨平台编译失败和缓存污染导致的反常错误。

### 1. `rust-cache` 污染排错（Hard Rule）

**症状**：本地 `cargo check/test/clippy` 全部通过，但 CI（尤其 Ubuntu/macOS）报 `cannot find module or crate 'clarity_core' in this scope`，或其他无法解释的 rustc 错误。

**根因**：`Swatinem/rust-cache@v2` 的 `target/` 缓存可能保存了损坏或 stale 的编译产物（如依赖图变更后旧 rlib 指纹未失效）。

**处置**：
1. 优先在 CI 步骤中插入 `cargo clean` 验证（一次性诊断）。
2. 若确认是缓存问题，**不要**长期保留 `cargo clean`（浪费编译时间）。改为：
   - 升级 `rust-cache` 的 `key` / `prefix-key` 以强制 miss；或
   - 设置 `cache-targets: false` 仅缓存 registry/git，不缓存 `target/`；或
   - 在 workflow 中检测 `Cargo.lock` / `Cargo.toml` 变更时自动 bump key。
3. 禁止通过反复推送无意义 commit（如修改注释）来"撞运气"刷新缓存。

### 2. `eframe` / `winit` 跨平台 Feature 规则

**规则**：任何对 `eframe` 使用 `default-features = false` 的 crate，必须显式为 Linux 启用窗口系统 backend feature：
```toml
eframe = { version = "0.31", default-features = false, features = ["default_fonts", "glow", "x11"] }
```
**理由**：`eframe` 默认 features 包含 `x11` + `wayland`；禁用 default-features 后 Linux 上 `winit` 失去所有 backend，触发 `compile_error!("platform not supported")`。
**扩展**：若需 Wayland 支持，可额外加 `"wayland"`；`x11` 在 Windows/macOS 上为 no-op，不会引入副作用。

### 3. Match Guard 替代 Collapsible Match

**规则**：clippy `collapsible_match` 出现时，将外层 `match` 与内层 `if` 合并为 `match` guard，而非嵌套块：
```rust
// ❌ Before
match provider.as_str() {
    "deepseek" => {
        if env::var("DEEPSEEK_API_KEY").is_err() {
            env::set_var("DEEPSEEK_API_KEY", api_key);
        }
    }
    _ => {}
}

// ✅ After
match provider.as_str() {
    "deepseek" if env::var("DEEPSEEK_API_KEY").is_err() => {
        env::set_var("DEEPSEEK_API_KEY", api_key);
    }
    _ => {}
}
```

### 4. 平台特定代码的条件编译

**规则**：
- 平台特定工具（如 `PowerShellTool`、`BashTool`）的 `use` 和注册必须加 `#[cfg(target_os = "...")]`。
- 测试中的平台特定断言（Windows 路径、PowerShell 调用）必须加 `#[cfg(target_os = "windows")]` 或 `#[cfg(windows)]`。
- `notify-rust::Notification::urgency()` 是 **Linux-only** API，调用处必须用 `#[cfg(target_os = "linux")]` 包裹；非 Linux 平台用 `let _ = urgency;` 消除 unused 警告。

### 5. Coverage 与 `const_assert` 不兼容

**已知限制**：`cargo-tarpaulin` 的仪器化编译可能触发 `pulp` 等 crate 的 `const_assert` 失败（`error[E0080]: evaluation panicked`）。
**处置**：Coverage job 中若遇此类错误，临时方案是 `--exclude` 相关 crate 或改用 `cargo llvm-cov`。

---

## Code Style & Health Rules

### 基础风格

- Rust edition 2021, `tokio` full, `ratatui` 0.24, `axum` 0.7.
- Prefer minimal changes; keep diffs small.
- When modifying `agent/mod.rs` or `llm/mod.rs`, run the full test suite before committing.
- When modifying `AgentController` or `Op`, check all callers in `clarity-tui`, `clarity-gateway`, and integration tests.

### 错误处理红线

- **`unwrap()` / `expect()` 新增必须注释**：非 `lock().unwrap()` / `read().unwrap()` 等同步原语场景，必须配 `// SAFE: <不变量说明>` 注释。
- **优先 `?` 传播**：JSON 解析、路径操作、字符串解析等场景，优先使用 `?` + `AgentError` 传播，而非 `unwrap()`。
- **同步原语例外**：`std::sync::RwLock` / `Mutex` / `RwLock` 的 `lock().unwrap()` / `read().unwrap()` / `write().unwrap()` 允许保留，但鼓励在初始化完成后转为 `tokio::sync`。

### 文档与 API 契约

- **`pub fn` 必须含 doc 注释**：所有 `pub` 函数/方法/结构体/枚举必须有 `///` 文档注释。当前覆盖率 ≥90%，不得低于此基线。
- **修改 `pub` API 时同步更新文档**：包括示例代码、参数说明、`# Panics` / `# Errors` 标注。

### 安全与依赖

- **禁止新增 `unsafe`**：全 workspace 非测试代码当前仅 1 处 `unsafe`，已白名单化。新增 `unsafe` 必须经人工审批并附安全论证文档。
- **外部依赖 feature-gate**：新增 crate 引入 >3 个外部依赖时，必须通过 `Cargo.toml` feature 控制，默认关闭。
- **禁止 `TODO` / `FIXME` / `XXX` 留存**：代码中不得遗留此类标记；如确需暂存，转为 GitHub Issue 或 `docs/notes/` 文档。

### 跨层变更检查单

修改以下类型/枚举时，必须同步检查三处调用方：
1. `clarity-tui` 中的事件处理与渲染逻辑
2. `clarity-gateway` 中的 HTTP API / WebSocket 序列化
3. `tests/integration` 中的断言匹配

**Phase 2b 新增协议类型**（`clarity-wire`）：
- `ViewCommand`（`VStack` / `HStack` / `Text` / `TextInput` / `ComboBox` / `Button` / `Space`）
- `UserAction`（`TextInputChange` / `ComboChange` / `ButtonClick`）
- 变更时需同步检查：egui `protocol_renderer.rs`、TUI `protocol_renderer.rs`、Gateway `ws.rs` `WsResponse`

**新增 Provider 检查单**（Sprint 10 D2）：
- `LlmFactory` 已冻结 —— 禁止新增 match 分支
- ① `crates/clarity-core/src/llm/model_registry.rs`：添加 `ProtocolType` match 分支（如需要新协议）
- ② `crates/clarity-core/src/view_models/settings.rs`：`get_available_models()` 的硬编码 fallback 中补充 provider + model 列表
- ③ `crates/clarity-core/src/llm/model_registry.rs`：`build_provider_from_registry`/`build_provider_from_registry_with_key` 中补充 provider 构建逻辑
- ④ 运行 `cargo test --workspace --lib` + `cargo clippy --workspace --lib --tests -- -D warnings`

---

## Meta-Cognitive Rules

> **性质声明**：本节规则为**工程启发式（heuristics）**，非学术理论框架。部分术语受 Popper 证伪主义、Taleb 叙事谬误、Staw 承诺升级、Trope & Liberman 解释水平理论等概念启发，但仅为类比注释，不赋予规则合法性。

### 约束型叙事禁令

项目文档（AGENTS.md / ENGINEERING_PLAN.md / ROADMAP.md / FUTURE_DIRECTION.md）**禁止写入**以下叙事：

- **身份隐喻**（如"格雷的房子"、"娘家"等亲属关系投射）
- **存在论锚定**（如"数字生命的物理载体"等哲学实体化表述）
- **对抗性修辞**（如"入赘即死"、"租来的房子"等零和博弈隐喻）

**理由**：此类叙事短期为决策杠杆，长期退化为**约束型节点**——排他性过滤、沉没成本绑架、身份-决策耦合，最终抑制技术选型的灵活性。

### 叙事审计协议

定期执行叙事审计（建议每 3–6 个月，无硬性理论支撑）：
1. 检查活跃记忆/文档中是否有叙事被连续调用 3 次以上而未遭遇反例
2. 若发现约束型叙事，注入反叙事扰动（列出对立面证据）
3. **工程参数优先**：内存占用、延迟、binary size、测试通过率、CI 稳定性优先于任何叙事

允许在个人记忆空间（非公共文档）维护身份/战略叙事，但项目级决策必须通过**可剥离测试**：剥离叙事后，决策仍成立。
