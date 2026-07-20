# Agent 指引 — clarity-core

## 构建

```bash
cargo build -p clarity-core
```

## 测试

```bash
cargo test -p clarity-core --lib
```

## 关键文件

- `src/lib.rs` — 入口与核心类型重导出
- `src/agent/mod.rs` — `Agent` 结构体与状态机
- `src/agent/controller.rs` — `AgentController` 异步事件驱动控制器
- `src/tools/mod.rs` — `Tool` trait 与内置工具集
- `src/registry.rs` — `ToolRegistry` 工具注册表
- `src/llm/mod.rs` — LLM Provider 抽象与多厂商实现
- `src/mcp/mod.rs` — MCP 客户端与工具注入
- `src/subagents/mod.rs` — 子代理并行执行
- `src/background/mod.rs` — 后台任务管理器
- `src/memory/` — `PersistentMemoryStore`、`MemoryCompiler`（`clarity-memory` 的 core 侧封装）
- `src/knowledge.rs` — 对话 turn 更新 `KnowledgeField`；`index_compiled_memories` 把 `MemoryCompiler` 生成的 `.md` 产物索引进知识场
- `src/error.rs` — `AgentError` / `ToolError`

## 约定

- 错误处理使用 `AgentError` / `ToolError`
- 异步使用 `tokio`
- 工具实现需标注 `#[async_trait]`
- LLM Provider 需实现 `LlmProvider` trait
