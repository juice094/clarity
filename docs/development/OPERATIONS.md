---
title: Clarity 运维与部署指南
category: Status
date: 2026-06-25
tags: [status]
---

# Clarity 运维与部署指南

> 版本：v0.3.4-rc | 关联文档：[`ARCHITECTURE.md`](../ARCHITECTURE.md) · [`ROADMAP.md`](../planning/ROADMAP.md) · [`AGENTS.md`](../../AGENTS.md)

---

## 1. 二进制布局

Clarity Workspace 包含 **22 个活跃 workspace crate + 1 个归档 crate（`clarity-tauri`）+ 1 个集成测试 crate（`tests/integration`）**，其中 6 个产出可执行文件：

| 可执行文件 | Crate | 适用场景 | 说明 |
|-----------|-------|---------|------|
| `clarity-egui` | `clarity-egui` | 桌面 GUI | 单二进制，即时模式 egui，无 WebView 依赖 |
| `clarity-tui` | `clarity-tui` | 终端交互 | ratatui 终端 UI，`/` 前缀命令 |
| `clarity-headless` | `clarity-headless` | 自动化 / CI | CLI：`run` / `jumpy` 子命令，支持 stdin pipe |
| `clarity-gateway` | `clarity-gateway` | HTTP 服务 | Axum 双端口（18790 公开 / 18800 Admin） |
| `clarity-claw` | `clarity-claw` | 系统托盘守护 | Gateway WebSocket 客户端，OS 通知推送 |
| `clarity-slint` | `clarity-slint` | 实验性桌面 GUI | Slint 栈，不参与默认 CI |

**纯库 crate**（不可直接运行）：
- `clarity-core` — 核心引擎（Agent 循环、工具注册表、LLM Provider、MCP 集成）
- `clarity-wire` — 事件总线（SPMC broadcast）
- `clarity-memory` — 混合记忆系统（SQLite + BM25 + 向量）
- `clarity-contract` — 共享 trait 契约（`Tool`、`LlmProvider`）
- `clarity-mcp` — MCP 客户端库（stdio / SSE / HTTP / WebSocket）
- `clarity-llm` — LLM provider 抽象 + Candle GGUF 本地推理
- `clarity-tools` — 内置工具库（file / shell / web / devkit）
- `clarity-channels` — 外部通道抽象（WeChat iLink / Webhook）
- `clarity-subagents` — 子代理执行器（消费 `clarity-core`）
- `clarity-thread-store` — Thread 持久化抽象
- `clarity-rollout` — JSONL rollout 持久化
- `clarity-openclaw` — OpenClaw/KimiClaw Gateway WebSocket 客户端
- `clarity-secrets` — 加密 Secret 存储（`enc2:`）
- `clarity-telemetry` — 统一遥测（当前由 gateway 使用）
- `clarity-mobile-core` — 移动端 UniFFI FFI 核心（lib，供 Android/iOS 使用）

---

## 2. 资源需求

| 资源 | 基线 | 峰值 | 说明 |
|------|------|------|------|
| **CPU** | 1-2 核 | 4 核 | tokio 多线程 runtime；本地 LLM 推理时 CPU 满载 |
| **内存** | ~80 MB | ~4 GB+ | 基线不含模型；`local-llm` feature 加载 GGUF 时占模型大小 |
| **磁盘** | ~50 MB | ~1 GB | 二进制 + SQLite 会话存储；1000 轮对话约 10 MB |
| **网络** | 按需 | 持续 | 云 LLM API 流式调用、MCP SSE 长连接、Gateway WebSocket |

**本地 LLM 显存/内存警告**：
- Qwen2-1.5B-Q4_K_M.gguf ≈ 1.1 GB RAM
- DeepSeek-R1-Distill-Qwen-1.5B-Q4_K_M.gguf ≈ 1.1 GB RAM
- 首次下载由 `hf-hub` 缓存到 `~/.cache/huggingface/hub/`

---

## 3. 配置体系

### 3.1 环境变量

| 变量 | 作用域 | 说明 |
|------|--------|------|
| `RUST_LOG` | 全局 | `tracing-subscriber` 日志级别，如 `clarity_core=debug` |
| `CLARITY_MCP_ALLOWLIST` | MCP | 覆盖默认命令校验白名单（逗号分隔） |
| `CLARITY_GATEWAY_ADMIN_TOKEN` | Gateway | Admin 端口 (18800) 的 Bearer Token |
| `OPENAI_API_KEY` / `DEEPSEEK_API_KEY` / ... | LLM | 各云厂商 API Key（由 `llm_loader.rs` 读取） |

### 3.2 配置文件

```
~/.config/clarity/
├── config.toml           # 主配置：LLM provider、approval mode、能力开关
├── mcp.json              # MCP 服务器列表（Claude Desktop 兼容格式）
├── skills/               # 自定义 Skill 目录
│   └── *.md
└── sessions/             # 会话 JSONL 存储（由 SessionStore 管理）
```

