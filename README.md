# Project Clarity

一个基于 Rust 的本地优先 AI Agent 框架。

---

## 项目状态（截至 2026-04-09）

**当前阶段：核心功能稳定，进入实测验证期**

### 实际验证指标

| 指标 | 状态 | 备注 |
|------|------|------|
| 编译 | ✅ | `cargo check --workspace` 通过 |
| 测试 | ✅ | **334+** passed, 0 failed |
| 代码警告 | ✅ | `clippy --workspace --lib --bins --tests` 零警告 |
| 代码规模 | ~750 KB | 91 个 Rust 源文件 |
| Crates | 5 个 | workspace 配置 |
| 生产就绪 | 🟢 | 核心功能稳定，TUI 已可连接真实 LLM |

### 功能完成度（基于实际代码核查）

| 功能模块 | 状态 | 说明 |
|----------|------|------|
| **clarity-wire** | ✅ | Soul-UI 通信通道，8 个测试通过 |
| **approval** | ✅ | 审批运行时，支持 Interactive/Yolo/Plan 模式 |
| **compaction** | ✅ | 上下文压缩，防止 Token 爆炸 |
| **Agent 核心** | ✅ | ReAct 循环、工具调用、**Stream-first** 流式响应 |
| **LLM 连接层** | ✅ | Kimi / Kimi Code / Anthropic / DeepSeek / OpenAI；prompt cache key；共享 HTTP 连接池 |
| **子代理 LaborMarket** | ✅ | 类型注册表（coder/explore/plan）|
| **子代理 Runner** | ✅ | 完整实现，支持前台/恢复执行 |
| **clarity-memory 集成** | ✅ | clarity-memory → clarity-core 集成完成，57 个记忆测试通过 |
| **MCP Client** | ✅ | Stdio/HTTP 实测通过，E2E `filesystem_read_file` 联调成功，支持 `mcp.json` 自动注入 ToolRegistry |
| **Gateway Chat Completions** | ✅ | OpenAI 兼容 API，支持 `stream=true` SSE 流式输出，已接入 AgentController |
| **Gateway WebSocket** | ✅ | WebSocket streaming 已实现，3 个 WS 集成测试通过 |
| **Gateway 渠道** | ⚠️ | Discord/Telegram/Webhook 代码存在，待实测 |
| **BackgroundTaskManager** | ✅ | `DefaultAgentTaskExecutor` 已实现，后台 Worker 可执行真实 Agent 实例 |
| **TUI 交互** | ✅ | 鼠标滚轮、命令注册表 (`/model` `/help` `/stop` `/personality`)、**Tab 命令补全**、输入历史 (↑/↓)、深色主题、实时指标 HUD |

---

## 架构设计（2026-04-04 版）

```
┌─────────────────────────────────────────────────────────────┐
│                        应用层                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ clarity-tui │  │clarity-gateway│ │   Future: Web UI    │  │
│  │  (终端界面)  │  │  (HTTP API)   │ │   （画饼中）        │  │
│  └──────┬──────┘  └──────┬──────┘  └─────────────────────┘  │
└─────────┼────────────────┼──────────────────────────────────┘
          │                │
          ▼                ▼
┌─────────────────────────────────────────────────────────────┐
│                      核心引擎层                              │
│                    clarity-core                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │    Agent    │  │ ToolRegistry│  │   LlmProvider       │  │
│  │   (ReAct)   │  │  (工具注册)  │  │ (多模型支持)        │  │
│  └──────┬──────┘  └──────┬──────┘  └─────────────────────┘  │
│         │                │                                   │
│  ┌──────▼────────────────▼─────────────────────┐             │
│  │   Wire (✅) - Soul-UI 通信通道               │             │
│  │   Approval (✅) - 工具调用审批               │             │
│  │   Compaction (✅) - 上下文压缩             │             │
│  │   Subagents (✅) - 子代理系统（含 Runner）   │             │
│  └─────────────────────────────────────────────┘             │
└─────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────┐
│                      存储层                                  │
│                   clarity-memory                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  FileStore  │  │ SqliteStore │  │    HybridStore      │  │
│  │ (JSON文件)   │  │ (SQLite+FTS5)│   │ (热缓存+冷存储)      │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│  ⚠️ 注意: clarity-memory 后端完整，待 clarity-core 完全集成   │
└─────────────────────────────────────────────────────────────┘
```

---

## 核心特性

### 1. Agent 核心 (clarity-core)

