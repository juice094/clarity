# Clarity 工具系统发展路线图

> 目标：通过 MCP + 内置工具达到超越 Nanobot 的能力

## 现状分析

### 当前内置工具（15+ 个）

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
| `task_list` | 任务 | ✅ 稳定 | 列出后台任务 |
| `task_output` | 任务 | ✅ 稳定 | 获取任务输出 |
| `task_stop` | 任务 | ✅ 稳定 | 停止后台任务 |
| `ask_user` | 交互 | ✅ 稳定 | 向用户提问 |
| `notify` | 通知 | ✅ 稳定 | 发送系统通知 |
| `todo` | 待办 | ✅ 稳定 | 待办事项管理 |
| `plan` | 规划 | ✅ 稳定 | 结构化计划执行 |

### MCP 实现状态

| 功能 | 状态 | 说明 |
|------|------|------|
| stdio transport | ✅ 已实现 | JSON-RPC 2.0 over stdio |
| 工具发现 | ✅ 已实现 | `tools/list` 方法 |
| 工具调用 | ✅ 已实现 | `tools/call` 方法 |
| 多连接管理 | ✅ 已实现 | `McpManager` 支持 |
| Tool 适配器 | ✅ 已实现 | `McpToolAdapter` |
| 实际联调 | ⚠️ 未测试 | 需要与真实 MCP server 测试 |

## 扩展方案

### P0: 立即需要的基础工具（高优先级）

这些工具是 AI 助手日常工作的基础，必须作为内置工具实现。

#### 1. Web Search Tool 🌐 ✅ 已实现

**实现位置**: `crates/clarity-core/src/tools/web.rs`（第 56 行起）

**参数**:
```json
{
    "query": "Rust async runtime comparison",
    "num_results": 5,
    "recency_days": 7
}
```

**状态**: ✅ 已实现并注册（`registry.rs` 第 70 行）

---

#### 2. Web Fetch Tool 📄 ✅ 已实现

**实现位置**: `crates/clarity-core/src/tools/web.rs`（第 373 行起）

**参数**:
```json
{
    "url": "https://example.com/article",
    "format": "markdown",
    "max_length": 5000
}
```

**状态**: ✅ 已实现并注册（`registry.rs` 第 71 行）

---

#### 3. PowerShell Tool 🔧 ✅ 已修复

**实现位置**: `crates/clarity-core/src/tools/shell.rs`（第 180 行起）

**注册位置**: `registry.rs` 第 67 行 `let _ = registry.register(PowerShellTool::new());`

**状态**: ✅ 已实现并注册（Windows 平台自动启用）

---

### P1: 短期需要的增强工具

#### 4. Spawn/Background Task Tool ⚡

**需求**：执行长时间运行的任务，不阻塞主流程

```rust
pub struct SpawnTool;

// 参数
{
    "command": "cargo build --release",
    "cwd": "./project",
    "timeout_secs": 300,
    "background": false  // true = 不等待完成
}

// 返回
{
    "task_id": "uuid",
    "status": "running",  // "running" | "completed" | "failed"
    "exit_code": null,
    "output_url": "/tasks/uuid/output"
}
```

**状态**: 📋 待设计

---

#### 5. Cron/Scheduled Task Tool ⏰

**需求**：定时执行任务

```rust
pub struct CronTool;

// 参数 - 创建任务
{
    "action": "create",
    "schedule": "0 9 * * *",  // cron 表达式
    "command": "backup.sh",
    "name": "daily-backup"
}

// 参数 - 列出任务
{"action": "list"}

// 参数 - 删除任务
{"action": "delete", "name": "daily-backup"}
```

**状态**: 📋 待设计（依赖持久化存储）

---

#### 6. Image Processing Tool 🖼️

**需求**：基础图像处理

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

---

#### 7. Database Query Tool 🗄️

**需求**：连接和查询数据库

```rust
pub struct DatabaseTool;

// 参数
{
    "connection": "sqlite:///path/to/db.sqlite",
    "query": "SELECT * FROM users WHERE id = ?",
    "params": [123]
}
```

**状态**: 📋 待设计（安全考虑：只读模式默认）

---

### P2: 长期通过 MCP 获取的工具

这些工具通过 MCP server 提供，不需要在 Clarity 中重复实现。

#### 官方 MCP Servers

| Server | 功能 | 优先级 |
|--------|------|--------|
| `@modelcontextprotocol/server-filesystem` | 高级文件操作 | P0 |
| `@modelcontextprotocol/server-github` | GitHub API 集成 | P1 |
| `@modelcontextprotocol/server-git` | Git 操作 | P1 |
| `@modelcontextprotocol/server-postgres` | PostgreSQL 查询 | P1 |
| `@modelcontextprotocol/server-sqlite` | SQLite 查询 | P1 |
| `@modelcontextprotocol/server-puppeteer` | 浏览器自动化 | P2 |

#### 社区 MCP Servers

| Server | 功能 | 来源 |
|--------|------|------|
| `@kimi-cli/mcp-web-search` | 网页搜索 | 社区 |
| `@kimi-cli/mcp-commands` | 系统命令 | 社区 |
| `mcp-server-brave-search` | Brave 搜索 | 第三方 |

## 工具调用流程

