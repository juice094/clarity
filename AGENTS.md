<!-- DOC-CONTRACT: 本文件维护 Agent 开发所需的运行上下文、架构耦合警告和代码风格。不维护功能清单、竞品对比或历史变更——这些参见 README.md / docs/ARCHITECTURE.md / docs/architecture/architecture-positioning.md / CHANGELOG.md / docs/planning/sprint-archive.md。 -->

# Agent Guidance for Project Clarity

> **Scope:** 本文件治理 `C:/Users/22414/dev/clarity` 及其所有子目录。  
> **Default branch:** `main`  
> **Version:** `0.3.0`（`Cargo.toml`）  
> **Rust edition:** 2024 · **MSRV:** `1.85`  
> **License:** AGPL-3.0-or-later  
> **Repository:** https://github.com/juice094/clarity

本文件使用中文撰写，因为项目源码注释、文档与提交信息以中文为主。

---

## 1. 项目概览

Clarity 是一个 **Rust 原生、本地优先的个人 AI 运行时**。用同一套 Agent 引擎支撑多种入口（桌面 GUI、终端 TUI、Web IDE、无头 CLI、系统托盘），在本地完成 LLM 编排、工具调用、记忆持久化与审批流程。

关键事实：

- **单仓库 Workspace**，Rust 2024 edition，MSRV 1.85。
- **17 个活跃 crate + 1 个归档 crate**（`clarity-tauri`）。
- **前端 crate 之间禁止互相 import**，跨前端通信统一走 `clarity-wire`。
- **`clarity-core` 零依赖**于任何前端或网络 crate；`clarity-contract` 零内部依赖。
- **默认构建已包含本地 GGUF 推理**（`local-llm` feature），可选 CUDA 加速。

---

## 2. Crate 拓扑与关键不变量

| Crate | 类型 | 职责 |
|-------|------|------|
| `clarity-contract` | lib | 共享契约层：`LlmProvider`/`Tool`/`AgentError`/`FederationMessage` |
| `clarity-wire` | lib | UI ↔ Agent 事件总线（SPMC）、`ViewCommand`/`WireMessage` |
| `clarity-memory` | lib | SQLite/文件/混合记忆、BM25+向量、chunking |
| `clarity-mcp` | lib | MCP 客户端：stdio / SSE / HTTP / WebSocket |
| `clarity-llm` | lib | LLM provider 抽象 + 内置 provider + Candle GGUF |
| `clarity-tools` | lib | 内置工具库：file / shell / web / devkit / … |
| `clarity-secrets` | lib | ChaCha20-Poly1305 加密 Secret 存储（`enc2:`） |
| `clarity-channels` | lib | 外部消息通道：Discord / Slack / Telegram / Webhook / 微信 iLink |
| `clarity-subagents` | lib | 子代理执行器、并行调度、团队协调 |
| `clarity-core` | lib | Agent 循环（ReAct/Plan）、Approval、Skill、MCP 集成 |
| `clarity-telemetry` | lib | 统一遥测：WideEvent、metrics、traces、config audit |
| `clarity-gateway` | bin/lib | Axum HTTP/WebSocket 服务端、Web IDE、session store |
| `clarity-egui` | bin | 桌面 GUI（主前端栈），eframe + egui 纯 Rust |
| `clarity-tui` | bin | ratatui 终端界面 |
| `clarity-claw` | bin | 系统托盘后台监控 |
| `clarity-headless` | bin | 无头 CLI（脚本 / CI 场景） |
| `clarity-slint` | bin | 桌面 GUI 实验栈，Slint（不参与默认 CI） |
| `clarity-tauri` | bin | Tauri 前端（**已归档**，被 workspace 排除） |

**架构依赖方向**：

```text
contract ← {wire, memory, mcp, llm, tools, channels} ← core ← {gateway, egui, tui, claw, headless}
                                                          ↑
                                                    subagents（消费 core）
                                                    telemetry（横切关注）
```

**不可违反的不变量**：

1. `clarity-core` 不依赖任何前端 crate 或网络 crate。
2. `clarity-contract` 不依赖任何内部 crate。
3. 前端 crate 之间不互相 import；跨前端状态/事件走 `clarity-wire`。

---

## 3. 快速参考

### 常用命令

```bash
# 格式 / 编译 / 测试 / 审计
cargo fmt --all -- --check
cargo check --workspace --lib --bins --exclude clarity-slint
cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings
cargo test --workspace --lib --exclude clarity-slint
cargo test -p clarity-integration-tests --lib
cargo audit --deny unsound --deny yanked

# 运行入口
cargo run -p clarity-egui
cargo run -p clarity-tui
cargo run -p clarity-gateway
cargo run -p clarity-headless -- --prompt "Hello" --provider local --output json
```

详细命令、Feature、CUDA 构建见 [`docs/development/setup.md`](docs/development/setup.md)。

### 当前工作（S6）

正在进行 **Pretext 单页面 / 三栏布局迁移**（详见 [`docs/planning/plans/clarity-egui-pretext-layout-migration.md`](docs/planning/plans/clarity-egui-pretext-layout-migration.md)）。

