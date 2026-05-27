<div align="center">

# 🦀 Clarity

> **Rust 原生个人 AI 运行时 — 一个核心，所有入口。**

ReAct 智能体 · MCP 工具生态 · BM25+向量记忆 · 多入口（TUI/桌面/Web/托盘）

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-purple.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

[English](README.en.md) · [中文](README.zh.md)

</div>

---

## 📋 简介

一个统一的运行时，在**所有入口**编排 LLM、工具和记忆 — 终端、桌面、浏览器、无头脚本、系统托盘。核心引擎和 CLI 工具以**单二进制**分发，零外部运行时依赖（无需 Python、Node.js 或 Ollama）。桌面 GUI 采用纯 Rust 实现（eframe/egui），零 Web 依赖。

> **预构建安装包**：Windows `.msi` / `.exe` 可在 [GitHub Releases](https://github.com/juice094/clarity/releases) 下载，无需 Rust 工具链。

**定位边界**：Clarity 是「本地优先的 AI 开发运行时」，聚焦编码/工程工作流。无原生消息通道（WhatsApp/Telegram/Discord Bot）、无 Voice/Canvas、无移动端。需要多通道 inbox 或语音交互 → [OpenClaw](https://github.com/openclaw/openclaw) 更合适。

---

## 🌟 核心亮点

| 亮点 | 说明 |
|:---|:---|
| 🧠 **Agent 运行时** | ReAct/Plan 循环 + MCP 工具生态，Approval 三层（Interactive/Yolo/Plan） |
| 🖥️ **纯 Rust 多前端** | TUI (ratatui) · 桌面 GUI (eframe/egui) · Web IDE (Axum) · 无头 CLI · 系统托盘 |
| 🤖 **本地 LLM** | Candle 原生 GGUF 推理（Qwen2 / DeepSeek-R1-Distill），离线自动回退，零外部依赖 |
| 🧩 **混合记忆** | SQLite + BM25 + 向量搜索，6 个月时间衰减，跨会话持久化 |
| 💰 **预算保护** | 每轮/每日 USD 成本上限，超预算前自动拦截 |

> [完整技术特性与路线图 → docs/ROADMAP.md](docs/ROADMAP.md)

---

## 🔧 技术栈

| 层级 | 技术 |
|:---|:---|
| Agent 核心 | ReAct/Plan 循环, MCP stdio/SSE/WebSocket |
| 本地推理 | Candle (GGUF: Qwen2, DeepSeek-R1-Distill) |
| 记忆存储 | SQLite (WAL) + Tantivy BM25 + 向量搜索 |
| 桌面 GUI | eframe/egui（纯 Rust，零 Web 依赖） |
| 事件总线 | clarity-wire SPMC 通道 |

---

## 📁 项目结构

```
crates/
├── clarity-contract    # 共享契约层：LlmProvider/Tool/AgentError trait、FederationMessage
│                     # 零内部依赖，所有 crate 的建筑地基
├── clarity-wire       # UI ↔ Agent 事件总线（SPMC）+ ViewCommand 协议通道
│                     # 跨前端通信的唯一通道，前端 crate 禁止互相 import
├── clarity-memory     # BM25 + 向量混合搜索，chunking，四级压缩归档
├── clarity-mcp        # MCP 客户端 — stdio / SSE / HTTP / WebSocket 四传输
├── clarity-llm        # LLM provider 抽象 + 6 个内置 provider + Candle GGUF 本地推理
├── clarity-tools      # 内置工具库：file / shell / web / devkit / …
├── clarity-subagents  # 子代理执行器 + 并行调度器，消费 clarity-core
├── clarity-core       # Agent 循环（ReAct/Plan）、Approval、Skill、MCP 集成
│                     # 零前端/网络依赖，架构不变量
├── clarity-gateway    # Axum HTTP/WebSocket 服务端，Web IDE，session store
├── clarity-egui       # 桌面 GUI（主前端栈），eframe + egui 纯 Rust
├── clarity-tui        # ratatui 终端界面
├── clarity-claw       # 系统托盘后台监控
└── clarity-headless   # 无头 CLI（脚本 / CI 场景）
# clarity-tauri       # 已归档 — 迁移至外部备份
```

### 架构依赖方向

```
contract ← {wire, memory, mcp, llm, tools} ← core ← {gateway, egui, tui, claw, headless}
                                                    ↑
                                             subagents（消费 core）
```

**关键不变量**：
- `clarity-core`**零依赖**于任何前端或网络 crate
- `clarity-contract`**零内部依赖**，所有人基于它构建
- 前端 crate **永不互相 import** — 跨前端通信走 `clarity-wire`

> 这不是偶然 — 这是让单人开发者也能维护的架构边界。详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

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
cargo test --workspace --lib              # 927 个测试
cargo clippy --workspace --lib --bins --tests -- -D warnings
cargo audit --deny unsound --deny yanked
```

---

## 📄 许可证

[AGPL-3.0](LICENSE) — Copyright (c) 2026 juice094。若修改 Clarity 并通过网络提供服务，须向所有用户公开修改后的源代码。如需商业授权，请开 issue 讨论。

---

<div align="center">

**[⭐ Star](https://github.com/juice094/clarity) · [🐛 Issues](https://github.com/juice094/clarity/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