```
┌─────────────────────────────────────────────────────────────────┐
│                     Tool Execution Flow                          │
└─────────────────────────────────────────────────────────────────┘

     User Request
          │
          ▼
┌─────────────────┐
│   LLM Client    │────▶ Gets tool schemas from ToolRegistry
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  LLM Decision   │────▶ Decides which tool to call with what args
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  ToolRegistry   │────▶ Looks up tool by name
└────────┬────────┘
         │
         ▼
┌─────────────────┐     ┌─────────────────┐
│   Tool Type?    │────▶│  Built-in Tool  │
└─────────────────┘     └────────┬────────┘
         │                       │ execute()
         │ No                    ▼
         │              ┌─────────────────┐
         │              │  Direct Execution
         │              └────────┬────────┘
         │                       │
         ▼                       │
┌─────────────────┐              │
│  MCP Adapter    │              │
└────────┬────────┘              │
         │                       │
         ▼                       │
┌─────────────────┐              │
│  MCP Client     │              │
└────────┬────────┘              │
         │ JSON-RPC              │
         ▼                       │
┌─────────────────┐              │
│  MCP Server     │              │
│  (stdio/SSE)    │              │
└────────┬────────┘              │
         │                       │
         ▼                       ▼
┌─────────────────────────────────────┐
│           Result Aggregation         │
│  ┌─────────────┐  ┌─────────────┐   │
│  │   Success   │  │    Error    │   │
│  │  Return to  │  │  Retry /    │   │
│  │     LLM     │  │  Report     │   │
│  └─────────────┘  └─────────────┘   │
└─────────────────────────────────────┘
```

## 错误处理策略

### 1. 工具级错误

每个工具应该处理自己的特定错误：

```rust
impl Tool for WebSearchTool {
    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        // 参数验证
        let query = helpers::required_str(&args, "query")
            .map_err(|e| ToolError::invalid_params(format!("Invalid query: {}", e)))?;
        
        // 网络请求
        let response = reqwest::get(&url)
            .await
            .map_err(|e| ToolError::execution_failed(format!("Network error: {}", e)))?;
        
        // 响应验证
        if response.status().is_server_error() {
            return Err(ToolError::Unavailable(
                "Search service temporarily unavailable".to_string()
            ));
        }
        
        // ...
    }
}
```

### 2. 注册表级错误

```rust
pub async fn execute(&self, name: &str, args: Value, ctx: ToolContext) -> ToolResult<Value> {
    let tool = self.get(name)
        .map_err(|e| ToolError::execution_failed(format!("Registry error: {}", e)))?
        .ok_or_else(|| ToolError::not_found(name))?;
    
    // 执行超时处理
    match tokio::time::timeout(Duration::from_secs(ctx.timeout_secs), tool.execute(args, ctx)).await {
        Ok(result) => result,
        Err(_) => Err(ToolError::Timeout(ctx.timeout_secs)),
    }
}
```

### 3. MCP 级错误

```rust
impl Tool for McpToolAdapter {
    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        self.client
            .call_tool(&self.tool.name, args)
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

### 4. 重试策略

对于网络相关的工具（WebSearch, WebFetch），建议实现指数退避重试：

```rust
async fn fetch_with_retry(url: &str, max_retries: u32) -> Result<Response, Error> {
    let mut delay = Duration::from_millis(100);
    
    for attempt in 0..max_retries {
        match reqwest::get(url).await {
            Ok(resp) if resp.status().is_success() => return Ok(resp),
            Ok(resp) if resp.status().as_u16() == 429 => {
                // Rate limited, retry with backoff
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            Err(e) if attempt < max_retries - 1 => {
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            result => return result.map_err(|e| e.into()),
        }
    }
    
    Err(Error::MaxRetriesExceeded)
}
```

## 实现优先级时间线

```
Week 1-2 (P0 - Foundation)
├── WebSearchTool 完整实现
├── WebFetchTool 实现
├── PowerShellTool 注册修复
└── MCP Filesystem 联调测试

Week 3-4 (P1 - Enhancement)
├── Spawn/Background Task Tool
├── Image Processing Tool
└── Database Query Tool (SQLite)

Month 2+ (P2 - Ecosystem)
├── MCP Server 配置管理
├── GitHub/Git MCP Server 集成
├── 社区 MCP Servers 适配
└── 自定义 MCP Server 开发指南
```

## 与 Nanobot 能力对比

| 能力 | Nanobot | Clarity 现状 | Clarity 目标 |
|------|---------|--------------|--------------|
| 文件操作 | ✅ 完整 | ✅ 基础 | ✅ 完整（+MCP）|
| Shell 执行 | ✅ Bash | ✅ Bash | ✅ Bash+PowerShell |
| 代码搜索 | ✅ grep/ag | ✅ grep | ✅ grep |
| 网络搜索 | ✅ 内置 | ❌ 缺失 | ✅ WebSearchTool |
| 网页获取 | ✅ 内置 | ❌ 缺失 | ✅ WebFetchTool |
| Git 操作 | ✅ 内置 | ❌ 缺失 | ✅ MCP git server |
| 数据库 | ✅ 内置 | ❌ 缺失 | ✅ MCP postgres/sqlite |
| 浏览器自动化 | ⚠️ 有限 | ❌ 缺失 | ✅ MCP puppeteer |
| 扩展机制 | ❌ 无 | ✅ MCP | ✅ MCP ecosystem |

## 结论

通过 **内置工具 + MCP 集成** 的双轨策略，Clarity 可以：

1. **短期**：快速补齐基础能力（WebSearch, WebFetch）
2. **中期**：通过 MCP 接入丰富的外部工具生态
3. **长期**：成为 MCP 生态的积极参与者和贡献者

MCP 协议是 Clarity 超越 Nanobot 的关键——它让 Clarity 从单一工具集进化为可无限扩展的工具平台。
