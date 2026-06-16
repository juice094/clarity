<!-- DOC-CONTRACT: 本文件维护 Agent 开发所需的运行上下文、架构耦合警告和代码风格。不维护功能清单、竞品对比或历史变更——这些参见 README.md / docs/ARCHITECTURE.md / docs/architecture/architecture-positioning.md / CHANGELOG.md / docs/planning/sprint-archive.md。 -->

# Agent Guidance for Project Clarity

> **Scope:** 本文件治理 `C:/Users/22414/dev/clarity` 及其所有子目录。  
> **Default branch:** `main`  
> **Version:** `0.3.0`（`Cargo.toml`）/ `v0.3.4-rc`（开发中）  
> **Rust edition:** 2024 · **MSRV:** `1.85`（CI 使用 stable，推荐 `1.94+`）  
> **License:** AGPL-3.0-or-later  
> **Repository:** https://github.com/juice094/clarity

本文件使用中文撰写，因为项目源码注释、文档与提交信息以中文为主。

---

## 1. 项目概览

Clarity 是一个 **Rust 原生、本地优先的个人 AI 运行时**。用同一套 Agent 引擎支撑多种入口（桌面 GUI、终端 TUI、Web IDE、无头 CLI、系统托盘），在本地完成 LLM 编排、工具调用、记忆持久化与审批流程。

关键事实：

- **单仓库 Workspace**，Rust 2024 edition，MSRV 1.85，许可证 AGPL-3.0-or-later。
- **20 个 crate 目录 = 19 个活跃 crate + 1 个归档 crate**（`clarity-tauri` 被 workspace 排除）。新增 `clarity-thread-store`、`clarity-rollout`：Thread/Session 生命周期管理（API 设计受 Codex 启发，实现为原创代码）。
- **前端 crate 之间禁止互相 import**，跨前端通信统一走 `clarity-wire`。
- **`clarity-core` 零依赖**于任何前端或网络 crate；`clarity-contract` 零内部依赖。
- **默认构建已包含本地 GGUF 推理**（`local-llm` feature），可选 CUDA 加速（`local-llm-cuda`）。
- **零外部运行时依赖**：`cargo install` 生成的二进制即可运行，无需 Python、Node.js 或 Ollama。
- **定位边界**：聚焦编码/工程工作流的本地 AI 协作者。无原生消息通道客户端、无 Voice/Canvas、无移动端。

---

## 2. Crate 拓扑与关键不变量

```text
contract
    ▲
    ├── {wire, memory, mcp, llm, tools, channels, secrets}
    ├── rollout
    └── thread-store (→ rollout)
            │
            ▼
          core ← {gateway, egui, tui, claw, headless}
            ▲
    subagents（消费 core）
    telemetry（当前由 gateway 使用）
```

| Crate | 类型 | 职责 |
|-------|------|------|
| `clarity-contract` | lib | 共享契约层：`LlmProvider` / `Tool` / `AgentError` / `FederationMessage` |
| `clarity-wire` | lib | UI ↔ Agent 事件总线（SPMC）、`ViewCommand` / `WireMessage` |
| `clarity-memory` | lib | SQLite/文件/混合记忆、BM25+向量、chunking |
| `clarity-mcp` | lib | MCP 客户端：stdio / SSE / HTTP / WebSocket |
| `clarity-llm` | lib | LLM provider 抽象 + 内置 provider + Candle GGUF |
| `clarity-tools` | lib | 内置工具库：file / shell / web / devkit / … |
| `clarity-secrets` | lib | ChaCha20-Poly1305 加密 Secret 存储（`enc2:`） |
| `clarity-channels` | lib | 外部消息通道：Discord / Slack / Telegram / Webhook / 微信 iLink |
| `clarity-subagents` | lib | 子代理执行器、并行调度、团队协调 |
| `clarity-thread-store` | lib | Thread 持久化抽象：`ThreadStore` trait（API 设计受 Codex 启发） |
| `clarity-rollout` | lib | JSONL rollout 持久化：事件日志、压缩、回放（设计受 Codex 启发） |
| `clarity-core` | lib | Agent 循环（ReAct/Plan）、Approval、Skill、MCP 集成 |
| `clarity-telemetry` | lib | 统一遥测：WideEvent、metrics、traces、config audit |
| `clarity-gateway` | bin/lib | Axum HTTP/WebSocket 服务端、Web IDE、session store |
| `clarity-egui` | bin | 桌面 GUI（主前端栈），eframe + egui 纯 Rust |
| `clarity-tui` | bin | ratatui 终端界面 |
| `clarity-claw` | bin | 系统托盘后台监控 |
| `clarity-headless` | bin | 无头 CLI（脚本 / CI 场景） |
| `clarity-slint` | bin | 桌面 GUI 实验栈，Slint（不参与默认 CI） |
| `clarity-tauri` | bin | Tauri 前端（**已归档**，被 workspace 排除） |

