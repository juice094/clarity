# Project Clarity

一个基于 Rust 的本地优先 AI Agent 框架。

---

## 项目状态（截至 2026-04-09）

**当前阶段：核心功能稳定，进入实测验证期**

### 实际验证指标

| 指标 | 状态 | 备注 |
|------|------|------|
| 编译 | ✅ | `cargo check --workspace` 通过 |
| 测试 | ✅ | **~380+** passed, 0 failed |
| 代码警告 | ✅ | 3 警告（均为未使用变量/可变修饰符） |
| 代码规模 | ~750 KB | 91 个 Rust 源文件 |
| Crates | 5 个 | workspace 配置 |
| 生产就绪 | 🟢 | 核心功能稳定，待实测验证 |

### 功能完成度（基于实际代码核查）

| 功能模块 | 状态 | 说明 |
|----------|------|------|
| **clarity-wire** | ✅ | Soul-UI 通信通道，8 个测试通过 |
| **approval** | ✅ | 审批运行时，支持 Interactive/Yolo/Plan 模式 |
| **compaction** | ✅ | 上下文压缩，防止 Token 爆炸 |
| **Agent 核心** | ✅ | ReAct 循环、工具调用、流式响应 |
| **子代理 LaborMarket** | ✅ | 类型注册表（coder/explore/plan）|
| **子代理 Runner** | ✅ | 完整实现，支持前台/恢复执行 |
| **clarity-memory 集成** | ✅ | clarity-memory → clarity-core 集成完成，57 个记忆测试通过 |
| **MCP Client** | 🔄 | 骨架实现，`mcp.json` 配置支持进行中 |
| **Gateway WebSocket** | ✅ | WebSocket streaming 已实现，3 个 WS 集成测试通过 |
| **Gateway 渠道** | ⚠️ | Discord/Telegram/Webhook 代码存在，待实测 |
| **BackgroundTaskManager** | 🔄 | 骨架已实现（store/scheduler/worker），待 Gateway/TUI 集成活化 |

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
- ✅ **流式响应**: SSE 流式，支持实时 UI 更新
- ✅ **Wire 通信**: Soul-UI 解耦，支持多界面
- ✅ **上下文压缩**: 自动压缩长对话，防止 Token 爆炸
- ✅ **审批控制**: Interactive/Yolo/Plan 三种模式
- ✅ **多 LLM 支持**: Kimi, Anthropic, OpenAI 兼容, DeepSeek

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
cargo clippy --workspace              # 3 警告（均为未使用变量/可变修饰符）
```

### 运行 TUI

```powershell
# 方式 1: 使用 Kimi (ANTHROPIC 格式)
$env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
$env:ANTHROPIC_AUTH_TOKEN="sk-kimi-your-key"
cargo run -p clarity-tui

# 方式 2: 使用其他 provider
$env:KIMI_API_KEY="sk-xxx"
$env:DEEPSEEK_API_KEY="sk-xxx"
$env:OPENAI_API_KEY="sk-xxx"
```

---

## 已知限制

1. **MCP 配置文件支持待完善**: `mcp.json` 加载与动态 server 注册尚未完成
2. **后台任务系统骨架待活化**: `BackgroundTaskManager` 核心逻辑已实现，但与 Gateway/TUI 的端到端集成仍在推进
3. **Gateway 渠道**: Discord/Telegram/Webhook 代码存在但未实测
4. **Web UI**: 还在画饼阶段

---

## 后续规划

### Phase 1: 实测验证（当前 - 2 周）
- [ ] TUI 真实 LLM 联调
- [ ] Gateway HTTP API 端到端测试
- [ ] MCP Client + filesystem server 联调
- [ ] Gateway 渠道实测（Discord/Telegram）

### Phase 2: 稳定化（2-4 周）
- [ ] 错误处理完善
- [ ] 性能基准测试
- [ ] 跨平台测试
- [ ] 文档完善

### Phase 3: 能力扩展（4-8 周）
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
