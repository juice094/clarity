<div align="center">

# Clarity

**Rust 原生个人 AI 运行时**

ReAct/Plan 智能体 · MCP 生态 · BM25+向量记忆 · 多入口（TUI/桌面/网页/托盘/无头/移动端 FFI）

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-purple.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![GitHub release](https://img.shields.io/github/v/release/juice094/clarity?logo=github)](https://github.com/juice094/clarity/releases)
[![Issues](https://img.shields.io/github/issues/juice094/clarity)](https://github.com/juice094/clarity/issues)

[English](README.en.md) | 中文

</div>

---

## 是什么 & 为什么

你有十几个 AI 工具：聊天界面、编程助手、任务运行器、记忆插件。每个只覆盖你工作流的一小块。**没有一个是完整的。**

**Clarity 是一个单一运行时，在你使用的每个入口点（终端、桌面、浏览器、无头脚本、系统托盘、移动端 FFI）编排 LLM、工具和记忆。** 一个智能体核心，多种界面。你的会话、记忆和任务持久化并随你迁移。

使用 Rust 构建。每个前端均以**单二进制文件**分发，**零外部运行时依赖**（无需 Python、Node.js 或 Ollama）。桌面 GUI（eframe/egui）是纯 Rust 实现，零 Web 依赖。

> **预构建安装包**：Windows `.msi` / `.exe` 与 Linux 二进制可在 [GitHub Releases](https://github.com/juice094/clarity/releases) 下载，无需 Rust 工具链。

---

## 30 秒快速开始

```bash
# 1. 克隆
git clone https://github.com/juice094/clarity.git && cd clarity

# 2. 安装二进制（选一个）
cargo install --path crates/clarity-egui      # 桌面 GUI
cargo install --path crates/clarity-tui       # 终端 UI
cargo install --path crates/clarity-gateway   # Web IDE
cargo install --path crates/clarity-headless  # 脚本/CI 用 CLI

# 3. 运行
clarity-egui
```

**没有 API key？没问题。** 将 `.gguf` 模型放在 `~/models/` 中，在设置中选择 **Local (GGUF)**。离线时 Clarity 自动回退到本地推理。

---

## 核心能力

| 能力 | 含义 |
|-----------|---------------|
| **本地优先 LLM** | 通过 Candle 原生 GGUF 推理。Qwen2、DeepSeek-R1-Distill 等 —— 无需 Ollama，无需 API key，无需网络。 |
| **Plan 模式** | LLM 先写结构化执行计划；批量运行步骤，无需逐工具中断。 |
| **混合记忆** | SQLite + BM25 + 向量搜索。对话跨会话持久化，并自动整合为长期记忆。 |
| **多入口** | 同一智能体核心，六种界面：TUI、桌面 GUI、Web IDE、无头 CLI、系统托盘、移动端 FFI。 |
| **审批系统** | Interactive / Smart / Plan / Yolo —— 运行时切换。 |
| **离线回退** | 网络监控探测。离线时自动切换到本地模型；恢复后切回云端提供商。 |

**支持的提供商**：`openai`、`anthropic`、`kimi`、`kimi-code`、`deepseek`、`ollama`、`local`（Candle GGUF）。

---

## 架构

```
crates/
├── clarity-contract        # 共享契约：LlmProvider/Tool/AgentError trait —— 零内部依赖
├── clarity-wire           # UI ↔ Agent 事件总线（SPMC）+ ViewCommand 协议通道
├── clarity-memory         # BM25 + 向量混合搜索、分块、四级压缩管线
├── clarity-mcp            # MCP 客户端 —— stdio / SSE / HTTP / WebSocket
├── clarity-llm            # LLM provider 抽象 + 内置 provider + Candle GGUF
├── clarity-tools          # 内置工具库（file/shell/web/devkit 等）
├── clarity-channels       # 外部通道抽象；当前实现 WeChat iLink；Webhook 默认可用
├── clarity-subagents      # 子代理执行器 + 并行调度器
├── clarity-thread-store   # Thread 持久化抽象；依赖 clarity-rollout
├── clarity-rollout        # JSONL rollout 持久化（设计受 Codex 启发）
├── clarity-secrets        # 凭证加密与本地安全存储
├── clarity-telemetry      # 统一遥测
├── clarity-core           # 智能体循环（ReAct/Plan）、审批、Skill、MCP 整合
├── clarity-gateway        # Axum HTTP/WebSocket 服务器、Web UI、会话存储
├── clarity-egui           # 桌面 GUI（eframe/egui）—— 主 UI 栈
├── clarity-tui            # ratatui 终端界面
├── clarity-claw           # 统一客户端 Claw 节点：UI 无关库 + 系统托盘常驻二进制（Gateway WebSocket 客户端、OpenClaw/KimiClaw 兼容层、设备发现/身份/配对、角色上下文同步）
├── clarity-headless       # 脚本 / CI 用 Headless CLI
├── clarity-mobile-core    # 移动端 UniFFI FFI 核心（Android/iOS）
├── clarity-slint          # 桌面 GUI 实验栈（Slint），不参与默认 CI
├── clarity-anthropic-proxy # Anthropic Messages API 网关（默认 DeepSeek device，协议转换在 clarity-llm）
└── clarity-tauri          # 已归档 — 不参与默认 workspace 构建
```

**依赖方向**

```
contract ← {wire, memory, mcp, llm, tools, channels, secrets, rollout}
               ↑
          thread-store (→ rollout)
                                  ↓
                               core ← {gateway, egui, tui, claw, headless, mobile-core}
                                  ↑
                    {subagents, telemetry}（消费 core / 横切关注）
```

**关键不变量**

- `clarity-core` 对任何前端或网络 crate **零依赖**。
- `clarity-contract` 无任何内部依赖，是其他所有 crate 的底座。
- 前端 crate 之间**不直接互相依赖**，跨前端通信通过 `clarity-wire`。

---

## 开发

```bash
# 运行完整验证套件（CI 执行的）
cargo test --workspace --lib --exclude clarity-slint      # 1550+ 测试，0 失败
cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings
cargo fmt --all -- --check
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
| [`docs/architecture/architecture-positioning.md`](docs/architecture/architecture-positioning.md) | 开发者 | 项目定位与生态关系 |
| [`AGENTS.md`](AGENTS.md) | AI 智能体 / 贡献者 | 环境指南、已知问题、耦合说明 |
| [`CHANGELOG.md`](CHANGELOG.md) | 用户 | 版本历史和迁移说明 |
| [`docs/planning/ROADMAP.md`](docs/planning/ROADMAP.md) | 用户 / 贡献者 | 未来方向 |
| [`docs/planning/PROJECT_STATUS.md`](docs/planning/PROJECT_STATUS.md) | 全员 | 当前状态、指标、债务 |

---

## 社区与支持

- **Bug 报告**：请使用 [GitHub Issues](https://github.com/juice094/clarity/issues) 并选择 bug 模板。
- **功能讨论**：先在 [GitHub Discussions](https://github.com/juice094/clarity/discussions) 发起。
- **安全漏洞**：请查阅 [SECURITY.md](SECURITY.md)，通过私有渠道报告。
- **参与贡献**：请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 与 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。

---

## 许可证

[GNU Affero General Public License v3.0（或更高版本）](LICENSE) — Copyright (c) 2026 juice094 and contributors.

完整法律措辞与商业授权细节见英文版 [`README.md`](README.md) §License。

---

<div align="center">

**[⭐ Star](https://github.com/juice094/clarity) · [🐛 Issues](https://github.com/juice094/clarity/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