> **新增说明**：`clarity-thread-store` 与 `clarity-rollout` 的 API 设计受到 OpenAI Codex（Apache-2.0）的架构启发；实现为 Clarity 原创代码，按 AGPL-3.0-or-later 发布。相关 crate 的 `NOTICES.md` 保留了灵感来源致谢。

**不可违反的不变量**：

1. `clarity-core` 不依赖任何前端 crate 或网络 crate。
2. `clarity-contract` 不依赖任何内部 crate。
3. 前端 crate 之间不互相 import；跨前端状态/事件走 `clarity-wire`。
4. 禁止在异步上下文中执行阻塞 I/O；使用 `tokio::task::spawn_blocking`。

---

## 3. 关键配置文件

| 文件 | 作用 |
|------|------|
| `Cargo.toml` | Workspace 配置、共享依赖、lint、profile |
| `.cargo/config.toml` | 增量编译、Slint 快捷命令、dev profile 调优 |
| `crates/*/Cargo.toml` | 各 crate 依赖、features、bin/lib 声明 |
| `.github/workflows/ci.yml` | 11-job CI：check / hermes-feature-check / test / integration / binary / doc-test / session-migration / clippy / fmt / audit / doc-guard / coverage |
| `.github/workflows/release.yml` | Tag 触发 release，产出 Windows `.msi`/`.exe`、Linux binary、SHA256 校验 |
| `scripts/verify.ps1` | PowerShell 一键验收：README+AGENTS 存在性、编译、测试、Clippy、格式化，并可生成 JSON 报告（`-Report`） |
| `docs/development/setup.md` | 完整构建/测试/feature/CUDA 说明 |
| `docs/development/provider-config.md` | Provider、models.toml、环境变量配置指南 |
| `docs/development/CODE-CHANGE-PRINCIPLES.md` | 跨 crate 代码改动七大原则（P1–P7） |
| `SECURITY.md` | 安全策略、漏洞报告、已知边界 |

---

## 4. 技术栈与运行架构

| 层级 | 技术 |
|------|------|
| Agent 核心 | ReAct/Plan 循环、MCP stdio/SSE/HTTP/WebSocket、Approval 四层模式 |
| 本地推理 | Candle 原生 GGUF（Qwen2 / Qwen2.5 / DeepSeek-R1-Distill） |
| 记忆存储 | SQLite（WAL）+ BM25 + 向量搜索 + 四级压缩归档 |
| 桌面 GUI | eframe 0.31 / egui 0.31 / lucide-icons（纯 Rust，零 Web 依赖） |
| 终端 TUI | ratatui 0.30 / crossterm 0.29 |
| Web IDE | Axum 0.7 / tower-http / SSE / WebSocket |
| 事件总线 | `clarity-wire` SPMC 通道 |
| 加密 | ChaCha20-Poly1305（`clarity-secrets`） |
| TLS | `rustls-tls`（纯 Rust），`openssl` 已从依赖树移除 |

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
├── .cargo/                 # cargo 配置、增量编译
├── .clarity/               # 本地运行时数据（sessions、tasks、编译产物等）
├── .github/workflows/      # CI / Release
├── crates/                 # 20 个 crate 目录（19 活跃 + 1 归档）
├── docs/                   # 架构、开发、安全、规划文档
├── examples/               # 独立示例脚本
├── scripts/                # verify.ps1 等
├── skills/                 # Agent 技能模板
├── tests/integration/      # 集成测试 crate
├── Cargo.toml              # workspace 根
└── AGENTS.md               # 本文件
```

### 5.2 `clarity-core` 核心模块（按源码目录）

| 模块 | 路径 | 职责 |
|------|------|------|
| Agent 循环 | `src/agent/` | `Agent`、ReAct/Plan、controller、streaming、execution、compaction |
| 工具 | `src/tools/`（由 `clarity-tools` 提供） | 文件、Shell、Web、任务、团队、MCP 等 |
| LLM | `src/llm/`（由 `clarity-llm` 提供） | provider trait、factory、registry、本地 GGUF |
| MCP | `src/mcp/` | 客户端、transport、config、devkit、enhanced |
| 后台任务 | `src/background/` | `BackgroundTaskManager`、executor、scheduler、store |
| 记忆 | `src/memory/` | `PersistentMemoryStore`、`MemoryCompiler`、`SharedMemoryTicker` |
| 审批 | `src/approval/` | Approval 模式、规则引擎 |
| Skill | `src/skills/` | Markdown+YAML 技能加载、注册、发现 |
| 压缩 | `src/compaction.rs` | 上下文压缩、Token 爆炸防护 |
| 自适应 | `src/adaptive/` | `AdaptiveModelRouter`、profile、predictor、compression |
| 快照 | `src/agent/snapshot/` | Side-Git 快照隔离 |
| LSP | `src/agent/lsp/` | 语言服务器代理 |
| Server | `src/server/` | JSON-RPC over stdio |
| UI 状态 | `src/ui/` | `ViewState` 状态机（跨前端共享） |

### 5.3 `clarity-egui` 结构要点

- `main.rs::update()` 每帧调用 `design_system::install_theme()`。
- `App::render_layout_shell()` 是 chrome / 主视图 / 浮层 / 模态框唯一编排入口。
- `panels/` 按职责分组：`chat/`、`work/`、`settings/`、`modals/`、`sidebar/`、`workspace/`、`system/`、`legacy/`。
- `widgets/` 存放可复用组件；`theme.rs` 是 design token 单源。
- `stores/` 已拆分为按域子模块，保持原导入路径。
- 已接入 Pretext 文字测量后端（`pretext-core` / `pretext-fontdb`），`MessageBubble` 与 `widgets/rich_paragraph.rs` 已转为 pretext-aware。

---

## 6. 构建与测试命令

### 6.1 常用命令

```bash
# 格式检查
cargo fmt --all -- --check