- ✅ **ReAct 循环**: 完整的思考-行动-观察循环
- ✅ **Stream-first 流式响应**: 先 `stream()` 后 fallback `complete()`，消除 double-request
- ✅ **Wire 通信**: Soul-UI 解耦，支持多界面
- ✅ **上下文压缩**: 自动压缩长对话，防止 Token 爆炸
- ✅ **审批控制**: Interactive/Yolo/Plan 三种模式
- ✅ **多 LLM 支持**: Kimi, Kimi Code, Anthropic, OpenAI 兼容, DeepSeek
- ✅ **Prompt Cache**: 自动注入 `prompt_cache_key`，支持会话级缓存路由
- ✅ **共享 HTTP 客户端**: 连接池复用，300s 请求超时
- ✅ **人格切换**: 默认 `Direct` 工程人格，支持 `/personality [direct|hanako|butter|ming]` 热切换

### 2. 子代理系统 (clarity-core/src/subagents/)

- ✅ **LaborMarket**: 子代理类型注册表（coder/explore/plan）
- ✅ **SubagentStore**: 状态存储
- ✅ **SubagentBuilder**: 构建器
- ✅ **Runner**: 完整实现，支持前台/后台/恢复执行

### 3. 记忆系统

- ✅ **clarity-memory**: File / SQLite / Hybrid 存储后端，57 个测试通过
- ✅ **PersistentMemoryStore**: 接口实现完成
- ✅ **记忆触发器**: MemoryTicker 已实现

### 4. 工具系统

- ✅ **8 个内置工具**: file_read/write/edit, glob, grep, bash, web_search/fetch
- ✅ **真实工具调用**: `tools` schema 已正确注入 LLM 请求，`complete()` 与 `stream()` 均支持
- ✅ **工具审批**: 危险操作需用户确认（Yolo 模式除外）
- ⚠️ **MCP Client**: 骨架实现，待真实 server 联调

---

## 快速开始

### 环境要求

- Rust 1.75+
- Windows / Linux / macOS
- 一颗能承受编译时间的心（第一次编译有点久）

### 编译与测试

```bash
cd clarity
cargo build --workspace
cargo test --workspace --lib --tests  # ~380+ tests passing
cargo clippy --workspace              # 零警告
```

### 运行 TUI

```powershell
# 方式 1: 使用 Kimi Code (推荐，编程计划)
$env:KIMI_CODE_API_KEY="sk-kimi-your-key"
cargo run -p clarity-tui

# 方式 2: 使用 Moonshot Open Platform
$env:KIMI_API_KEY="sk-xxx"
cargo run -p clarity-tui

# 方式 3: 使用 Claude / DeepSeek / OpenAI
$env:ANTHROPIC_AUTH_TOKEN="sk-ant-xxx"
$env:DEEPSEEK_API_KEY="sk-xxx"
$env:OPENAI_API_KEY="sk-xxx"
cargo run -p clarity-tui
```

**TUI 快捷键**
- `Enter` 发送消息 / `Esc` 返回 Normal 模式
- `↑/↓` 在输入框中浏览历史记录，Normal 模式下滚动聊天记录
- **鼠标滚轮** 滚动聊天记录
- `Tab` 自动补全 `/` 命令
- `Ctrl+C` 停止生成（生成中）或返回 Normal 模式（空闲时）
- `Ctrl+D` 退出程序
- `/help` 查看可用命令 (`/model`, `/stop`, `/clear`, `/personality` 等)

---

## 已知限制

1. **MCP 配置文件支持待完善**: `mcp.json` 加载与动态 server 注册尚未完成
2. **后台任务系统骨架待活化**: `BackgroundTaskManager` 核心逻辑已实现，但与 Gateway/TUI 的端到端集成仍在推进
3. **Gateway 渠道**: Discord/Telegram/Webhook 代码存在但未实测
4. **Web UI**: 还在画饼阶段

---

## 后续规划

### Phase 1: 实测验证（当前 - 1 周）
- [x] TUI 真实 LLM 联调（Kimi Code / Moonshot）
- [x] Stream-first 架构落地 + Prompt Cache
- [x] Personality 最小化/工程化改造（Direct 人格已上线）
- [x] TUI 交互补完（命令补全、历史、Ctrl+C 安全处理）
- [ ] Gateway HTTP API 端到端测试
- [ ] MCP Client + filesystem server 联调

### Phase 2: 稳定化（1-3 周）
- [ ] 错误处理完善
- [ ] 性能基准测试
- [ ] 跨平台测试
- [ ] 文档完善

### Phase 3: 能力扩展（3-8 周）
- [ ] MCP SSE transport 实现
- [ ] 向量检索优化
- [ ] 多 Agent Profile 管理
- [ ] TUI 配置文件支持

---

## 项目哲学

> "小而精，而非大而全"  
> "先让代码能跑，再让文档能看"  
> "如果文档和代码冲突，相信代码，然后来修文档"

---

## 许可证

MIT License - 随便用，但出了问题别找我（除非你想提 PR 修复）

---

*本文档最后更新：2026-04-09*  
*开发者：Clarity Team + AI Assistant*
