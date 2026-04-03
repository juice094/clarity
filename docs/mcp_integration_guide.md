# MCP 集成使用文档

> Model Context Protocol (MCP) 让 Clarity 能够连接无限的外部工具生态

## 目录

- [快速开始](#快速开始)
- [架构概览](#架构概览)
- [核心组件](#核心组件)
- [使用指南](#使用指南)
- [工具调用流程](#工具调用流程)
- [错误处理](#错误处理)
- [配置示例](#配置示例)
- [故障排除](#故障排除)

---

## 快速开始

### 1. 安装依赖

确保你有 Node.js 和 npx：

```bash
node --version  # v18+
npx --version
```

### 2. 运行示例

```bash
# 连接 filesystem MCP server
cargo run --example mcp_filesystem_demo -- "."

# 连接其他 MCP servers
cargo run --example mcp_demo -- npx -y @modelcontextprotocol/server-filesystem .
```

### 3. 在代码中使用

```rust
use clarity_core::mcp::{McpClient, McpToolAdapter};
use clarity_core::ToolRegistry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 连接 MCP server
    let client = McpClient::connect_stdio(
        "npx",
        &["-y", "@modelcontextprotocol/server-filesystem", "."]
    ).await?;
    
    // 注册工具
    let registry = ToolRegistry::new();
    for tool in client.list_tools().await? {
        registry.register(McpToolAdapter::new(client.clone(), tool))?;
    }
    
    // 现在 registry 中包含 MCP 工具！
    Ok(())
}
```

---

## 架构概览

```
┌─────────────────────────────────────────────────────────────────┐
│                      Clarity Application                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│   │    Agent     │───▶│ ToolRegistry │───▶│ Builtin Tools│     │
│   │              │◀───│              │◀───│              │     │
│   └──────────────┘    └──────┬───────┘    └──────────────┘     │
│                              │                                   │
│                              │    ┌──────────────┐              │
│                              └───▶│McpToolAdapter│              │
│                                   └──────┬───────┘              │
│                                          │                       │
│                    ┌─────────────────────┼──────────────────┐  │
│                    │        MCP Layer    │                   │  │
│                    │                     ▼                   │  │
│                    │  ┌─────────────────────────┐           │  │
│                    │  │       McpClient         │           │  │
│                    │  │  - JSON-RPC over stdio  │           │  │
│                    │  │  - Connection mgmt      │           │  │
│                    │  └───────────┬─────────────┘           │  │
│                    │              │                         │  │
│                    │  ┌───────────▼─────────────┐           │  │
│                    │  │      McpManager         │           │  │
│                    │  │  - Multiple connections │           │  │
│                    │  │  - Tool aggregation     │           │  │
│                    │  └─────────────────────────┘           │  │
│                    └─────────────────────────────────────────┘  │
│                              │                                   │
│                              │ JSON-RPC 2.0 / NDJSON            │
│                              ▼                                   │
│   ┌──────────────────────────────────────────────────────────┐  │
│   │              MCP Server (External Process)                │  │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │  │
│   │  │   Tools     │  │   Resources │  │   Prompts   │       │  │
│   │  └─────────────┘  └─────────────┘  └─────────────┘       │  │
│   └──────────────────────────────────────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 核心组件

### 1. McpClient

单个 MCP 服务器连接的管理器。

```rust
use clarity_core::mcp::McpClient;

// 通过 stdio 连接
let client = McpClient::connect_stdio(
    "npx",
    &["-y", "@modelcontextprotocol/server-filesystem", "/path/to/dir"]
).await?;

// 检查连接状态
if client.is_connected().await {
    println!("Connected!");
}

// 获取可用工具
let tools = client.list_tools().await?;
for tool in &tools {
    println!("{}: {}", tool.name, tool.description);
}

// 调用工具
let result = client.call_tool("read_file", json!({
    "path": "/path/to/file.txt"
})).await?;

// 断开连接
client.disconnect().await?;
```

### 2. McpToolAdapter

将 MCP 工具适配为 Clarity 的 `Tool` trait。

```rust
use clarity_core::mcp::{McpClient, McpToolAdapter};
use clarity_core::tools::Tool;

// 获取工具定义
let mcp_tool = client.list_tools().await?.remove(0);

// 创建适配器
let adapter = McpToolAdapter::new(client.clone(), mcp_tool);

// 像普通工具一样使用
println!("Name: {}", adapter.name());
println!("Description: {}", adapter.description());
println!("Parameters: {}", adapter.parameters());

// 执行
let result = adapter.execute(json!({"path": "/tmp/test"}), ctx).await?;
```

### 3. McpManager

管理多个 MCP 连接。

```rust
use clarity_core::mcp::McpManager;

let manager = McpManager::new();

// 添加连接
manager.connect_stdio(
    "filesystem",
    "npx",
    &["-y", "@modelcontextprotocol/server-filesystem", "."]
).await?;

manager.connect_stdio(
    "github",
    "npx", 
    &["-y", "@modelcontextprotocol/server-github"]
).await?;

// 列出连接
let clients = manager.list_clients().await;
println!("Connected: {:?}", clients); // ["filesystem", "github"]

// 获取所有工具
let all_tools = manager.get_all_tools().await;

// 获取特定客户端
if let Some(client) = manager.get_client("filesystem").await {
    // 使用 client...
}

// 断开所有连接
manager.disconnect_all().await?;
```

---

## 使用指南

### 场景 1：单一 MCP Server

```rust
use clarity_core::{Agent, ToolRegistry};
use clarity_core::mcp::{McpClient, McpToolAdapter};

async fn single_server_example() -> anyhow::Result<()> {
    // 1. 创建 registry
    let registry = ToolRegistry::with_builtin_tools();
    
    // 2. 连接 MCP server
    let client = McpClient::connect_stdio(
        "npx",
        &["-y", "@modelcontextprotocol/server-filesystem", "."]
    ).await?;
    
    // 3. 注册 MCP 工具
    for tool in client.list_tools().await? {
        let adapter = McpToolAdapter::new(client.clone(), tool);
        registry.register(adapter)?;
    }
    
    // 4. 创建 agent
    let agent = Agent::new(registry);
    
    // 5. 运行 - Agent 可以使用所有工具！
    agent.run("List all files in the current directory").await?;
    
    Ok(())
}
```

### 场景 2：多个 MCP Servers

```rust
use clarity_core::mcp::McpManager;

async fn multi_server_example() -> anyhow::Result<()> {
    let registry = ToolRegistry::with_builtin_tools();
    let manager = McpManager::new();
    
    // 连接多个 servers
    let servers = vec![
        ("filesystem", "npx", vec!["-y", "@modelcontextprotocol/server-filesystem", "."]),
        ("github", "npx", vec!["-y", "@modelcontextprotocol/server-github"]),
        ("sqlite", "npx", vec!["-y", "@modelcontextprotocol/server-sqlite", "/path/to/db.sqlite"]),
    ];
    
    for (name, cmd, args) in servers {
        if let Err(e) = manager.connect_stdio(name, cmd, &args).await {
            eprintln!("Failed to connect {}: {}", name, e);
        }
    }
    
    // 注册所有 tools
    for adapter in manager.get_all_tools().await {
        if let Err(e) = registry.register(adapter) {
            eprintln!("Failed to register tool: {}", e);
        }
    }
    
    println!("Total tools: {}", registry.len()?);
    
    // 使用 registry 创建 agent...
    
    Ok(())
}
```

### 场景 3：动态工具发现

```rust
use clarity_core::Agent;

async fn dynamic_discovery_example(agent: &Agent) -> anyhow::Result<()> {
    // 让 LLM 知道有哪些工具可用
    let schemas = agent.registry().get_tool_schemas()?;
    
    println!("Available tools:");
    for schema in schemas.as_array().unwrap() {
        let func = schema.get("function").unwrap();
        println!("  - {}", func.get("name").unwrap().as_str().unwrap());
    }
    
    // LLM 现在可以动态选择合适的工具
    Ok(())
}
```

---

## 工具调用流程

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Tool Call Sequence                                │
└─────────────────────────────────────────────────────────────────────────┘

  User                    Agent                  ToolRegistry
    │                        │                         │
    │  "List files"          │                         │
    │───────────────────────▶│                         │
    │                        │                         │
    │                        │  Get tool schemas       │
    │                        │────────────────────────▶│
    │                        │◀────────────────────────│
    │                        │  [web_search, bash,      │
    │                        │   list_directory, ...]   │
    │                        │                         │
    │                        │  Call LLM with schemas  │
    │                        │  and user request       │
    │                        │──────────────────────┐  │
    │                        │                      │  │
    │                        │◀─────────────────────┘  │
    │                        │  LLM decides:           │
    │                        │  call "list_directory"  │
    │                        │  with args {path: "."}  │
    │                        │                         │
    │                        │  Execute tool           │
    │                        │────────────────────────▶│
    │                        │                         │
    │                        │                         │  Lookup tool
    │                        │                         │  by name
    │                        │                         │     │
    │                        │                         │     ▼
    │                        │                         │  ┌─────────────┐
    │                        │                         │  │ Is builtin? │
    │                        │                         │  └──────┬──────┘
    │                        │                         │         │
    │                        │                         │    Yes  │   No
    │                        │                         │         │
    │                        │                         │         ▼
    │                        │                         │  ┌─────────────┐
    │                        │                         │  │McpToolAdapter
    │                        │                         │  └──────┬──────┘
    │                        │                         │         │
    │                        │                         │         ▼
    │                        │                         │  JSON-RPC call
    │                        │                         │  to MCP server
    │                        │                         │         │
    │                        │                         │◀────────┘
    │                        │                         │  Return result
    │                        │                         │
    │                        │◀────────────────────────│
    │                        │  Result: {...}          │
    │                        │                         │
    │  "Here are the files"  │                         │
    │◀───────────────────────│                         │
    │                        │                         │
```

---

## 错误处理

### 常见错误类型

```rust
use clarity_core::error::AgentError;

match result {
    Err(AgentError::ToolExecutionFailed(tool, msg)) => {
        // MCP tool returned an error
        eprintln!("Tool '{}' failed: {}", tool, msg);
    }
    Err(AgentError::Registry(msg)) => {
        // Connection or protocol error
        eprintln!("MCP error: {}", msg);
    }
    Err(AgentError::Tool(ToolError::Timeout(secs))) => {
        // Request timeout
        eprintln!("Request timed out after {} seconds", secs);
    }
    _ => {}
}
```

### 最佳实践

1. **总是处理连接失败**

```rust
match McpClient::connect_stdio("npx", &args).await {
    Ok(client) => client,
    Err(e) => {
        eprintln!("Warning: Could not connect to MCP server: {}", e);
        eprintln!("Continuing without MCP tools...");
        // Continue with builtin tools only
        return Ok(());
    }
}
```

2. **实现重试逻辑**

```rust
async fn connect_with_retry(cmd: &str, args: &[&str], max_retries: u32) -> Result<McpClient, AgentError> {
    let mut last_error = None;
    
    for attempt in 0..max_retries {
        match McpClient::connect_stdio(cmd, args).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                last_error = Some(e);
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
            }
        }
    }
    
    Err(last_error.unwrap())
}
```

3. **优雅降级**

```rust
// Try MCP first, fallback to builtin
let search_tool: Box<dyn Tool> = if let Ok(client) = connect_search_server().await {
    Box::new(McpToolAdapter::new(client, search_tool_def))
} else {
    Box::new(WebSearchTool::new()) // Fallback to builtin
};
```

---

## 配置示例

### 配置文件 (mcp.json)

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/projects"],
      "env": {}
    },
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "${GITHUB_TOKEN}"
      }
    },
    "sqlite": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-sqlite", "/path/to/data.db"]
    }
  }
}
```

### 加载配置

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct McpConfig {
    #[serde(rename = "mcpServers")]
    servers: HashMap<String, ServerConfig>,
}

#[derive(Deserialize)]
struct ServerConfig {
    command: String,
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

async fn load_mcp_config(path: &str) -> anyhow::Result<()> {
    let config: McpConfig = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let manager = McpManager::new();
    
    for (name, server) in config.servers {
        let env: Vec<(&str, &str)> = server
            .env
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        
        manager.connect_stdio(&name, &server.command, &server.args)
            .await?;
    }
    
    Ok(())
}
```

---

## 故障排除

### 问题 1：npx 命令未找到

```
Error: Failed to spawn MCP server: 系统找不到指定的文件。
```

**解决方案：**

```bash
# 安装 Node.js
# Windows: 从 https://nodejs.org/ 下载安装程序
# 或使用 winget
winget install OpenJS.NodeJS

# 验证安装
node --version
npx --version
```

### 问题 2：MCP Server 启动超时

```
Error: MCP request timed out
```

**解决方案：**

```rust
// 增加超时时间
let client = McpClient::connect_stdio("npx", &args)
    .await
    .map_err(|e| {
        if e.to_string().contains("timed out") {
            eprintln!("Server took too long to start. Try:");
            eprintln!("  1. Check your internet connection");
            eprintln!("  2. Run 'npx @modelcontextprotocol/server-filesystem' manually first");
        }
        e
    })?;
```

### 问题 3：权限被拒绝

```
Error: Tool 'read_file' execution failed: EACCES: permission denied
```

**解决方案：**

- 检查 MCP server 的 allowed_paths 配置
- 确保 Clarity 有权限访问指定目录

### 问题 4：JSON-RPC 解析错误

```
Warning: Failed to parse MCP response: ...
```

**解决方案：**

- 检查 MCP server 版本兼容性
- 查看 server 的 stderr 输出以获取详细错误信息

```rust
// 启用 debug logging
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

---

## 参考资源

- [Model Context Protocol 官方文档](https://modelcontextprotocol.io/)
- [MCP Servers GitHub](https://github.com/modelcontextprotocol/servers)
- [JSON-RPC 2.0 规范](https://www.jsonrpc.org/specification)

---

## 附录：支持的 MCP Servers

### 官方 Servers

| Server | 安装命令 | 用途 |
|--------|----------|------|
| filesystem | `npx -y @modelcontextprotocol/server-filesystem` | 文件操作 |
| github | `npx -y @modelcontextprotocol/server-github` | GitHub API |
| git | `npx -y @modelcontextprotocol/server-git` | Git 操作 |
| postgres | `npx -y @modelcontextprotocol/server-postgres` | PostgreSQL |
| sqlite | `npx -y @modelcontextprotocol/server-sqlite` | SQLite |
| puppeteer | `npx -y @modelcontextprotocol/server-puppeteer` | 浏览器自动化 |

### 社区 Servers

查看 [MCP Community Servers](https://github.com/modelcontextprotocol/servers/tree/main/src) 获取完整列表。
