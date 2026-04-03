# Project Clarity

一个基于 Rust 的本地优先 AI Agent 框架。

> **⚠️ 不可靠交付说明 / UNRELIABLE DELIVERY NOTICE**  
> 本文档由人类开发者与 AI 助手共同编写，内容可能包含过度乐观的描述、未完成的愿景，以及暂时还没被发现的 bug。请保持怀疑态度阅读，并以实际代码为准。  
>  
> *如果你发现文档与代码不符，相信代码，然后来嘲笑文档。*

---

## 项目状态（截至 2026-04-04）

**当前阶段：从"能编译"走向"能用"的过渡期**

### 实际验证指标

| 指标 | 状态 | 备注 |
|------|------|------|
| 编译 | ✅ | `cargo check --workspace` 通过 |
| 测试 | ✅ | **180+** passed, 0 failed (新增子代理 Runner 测试) |
| 代码警告 | ⚠️ | 3 个未使用函数警告（clarity-memory 模块）|
| 代码规模 | ~645 KB | 68 个 Rust 源文件 |
| 生产就绪 | 🟡 | 核心功能已落地，需实测验证 |

### 功能完成度（基于实际代码核查）

| 功能模块 | 状态 | 说明 |
|----------|------|------|
| **clarity-wire** | ✅ | Soul-UI 通信通道，8 个测试通过 |
| **approval** | ✅ | 审批运行时，支持 Interactive/Yolo/Plan 模式 |
| **compaction** | ✅ | 上下文压缩，防止 Token 爆炸 |
| **Agent 核心** | ✅ | ReAct 循环、工具调用、流式响应 |
| **子代理 LaborMarket** | ✅ | 类型注册表（coder/explore/plan）|
| **子代理 Runner** | ✅ | 完整实现，支持前台/恢复执行 |
| **内存存储** | ⚠️ | File/SQLite/Hybrid 后端存在，但 clarity-core 中 PersistentMemoryStore 是占位符 |
| **MCP Client** | ⚠️ | 代码框架存在，待实测验证 |
| **Gateway 渠道** | ⚠️ | Discord/Telegram/Webhook 代码存在，待实测 |

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
│  │   Subagents (⚠️) - 子代理系统（无 Runner）   │             │
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
│  ⚠️ 注意: clarity-core 中 PersistentMemoryStore 是占位符实现   │
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
- ❌ **Runner**: 尚未实现（子代理暂时无法实际运行）

### 3. 记忆系统

- ⚠️ **clarity-memory**: File / SQLite / Hybrid 存储后端，57 个测试通过
- ⚠️ **clarity-core 集成**: PersistentMemoryStore 是占位符（返回空实现）
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
cargo test --workspace --lib  # 169 tests passing
cargo clippy --workspace       # 3 个警告（未使用函数）
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

## 已知限制（诚实版）

1. **子代理系统**: 有注册表、存储、构建器，但没有 Runner，所以暂时跑不起来
2. **PersistentMemoryStore**: 是占位符实现，不会真正持久化数据
3. **MCP**: 骨架实现，待真实 server 联调
4. **Gateway 渠道**: Discord/Telegram/Webhook 代码存在但未实测
5. **Web UI**: 还在画饼阶段

---

## 后续规划（可能靠谱的路线图）

### Phase 1: 子代理完善（当前）
- [ ] SubagentRunner - 实际执行子代理任务
- [ ] 前台/后台运行模式
- [ ] 父子代理上下文传递

### Phase 2: 记忆系统完善（当前）
- [ ] 将 clarity-memory 真正集成到 clarity-core
- [ ] 替换 PersistentMemoryStore 占位符

### Phase 3: 实测验证（当前 - 2 周）
- [ ] TUI 真实 LLM 联调
- [ ] Gateway HTTP API 端到端测试
- [ ] MCP Client + filesystem server 联调
- [ ] Gateway 渠道实测（Discord/Telegram）

### Phase 4: 稳定化（待定）
- [ ] 错误处理完善
- [ ] 性能基准测试
- [ ] 跨平台测试
- [ ] 文档完善

---

## 项目哲学

> "小而精，而非大而全"  
> "先让代码能跑，再让文档能看"  
> "如果文档和代码冲突，相信代码，然后来修文档"

---

## 许可证

MIT License - 随便用，但出了问题别找我（除非你想提 PR 修复）

---

*本文档最后更新：2026-04-04*  
*开发者：Clarity Team + AI Assistant*  
*可靠性声明：如文首所述，保持怀疑，看代码说话。*