# 编译（不含实验性 clarity-slint）
cargo check --workspace --lib --bins --exclude clarity-slint

# Clippy 零警告
cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings

# 单元测试
cargo test --workspace --lib --exclude clarity-slint

# 二进制测试
cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2

# 文档测试
cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2

# 集成测试
cargo test -p clarity-integration-tests --lib

# 安全审计
cargo audit --deny unsound --deny yanked

# 文档构建
cargo doc --workspace --no-deps --exclude clarity-slint
```

### 6.2 当前测试基线（2026-06-16）

| 测试类型 | 通过 | 失败 | 忽略 |
|----------|------|------|------|
| `cargo test --workspace --lib --exclude clarity-slint` | 1190 | 0 | 8 |
| `cargo test --workspace --bins --exclude clarity-slint` | 171 | 0 | 2 |
| `cargo test --workspace --doc --exclude clarity-slint` | 34 | 0 | 3 |
| `cargo test -p clarity-integration-tests --lib` | 26 | 0 | 0 |
| `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` | 0 warning | 0 | - |
| `cargo fmt --all -- --check` | pass | 0 | - |

> 提交前必须保证上述命令全部通过。`clarity-slint` 为实验栈，不参与默认 CI。

### 6.3 一键验收

```powershell
.\scripts\verify.ps1 --all -Strict
```

该脚本逐 crate 检查 README、AGENTS、编译、测试、Clippy、格式化，并可生成 JSON 报告（`-Report`）。

### 6.4 Feature 与构建变体

| Feature | 作用 | 使用场景 |
|---------|------|----------|
| `local-llm` | 启用 Candle GGUF 本地推理 | 默认开启 |
| `local-llm-cuda` | 本地推理 CUDA 加速 | Windows + NVIDIA CUDA |
| `mcp` | 启用 MCP 集成 | `clarity-core` 默认 |
| `session-migration` | Session V1→V2 迁移工具 | `clarity-core` 可选 |
| `line-mode` | egui 行级渲染管线 | `clarity-egui` 可选 |
| `slack` / `discord` / `telegram` / `webhook` | Gateway 通道 feature | 默认仅 `webhook` |
| `telemetry-api` | Gateway 遥测 REST API | `clarity-gateway` 可选 |
| `hermes` | 各前端 / `clarity-core` / `clarity-memory` 可选的 hermes-memory SQLite 后端 | 实验性，默认关闭；通过 `CLARITY_MEMORY_BACKEND=hermes` 启用 |

CUDA 构建示例（Windows）：

```powershell
$env:NVCC_CCBIN="C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.40.33807\bin\Hostx64\x64"
cargo check -p clarity-core --features local-llm-cuda
cargo run -p clarity-egui --features cuda
```

---

## 7. 代码风格与健康规则

### 7.1 工程红线（Workspace lint）

| 规则 | 来源 | 说明 |
|------|------|------|
| `missing_docs = "deny"` | `Cargo.toml` | 所有 `pub` 项必须有 `///` 文档注释 |
| `unsafe_code = "deny"` | `Cargo.toml` | 禁止新增 `unsafe`；已有 1 处白名单（`clarity-memory`） |
| `unwrap_used = "deny"` | Clippy | 新增 `unwrap()` 必须配 `// SAFE: <不变量说明>` |
| `expect_used = "deny"` | Clippy | 同上 |
| `panic = "deny"` | Clippy | 禁止新增 `panic!` |
| 无 `TODO/FIXME/XXX` | 项目纪律 | 暂存事项转入 GitHub Issue 或 `docs/notes/` |

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

