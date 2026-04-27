# Clarity 工具系统发展路线图

> 目标：通过 MCP + 内置工具达到超越 Nanobot 的能力  
> 关联文档：[`FUTURE_DIRECTION.md`](FUTURE_DIRECTION.md) Phase A1（WebSocket MCP 传输）

---

## 现状分析

### 当前内置工具（17+ 个）

| 工具 | 类别 | 状态 | 说明 |
|------|------|------|------|
| `file_read` | 文件 | ✅ 稳定 | 读取文件内容，支持 offset/limit |
| `file_write` | 文件 | ✅ 稳定 | 写入文件，自动创建父目录 |
| `file_edit` | 文件 | ✅ 稳定 | 字符串替换编辑 |
| `glob` | 搜索 | ✅ 稳定 | 文件模式匹配 |
| `grep` | 搜索 | ✅ 稳定 | 内容搜索，支持 regex |
| `bash` | Shell | ✅ 稳定 | Bash 命令执行 |
| `powershell` | Shell | ✅ 稳定 | PowerShell 命令执行（Windows） |
| `web_search` | 网络 | ✅ 稳定 | DuckDuckGo 网页搜索 |
| `web_fetch` | 网络 | ✅ 稳定 | 网页内容获取（支持 markdown/text/html） |
| `think` | 思考 | ✅ 稳定 | 结构化思考工具 |
| `task_create` | 任务 | ✅ 稳定 | 创建后台任务（原 SpawnTool 概念已覆盖） |
| `task_list` | 任务 | ✅ 稳定 | 列出后台任务 |
| `task_output` | 任务 | ✅ 稳定 | 获取任务输出 |
| `task_stop` | 任务 | ✅ 稳定 | 停止后台任务 |
| `schedule_cron` | 定时 | ✅ 稳定 | 创建定时任务 |
| `list_cron` | 定时 | ✅ 稳定 | 列出定时任务 |
| `cancel_cron` | 定时 | ✅ 稳定 | 取消定时任务 |
| `ask_user` | 交互 | ✅ 稳定 | 向用户提问 |
| `notify` | 通知 | ✅ 稳定 | 发送系统通知 |
| `todo` | 待办 | ✅ 稳定 | 待办事项管理 |
| `plan` | 规划 | ✅ 稳定 | 结构化计划执行 |
| `channel_send` | 渠道 | ✅ 稳定 | 飞书/钉钉/Slack/Webhook 主动消息发送 |

### MCP 实现状态

| 功能 | 状态 | 说明 |
|------|------|------|
| stdio transport | ✅ 已实现 | JSON-RPC 2.0 over stdio |
| HTTP transport | ✅ 已实现 | POST 请求 |
| SSE transport | ✅ 已实现 | Endpoint discovery + reconnection + handshake |
| 工具发现 | ✅ 已实现 | `tools/list` 方法 |
| 工具调用 | ✅ 已实现 | `tools/call` 方法 |
| 多连接管理 | ✅ 已实现 | `McpManager` 支持 |
| Tool 适配器 | ✅ 已实现 | `McpToolAdapter` |
| WebSocket transport | ⏸️ 未启动 | 见 [`FUTURE_DIRECTION.md`](FUTURE_DIRECTION.md) Phase A1 |
| 实际联调 | ⚠️ 待验证 | 需与真实 MCP server 完成端到端测试 |

---

## 扩展方案

### P0: 基础设施（高优先级）

#### 1. MCP WebSocket 传输 ⏸️

- 在 `McpTransport` 枚举新增 `WebSocket { url, headers }` 变体
- 基于 `tokio-tungstenite` 实现 `McpClient` trait
- **状态**: ⏸️ 未启动，归入 [`FUTURE_DIRECTION.md`](FUTURE_DIRECTION.md) Phase A1

#### 2. MCP 端到端联调验证 🔄

- 与官方 `@modelcontextprotocol/server-filesystem` 完成真实 tool call
- 与 `@modelcontextprotocol/server-git` 验证 HTTP transport
- **状态**: 🔄 待执行，建议在 Phase A 中一并完成

---

### P1: 短期增强工具

#### 3. Image Processing Tool 🖼️

