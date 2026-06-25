<div align="center">

# 🦀 Clarity

> **Rust 原生个人 AI 运行时 — 一个核心，所有入口。**

ReAct 智能体 · MCP 工具生态 · BM25+向量记忆 · 多入口（TUI/桌面/Web/托盘/移动端 FFI）

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-purple.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![GitHub release](https://img.shields.io/github/v/release/juice094/clarity?logo=github)](https://github.com/juice094/clarity/releases)
[![Issues](https://img.shields.io/github/issues/juice094/clarity)](https://github.com/juice094/clarity/issues)

[English](README.en.md) · [中文](README.zh.md)

</div>

---

## 📋 简介

一个统一的运行时，在**所有入口**编排 LLM、工具和记忆 — 终端、桌面、浏览器、无头脚本、系统托盘、移动端 FFI。每个前端均以**单二进制**分发，零外部运行时依赖（无需 Python、Node.js 或 Ollama）。桌面 GUI 采用纯 Rust 实现（eframe/egui），零 Web 依赖。

> **预构建安装包**：Windows `.msi` / `.exe` 与 Linux 二进制可在 [GitHub Releases](https://github.com/juice094/clarity/releases) 下载，无需 Rust 工具链。

**定位边界**：Clarity 是「本地优先的 AI 开发运行时」，聚焦编码/工程工作流。无原生消息通道客户端（`clarity-channels` 当前仅实现 WeChat iLink；Discord/Slack/Telegram 未启用）、无 Voice/Canvas、无完整移动端 UI（仅 Rust FFI 核心）。需要多通道 inbox 或语音交互 → [OpenClaw](https://github.com/openclaw/openclaw) 更合适。

---

## 🌟 核心亮点

| 亮点 | 说明 |
|:---|:---|
| 🧠 **Agent 运行时** | ReAct/Plan 循环 + MCP 工具生态，Approval 四层（Interactive/Smart/Plan/Yolo） |
| 🖥️ **纯 Rust 多前端** | TUI (ratatui) · 桌面 GUI (eframe/egui) · Web IDE (Axum) · 无头 CLI · 系统托盘 · 移动端 FFI |
| 🤖 **本地 LLM** | Candle 原生 GGUF 推理（Qwen2 / DeepSeek-R1-Distill），离线自动回退，零外部依赖 |
| 🧩 **混合记忆** | SQLite + BM25 + 向量搜索，跨会话持久化 |
| 💰 **预算保护** | 每轮/每日 USD 成本上限，超预算前自动拦截 |

> [完整路线图 → docs/planning/ROADMAP.md](docs/planning/ROADMAP.md)

---

## 🔧 技术栈

| 层级 | 技术 |
|:---|:---|
| Agent 核心 | ReAct/Plan 循环, MCP stdio/SSE/WebSocket |
| 本地推理 | Candle (GGUF: Qwen2, DeepSeek-R1-Distill) |
| 记忆存储 | SQLite (WAL) + BM25 + 向量搜索 |
| 桌面 GUI | eframe/egui（纯 Rust，零 Web 依赖） |
| 事件总线 | clarity-wire SPMC 通道 |

---

## 📁 项目结构

```
crates/
├── clarity-contract        # 共享契约层：LlmProvider/Tool/AgentError trait、FederationMessage
├── clarity-wire           # UI ↔ Agent 事件总线（SPMC）+ ViewCommand 协议通道
├── clarity-memory         # BM25 + 向量混合搜索，chunking，四级压缩归档
├── clarity-mcp            # MCP 客户端 — stdio / SSE / HTTP / WebSocket 四传输
├── clarity-llm            # LLM provider 抽象 + 内置 provider + Candle GGUF 本地推理
├── clarity-tools          # 内置工具库：file / shell / web / devkit / …
├── clarity-channels       # 外部通道抽象；当前实现 WeChat iLink；Webhook 默认可用
├── clarity-subagents      # 子代理执行器 + 并行调度器，消费 clarity-core
├── clarity-thread-store   # Thread 持久化抽象；依赖 clarity-rollout
├── clarity-rollout        # JSONL rollout 持久化（设计受 Codex 启发）
├── clarity-openclaw       # OpenClaw/KimiClaw Gateway WebSocket 客户端、设备身份
├── clarity-secrets        # 凭证加密与本地安全存储
├── clarity-telemetry      # 统一遥测：WideEvent、metrics、traces、config audit
├── clarity-core           # Agent 循环（ReAct/Plan）、Approval、Skill、MCP 集成
├── clarity-gateway        # Axum HTTP/WebSocket 服务端，Web IDE，session store
├── clarity-egui           # 桌面 GUI（主前端栈），eframe + egui 纯 Rust
├── clarity-tui            # ratatui 终端界面
├── clarity-claw           # 系统托盘后台监控（Gateway WebSocket 客户端）
├── clarity-headless       # 无头 CLI（脚本 / CI 场景）
├── clarity-mobile-core    # 移动端 UniFFI FFI 核心（Android/iOS）
├── clarity-slint          # 桌面 GUI 实验栈（Slint），不参与默认 CI
├── clarity-anthropic-proxy # Anthropic Messages API → DeepSeek 代理（工具二进制）
└── clarity-tauri          # 已归档 — 不参与默认 workspace 构建
```

### 架构依赖方向

```
contract ← {wire, memory, mcp, llm, tools, channels, secrets, openclaw, rollout}
               ↑
          thread-store (→ rollout)
                                  ↓
                               core ← {gateway, egui, tui, claw, headless, mobile-core}
                                  ↑
                    {subagents, telemetry}（消费 core / 横切关注）
```

**关键不变量**：
- `clarity-core`**零依赖**于任何前端或网络 crate
- `clarity-contract`**零内部依赖**，所有人基于它构建
- 前端 crate **永不互相 import** — 跨前端通信走 `clarity-wire`

> 详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) 与 [docs/architecture/architecture-positioning.md](docs/architecture/architecture-positioning.md)。

---

## 🚀 快速开始

```bash
# 1. 克隆仓库
git clone https://github.com/juice094/clarity.git && cd clarity

# 2. 编译并运行桌面 GUI
cargo run -p clarity-egui

# 3. 或安装指定前端
cargo install --path crates/clarity-egui   # 桌面 GUI
cargo install --path crates/clarity-tui    # 终端界面
cargo install --path crates/clarity-gateway # Web IDE
cargo install --path crates/clarity-headless # 无头 CLI
```

**无 API Key？** 在 `~/models/` 放入 `.gguf` 模型 → 设置中选择 **本地 (GGUF)** → 自动离线推理。

---

## 🤝 参与贡献

详见 [CONTRIBUTING.md](CONTRIBUTING.md) — 环境搭建、架构总览、编码规范。快速验证：

```bash
cargo test --workspace --lib --exclude clarity-slint              # 1550+ 个测试
cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings
cargo audit --deny unsound --deny yanked
```

---

## 💬 社区与支持

- **Bug 报告**：请使用 [GitHub Issues](https://github.com/juice094/clarity/issues) 并选择 bug 模板。
- **功能讨论**：先在 [GitHub Discussions](https://github.com/juice094/clarity/discussions) 发起。
- **安全漏洞**：请查阅 [SECURITY.md](SECURITY.md)，通过私有渠道报告。
- **参与贡献**：请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 与 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。

---

## 📄 许可证

[AGPL-3.0](LICENSE) — Copyright (c) 2026 juice094。若修改 Clarity 并通过网络提供服务，须向所有用户公开修改后的源代码。如需商业授权，请开 issue 讨论。

---

<div align="center">

**[⭐ Star](https://github.com/juice094/clarity) · [🐛 Issues](https://github.com/juice094/clarity/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