Scope：`core`、`memory`、`gateway`、`egui`、`tui`、`claw`、`wire`、`headless`、`ci`、`docs`。

- 一个 commit 只处理一个关注点。
- 每个 commit 必须独立可编译（`P5`）。
- 修改 `clarity-core`、`llm`、`AgentController`、`Op`、`WireMessage` 后必须跑完整测试集。

### 7.3 跨层变更检查单

修改 `WireMessage`、`ViewCommand`、`UserAction` 或 `clarity-core` 核心类型时，必须同步检查：

1. `clarity-tui` 中的事件处理与渲染逻辑
2. `clarity-gateway` 中的 HTTP API / WebSocket 序列化
3. `tests/integration` 中的断言匹配
4. egui `protocol_renderer.rs`、TUI `protocol_renderer.rs`、Gateway `ws.rs`

### 7.4 egui 前端规范

- 所有用户可见字符串通过 `i18n`（`t!("key")`），禁止硬编码中英文。
- 使用 `Frame::new()` 保持主题一致。
- 面板渲染函数控制在 300 行以内；超出则拆分子组件。
- 优先使用 `ScrollArea` + `AlwaysHidden` 滚动条以保持玻璃拟态风格。
- 模态框使用 `Frame::window` + `radius_lg` + 遮罩层 + Escape/点击外部关闭。
- Design token 强制：在 `crates/clarity-egui/src/{panels,components,widgets}/**` 下，任何 `> 8.0` 的浮点字面量必须路由到 `theme.space_* / text_* / radius_*` 或加 `// LAYOUT-EXEMPT: <理由>`。

### 7.5 代码改动七大原则

详见 `docs/development/CODE-CHANGE-PRINCIPLES.md`：

- **P1** — 单向迁移：禁止双向桥接。
- **P2** — 删除优先：每个 PR 净删除代码或不增加 dead code。
- **P3** — 单源真相：每个概念只有一个写入点。
- **P4** — 测试先行：重构前先有测试 baseline。
- **P5** — 编译可分：每个 commit 独立可编译。
- **P6** — Theme Token 强制：egui 布局字面量必须 token 化。
- **P7** — 协议层不前瞻：新增协议类型必须同时有 producer、consumer 和端到端测试。

---

## 8. 测试策略

### 8.1 测试分层

| 类型 | 命令 | 说明 |
|------|------|------|
| 单元测试 | `cargo test --workspace --lib --exclude clarity-slint` | 各 crate 内 `#[cfg(test)]` |
| 二进制测试 | `cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2` | bin target 逻辑测试 |
| 文档测试 | `cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2` | `rustdoc` 示例 |
| 集成测试 | `cargo test -p clarity-integration-tests --lib` | adaptive_loop / session_v2_migration / telemetry_end_to_end / thread_api |
| 覆盖率 | `cargo llvm-cov --workspace --lib --exclude clarity-slint` | CI 产出 LCOV/HTML |

### 8.2 测试纪律

- 新增 Rust 模块必须含 `#[cfg(test)]` 单元测试。
- Bug fix 必须配回归测试（先红后绿）。
- egui 面板/组件变更需补充手动 QA 或视觉回归检查。
- 性能改动需补充 benchmark 或延迟测量。
- `clarity-egui` 当前以纯逻辑/小部件单元测试为主；面板级 UI 集成测试待 Pretext 三栏布局稳定后引入 `egui_kittest` snapshot。

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
| TLS | `rustls-tls`（纯 Rust），已移除 `openssl` |
| 快照隔离 | Side-Git 快照使用独立 bare 仓库 `~/.clarity/snapshots/` |

### 9.2 漏洞报告