**需求**：基础图像处理（resize / convert / thumbnail）

```rust
pub struct ImageTool;

// 参数
{
    "action": "resize",     // "resize" | "convert" | "thumbnail"
    "input": "image.png",
    "output": "image.jpg",
    "width": 800,
    "height": 600,
    "quality": 85
}
```

**状态**: 📋 待设计  
**约束**：需评估图像处理库（`image` crate）对编译时间和 binary size 的影响；项目广度 ≤ 5 核心工具。

#### 4. Database Query Tool 🗄️

**需求**：连接和查询数据库（SQLite 优先）

```rust
pub struct DatabaseTool;

// 参数
{
    "connection": "sqlite:///path/to/db.sqlite",
    "query": "SELECT * FROM users WHERE id = ?",
    "params": [123]
}
```

**状态**: 📋 待设计  
**约束**：安全考虑 — 默认只读模式；写操作需显式开启 `db-write` feature。

---

### P2: 通过 MCP 获取的工具（不重复实现）

Clarity 核心策略：通用能力通过 MCP server 扩展，内置工具只保留高频/安全敏感场景。

#### 官方 MCP Servers

| Server | 功能 | 优先级 | 状态 |
|--------|------|--------|------|
| `@modelcontextprotocol/server-filesystem` | 高级文件操作 | P1 | 待联调 |
| `@modelcontextprotocol/server-github` | GitHub API 集成 | P2 | — |
| `@modelcontextprotocol/server-git` | Git 操作 | P2 | — |
| `@modelcontextprotocol/server-postgres` | PostgreSQL 查询 | P2 | — |
| `@modelcontextprotocol/server-sqlite` | SQLite 查询 | P2 | — |
| `@modelcontextprotocol/server-puppeteer` | 浏览器自动化 | P3 | — |

#### 社区 MCP Servers

| Server | 功能 | 来源 |
|--------|------|------|
| `@kimi-cli/mcp-web-search` | 网页搜索 | 社区 |
| `@kimi-cli/mcp-commands` | 系统命令 | 社区 |
| `mcp-server-brave-search` | Brave 搜索 | 第三方 |

---

## 已归档内容

以下条目原属本路线图，因与现有实现重叠或方向调整，已归档：

- ~~Spawn/Background Task Tool~~ → 由 `task_create` + `BackgroundTaskManager` 覆盖
- ~~Cron/Scheduled Task Tool~~ → 由 `schedule_cron` / `list_cron` / `cancel_cron` 覆盖（已实现）

旧版完整内容见 `docs/archive/`（若需回溯）。

---

## 错误处理策略

### 工具级错误

每个工具应处理自己的特定错误，映射到 `ToolError` 分类：

```rust
impl Tool for WebSearchTool {
    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        let query = helpers::required_str(&args, "query")
            .map_err(|e| ToolError::invalid_params(format!("Invalid query: {}", e)))?;
        // ...
    }
}
```

### 注册表级错误

- 工具未找到 → `ToolError::not_found(name)`
- 执行超时 → `ToolError::Timeout(ctx.timeout_secs)`

### MCP 级错误

```rust
impl Tool for McpToolAdapter {
    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        self.client.call_tool(&self.tool.name, args)
            .await
            .map_err(|e| match e {
                AgentError::ToolExecutionFailed(name, msg) => {
                    ToolError::execution_failed(format!("MCP tool '{}' failed: {}", name, msg))
                }
                AgentError::Registry(msg) => {
                    ToolError::Unavailable(format!("MCP server unavailable: {}", msg))
                }
                _ => ToolError::execution_failed(format!("MCP error: {}", e)),
            })
    }
}
```

---

## 实现优先级时间线

```
Phase A（v0.3.1-2）— 基础设施联通
├── MCP WebSocket 传输
├── MCP 端到端联调验证（与真实 server）
└── 内置工具状态冻结（不新增）

Phase B+（v0.3.3+）— 按需扩展
├── Image Processing Tool（评估后决定内置或 MCP）
├── Database Query Tool（评估后决定内置或 MCP）
└── 更多官方 MCP server 联调
```

---

*上次更新：2026-04-26（v0.3.0 发布后审计）。*
