<!-- DOC-CONTRACT: 本文件维护 Agent 开发所需的运行上下文、架构耦合警告和代码风格。不维护功能清单、竞品对比或历史变更——这些参见 README.md / docs/ARCHITECTURE.md / docs/architecture/architecture-positioning.md / CHANGELOG.md / docs/planning/sprint-archive.md。 -->

# Agent Guidance for Project Clarity

> **Scope:** 本文件治理 `C:/Users/22414/dev/clarity` 及其所有子目录。  
> **Default branch:** `main`  
> **Version:** `0.4.0`（`Cargo.toml` workspace）  
> **Rust edition:** 2024 · **MSRV:** `1.85`（CI 使用 stable，推荐 `1.94+`）  
> **License:** AGPL-3.0-or-later  
> **Repository:** https://github.com/juice094/clarity

本文件使用中文撰写，因为项目源码注释、文档与提交信息以中文为主。

---

## 0. Ponytail 底层认知

本项目的 Agent 行为以 [Ponytail](https://github.com/DietrichGebert/ponytail) 的 lazy senior dev 原则为底层约束，叠加在后续各章节之上：

> 在写代码之前，按以下阶梯停在第一级可用的方案上：
> 1. 这东西真的需要建吗？（YAGNI）
> 2. 标准库已经做了？用它。
> 3. 原生平台能力已经覆盖？用它。
> 4. 已安装的依赖已经能解？用它。
> 5. 可以写成一行？就写成一行。
> 6. 只有到这一步，才写“刚好能跑”的最小代码。

执行纪律：
- 未被显式要求的抽象不要加；能避免的新依赖不要加；没人要的样板不要加。
- 删除优先于添加；无聊优先于聪明；文件越少越好。
- 对复杂需求要反问："你真的需要 X，还是 Y 已经够了？"
- 两种标准库方案等价时，选边缘情况正确的那个；lazy 意味着更少代码，而不是更脆弱。
- 若故意采用简化方案，用 `// ponytail:` 注释标记上限与升级路径。

不偷懒的边界：信任边界的输入校验、防止数据丢失的错误处理、安全、无障碍、显式要求的功能、非平凡逻辑的最小可运行检查（一个 assert/自测或最小测试）。没有这些检查的 lazy 代码是未完成的。

---

## 1. 项目概览

Clarity 是一个 **Rust 原生、本地优先的个人 AI 运行时**。同一套 Agent 引擎支撑多种入口（桌面 GUI、终端 TUI、Web IDE、无头 CLI、系统托盘、移动端 FFI），在本地完成 LLM 编排、工具调用、记忆持久化与审批流程。

关键事实：

- **单仓库 Workspace**，Rust 2024 edition，MSRV 1.85，许可证 AGPL-3.0-or-later。
- **Workspace 共 32 个 package**：25 个位于 `crates/` 的 `clarity-*` 业务 crate、6 个 `syncthing-*` 相关 crate（`third_party/syncthing-rust`）、1 个集成测试 crate `clarity-integration-tests`（`tests/integration`）。
- **已归档 crate**：`clarity-slint`、`clarity-tauri` 已从 workspace 移出，存放在 `.archive/` 中，不参与默认 CI。
- **前端 GUI 正在 P1c/P1d 重构**：原 `clarity-egui` 单 crate 正在被拆分为 `clarity-ui` / `clarity-shell` / `clarity-apps` / `clarity-chrome` 四层；`clarity-egui` 逐渐退化为具体 host/装配 crate。
- **前端 crate 之间禁止互相 import**，跨前端通信统一走 `clarity-wire`。
- **`clarity-core` 零依赖**于任何前端或网络 crate；`clarity-contract` 零内部依赖。
- **默认构建已包含本地 GGUF 推理**（`local-llm` feature），可选 CUDA 加速（`local-llm-cuda`）。
- **零外部运行时依赖**：`cargo install` 生成的二进制即可运行，无需 Python、Node.js 或 Ollama。
- **定位边界**：聚焦编码/工程工作流的本地 AI 协作者。无原生消息通道客户端（`clarity-channels` 仅提供通道抽象与 WeChat iLink 实验实现；Discord/Slack/Telegram 未启用）、无 Voice/Canvas、无完整移动端 UI（仅 Rust FFI 核心）。

---

## 2. 关键配置文件

| 文件 | 作用 |
|------|------|
| `Cargo.toml` | Workspace 配置、共享依赖、lint、profile。`members = ["crates/*", "tests/integration"]`。 |
| `.cargo/config.toml` | 增量编译、国内 crates.io 镜像（USTC）、已归档的 Slint 快捷命令。 |
| `crates/*/Cargo.toml` | 各 crate 依赖、features、bin/lib 声明。 |
| `.github/workflows/ci.yml` | 12-job CI：check / hermes-feature-check / test / integration-test / binary-test / doc-test / session-migration-test / clippy / fmt / audit / doc-guard / coverage。 |
| `.github/workflows/release.yml` | Tag 触发 release，产出 Windows `.msi`/`.exe`、Linux binary、SHA256 校验。 |
| `scripts/verify.ps1` | PowerShell 一键验收：README+AGENTS 存在性、编译、测试、Clippy、格式化，可生成 JSON 报告（`-Report`）。 |
| `scripts/test_runner.py` | Python 测试编排：统一跑 lib/bin/doc/integration 四层并生成 Markdown/JSON 报告。 |
| `scripts/doctor.py` | 环境健康检查（测试人员/新成员首选）。 |
| `docs/development/setup.md` | 完整构建/测试/feature/CUDA 说明。 |
| `docs/development/provider-config.md` | Provider、models.toml、环境变量配置指南。 |
| `docs/development/CODE-CHANGE-PRINCIPLES.md` | 跨 crate 代码改动七大原则（P1–P7）。 |
| `docs/testing/TEST_STRATEGY.md` | 测试分层、覆盖矩阵与演进路线。 |
| `docs/mobile-architecture.md` | Android/iOS 移动端 UniFFI 架构与实施路线。 |
| `SECURITY.md` | 安全策略、漏洞报告、已知边界。 |

---

## 3. Crate 拓扑与关键不变量

```text
contract
    ▲
    ├── {wire, memory, knowledge, mcp, llm, tools, channels, secrets, rollout}
    ├── thread-store (→ rollout)
    │
    ▼
  core ← {gateway, egui, tui, claw, headless, mobile-core}
    ▲
    ├── subagents（消费 core）
    └── telemetry（当前由 gateway 使用）
```

### 3.1 活跃 Clarity crate

| Crate | 类型 | 职责 |
|-------|------|------|
| `clarity-contract` | lib | 共享契约层：`LlmProvider` / `Tool` / `AgentError` / `FederationMessage` / `ThreadId` / `RolloutItem`。零内部依赖。 |
| `clarity-wire` | lib | UI ↔ Agent 事件总线（SPMC）、`ViewCommand` / `WireMessage`。 |
| `clarity-memory` | lib | SQLite/文件/混合记忆、BM25+向量、chunking、四级压缩归档。 |
| `clarity-knowledge` | lib | 本地知识索引与 AI 原生交互：`KnowledgeIndex` / `KnowledgeGraph` / `HybridRetriever` / `KnowledgeField`。支持 Markdown/wikilink/标签解析、混合检索、图传播激活与休眠，为 Agent 提供动态知识场。 |
| `clarity-mcp` | lib | MCP 客户端：stdio / SSE / HTTP / WebSocket。 |
| `clarity-llm` | lib | LLM provider 抽象 + 内置 provider + Candle GGUF。 |
| `clarity-tools` | lib | 内置工具库：file / shell / web / devkit / team / task / … |
| `clarity-secrets` | lib | ChaCha20-Poly1305 加密 Secret 存储（`enc2:`）。 |
| `clarity-channels` | lib | 外部消息通道抽象；当前实现 WeChat iLink（`chkit`）；Discord/Slack/Telegram 因上游 `rustls-webpki` 问题默认禁用；Webhook 默认启用。 |
| `clarity-subagents` | lib | 子代理执行器、并行调度、团队协调。 |
| `clarity-thread-store` | lib | Thread 持久化抽象：`ThreadStore` trait（API 设计受 Codex 启发）。 |
| `clarity-rollout` | lib | JSONL rollout 持久化：事件日志、压缩、回放（设计受 Codex 启发）。 |
| `clarity-core` | lib | Agent 循环（ReAct/Plan）、Approval、Skill、MCP 集成、Thread 生命周期。 |
| `clarity-telemetry` | lib | 统一遥测：WideEvent、metrics、traces、config audit。 |
| `clarity-mobile-core` | lib | 移动端 UniFFI FFI 核心，暴露 Runtime/事件/配置/记忆接口给 Kotlin/Swift。 |
| `clarity-ui` | lib | 共享 egui UI 原语与设计 token：theme、animation、i18n、design_system、可复用 widgets。 |
| `clarity-shell` | lib | egui 应用宿主：定义 `ClarityApp` / `AppState` / `ClarityAppContext` / `ClarityAppResponse`，为 sub-app 提供生命周期与依赖注入。 |
| `clarity-apps` | lib | 子应用实现：Chat / Settings / Dashboard / About / Provider。 |
| `clarity-chrome` | lib | 通用窗口 chrome：tabs、sidebar、status bar、modals、onboarding、resize handles；通过 `ChromeRenderer` 委托具体绘制。 |
| `clarity-gateway` | bin/lib | Axum HTTP/WebSocket 服务端、Web IDE、session store。 |
| `clarity-egui` | bin | 桌面 GUI（主前端栈），eframe + egui 纯 Rust；当前正在将面板/子应用迁出到 `clarity-apps`，自身退化为 host。 |
| `clarity-tui` | bin | ratatui 0.30 终端界面。 |
| `clarity-claw` | lib + bin | Clarity 分布式节点 client-side 统一入口：UI 无关的 Claw 客户端库（Gateway WebSocket、设备发现、配对、角色上下文同步、OpenClaw/KimiClaw 兼容层）+ 系统托盘常驻二进制。 |
| `clarity-headless` | bin | 无头 CLI（脚本 / CI 场景）。 |
| `clarity-anthropic-proxy` | bin | Anthropic Messages API 网关（默认走 DeepSeek device，协议转换由 `clarity-llm::anthropic` 适配器提供）。 |

### 3.2 第三方 syncthing-rust crate

`third_party/syncthing-rust` 作为 path 依赖参与构建，提供 Clarity 可靠性基础设施所需的生产模式：

| Crate | 职责 |
|-------|------|
| `bep-protocol` | Block Exchange Protocol 消息与握手。 |
| `syncthing-core` | 错误类型、配置、基础原语。 |
| `syncthing-net` | TLS/TCP/UDP/DERP/UPnP/STUN、拨号器、网络监控。 |
| `syncthing-sync` | 索引、合并、冲突解决、扫描器、拉取器、监督器。 |
| `syncthing-versioner` | 分级版本保留策略。 |
| `syncthing-test-utils` | 测试辅助。 |

> **新增说明**：`clarity-thread-store` 与 `clarity-rollout` 的 API 设计受到 OpenAI Codex（Apache-2.0）的架构启发；实现为 Clarity 原创代码，按 AGPL-3.0-or-later 发布。相关 crate 的 `NOTICES.md` 保留了灵感来源致谢。

### 3.3 已归档 crate

| Crate | 位置 | 状态 |
|-------|------|------|
| `clarity-slint` | `.archive/clarity-slint/` | Slint 实验前端，已从 workspace 移除。 |
| `clarity-tauri` | `.archive/clarity-tauri/` | Tauri 前端，已归档，不参与 workspace。 |

### 3.4 不可违反的不变量

1. `clarity-core` 不依赖任何前端 crate 或网络 crate（`egui`、`ratatui`、`axum` 等）。
2. `clarity-contract` 不依赖任何内部 crate。
3. 前端 crate 之间不互相 import；跨前端状态/事件走 `clarity-wire`。
4. 禁止在异步上下文中执行阻塞 I/O；使用 `tokio::task::spawn_blocking`。
5. `clarity-ui` / `clarity-shell` / `clarity-apps` / `clarity-chrome` 之间保持单向依赖：ui → shell/apps/chrome 可依赖 ui；shell 定义 app 契约；apps 实现契约；chrome 编排 apps。

> 详细 Worktree 与每个 crate 的关键路径见 §A「项目 Worktree 速查（OKF 知识图）」。

---

## 4. 技术栈与运行架构

| 层级 | 技术 |
|------|------|
| Agent 核心 | ReAct/Plan 循环、MCP stdio/SSE/HTTP/WebSocket、Approval 四层模式 |
| 本地推理 | Candle 原生 GGUF（Qwen2 / Qwen2.5 / DeepSeek-R1-Distill） |
| 记忆存储 | SQLite（WAL）+ BM25 + 向量搜索 + 四级压缩归档 |
| 桌面 GUI | eframe 0.35 / egui 0.35 / lucide-icons（纯 Rust，零 Web 依赖） |
| 终端 TUI | ratatui 0.30 / crossterm 0.29 |
| Web IDE | Axum 0.7 / tower-http / SSE / WebSocket |
| 移动端 | UniFFI 0.29 / `cargo-ndk` / Kotlin Compose / SwiftUI（计划中） |
| 事件总线 | `clarity-wire` SPMC 通道 |
| 加密 | ChaCha20-Poly1305（`clarity-secrets`） |
| TLS | `rustls-tls`（纯 Rust），`openssl` 已从依赖树移除 |
| 可靠性基础设施 | 融合 `syncthing-rust`：RetryConfig、Supervisor、NetMonitor、ToolOrchestrator、ToolResultCache、RacingProvider、WorkspaceDiff、RetentionPolicy、版本向量冲突解决 |

**核心运行入口**：

- `cargo run -p clarity-egui` — 桌面 GUI（主入口）。
- `cargo run -p clarity-tui` — 终端 TUI。
- `cargo run -p clarity-gateway` — Web IDE / HTTP+WebSocket 服务端。
- `cargo run -p clarity-claw` — 系统托盘任务监控。
- `cargo run -p clarity-headless -- --prompt "Hello" --provider local --output json` — 无头 CLI。

**Gateway 双端口**：

- `18790` — Public API（`0.0.0.0`）。
- `18800` — Admin + Web UI（`127.0.0.1` only）。

---

## 5. 代码组织

### 5.1 Workspace 结构

```text
clarity/
├── .archive/               # 已归档 crate（clarity-slint、clarity-tauri）
├── .cargo/                 # cargo 配置、增量编译、镜像
├── .clarity/               # 本地运行时数据（sessions、tasks、编译产物等）
├── .github/workflows/      # CI / Release
├── crates/                 # 25 个 clarity crate 目录
├── docs/                   # 架构、开发、安全、规划文档
├── examples/               # 独立示例脚本
├── mobile/                 # 移动端原生工程（Android 优先）
├── scripts/                # verify.ps1、test_runner.py、doctor.py 等
├── skills/                 # Agent 技能模板
├── tests/integration/      # 集成测试 crate
├── third_party/            # syncthing-rust 子模块
├── Cargo.toml              # workspace 根
└── AGENTS.md               # 本文件
```

### 5.2 `clarity-core` 核心模块（按源码目录）

| 模块 | 路径 | 职责 |
|------|------|------|
| Agent 循环 | `src/agent/` | `Agent`、ReAct/Plan、controller、run/flow/jumpy 子模块、streaming、execution、compaction、snapshot、LSP、`completion_inbox`（后台任务完成注入父 Agent 上下文） |
| 工具集成 | `src/tools/` | ToolRegistry、内置工具封装、MCP 工具映射；含编排工具 `agent` / `agent_swarm` / `task_*` / `schedule_cron`（`src/tools/agent.rs`、`task.rs`、`cron.rs`，经 `with_orchestrator` / `with_task_manager` / `with_cron_manager` 注入运行时依赖）；**具体工具实现位于 `clarity-tools` crate** |
| MCP | `src/mcp/` | 客户端集成、transport、config、devkit、enhanced |
| LLM 消费 | `src/llm` 不存在 | LLM provider 抽象位于 `clarity-llm` crate；`clarity-core` 通过 `LlmProvider` trait 消费 |
| 后台任务 | `src/background/` | `BackgroundTaskManager`、executor（支持 `set_agent_executor` 晚绑定）、scheduler、store；状态变更发射 `WireMessage::BackgroundTaskUpdate` |
| 记忆 | `src/memory/` | `PersistentMemoryStore`、`MemoryCompiler`、`SharedMemoryTicker`（`clarity-memory` 的 core 侧封装） |
| 审批 | `src/approval/` | Approval 模式、规则引擎 |
| Skill | `src/skills/` | Markdown+YAML 技能加载、注册、发现 |
| 压缩 | `src/compaction.rs` | 上下文压缩、Token 爆炸防护 |
| 自适应 | `src/adaptive/` | `AdaptiveModelRouter`、profile、predictor、compression |
| UI 状态 | `src/ui/` | `ViewState` 状态机、Router、FocusScope、RightRailPanel（跨前端共享） |
| 设置视图模型 | `src/view_models/` | `SettingsViewModel`、`SettingsSnapshot`、`ProviderModelEntry` |
| Session/Thread | `src/session/`、`src/thread/` | Session 上下文与持久化、Thread 生命周期 |
| OKF | `src/okf/` | Open Knowledge Format bundle/概念/知识图消费者 |
| 人格 | `src/personality/` | Personality 模板变量解析（当前 inactive） |
| 基础设施 | `src/config/`、`src/daemon/`、`src/hooks/`、`src/logging/`、`src/notifications/` | 配置、守护、钩子、日志、通知 |
| 实验性 Agent OS | `src/soul/`、`src/tier_bus/`、`src/hub/` | **EXPERIMENTAL / 未接入主循环** |

### 5.3 新 GUI 分层（P1c/P1d 重构中）

```text
clarity-egui (host / 装配 / 具体渲染回调)
    ├── clarity-chrome  ── Chrome<State, Renderer>，编排 titlebar / rails / main stage / modals
    ├── clarity-apps    ── ChatApp / SettingsApp / DashboardApp / AboutApp / ProviderRegistry
    ├── clarity-shell   ── ClarityApp trait、AppState trait、ClarityAppContext、ClarityAppResponse
    └── clarity-ui      ── Theme / design_system / animation / i18n / widgets
```

**当前迁移状态**：
- `clarity-apps` 已拥有 `ChatApp`、`SettingsApp`、`DashboardApp` 的壳与状态（`ChatStore`、`SettingsStore`、任务/团队/Cron store）。
- `clarity-egui::App` 实现 `clarity_shell::AppState`，并通过 `ChatRenderer` 临时 seam 把 chat 渲染体委托给 host。
- `clarity-shell` 提供 `ToastLevel`、`PairingState`、`BotInfo` 等跨应用共享类型。
- `clarity-chrome` 当前是轻量泛型框架，具体 chrome 绘制仍在 `clarity-egui::chrome`。

### 5.4 `clarity-egui` 结构要点

- `main.rs::update()` 每帧调用 `design_system::install_theme()`。
- `App::render_layout_shell()` 是 chrome / 主视图 / 浮层 / 模态框编排入口。
- 根目录关键文件：`app_logic.rs`、`app_state.rs`、`design_system.rs`、`theme.rs`、`layout.rs`、`i18n.rs`、`pretext.rs`、`pretext_alignment.rs`、`window_manager.rs`。
- `panels/` 按职责分组：`chat/`、`navigation_tree/`、`right_ide_panel/`、`settings/`、`modals/`、`system/`、`sidebar/` 等（`legacy/` 已在 S6 清理中删除）。`right_ide_panel/` 内含真实编排面板群：Task（任务列表/取消/查看结果）、Team（团队配置）、Dashboard（指标卡，`render_metrics` 与 `clarity-apps` 单源）、Subagents（egui-local tab，无 `RightRailPanel` 路由对应物，经 Bot 栏按钮开关）。
- `components/` 存放按域分组的可复用组件（`chat/`、`settings/`）。
- `widgets/` 存放可复用小部件；`theme.rs` 是 design token 单源。
- `handlers/` 处理 Agent/Wire 事件；`shortcuts/` 处理键盘路由；`services/` 封装后端交互；`stores/` 按域子模块组织。
- 已接入 Pretext 文字测量后端（`pretext-core` / `pretext-fontdb`），`MessageBubble` 与 `widgets/rich_paragraph.rs` 已转为 pretext-aware。
- `layout.rs` 提供 `LayoutMetrics` 与 `update_and_measure`，支撑 Pretext 三栏布局几何。
- `ui/debug_overlay.rs` 提供布局诊断覆盖层，快捷键 `Ctrl+Shift+L`。

### 5.5 `clarity-mobile-core` 结构要点

- `lib.rs` 导出模块与 UniFFI 入口。
- `runtime.rs` 持有 tokio Runtime 与 Agent 生命周期。
- `bridge.rs` 提供供 UniFFI 调用的同步 API（内部 `block_on`）。
- `events.rs` / `commands.rs` 映射移动端 `UiEvent` / `UserCommand`。
- `config.rs` / `memory.rs` / `sync.rs` 分别负责移动端配置、记忆查询、gateway/claw 同步。
- `build.rs` 调用 `uniffi_build` 生成绑定；`uniffi-bindgen.rs` 提供 bindgen CLI。
- 默认关闭 `clarity-core`/`clarity-llm` 的 `local-llm` feature，避免 Candle 在移动 ABI 上的 fullfp16 问题。

---

## 6. 构建与测试命令

### 6.1 常用命令

```bash
# 格式检查
cargo fmt --all -- --check

# 编译所有 lib / bin target
cargo check --workspace --lib --bins

# Clippy 零警告（完整 workspace）
cargo clippy --workspace --lib --bins --tests -- -D warnings

# 单元测试（完整 workspace）
cargo test --workspace --lib

# 二进制测试
cargo test --workspace --bins -- --test-threads=2

# 文档测试（完整 workspace）
cargo test --workspace --doc -- --test-threads=2

# 集成测试
cargo test -p clarity-integration-tests --lib

# 安全审计
cargo audit --deny unsound --deny yanked

# 文档构建（完整 workspace）
cargo doc --workspace --no-deps
```

> **注意**：`clarity-slint` 与 `clarity-tauri` 已移入 `.archive/`，不再是 workspace 成员，所有命令都不再需要 `--exclude clarity-slint`。

### 6.2 推荐本地验证流程

```bash
# 1. 格式化
cargo fmt --all -- --check

# 2. 单元测试（完整 workspace）
cargo test --workspace --lib

# 3. 二进制测试
cargo test --workspace --bins -- --test-threads=2

# 4. 文档测试
cargo test --workspace --doc -- --test-threads=2

# 5. 集成测试
cargo test -p clarity-integration-tests --lib

# 6. Clippy（完整 workspace）
cargo clippy --workspace --lib --bins --tests -- -D warnings
```

> 提交前至少保证上述流程通过。

### 6.3 一键验收与测试编排

```powershell
# PowerShell 一键验收（README/AGENTS/编译/测试/Clippy/格式化）
.\scripts\verify.ps1 --all -Strict

# Python 测试编排：统一跑 lib/bin/doc/integration 四层并生成 Markdown/JSON 报告
python scripts/test_runner.py
python scripts/test_runner.py --markdown target/test-report.md --json target/test-report.json

# 环境健康检查（测试人员/新成员首选）
python scripts/doctor.py
```

`verify.ps1` 从 `cargo metadata` 读取 workspace 成员（自动尊重 `Cargo.toml` 的 `exclude`），逐 crate 检查 README、AGENTS、编译、测试、Clippy、格式化，并可生成 JSON 报告（`-Report`）。

`scripts/test_runner.py` 与 `scripts/doctor.py` 是开发与测试编排工具，**Python 不是 Clarity 运行时依赖**；它们用于解耦测试执行与发布二进制，并为 CI/QA 提供结构化报告。

### 6.4 Feature 与构建变体

| Feature | 作用 | 使用场景 |
|---------|------|----------|
| `local-llm` | 启用 Candle GGUF 本地推理 | `clarity-llm` / `clarity-core` 默认开启 |
| `local-llm-cuda` | 本地推理 CUDA 加速 | Windows + NVIDIA CUDA |
| `mcp` | 启用 MCP 集成 | `clarity-core` 默认 |
| `session-migration` | Session V1→V2 迁移工具 | `clarity-core` / `clarity-headless` 可选 |
| `line-mode` | egui 行级渲染管线 | `clarity-egui` 可选 |
| `slack` / `discord` / `telegram` / `webhook` | Gateway 通道 feature | 默认仅 `webhook` |
| `telemetry-api` | Gateway 遥测 REST API | `clarity-gateway` 可选 |
| `anthropic-api` | Gateway Anthropic Messages API 兼容端点 `POST /v1/messages` | `clarity-gateway` 可选，默认关闭 |
| `hermes` | `clarity-memory` / `clarity-core` / `clarity-egui` / `clarity-tui` / `clarity-gateway` / `clarity-headless` 可选的 hermes-memory SQLite 后端 | 实验性，默认关闭；通过 `CLARITY_MEMORY_BACKEND=hermes` 启用。**注意**：需要本地 `hermes-memory` 仓库位于 `../../../hermes-memory/` 相对路径。 |
| `svg` | egui SVG 渲染支持（resvg + tiny-skia） | `clarity-egui` 可选 |

CUDA 构建示例（Windows）：

```powershell
$env:NVCC_CCBIN="C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.40.33807\bin\Hostx64\x64"
cargo check -p clarity-llm --features local-llm-cuda
cargo run -p clarity-egui --features cuda
```

---

## 7. 代码风格与健康规则

### 7.1 工程红线（Workspace lint）

| 规则 | 来源 | 说明 |
|------|------|------|
| `missing_docs = "deny"` | `Cargo.toml` | 所有 `pub` 项必须有 `///` 文档注释 |
| `unsafe_code = "deny"` | `Cargo.toml` | 禁止新增 `unsafe`；白名单 2 处：`clarity-memory`；`clarity-knowledge`（`local-embedding` feature 内 sqlite-vec `sqlite3_auto_extension` 注册，2026-07-20 批准） |
| `unwrap_used = "deny"` | Clippy | 新增 `unwrap()` 必须配 `// SAFE: <不变量说明>` |
| `expect_used = "deny"` | Clippy | 同上 |
| `panic = "deny"` | Clippy | 禁止新增 `panic!` |
| 无 `TODO/FIXME/XXX` | 项目纪律 | 暂存事项转入 GitHub Issue 或 `docs/notes/` |

测试代码允许临时放宽上述限制。各 crate 根通常包含：

```rust
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        unsafe_code
    )
)]
```

### 7.2 提交规范

格式：`<type>(<scope>): <imperative summary>`

| Type | 场景 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `docs` | 仅文档变更 |
| `refactor` | 无行为变更重构 |
| `test` | 测试增修 |
| `chore` | 依赖/CI/格式化 |
| `perf` | 性能优化 |

Scope：`core`、`memory`、`gateway`、`egui`、`tui`、`claw`、`wire`、`headless`、`mobile`、`ui`、`shell`、`apps`、`chrome`、`ci`、`docs`。

- 一个 commit 只处理一个关注点。
- 每个 commit 必须独立可编译（`P5`）。
- 修改 `clarity-core`、`llm`、`AgentController`、`Op`、`WireMessage` 后必须跑完整测试集。

### 7.3 跨层变更检查单

修改 `WireMessage`、`ViewCommand`、`UserAction` 或 `clarity-core` 核心类型时，必须同步检查：

1. `clarity-tui` 中的事件处理与渲染逻辑
2. `clarity-gateway` 中的 HTTP API / WebSocket 序列化
3. `tests/integration` 中的断言匹配
4. egui `protocol_renderer.rs`、TUI `protocol_renderer.rs`、Gateway `ws.rs`
5. `clarity-mobile-core` 的 FFI 事件/命令映射

### 7.4 egui 前端规范

- 所有用户可见字符串通过 `i18n`（`t!("key")`），禁止硬编码中英文。
- 使用 `Frame::new()` 保持主题一致。
- 面板渲染函数控制在 300 行以内；超出则拆分子组件。
- 优先使用 `ScrollArea` + `AlwaysHidden` 滚动条以保持玻璃拟态风格。
- 模态框使用 `Frame::window` + `radius_lg` + 遮罩层 + Escape/点击外部关闭。
- Design token 强制：在 `crates/clarity-egui/src/{panels,components,widgets}/**` 与对应新 crate（`clarity-ui`、`clarity-apps`、`clarity-chrome`）下，任何 `> 8.0` 的浮点字面量必须路由到 `theme.space_* / text_* / radius_*` 或加 `// LAYOUT-EXEMPT: <理由>`。

### 7.5 代码改动七大原则

详见 `docs/development/CODE-CHANGE-PRINCIPLES.md`：

- **P1** — 单向迁移：禁止双向桥接。
- **P2** — 删除优先：每个 PR 净删除代码或不增加 dead code。
- **P3** — 单源真相：每个概念只有一个写入点。
- **P4** — 测试先行：重构前先有测试 baseline。
- **P5** — 编译可分：每个 commit 独立可编译。
- **P6** — Theme Token 强制：egui 布局字面量必须 token 化。
- **P7** — 协议层不前瞻：新增协议类型必须同时有 producer、consumer 和端到端测试。

### 7.6 Ponytail lazy-senior-dev 原则（Rust 本地化）

本仓库额外吸收 [Ponytail](https://github.com/DietrichGebert/ponytail) 的 lazy-senior-dev 风格，作为 P1–P7 的补充，目标是**在保持安全与正确的前提下写得更少**。

| 原则 | Rust/Clarity 实践 |
|------|-------------------|
| **YAGNI** | 不为未来扩展预写 trait wrapper、泛型层、配置开关。新增抽象必须被当前至少两个调用方需要。 |
| **优先 stdlib / 已有依赖** | 能用 `std::fs`、`std::path`、`std::collections`、`tokio::sync` 解决的问题，不引入新 crate。 |
| **删除优于添加** | 每个 PR 尽量净删代码；移除 dead code、unused feature、obsolete 注释。 |
| **显式标记 shortcut** | 任何故意简化且已知上限的实现必须加 `// ponytail: <上限>；<升级路径>`，例如 `// ponytail: O(n²) scan; replace with index if items > 1000`。 |
| **信任边界必须校验** | 路径、用户输入、网络响应、MCP 命令在边界处校验，与现有 `sanitize_path`、`validate_mcp_command` 等规则一致。 |
| **非平凡逻辑留一个可运行检查** | 新增纯函数、状态机、算法必须配单元测试；egui 逻辑优先写纯函数测试。Trivial one-liner 可免测。 |
| **Boring over clever** | 同样的功能，选择未来维护者 3 点能看懂的写法；避免宏技巧、隐式 trait 魔术。 |
| **输入验证 + 错误处理防数据丢失** | lazy 不等于省略错误处理；任何可能失败的 IO/序列化/网络操作必须处理，禁止 `unwrap()` 无 `// SAFE:` 注释。 |

> **应用方式**：新增/修改代码时按上表自问；代码审查时检查 `// ponytail:` 标记与测试覆盖。不追求一次性全仓库重构，而是**每次改动让相关文件比修改前更薄**。

---

## 8. 测试策略

### 8.1 测试分层

| 类型 | 命令 | 说明 |
|------|------|------|
| 单元测试 | `cargo test --workspace --lib` | 各 crate 内 `#[cfg(test)]` |
| 二进制测试 | `cargo test --workspace --bins -- --test-threads=2` | bin target 逻辑测试 |
| 文档测试 | `cargo test --workspace --doc -- --test-threads=2` | `rustdoc` 示例 |
| 集成测试 | `cargo test -p clarity-integration-tests --lib` | acp_bridge / adaptive_loop / session_v2_migration / subagent_api / subagent_ws / telemetry_end_to_end / thread_api |
| 覆盖率 | `cargo llvm-cov --workspace --lib --lcov --output-path lcov.info` | CI 产出 LCOV/HTML |

### 8.2 测试纪律

- 新增 Rust 模块必须含 `#[cfg(test)]` 单元测试。
- Bug fix 必须配回归测试（先红后绿）。
- egui 面板/组件变更需补充手动 QA 或视觉回归检查。
- 性能改动需补充 benchmark 或延迟测量。
- `clarity-egui` 当前以纯逻辑/小部件单元测试为主；面板级 UI 集成测试待 Pretext 三栏布局稳定后引入 `egui_kittest` snapshot。

### 8.3 手动 QA 清单（UI 变更）

- [ ] egui 窗口打开无 panic、无布局溢出
- [ ] Settings 弹窗可打开、保存、重启后持久化
- [ ] 语言切换（中文/English）生效且持久化
- [ ] 断网时 Offline banner 出现
- [ ] 本地模型选择正确扫描 `~/models/`
- [ ] Tab bar 全宽渲染且 active 状态正确
- [ ] 左侧边栏全局工具栏按钮（Online/Token/Lang/Skills/MCP/Settings）全部可用

---

## 9. 安全与部署

### 9.1 安全模型

| 层 | 机制 |
|----|------|
| 路径遍历 | `resolve_path()` / `sanitize_path()` 限制在工作目录内 |
| MCP 命令注入 | `validate_mcp_command()` 拦截 shell 元字符与相对路径 |
| 敏感文件 | 自动检测 `.env`、SSH key、kubeconfig |
| 工具审批 | `requires_approval()` 门控高风险工具 |
| API Key | 支持 `${env:VAR}` 语法避免明文落盘；`clarity-secrets` 提供 `enc2:` 加密 |
| TLS | `rustls-tls`（纯 Rust），`openssl` 已从依赖树移除 |
| 快照隔离 | Side-Git 快照使用独立 bare 仓库 `~/.clarity/snapshots/` |
| 移动端密钥 | 推荐 Android Keystore / iOS Keychain；Rust FFI 层不持久化明文 API key |

### 9.2 漏洞报告

- 首选：[GitHub Security Advisory](https://github.com/juice094/clarity/security/advisories/new)（私密）。
- 备用：邮件 `juice094@users.noreply.github.com`，主题 `[Clarity Security] <简述>`。
- 响应时间：Critical 补丁 14 天内，High 补丁 30 天内。

### 9.3 部署流程

- **CI**：`.github/workflows/ci.yml` 在 `push`/`pull_request` 到 `main` 时触发，覆盖 ubuntu-latest / windows-latest / macos-latest；已移除对归档 crate `clarity-slint` 的 `--exclude` 引用。
- **Release**：`.github/workflows/release.yml` 在 `v*` tag 推送时触发：
  - Windows：`cargo build --release -p clarity-egui` → 自签名代码签名 → `cargo-wix` 生成 `.msi`。
  - Linux：构建二进制并上传。
  - 统一生成 SHA256 校验和并发布 GitHub Release。
- **本地安装**：`cargo install --path crates/clarity-egui` 等。

---

## 10. 环境变量速查

```powershell
$env:KIMI_CODE_API_KEY="sk-kimi-..."
$env:KIMI_API_KEY="sk-..."
$env:ANTHROPIC_AUTH_TOKEN="..."
$env:DEEPSEEK_API_KEY="sk-..."
$env:OPENAI_API_KEY="sk-..."
$env:OLLAMA_HOST="http://localhost:11434"
$env:CLARITY_LOCAL_MODEL_PATH="C:\path\to\model.gguf"
$env:CLARITY_MODELS_CONFIG="C:\path\to\models.toml"
$env:CLARITY_APPROVAL_MODE="interactive"   # interactive | smart | plan | yolo
$env:CLARITY_MEMORY_BACKEND="hermes"        # 可选，启用 hermes-memory SQLite 后端
```

Provider 配置、models.toml、加密 key 详见 [`docs/development/provider-config.md`](docs/development/provider-config.md)。

---

## 11. 当前工作与已知限制

> 本节只保留对 Agent 行为有影响的结构性事实；具体功能清单、里程碑和历史状态参见 [`docs/planning/PROJECT_STATUS.md`](docs/planning/PROJECT_STATUS.md)、[`ROADMAP.md`](ROADMAP.md) 与 [`CHANGELOG.md`](CHANGELOG.md)。

### 11.1 已稳定落地的关键能力（摘要）

- 多入口前端矩阵：egui 桌面 GUI、ratatui TUI、Axum Gateway/Web IDE、headless CLI、claw 系统托盘节点。
- 新 GUI 分层：`clarity-ui` / `clarity-shell` / `clarity-apps` / `clarity-chrome` 已承担设计系统、应用宿主、子应用、chrome 框架职责。
- Pretext 文字测量与三栏布局；`clarity_core::ui::ViewState` 状态机统一；项目模型与上下文驱动；Provider / Secret 体系；Thread / Session 持久化；移动端 FFI 核心；Claw 协议统一；syncthing-rust 可靠性基础设施融合；Knowledge Field 基础；前端架构审计与性能/交互优化。
- 子代理编排（2026-07-28）：LLM 可调 `agent` / `agent_swarm` 内置工具发起单/并行子代理（`{{item}}` 模板、默认超时 1800s）；`task_create` 创建的后台任务真实执行（manager 绑定 + executor 晚绑定）；完成通知经 `CompletionInbox` 注入父 Agent 上下文；wire 新增子代理/后台任务事件五 variant，egui 收敛 wire 单源、TUI/Gateway/mobile 各自消费；egui 右栏 Task/Team/Dashboard/Subagents 编排面板群落地。详见 [`docs/planning/plans/2026-07-28-egui-frontend-subagent-sprints.md`](docs/planning/plans/2026-07-28-egui-frontend-subagent-sprints.md)。

### 11.2 实验性 / 未完成方向

- **外部消息通道**：`clarity-channels` 当前仅实现 WeChat iLink（`chkit`）；Discord / Slack / Telegram 因上游 `rustls-webpki` 问题默认禁用；Webhook 默认可用。
- **移动端完整 UI**：仅 Rust FFI 核心落地，Android / iOS 完整 UI 仍在 `mobile/` 与 `docs/mobile-architecture.md` 路线图中。
- **Hermes 记忆后端**：可选 `hermes` feature，需本地 `../../../hermes-memory/` 仓库，处于实验阶段。
- **Agent OS 模块**：`clarity-core::soul` / `tier_bus` / `hub` 及 `clarity-egui::window_manager` 为实验性愿景，**未接入主 ReAct/Plan 循环**。
- **GUI 分层迁移**：`clarity-egui` 仍在通过 `ChatRenderer` 等临时 seam 与 `clarity-apps` 协作；完全迁移完成后 `clarity-egui` 将退化为纯 host crate。

### 11.3 已知限制

- Discord / Telegram 默认禁用，等待上游 `rustls-webpki` 修复。
- Gateway HTTP Chat Completions 默认无状态；完整 session 请用 WebSocket 或传 `session_id`。
- `hermes` feature 依赖位于 `../../../hermes-memory/` 的本地仓库，CI 与未检出该仓库的环境需跳过 hermes 相关检查。
- `agent` / `agent_swarm` 工具路径（orchestrator → `ParallelExecutor`）的子代理尚不发射 wire 事件，需把 wire 穿透 `ToolContext`（contract 变更，待做）。
- egui turn header 的 `token_count` 未接线（wire Usage 已能携带 turn_id，剩 message→turn 聚合逻辑）。
- egui/gateway 的长期 `BackgroundTaskManager` 未挂 app 级 wire，`BackgroundTaskUpdate` 在 egui 侧显式忽略（任务 UI 走 TaskStore 轮询）。
- TUI/headless 的 task 工具仍走 legacy `~/.clarity/tasks` 路径（未调 `with_task_manager`），创建的任务无消费者。
- Team 面板是配置列表；`team_create` 工具只写配置不执行（`TeamCoordinator` 无工具/REST 入口）。

---

## 12. 更多参考

| 主题 | 文档 |
|------|------|
| 构建/测试/验证 | [`docs/development/setup.md`](docs/development/setup.md) |
| 测试策略 | [`docs/testing/TEST_STRATEGY.md`](docs/testing/TEST_STRATEGY.md) |
| 测试人员上手指南 | [`docs/testing/TESTER_GUIDE.md`](docs/testing/TESTER_GUIDE.md) |
| 模块解构与工程路线对照 | [`docs/planning/CLARITY_MODULE_RESEARCH.md`](docs/planning/CLARITY_MODULE_RESEARCH.md) |
| 架构健康迭代 | [`docs/development/ARCHITECTURE_HEALTH.md`](docs/development/ARCHITECTURE_HEALTH.md) |
| Provider 配置 | [`docs/development/provider-config.md`](docs/development/provider-config.md) |
| 代码改动原则 | [`docs/development/CODE-CHANGE-PRINCIPLES.md`](docs/development/CODE-CHANGE-PRINCIPLES.md) |
| 移动端架构 | [`docs/mobile-architecture.md`](docs/mobile-architecture.md) |
| 代码级架构 | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| 技术栈与 Crate 拓扑 | [`docs/architecture/tech-stack.md`](docs/architecture/tech-stack.md) |
| 项目定位与生态关系 | [`docs/architecture/architecture-positioning.md`](docs/architecture/architecture-positioning.md) |
| 当前阶段与已知问题 | [`docs/planning/current-phase.md`](docs/planning/current-phase.md) |
| 项目状态报告 | [`docs/planning/PROJECT_STATUS.md`](docs/planning/PROJECT_STATUS.md) |
| 路线图 | [`docs/planning/ROADMAP.md`](docs/planning/ROADMAP.md) |
| 安全与运维 | [`docs/security/operations.md`](docs/security/operations.md) |
| 贡献指南 | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| 变更日志 | [`CHANGELOG.md`](CHANGELOG.md) |
| **协议层设计与映射** | [`docs/architecture/protocol-layer.md`](docs/architecture/protocol-layer.md) |
| **Claw 协议策略**（Gateway WebSocket / OpenClaw fallback） | [`docs/architecture/claw-protocol.md`](docs/architecture/claw-protocol.md) |
| **生命周期与管线图例** | [`docs/architecture/lifecycle-diagrams.md`](docs/architecture/lifecycle-diagrams.md) |

---

## 13. 架构文档维护纪律

1. **新增/删除 crate 后**，必须同步更新以下文件中的 crate 拓扑：
   - `Cargo.toml` workspace members
   - `docs/ARCHITECTURE.md` §Crate Topology
   - `docs/architecture/tech-stack.md` §Crate 拓扑 / §架构依赖方向
   - `docs/architecture/map-topology.md` §Crate 依赖图
   - `AGENTS.md` §3 Crate 拓扑、§A 项目 Worktree 速查
2. **引入外部项目思想/设计参考时**，禁止使用 "derived from"、"ported from"、"original source files"、"derivative work" 等源码归属措辞；统一使用 "架构启发"、"设计参考"。
3. **实验性模块必须标注 `EXPERIMENTAL`**，不得与稳定接口混为一谈；未接入主循环的愿景功能必须标注 "愿景/未实现"。
4. **禁止把个人开发环境的本地路径**（如 `dev/third_party/xxx`、`Desktop/xxx`、`AppData/...`）写入项目架构文档；外部项目仅说明名称和关系类别即可。
5. **NOTICES.md 仅用于致谢思想/设计来源**，不用于声明代码派生关系；若不存在实际代码引用，不得保留源码归属性语言。
6. **每 crate 必须同时存在 `README.md` 与 `AGENTS.md`**，以满足 CI `doc-guard` 检查；新增 crate 时应在首 commit 一并创建。

> **当前缺口**：`clarity-ui`、`clarity-shell`、`clarity-apps`、`clarity-chrome` 四个 crate 目前同时缺失 `README.md` 与 `AGENTS.md`，违反本纪律第 6 条。应在完成 GUI 分层迁移时补齐，或在最近的 doc-guard 修复 commit 中优先处理。

---

## A. 项目 Worktree 速查（OKF Bundle）

> 本附录遵循 Google 开源的 **Open Knowledge Format（OKF）**：每个 crate 是一个独立的 Markdown 概念文件，使用 YAML frontmatter 描述类型、分层、依赖与被依赖关系，便于 AI Agent 跨会话/跨组织消费。
>
> **OKF Bundle 入口**：[`docs/okf/clarity-worktree/index.md`](docs/okf/clarity-worktree/index.md)  
> **概念文件目录**：[`docs/okf/clarity-worktree/concepts/`](docs/okf/clarity-worktree/concepts/)  
> 代码级精确架构仍归 [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) 与 [`docs/architecture/map-topology.md`](docs/architecture/map-topology.md) 管理。

### A.1 分层概念图

```text
[Presentation Layer — 入口前端]
  ├─ clarity-egui          (bin)  桌面 GUI（主入口，host/装配角色）
  ├─ clarity-tui           (bin)  终端 UI
  ├─ clarity-gateway       (bin/lib)  Web IDE / HTTP + WebSocket
  ├─ clarity-claw          (lib + bin)  Claw 客户端库 + 系统托盘节点
  ├─ clarity-headless      (bin)  无头 CLI / CI
  └─ clarity-mobile-core   (lib)  移动端 UniFFI FFI 核心

[Agent Kernel — 运行时核心]
  └─ clarity-core          (lib)  ReAct/Plan、Approval、Skill、MCP、Thread 生命周期

[Infrastructure / Capability — 能力 crate]
  ├─ clarity-memory        (lib)  SQLite + BM25 + 向量搜索
  ├─ clarity-knowledge     (lib)  本地知识索引、知识图、动态知识场
  ├─ clarity-llm           (lib)  Provider 抽象 + Candle GGUF 本地推理
  ├─ clarity-tools         (lib)  内置工具库
  ├─ clarity-mcp           (lib)  MCP 客户端（stdio/SSE/HTTP/WS）
  ├─ clarity-channels      (lib)  外部通道抽象（WeChat iLink / Webhook）
  ├─ clarity-secrets       (lib)  加密 Secret 存储（enc2:）
  ├─ clarity-telemetry     (lib)  统一遥测（当前由 gateway 使用）
  ├─ clarity-subagents     (lib)  子代理执行器（消费 core）
  ├─ clarity-thread-store  (lib)  Thread 持久化抽象
  └─ clarity-rollout       (lib)  JSONL rollout 持久化

[GUI Layer — egui 新分层]
  ├─ clarity-ui            (lib)  共享 UI 原语与设计 token
  ├─ clarity-shell         (lib)  应用宿主与 ClarityApp 契约
  ├─ clarity-apps          (lib)  Chat / Settings / Dashboard / About / Provider
  └─ clarity-chrome        (lib)  通用窗口 chrome 框架

[Contract / Protocol — 零业务逻辑的契约层]
  ├─ clarity-contract      (lib)  共享 trait / 类型（零内部依赖）
  └─ clarity-wire          (lib)  UI ↔ Agent SPMC 事件总线

[Third Party]
  └─ syncthing-rust        (workspace)  可靠性基础设施（net/sync/versioner/protocol）

[Archived]
  ├─ clarity-slint         (bin)  实验性 Slint GUI（已移入 .archive/）
  └─ clarity-tauri         (bin)  已归档 Tauri 前端（已移入 .archive/）
```

### A.2 OKF 概念节点

每个 crate 的完整元数据（frontmatter）与职责说明见对应概念文件：

- [clarity-contract](docs/okf/clarity-worktree/concepts/clarity-contract.md)
- [clarity-wire](docs/okf/clarity-worktree/concepts/clarity-wire.md)
- [clarity-core](docs/okf/clarity-worktree/concepts/clarity-core.md)
- [clarity-memory](docs/okf/clarity-worktree/concepts/clarity-memory.md)
- [clarity-knowledge](docs/okf/clarity-worktree/concepts/clarity-knowledge.md)
- [clarity-llm](docs/okf/clarity-worktree/concepts/clarity-llm.md)
- [clarity-mcp](docs/okf/clarity-worktree/concepts/clarity-mcp.md)
- [clarity-tools](docs/okf/clarity-worktree/concepts/clarity-tools.md)
- [clarity-channels](docs/okf/clarity-worktree/concepts/clarity-channels.md)
- [clarity-secrets](docs/okf/clarity-worktree/concepts/clarity-secrets.md)
- [clarity-subagents](docs/okf/clarity-worktree/concepts/clarity-subagents.md)
- [clarity-rollout](docs/okf/clarity-worktree/concepts/clarity-rollout.md)
- [clarity-thread-store](docs/okf/clarity-worktree/concepts/clarity-thread-store.md)
- [clarity-telemetry](docs/okf/clarity-worktree/concepts/clarity-telemetry.md)
- [clarity-gateway](docs/okf/clarity-worktree/concepts/clarity-gateway.md)
- [clarity-egui](docs/okf/clarity-worktree/concepts/clarity-egui.md)
- [clarity-tui](docs/okf/clarity-worktree/concepts/clarity-tui.md)
- [clarity-claw](docs/okf/clarity-worktree/concepts/clarity-claw.md)
- [clarity-headless](docs/okf/clarity-worktree/concepts/clarity-headless.md)
- [clarity-mobile-core](docs/okf/clarity-worktree/concepts/clarity-mobile-core.md)
- [clarity-ui](docs/okf/clarity-worktree/concepts/clarity-ui.md)
- [clarity-shell](docs/okf/clarity-worktree/concepts/clarity-shell.md)
- [clarity-apps](docs/okf/clarity-worktree/concepts/clarity-apps.md)
- [clarity-chrome](docs/okf/clarity-worktree/concepts/clarity-chrome.md)
- [clarity-anthropic-proxy](docs/okf/clarity-worktree/concepts/clarity-anthropic-proxy.md)
- [clarity-slint](docs/okf/clarity-worktree/concepts/clarity-slint.md)
- [clarity-tauri](docs/okf/clarity-worktree/concepts/clarity-tauri.md)

### A.3 生成脚本

OKF bundle 由 [`scripts/generate_okf_worktree.py`](scripts/generate_okf_worktree.py) 生成。当新增/删除 crate 或变更依赖关系时，应更新该脚本并重新运行，以保持 bundle 与代码一致。

*最后更新：2026-07-26*
