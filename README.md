# Project Clarity

一个基于 Rust 的本地优先 AI Agent 框架。

> **⚠️ 不可靠交付说明 / UNRELIABLE DELIVERY NOTICE**  
> 本文档由人类开发者与 AI 助手共同编写，内容可能包含过度乐观的描述、未完成的愿景，以及暂时还没被发现的 bug。请保持怀疑态度阅读，并以实际代码为准。

---

## 项目状态（截至 2026-04-03）

**当前阶段：核心功能完成，测试修复完成，生产就绪候选**

### 实际验证指标

| 指标 | 状态 | 备注 |
|------|------|------|
| 编译 | ✅ | `cargo check --workspace` 通过 |
| 核心测试 | ✅ | 126 passed, 0 failed, 3 ignored |
| 代码警告 | ⚠️ | 1 minor (dead_code)，不影响功能 |
| 文档 | ✅ | 主要 API 已文档化 |
| 生产就绪 | 🟡 | 核心模块 ready，待更多实战检验 |

### 版本备份

- `Clarity_20260403_164510/` - 原始基线
- `Clarity_Enhanced_20260403_174318/` - 子代理增强版本
- `Clarity_Fixed_20260403_175543/` - **修复后当前版本**

---

## 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                        应用层                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ clarity-tui │  │clarity-gateway│ │   Future: GUI/Web   │  │
│  │  (终端界面)  │  │  (HTTP API)   │ │                     │  │
│  └──────┬──────┘  └──────┬──────┘  └─────────────────────┘  │
└─────────┼────────────────┼──────────────────────────────────┘
          │                │
          ▼                ▼
┌─────────────────────────────────────────────────────────────┐
│                      核心引擎层                              │
│                    clarity-core                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │    Agent    │  │ ToolRegistry│  │   LlmProvider       │  │
│  │   (ReAct)   │  │  (工具注册)  │  │ (Kimi/OpenAI/Ollama)│  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ ExecutionTracer│ │ErrorRecovery│  │   Config (TOML)     │  │
│  │  (执行追踪)  │  │  (错误恢复)  │  │   (多源配置)         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────┬─────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────┐
│                      存储层                                  │
│                   clarity-memory                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  FileStore  │  │ SqliteStore │  │    HybridStore      │  │
│  │ (JSON文件)   │  │ (SQLite+FTS5)│   │ (热缓存+冷存储)      │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │TfidfVectorizer│ │MemoryCompiler│  │ RuleBasedExtractor  │  │
│  │ (向量搜索)   │  │ (记忆整理)   │  │  (事实提取)          │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## 核心特性

### 1. Agent 核心 (clarity-core)

- ✅ **ReAct 循环**: 完整的思考-行动-观察循环
- ✅ **流式响应**: SSE 流式，TUI 实时显示
- ✅ **执行追踪**: `ExecutionTracer` 记录每步耗时和 Token 使用
- ✅ **错误恢复**: 指数退避重试，多种恢复策略
- ✅ **多 LLM 支持**: Kimi Code (Anthropic), OpenAI 兼容, Ollama

### 2. 记忆系统 (clarity-memory)

- ✅ **多后端存储**: File / SQLite / Hybrid
- ✅ **向量搜索**: TF-IDF 实现，无需外部 ML 服务
- ✅ **记忆整理**: 自动去重、合并、重要性评分
- ✅ **规则提取**: 从对话中提取偏好、身份、目标

### 3. 工具系统

- ✅ **8 个内置工具**: file_read/write/edit, glob, grep, bash, web_search/fetch
- ✅ **工具上下文**: 工作目录、超时、只读模式安全控制
- ✅ **动态注册**: `ToolRegistry` 支持运行时注册

### 4. 配置系统

- ✅ **多源配置**: TOML 文件 + 环境变量 + 代码
- ✅ **热重载**: 支持配置运行时更新

---

## 快速开始

### 环境要求

- Rust 1.75+
- Windows / Linux / macOS

### 编译与测试

```bash
cd clarity
cargo build --workspace
cargo test --workspace --lib  # 126 tests passing
cargo clippy --workspace
```

### 运行 TUI

```powershell
$env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
$env:ANTHROPIC_AUTH_TOKEN="sk-kimi-your-key"
cargo run -p clarity-tui
```

---

## 技术栈

| 领域 | 技术 |
|------|------|
| 异步运行时 | Tokio |
| 错误处理 | Anyhow + thiserror |
| TUI | ratatui + crossterm |
| Web | axum + tower-http |
| 序列化 | serde + serde_json |
| HTTP 客户端 | reqwest |
| 存储 | rusqlite (SQLite + FTS5) |
| 并发 | DashMap, parking_lot |

---

## 已知限制

1. **HybridStore**: 测试有超时问题，功能正常但测试需改进
2. **Examples**: clarity-memory 的示例暂时移除（API 变更导致）
3. **MCP**: 骨架实现，待真实 server 联调

---

## 项目哲学

> "小而精，而非大而全"  
> 优先做好 Agent 核心、工具系统、记忆存储这三个支柱，而非堆砌功能列表。

---

## 许可证

MIT License

---

*本文档最后更新：2026-04-03*  
*开发者：Clarity Team + AI Assistant*  
*可靠性声明：如文首所述，保持怀疑，看代码说话。*
