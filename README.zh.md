<div align="center">

# Clarity

**Rust 原生个人 AI 运行时**

ReAct/Plan 智能体 · MCP 生态 · BM25+向量记忆 · 多入口（TUI/桌面/网页/托盘/无头）

[![CI](https://github.com/juice094/clarity/actions/workflows/ci.yml/badge.svg)](https://github.com/juice094/clarity/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
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
├── clarity-core      # 智能体循环、工具、记忆、MCP、子代理
├── clarity-memory    # BM25 + 向量混合搜索、分块、编译
├── clarity-gateway   # Axum HTTP 服务器、Web UI、会话存储
├── clarity-egui      # 桌面 GUI（eframe/egui）—— 主 UI 栈
├── clarity-tui       # ratatui 终端界面
├── clarity-claw      # 系统托盘后台监控
├── clarity-wire      # UI↔Agent 事件总线（SPMC）
└── clarity-headless  # 脚本/CI 用无头 CLI
```

> `clarity-tauri`（Tauri 2 + React 前端）已归档并移出仓库。详见 [CHANGELOG](CHANGELOG.md)。

**关键不变量**：`clarity-core` 对任何前端或网络 crate 零依赖。所有前端通过统一 API 消费核心。这不是偶然 —— 它是让项目可由单人维护的架构边界。

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

[MIT](LICENSE) — Copyright (c) 2026 juice094 and contributors.

---

<div align="center">

**[⭐ Star](https://github.com/juice094/clarity) · [🐛 Issues](https://github.com/juice094/clarity/issues) · [🤝 Contribute](CONTRIBUTING.md)**

</div>
