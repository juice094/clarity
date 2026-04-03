# Project Clarity 架构文档

## 概述

Project Clarity 采用 **CCA (Clarity Component Architecture)** 三层架构设计：

1. **UI 层** - 用户交互界面
2. **核心层** - 业务逻辑与领域模型
3. **网关层** - 外部系统接入

## 架构图

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              用户层                                      │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                 │
│  │  Terminal   │    │   Web UI    │    │   API 客户端 │                 │
│  │   用户      │    │    用户     │    │             │                 │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘                 │
└─────────┼──────────────────┼──────────────────┼─────────────────────────┘
          │                  │                  │
          ▼                  ▼                  ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                              接入层                                      │
│  ┌─────────────────┐                      ┌──────────────────────────┐  │
│  │   clarity-tui   │                      │     clarity-gateway      │  │
│  │  ┌───────────┐  │                      │  ┌────────────────────┐  │  │
│  │  │  Ratatui  │  │                      │  │   Axum Web Server  │  │  │
│  │  │   界面    │  │                      │  │  ├─ REST API       │  │  │
│  │  ├───────────┤  │                      │  │  ├─ WebSocket      │  │  │
│  │  │  事件循环  │  │                      │  │  └─ Static Files   │  │  │
│  │  │  渲染器   │  │                      │  ├────────────────────┤  │  │
│  │  ├───────────┤  │                      │  │   Channel Router   │  │  │
│  │  │  状态管理  │  │                      │  │   (Session Mgmt)   │  │  │
│  │  └───────────┘  │                      │  └────────────────────┘  │  │
│  └────────┬────────┘                      └────────────┬─────────────┘  │
└───────────┼────────────────────────────────────────────┼────────────────┘
            │                                            │
            └──────────────────┬─────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                              核心层                                      │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                        clarity-core                             │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │   │
│  │  │  Tool System │  │  MCP Engine  │  │   Session Manager    │  │   │
│  │  │  ├─Registry  │  │  ├─Protocol  │  │   ├─State Machine    │  │   │
│  │  │  ├─Executor  │  │  ├─Transport │  │   ├─Context Tracking │  │   │
│  │  │  └─Schema    │  │  └─Handler   │  │   └─History Mgmt     │  │   │
│  │  └──────────────┘  └──────────────┘  └──────────────────────┘  │   │
│  │                                                                 │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │   │
│  │  │  Async Runtime│  │ Error Handler│  │    Event Bus         │  │   │
│  │  │  (Tokio)      │  │ (Anyhow/     │  │   (Pub/Sub)          │  │   │
│  │  │               │  │  thiserror)  │  │                      │  │   │
│  │  └──────────────┘  └──────────────┘  └──────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           外部系统层                                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │ MCP 服务A │  │ MCP 服务B │  │ MCP 服务C │  │ 文件系统  │  │ 其他工具  │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
```

## 三层架构详解

### 第一层：UI 层 (clarity-tui)

**职责**：
- 提供终端用户界面
- 处理用户输入和键盘事件
- 管理界面状态和渲染

**关键组件**：
- `App` - 应用主循环
- `ui()` - 界面渲染函数
- `widgets/` - 可复用 UI 组件
- `events/` - 事件处理系统

**依赖关系**：
```
clarity-tui → clarity-core
            → ratatui, crossterm
```

### 第二层：核心层 (clarity-core)

**职责**：
- 定义领域模型和 traits
- 实现工具注册和执行
- MCP 协议处理
- 会话生命周期管理

**关键模块**：

#### 1. Tool System
- `Tool` trait - 工具接口定义
- `ToolRegistry` - 工具注册中心
- `ToolExecutor` - 工具执行引擎
- 内置工具：`ReadFile`, `WriteFile`, `Grep`, `Shell`

#### 2. MCP Engine
- `McpClient` - MCP 客户端
- `McpTransport` - 传输层抽象
- `McpHandler` - 消息处理器
- 支持 SSE/Stdio 传输

#### 3. Session Manager
- `Session` - 会话状态
- `Context` - 执行上下文
- `History` - 执行历史记录

### 第三层：网关层 (clarity-gateway)

**职责**：
- 提供 HTTP/WebSocket API
- 管理客户端连接
- 路由请求到核心层

**关键组件**：
- `Router` - Axum 路由
- `WebSocketHandler` - WebSocket 处理
- `ChannelManager` - 会话频道管理
- `StaticServer` - 静态资源服务

## 数据流

### 1. 工具调用流

```
用户输入 → TUI/Gateway → ToolRegistry → ToolExecutor → 工具执行 → 结果返回
```

### 2. MCP 请求流

```
AI 请求 → MCP Client → Transport → MCP Server → 结果解析 → 返回 AI
```

### 3. WebSocket 消息流

```
浏览器 → Gateway → Channel Router → Session → Core Engine → 响应 → 浏览器
```

## Crate 职责划分

| 职责 | clarity-core | clarity-tui | clarity-gateway |
|------|-------------|-------------|-----------------|
| 领域模型 | ✅ | ❌ | ❌ |
| 工具执行 | ✅ | ❌ | ❌ |
| MCP 协议 | ✅ | ❌ | ❌ |
| 终端渲染 | ❌ | ✅ | ❌ |
| 事件处理 | ❌ | ✅ | ❌ |
| HTTP API | ❌ | ❌ | ✅ |
| WebSocket | ❌ | ❌ | ✅ |
| 静态资源 | ❌ | ❌ | ✅ |

## 扩展点

### 1. 添加新工具

在 `clarity-core/src/tools/` 中：

```rust
#[derive(Debug)]
pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "描述" }
    fn schema(&self) -> Value { /* JSON Schema */ }
    
    async fn execute(&self, input: Value) -> Result<ToolOutput> {
        // 实现逻辑
    }
}
```

### 2. 添加 MCP 传输

实现 `McpTransport` trait：

```rust
#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn connect(&self) -> Result<()>;
    async fn send(&self, message: McpMessage) -> Result<()>;
    async fn receive(&self) -> Result<McpMessage>;
}
```

### 3. 添加 TUI 组件

在 `clarity-tui/src/widgets/` 中：

```rust
pub fn render_my_widget(f: &mut Frame, area: Rect, state: &MyState) {
    // 渲染逻辑
}
```

## 技术栈

- **异步运行时**: Tokio
- **序列化**: Serde + Serde_json
- **错误处理**: Anyhow + Thiserror
- **日志**: Tracing + Tracing-subscriber
- **TUI**: Ratatui + Crossterm
- **Web**: Axum + Tower-http
- **MCP**: rmcp (Rust MCP SDK)

## 性能考虑

1. **并发**: 充分利用 Tokio 的异步特性
2. **内存**: 零拷贝设计，避免不必要的克隆
3. **缓存**: 工具结果缓存，会话状态复用
4. **连接池**: MCP 连接池管理

## 安全考虑

1. **沙箱**: 工具执行环境隔离
2. **权限**: 基于会话的权限控制
3. **审计**: 完整的操作日志
4. **限流**: API 速率限制
