# Project Clarity

> CCA 架构 - Claude-Code 体验 + OpenClaw 网关

## 项目愿景

Project Clarity 是一个现代化的 AI 代理框架，旨在提供：

- **Claude-Code 级别的终端体验** - 流畅、智能、开发者友好的 TUI 界面
- **OpenClaw 网关架构** - 可扩展、安全、高性能的 MCP (Model Context Protocol) 网关
- **模块化设计** - 三层架构清晰分离，支持灵活扩展

## 快速开始

### 环境要求

- Rust 1.75+
- Windows / Linux / macOS

### 配置 LLM

#### 🎯 推荐：Kimi Code（与 Claude Code 相同配置）

**与 Claude Code 完全兼容的环境变量：

```powershell
# 与 Claude Code 完全一致！
$env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
$env:ANTHROPIC_AUTH_TOKEN="sk-kimi-your-key"

# 运行
cargo run --example claude_code_compat
```

**特点**：
- ✅ 与 Claude Code 相同的环境变量
- ✅ 每周 1024 次免费
- ✅ 自动检测 Anthropic 协议

**注意**：Kimi Code 会验证客户端身份，仅限白名单客户端

#### 方式二：OpenAI 兼容

```powershell
$env:OPENAI_API_KEY="sk-..."
$env:OPENAI_BASE_URL="https://api.openai.com/v1"
$env:OPENAI_MODEL="gpt-4"
```

#### 方式三：本地模型 (Ollama)

```powershell
# 无需 API Key，本地运行
$env:OLLAMA_HOST="http://localhost:11434"
# 在代码中使用 OpenAiCompatibleLlm::ollama("llama3.2")
```

### 安装

```bash
# 克隆仓库
git clone https://github.com/clarity/clarity.git
cd clarity

# 构建整个工作区
cargo build --release

# 运行示例（需要配置 KIMI_API_KEY）
cargo run --example kimi_demo

# 运行 TUI 客户端
cargo run -p clarity-tui

# 运行网关服务
cargo run -p clarity-gateway
```

### 开发模式

```bash
# 运行测试
cargo test --workspace

# 运行特定 crate 的测试
cargo test -p clarity-core

# 检查代码
cargo clippy --workspace

# 格式化代码
cargo fmt --all
```

## 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                     Project Clarity                         │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  clarity-tui │  │ clarity-core │  │   gateway    │      │
│  │  (UI Layer)  │  │ (Core Layer) │  │ (Gateway)    │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                 │                 │              │
│         └─────────────────┼─────────────────┘              │
│                           │                                │
│                    ┌──────┴──────┐                         │
│                    │  Tool Reg.  │                         │
│                    │  MCP Proc.  │                         │
│                    │  Session M. │                         │
│                    └─────────────┘                         │
└─────────────────────────────────────────────────────────────┘
```

## Crate 说明

| Crate | 描述 | 路径 |
|-------|------|------|
| `clarity-core` | 核心引擎 - 工具注册、MCP 处理、会话管理 | `crates/clarity-core` |
| `clarity-tui` | 终端 UI - Ratatui 驱动的交互界面 | `crates/clarity-tui` |
| `clarity-gateway` | HTTP/WebSocket 网关服务 | `crates/clarity-gateway` |

## 示例

```bash
# Kimi Code（会员权益，每周 1024 次免费）
$env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
$env:ANTHROPIC_AUTH_TOKEN="sk-kimi-your-key"
cargo run --example kimi_demo

# Kimi API（开放平台，按量计费）
$env:KIMI_API_KEY="sk-your-moonshot-key"
$env:KIMI_BASE_URL="https://api.moonshot.cn/v1"
cargo run --example kimi_demo

# Ollama 本地模型（无需网络）
cargo run --example ollama_demo

# 自定义工具示例
cargo run --example custom_tool
```

## 文档

- [架构文档](./ARCHITECTURE.md) - 详细架构设计
- [API 文档](./docs/api.md) - 网关 API 参考
- [开发指南](./docs/dev.md) - 贡献指南

## 许可证

MIT License - 详见 [LICENSE](./LICENSE)

## 贡献

欢迎提交 Issue 和 PR！请先阅读 [CONTRIBUTING.md](./CONTRIBUTING.md)
