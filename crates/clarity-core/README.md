# clarity-core

Clarity 核心引擎：Agent 循环、工具注册表、LLM 客户端、MCP 集成、子代理与后台任务调度。

## 职责

- **Agent 循环** — 管理 LLM 与工具之间的交互，支持迭代执行、错误恢复与流式输出
- **工具注册表** — 动态发现并注册内置工具与 MCP 外部工具，提供 JSON Schema 给 LLM
- **LLM 客户端** — 统一封装 OpenAI、Anthropic、Kimi、DeepSeek、Ollama 及本地 Candle GGUF 推理，支持自动切换
- **MCP 集成** — 通过 stdio/SSE 连接外部 MCP Server，将远程工具注入本地注册表
- **子代理** — 支持并行执行多个子代理任务，并聚合结果
- **后台任务** — 独立进程级别的 Agent 执行器，可脱离主会话长时间运行
- **记忆系统** — 集成 clarity-memory，提供会话级与长期记忆能力
- **审批系统** — Interactive / Yolo / Plan 三种模式控制工具执行权限

## 关键类型

- `Agent` — 核心 Agent 结构体，封装注册表、配置、LLM 与状态机
- `ToolRegistry` — 工具注册表，管理所有可用工具的生命周期
- `Tool` / `ToolContext` / `ToolResult` — 工具 trait 与执行上下文
- `AgentController` / `ControllerEvent` — 异步控制器与事件流
- `AgentError` / `ToolError` — 统一错误类型
- `BackgroundTaskManager` — 后台任务管理器
- `McpRegistry` / `McpClientBuilder` — MCP 客户端与注册表

## 测试

```bash
cargo test -p clarity-core --lib
```
