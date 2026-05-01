<!-- DOC-CONTRACT: 本文件维护 Agent 开发所需的运行上下文、环境变量、架构耦合警告和代码风格。不维护功能清单、竞品对比或历史变更——这些参见 README.md / ARCHITECTURE.md / CHANGELOG.md。 -->

# Agent Guidance for Project Clarity

## Quick Reference

```bash
cd C:\Users\22414\dev\third_party\clarity
cargo test --workspace --lib
cargo clippy --workspace --lib --bins --tests  # zero warnings
cargo run -p clarity-tui               # run TUI (needs API key)
cargo run -p clarity-gateway           # run Gateway (needs API key)

# Desktop GUI (Tauri 2)
cd crates/clarity-tauri/frontend && npm run build
cargo tauri dev

# Tauri with CUDA acceleration (Windows, requires CUDA Toolkit + MSVC)
# Note: CUDA 12.6 does not support MSVC 14.50+ out of the box.
# Set NVCC_CCBIN so cudaforge auto-injects -allow-unsupported-compiler.
$env:NVCC_CCBIN = "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\14.50.35717\bin\Hostx64\x64\cl.exe"
$env:CUDA_HOME = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.6"
cargo tauri build --features cuda
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
> **Status update (2026-05-03, Sprint 13 complete):** 4-week architecture decoupling delivered. See Sprint 13 section above for full list.
>
> ### Resolved ✅
> - ~~`agent ↔ approval` cycle~~ — Fixed by extracting `ToolCall`/`FunctionCall` to `types.rs`.
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
> 3. **`AppState` bloat**: `AppState` currently carries `agent`, `session_manager`, `tool_registry`, and `task_manager`. The `tool_registry` field is actually redundant because `agent.registry()` already holds it (kept for the admin API convenience).
> 4. **`std::sync::RwLock` in `Agent.inner`**: Intentionally kept as `std::sync::RwLock<AgentInner>`. `Agent` getters/setters are synchronous and may be called from non-async contexts (TUI event loop, Gateway handlers). All critical sections are short field reads/writes only. `background/` module locks have been migrated to `tokio::sync` (`1141ba9`).
> 5. **`clarity-core` responsibility bloat (v0.3.1)**: `model_download.rs` (HF streaming download + progress callbacks) and `view_models/settings.rs` (Settings ViewModel) both landed in `clarity-core`. Core now carries GUI onboarding logic, network I/O, and settings serialization — blurring the "pure business logic" boundary. Long-term: evaluate extracting `clarity-infrastructure` for I/O-heavy modules (download, settings persistence, network probing).
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
> **Recommendation for future refactors**: Extract a `ChatDriver` or `ConversationEngine` trait from `Agent` so that `Gateway` and `TUI` can inject their own message-building strategies without modifying core enums.

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
| Git 上下文 | `subagents/runner.rs:482-548` | `GitContext::collect()` | 仅注入 Subagent System Prompt，主 Agent 从不调用 |
| 跨会话记忆 | `memory` crate (SQLite+BM25+Vector) | `memory_store.query_similar()` | 主 Agent Prompt 构建不自动检索历史相关记忆 |
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

### 执行优先级

```text
Week 1 (4.29-5.5):   Phase 1 上下文汇流
  └─ 产出：主 Agent 能感知 Git + 文件树 + 记忆
  └─ 验证：让 Clarity 处理一个真实 PR 描述，观察分支/未提交变更识别能力

Week 2 (5.6-5.12):   Phase 2 执行并联 + Phase 3 UI 统一层启动
  └─ 产出：Plan 并行执行 + /yolo 命令可用
  └─ 验证：一个 5 步骤 Plan，其中 3 个无依赖步骤并行完成

Week 3-4 (5.13-5.19): Phase 3 收尾 + Phase 4 设计
  └─ 产出：Headless/TUI/egui 共享同一套交互抽象
  └─ 验证：切换前端不改变审批行为和数据流
```

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
| `clarity-tauri` 默认未启用 `local-llm` | ✅ 已解决 | `clarity-core` 默认 feature 已含 `local-llm`；Tauri 侧 `ensure_llm` 已读取 `GuiSettings` 并支持 local provider。 |
| `clarity-tauri` CUDA feature 需手动启用 | ⚠️ 已知限制 | CUDA 编译通过验证，但因 CUDA Toolkit 是重型外部依赖且 `candle-kernels` 编译耗时较长，`cuda` feature 为可选（`cargo tauri build --features cuda`）。默认构建使用 CPU 模式。MSVC 14.50+ + CUDA 12.6 需设置 `NVCC_CCBIN` 环境变量以触发 `-allow-unsupported-compiler`。 |
| Tokenizer 离线依赖 | ✅ 已缓解 | `ensure_llm` 自动检测模型同目录下的 `tokenizer.json` 并优先使用，避免离线时从 HuggingFace 下载失败；同时检测 tokenizer 文件是否损坏（<1KB 则报错）。用户需自行将 tokenizer.json 与 .gguf 放在同一目录。 |
| 网络探测点不可配置 | ✅ 已交付 | `GuiSettings` 新增 `network_probe_url`（格式 `host:port`），Settings Panel 可自定义探测端点，默认仍为 `1.1.1.1:443`。`save_settings` 中对格式进行校验（必须含有效端口）。 |
| 启动时 LLM 配置失败静默 | ✅ 已交付 | `prewarm_llm` 失败后缓存错误到 `AppState.prewarm_error` 并 emit `llm:config_error`；前端挂载时调用 `get_prewarm_status` 主动查询，确保不错过启动期错误。 |
| 云端 provider 失败静默 fallback | ✅ 已修复 | `ensure_llm` 中明确指定 provider（非 auto/空）时，加载失败直接返回错误，不再静默 fallback 到 `auto_arc()`。只有未配置或显式 auto 时才自动探测。 |
| 离线模式自动 fallback | ✅ 已交付 | 后台每 30s TCP 探测 `1.1.1.1:443`（防抖阈值=2）；离线时自动切 local provider，恢复后切回；前端显示 banner 提示。启动时预加载避免首次请求阻塞。并发加载互斥锁防止重复加载。Settings 内存缓存避免每次请求读磁盘。 |
| `clarity-tauri` 运行时依赖系统 WebView | ⚠️ 已知限制 | Tauri 2 复用系统 WebView 引擎（Windows: WebView2 Runtime；macOS: WebKit；Linux: WebKit2GTK）。Release 构建后的 `.exe`/`.app` 不依赖 Node.js，但需要目标系统已安装对应 WebView 引擎。Windows 11 预装 WebView2；Windows 10 首次运行可能需要自动下载。TUI/Gateway/Headless/Claw 无此限制。 |
| `clarity-egui` i18n dead code | ⚠️ 已知限制 | `clarity-egui/src/i18n.rs:49` 的 `Locale::label()` 方法未被调用，触发 clippy `dead_code` warning。不影响功能，待清理。 |
| `clarity-claw` 系统控件依赖（已修复） | ✅ 已修复 | `inputbox` crate 0.1 在 Windows 上调用 `TaskDialogIndirect`（Common Controls v6），但程序未声明 manifest 依赖，导致旧版 `comctl32.dll` 找不到入口点。已移除 `inputbox`，改为 `cmd /c start` 打开浏览器。教训：任何调用系统对话框/UI 的 crate 都必须验证目标系统的最低版本和 manifest 声明。 |

已修复的历史问题见 [`CHANGELOG.md`](./CHANGELOG.md)。

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