- **Phase A 已完成**：新增 `LeftRailSection` / `RightRailSection` / `ViewState` rail 字段；移除 `UiStore.sidebar_collapsed` 并将旧 sidebar 折叠状态迁移到 `view_state.left_rail_expanded`；`clarity-egui` 新增 `render_left_rail` / `render_main_stage` / `render_right_rail`，形成三栏外壳。
- **Phase B 已完成**：新增 `panels/right_rail/` 模块，实现 `StatusCard`、`ToolsCard`、`SubagentCard`、`MemoryCard`；`task` / `team` 面板逻辑迁移到右 rail 卡片；`legacy/task.rs` 与 `legacy/team.rs` 已删除；Skill / MCP 仍保留为模态弹窗，ToolsCard 提供 Manage / Configure 入口。
- **Phase C/D 待进行**：左侧边栏双层化/flatten、插件面板、pretext 文本测量接入。

变更涉及 `clarity-core::ui::ViewState` 和 `clarity-egui` 渲染层；跨前端类型变更已同步导出到 `clarity_core::ui`。

### 当前测试基线

| 测试类型 | 通过 | 失败 | 忽略 |
|----------|------|------|------|
| `cargo test --workspace --lib --exclude clarity-slint` | 1093 | 0 | 8 |
| `cargo test --workspace --bins --exclude clarity-slint` | 139 | 0 | 0 |
| `cargo test -p clarity-integration-tests --lib` | 16 | 0 | 0 |
| `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` | 0 warning | 0 | - |

### 环境变量速查

```powershell
$env:KIMI_CODE_API_KEY="sk-kimi-..."
$env:KIMI_API_KEY="sk-..."
$env:ANTHROPIC_AUTH_TOKEN="..."
$env:DEEPSEEK_API_KEY="..."
$env:OPENAI_API_KEY="..."
$env:CLARITY_LOCAL_MODEL_PATH="C:\path\to\model.gguf"
$env:CLARITY_MODELS_CONFIG="C:\path\to\models.toml"
$env:CLARITY_APPROVAL_MODE="yolo"   # interactive | smart | plan | yolo
```

Provider 配置、models.toml、加密 key 详见 [`docs/development/provider-config.md`](docs/development/provider-config.md)。

---

## 4. 代码风格与健康规则

### 工程红线

| 规则 | 来源 | 说明 |
|------|------|------|
| `missing_docs = "deny"` | Workspace lint | 所有 `pub` 项必须有 `///` 文档注释 |
| `unsafe_code = "deny"` | Workspace lint | 禁止新增 `unsafe`；已有 1 处白名单 |
| `unwrap_used = "deny"` | Clippy lint | 新增 `unwrap()` 必须配 `// SAFE: <不变量说明>` |
| `expect_used = "deny"` | Clippy lint | 同上 |
| `panic = "deny"` | Clippy lint | 禁止新增 `panic!` |
| 无 `TODO/FIXME/XXX` | 项目纪律 | 暂存事项转入 GitHub Issue 或 `docs/notes/` |

### 跨层变更检查单

修改 `WireMessage`、`ViewCommand`、`UserAction` 或 `clarity-core` 核心类型时，必须同步检查：

1. `clarity-tui` 中的事件处理与渲染逻辑
2. `clarity-gateway` 中的 HTTP API / WebSocket 序列化
3. `tests/integration` 中的断言匹配
4. egui `protocol_renderer.rs`、TUI `protocol_renderer.rs`、Gateway `ws.rs`

---

## 5. Agent 协作约定

如果你是 AI 子代理，请遵守以下约定：

1. **先读边界**：修改任何 crate 前，确认它依赖谁、被谁依赖，尤其不要破坏 `clarity-core` 零前端/网络依赖。
2. **先跑测试**：修改 `agent/mod.rs`、`llm/mod.rs`、`AgentController`、`Op`、`WireMessage` 后，必须跑完整测试集。
3. **小步提交**：一个 commit 只处理一个关注点；commit message 使用 `<type>(<scope>): <imperative summary>`。
4. **更新文档**：修改 crate 边界、模块结构、关键类型、feature、环境变量后，**必须同步更新 `AGENTS.md`** 和相关 `README.md`/`docs/`。
5. **禁止的行为**：
   - 在 `clarity-core` 引入前端/网络依赖。
   - 新增 `unsafe` 无审批文档。
   - 代码中遗留 `TODO/FIXME/XXX`。
   - 硬编码真实密钥。

---

## 6. 更多参考

| 主题 | 文档 |
|------|------|
| 构建/测试/验证 | [`docs/development/setup.md`](docs/development/setup.md) |
| Provider 配置 | [`docs/development/provider-config.md`](docs/development/provider-config.md) |
| 代码级架构 | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| 技术栈与 Crate 拓扑 | [`docs/architecture/tech-stack.md`](docs/architecture/tech-stack.md) |
| 项目定位与生态关系 | [`docs/architecture/architecture-positioning.md`](docs/architecture/architecture-positioning.md) |
| 当前阶段与已知问题 | [`docs/planning/current-phase.md`](docs/planning/current-phase.md) |
| 项目状态报告 | [`docs/planning/PROJECT_STATUS.md`](docs/planning/PROJECT_STATUS.md) |
| 路线图 | [`docs/planning/ROADMAP.md`](docs/planning/ROADMAP.md) |
| 代码变更原则 | [`docs/development/CODE-CHANGE-PRINCIPLES.md`](docs/development/CODE-CHANGE-PRINCIPLES.md) |
| 安全与运维 | [`docs/security/operations.md`](docs/security/operations.md) |
| 贡献指南 | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| 变更日志 | [`CHANGELOG.md`](CHANGELOG.md) |

---

*最后更新：2026-06-13*