- 首选：[GitHub Security Advisory](https://github.com/juice094/clarity/security/advisories/new)（私密）。
- 备用：邮件 `juice094@users.noreply.github.com`，主题 `[Clarity Security] <简述>`。
- 响应时间：Critical 补丁 14 天内，High 补丁 30 天内。

### 9.3 部署流程

- **CI**：`.github/workflows/ci.yml` 在 `push`/`pull_request` 到 `main` 时触发，覆盖 ubuntu-latest / windows-latest / macos-latest。
- **Release**：`.github/workflows/release.yml` 在 `v*` tag 推送时触发：
  - Windows：`cargo build --release -p clarity-egui` → 代码签名 → `cargo-wix` 生成 `.msi`。
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

- **S6 Pretext 三栏布局迁移**：左 icon rail + 中主舞台 + 右工具 rail 的单页面外壳已落地；右 rail 卡片（Status / Tools / Subagents / Memory）已接入真实内容；`legacy/task.rs` 与 `legacy/team.rs` 已删除；布局几何精化（默认窗口 1280×800、sidebar 200px、header 全宽、内容居中）已完成。
- **Pretext 文字测量接入**：`clarity-egui` 已接入 `pretext-core` / `pretext-fontdb`；`MessageBubble` 已迁移为 pretext-aware；默认启用 pretext 高度估算；回归测试与 release 性能基准通过。
- **Phase 1.5 状态机迁移已完成**：所有遗留 boolean modal / turn / expansion 标志已迁移到 `view_state.modal` / `view_state.turn` / `view_state.expansions`；`clarity-egui` 全局 `#![allow(dead_code)]` 已移除。
- **Phase E 设计系统替换已完成**：`design_system` 语义原语已落地到右 rail 全部卡片（status / tools / subagent / memory / context / progress）及关键 widgets（provider_row / sidebar_card / user_avatar）；未使用原语已清理，`design_system.rs` 无模块级 `#[allow(dead_code)]`。
- **布局诊断覆盖层**：`clarity-egui/src/ui/debug_overlay.rs` 提供红/绿/蓝/黄布局诊断，快捷键 `Ctrl+Shift+L`。详见 `crates/clarity-egui/EGUI_LAYOUT_DEBUG.md`。
- **人机协作图片标注器**：新增 `assets/ui_annotator.html` + schema + `render_annotations.py`，建立“用户框选 → JSON → AI 生成/修正 egui 代码”的协作闭环。
- **已知限制**：
  - Discord/Telegram 默认禁用，等待上游 `rustls-webpki` 修复。
  - Gateway HTTP Chat Completions 默认无状态；完整 session 请用 WebSocket 或传 `session_id`。

---

## 12. 更多参考

| 主题 | 文档 |
|------|------|
| 构建/测试/验证 | [`docs/development/setup.md`](docs/development/setup.md) |
| Provider 配置 | [`docs/development/provider-config.md`](docs/development/provider-config.md) |
| 代码改动原则 | [`docs/development/CODE-CHANGE-PRINCIPLES.md`](docs/development/CODE-CHANGE-PRINCIPLES.md) |
| 代码级架构 | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| 技术栈与 Crate 拓扑 | [`docs/architecture/tech-stack.md`](docs/architecture/tech-stack.md) |
| 项目定位与生态关系 | [`docs/architecture/architecture-positioning.md`](docs/architecture/architecture-positioning.md) |
| 当前阶段与已知问题 | [`docs/planning/current-phase.md`](docs/planning/current-phase.md) |
| 项目状态报告 | [`docs/planning/PROJECT_STATUS.md`](docs/planning/PROJECT_STATUS.md) |
| 路线图 | [`docs/planning/ROADMAP.md`](docs/planning/ROADMAP.md) |
| 安全与运维 | [`docs/security/operations.md`](docs/security/operations.md) |
| 贡献指南 | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| 变更日志 | [`CHANGELOG.md`](CHANGELOG.md) |

---

## 13. 架构文档维护纪律

1. **新增/删除 crate 后**，必须同步更新以下文件中的 crate 拓扑：
   - `Cargo.toml` workspace members
   - `docs/ARCHITECTURE.md` §Crate Topology
   - `docs/architecture/tech-stack.md` §Crate 拓扑 / §架构依赖方向
   - `docs/architecture/map-topology.md` §Crate 依赖图
   - `AGENTS.md` §Crate 拓扑
2. **引入外部项目思想/设计参考时**，禁止使用 "derived from"、"ported from"、"original source files"、"derivative work" 等源码归属措辞；统一使用 "架构启发"、"设计参考"。
3. **实验性模块必须标注 `EXPERIMENTAL`**，不得与稳定接口混为一谈；未接入主循环的愿景功能必须标注 "愿景/未实现"。
4. **禁止把个人开发环境的本地路径**（如 `dev/third_party/xxx`、`Desktop/xxx`、`AppData/...`）写入项目架构文档；外部项目仅说明名称和关系类别即可。
5. **NOTICES.md 仅用于致谢思想/设计来源**，不用于声明代码派生关系；若不存在实际代码引用，不得保留源码归属性语言。

---

*最后更新：2026-06-16*
