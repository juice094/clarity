<div align="center">

# Clarity

**Rust 原生个人 AI 运行时**

ReAct/Plan 智能体 · MCP 生态 · BM25+向量记忆 · 多入口（TUI/桌面/网页/托盘/无头）

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-purple.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

[English](README.en.md) | 中文

</div>

---

## 是什么 & 为什么

你有十几个 AI 工具：聊天界面、编程助手、任务运行器、记忆插件。每个只覆盖你工作流的一小块。**没有一个是完整的。**

**Clarity 是一个单一运行时，在你使用的每个入口点（终端、桌面、浏览器、无头脚本、系统托盘）编排 LLM、工具和记忆。** 一个智能体核心，多种界面。你的会话、记忆和任务持久化并随你迁移。

使用 Rust 构建。核心引擎和 CLI 工具以**单二进制文件**分发，**零外部运行时依赖**（无需 Python、Node.js 或 Ollama）。桌面 GUI（eframe/egui）是纯 Rust 实现，零 Web 依赖 —— 没有 Node.js，没有 WebView，没有 Electron。

> **预构建安装包**：Windows `.exe` 可在 [GitHub Releases](https://github.com/juice094/clarity/releases) 下载。无需 Rust 工具链。

---

## 30 秒快速开始

```bash
# 1. 克隆
git clone https://github.com/juice094/clarity.git && cd clarity

# 2. 安装二进制（选一个）
cargo install --path crates/clarity-egui      # 桌面 GUI — 零运行时依赖，纯 Rust
cargo install --path crates/clarity-tui       # 终端 UI — 零运行时依赖
cargo install --path crates/clarity-gateway   # Web IDE — 零运行时依赖
cargo install --path crates/clarity-headless  # 脚本/CI 用 CLI — 零运行时依赖

# 3. 运行
clarity-egui
```

**桌面 GUI**（eframe + egui，纯 Rust —— 没有 Node.js，没有 WebView）：
```bash
cargo run -p clarity-egui
```

**没有 API key？没问题。** 将 `.gguf` 模型放在 `~/models/` 中，在设置中选择 **Local (GGUF)**。离线时 Clarity 自动回退到本地推理。

---

## 核心能力

| 能力 | 含义 |
|-----------|---------------|
| **本地优先 LLM** | 通过 Candle 原生 GGUF 推理。Qwen2、DeepSeek-R1-Distill 等 —— 无需 Ollama，无需 API key，无需网络。 |
| **Plan 模式** | LLM 先写结构化执行计划；批量运行步骤，无需逐工具中断。 |
| **混合记忆** | SQLite + BM25 + 向量搜索。对话跨会话持久化，并自动整合为长期记忆。 |
| **多入口** | 同一智能体核心，五种界面：TUI（`ratatui`）、桌面 GUI（`eframe/egui`）、Web IDE（`Axum`）、无头 CLI、系统托盘（`claw`）。 |
| **审批系统** | Interactive / Yolo / Plan —— 运行时切换。V1 规则引擎自动批准低风险工具。 |
| **离线回退** | 网络监控每 30 秒探测一次。离线时自动切换到本地模型；恢复后切回云端提供商。 |
| **首次使用体验** | 引导流程检测缺失模型，指导下载，或提示云端提供商设置。无需手动配置。 |
| **动态提示** | `SystemPromptBuilder` 组装上下文感知提示（审批通知、离线状态、模板变量）。 |
| **模型热切换** | 在设置中更改提供商/模型无需重启。API key 本地存储，永不离开本机。 |
| **i18n** | 中/英文语言切换，偏好持久化。 |

**支持的提供商**：`openai`、`anthropic`、`kimi`、`kimi-code`、`deepseek`、`ollama`、`local`（Candle GGUF）。

---

## 架构

```
crates/
├── clarity-contract   # 共享契约：LlmProvider/Tool/AgentError trait、
│                      # FederationMessage、CapabilityToken —— 零内部依赖。
├── clarity-wire       # UI ↔ Agent 事件总线（SPMC）+ ViewCommand 协议通道。
├── clarity-memory     # BM25 + 向量混合搜索、分块、四级压缩管线。
├── clarity-mcp        # MCP 客户端 —— stdio / SSE / HTTP / WebSocket 四种 transport。
├── clarity-llm        # LLM provider 抽象 + 6 个内置 provider + Candle GGUF。
├── clarity-tools      # 内置工具库（file/shell/web/devkit 等）。
├── clarity-subagents  # 子代理执行器 + 并行调度器。
├── clarity-core       # 智能体循环（ReAct/Plan）、审批、Skill、MCP 整合。
├── clarity-gateway    # Axum HTTP/WebSocket 服务器、Web UI、会话存储。
├── clarity-egui       # 桌面 GUI（eframe/egui）—— 主 UI 栈。
├── clarity-tui        # ratatui 终端界面。
├── clarity-claw       # 系统托盘后台守护进程。
└── clarity-headless   # 脚本 / CI 用 Headless CLI。
# clarity-tauri        # 已归档 —— 移至外部备份，详见 CHANGELOG。
```

**依赖方向**

```
contract  ←  {wire, memory, mcp, llm, tools}  ←  core  ←  {gateway, egui, tui, claw, headless}
                                                    ↑
                                            subagents（消费 core）
```

**关键不变量**

- `clarity-core` 对任何前端或网络 crate **零依赖**。
- `clarity-contract` 无任何内部依赖，是其他所有 crate 的底座。
- 前端 crate 之间**不直接互相依赖**，跨前端通信通过 `clarity-wire`。

这不是偶然 —— 它是让项目可由单人维护的架构边界。

---

## 开发

```bash
# 运行完整验证套件（CI 执行的）
cargo test --workspace --lib                          # 524 测试，0 失败，4 忽略
cargo clippy --workspace --lib --bins --tests -- -D warnings  # 零警告
cargo fmt --all -- --check
cargo doc --no-deps                                   # 零文档警告
cargo audit --deny unsound --deny yanked

# 运行单个组件
cargo run -p clarity-egui
cargo run -p clarity-gateway
cargo run -p clarity-tui
cargo run -p clarity-claw
```

详见 [CONTRIBUTING.md](CONTRIBUTING.md) 完整开发指南、架构图和贡献工作流。

---

## 文档索引

| 文档 | 受众 | 用途 |
|----------|----------|---------|
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | 贡献者 | 设置、架构、工作流、编码标准 |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | 开发者 | 代码级精确架构参考 |
| [`AGENTS.md`](AGENTS.md) | AI 智能体 / 贡献者 | 环境指南、已知问题、耦合说明 |
| [`CHANGELOG.md`](CHANGELOG.md) | 用户 | 版本历史和迁移说明 |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | 用户 / 贡献者 | 未来方向和风险评估 |
| [`docs/methodology-shape-up.md`](docs/methodology-shape-up.md) | 维护者 | 工程方法论（Cynefin、TOC、Shape Up） |

---

## 许可证

[GNU Affero General Public License v3.0（或更高版本）](LICENSE) — Copyright (c) 2026 juice094 and contributors.

- 可自由用于个人、教育、研究用途。
- **网络 copyleft**：若以网络服务形式（如自托管 `clarity-gateway` 作为 SaaS）提供修改版本，**必须**以 AGPL-3.0 向服务的所有用户公开修改后的源代码。
- 不得在没有显式许可的情况下将本项目或其衍生品重新授权为闭源或更宽松的协议。

完整法律措辞与商业授权细节见英文版 [`README.md`](README.md) §License。

---

<div align="center">

**[⭐ Star](https://github.com/juice094/clarity) · [🐛 Issues](https://github.com/juice094/clarity/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