### 3.3 CLI 参数

```bash
# Headless 模式
clarity-headless run \
  --prompt "Explain Rust lifetimes" \
  --provider openai \
  --approval interactive \
  --max-iterations 20

clarity-headless jumpy \
  --skill rust-refactor \
  --predictor llm \
  --commitment 0.8

# Gateway 模式
clarity-gateway  # 无参数，配置来自 config.toml + 环境变量
```

---

## 4. 日志与观测

### 4.1 日志架构

- **框架**：`tracing` + `tracing-subscriber`
- **过滤器**：`EnvFilter`，默认 `INFO`，可通过 `RUST_LOG` 覆盖
- **输出目标**：
  - `clarity-egui` / `clarity-tui` / `clarity-headless` → stderr
  - `clarity-gateway` → stderr + 可选文件滚动（待实现）
  - `clarity-claw` → OS 通知中心（Windows Toast / macOS Notification Center）

### 4.2 常用诊断命令

```bash
# 查看所有模块的 debug 日志
RUST_LOG=debug cargo run -p clarity-gateway

# 仅查看 MCP 客户端日志
RUST_LOG=clarity_mcp=trace cargo run -p clarity-headless -- run ...

# 查看 Agent 循环详情
RUST_LOG=clarity_core::agent=debug cargo run -p clarity-tui
```

### 4.3 指标与遥测（未来）

- **当前状态**：无 OTLP / Prometheus 导出
- **路线图**：Phase C（v0.4.0+）计划集成 `metrics` crate，暴露：
  - LLM 请求延迟 / Token 吞吐量
  - 工具执行成功率
  - 内存压缩触发频率
  - Gateway 并发连接数

---

## 5. 部署模式

### 5.1 单二进制桌面（推荐个人用户）

```
┌─────────────────────────────┐
│        clarity-egui         │
│  (eframe + clarity-core)    │
└─────────────────────────────┘
```
- 零外部依赖，双击运行
- 自动创建 `~/.config/clarity/` 目录

### 5.2 终端 + Gateway 分离（推荐开发者）

```
┌─────────────┐     ┌─────────────────────────────┐
│ clarity-tui │────→│     clarity-gateway         │
│  (ratatui)  │ WS  │  (Axum 18790 + SQLite)      │
└─────────────┘     └─────────────────────────────┘
                            ↓
                    ┌───────────────┐
                    │ clarity-core  │
                    │ (Agent 循环)   │
                    └───────────────┘
```
- TUI 通过 WebSocket (`/ws`) 接收实时事件
- Gateway 提供 HTTP API 供外部工具调用

### 5.3 自动化 / CI（推荐流水线）

```bash
# systemd service 示例
[Unit]
Description=Clarity Headless Agent

[Service]
ExecStart=/usr/local/bin/clarity-headless run --prompt-file /opt/tasks/daily.md
Environment="RUST_LOG=info"
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

### 5.4 守护进程（推荐常驻）

```
OS Startup ──→ clarity-claw
                    ├── 启动 clarity-gateway（如未运行）
                    ├── 系统托盘图标 + 菜单
                    └── 后台任务完成时 OS 通知
```
- Windows：注册表 `Run` 键或任务计划程序
- macOS：`~/Library/LaunchAgents/`
- Linux：`systemd --user` 或 `.desktop` autostart

---

## 6. 故障排查速查

| 现象 | 排查步骤 |
|------|---------|
| 编译失败 | `cargo check --workspace --lib --bins --exclude clarity-slint`；检查 `local-llm` feature 是否启用 |
| 测试失败 | `cargo test --workspace --lib --exclude clarity-slint`；ignored 测试需外部条件（本地 GGUF 等） |
| Gateway 无法启动 | 检查 18790/18800 端口是否被占用；`lsof -i :18790` |
| MCP 工具加载失败 | 验证 `~/.config/clarity/mcp.json` JSON 语法；检查命令是否在 allowlist |
| 本地 LLM 加载失败 | 确认 `~/.cache/huggingface/hub/` 有模型；检查 `local-llm` feature |
| SQLite 性能下降 | 手动执行 `PRAGMA wal_checkpoint;` 或重启进程 |
| 日志过多 | 调整 `RUST_LOG=warn` 或按模块过滤 |
| Token 消耗过快 | 检查上下文窗口长度；启用 compaction 或缩短 `max_iterations` |

---

## 7. 备份与迁移

- **会话数据**：`~/.config/clarity/sessions/` 为纯 JSONL，可直接复制
- **记忆数据库**：`~/.config/clarity/memory.db*`（主库 + WAL + SHM），迁移时需三文件一起复制
- **配置文件**：`config.toml` 和 `mcp.json` 为文本，版本控制友好
- **模型缓存**：`~/.cache/huggingface/hub/` 体积大，建议按需迁移
