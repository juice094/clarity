# clarity-gateway

Clarity HTTP Gateway：提供 REST API、Web UI、WebSocket 实时通信与会话持久化。

## 职责

- **双端口服务** — API 端口（18790）对外提供能力，Admin 端口（18800）仅限本地回环
- **REST API** — OpenAI 兼容的 `/v1/chat/completions`，以及任务、文件、配置等管理接口
- **Web UI** — 内嵌静态页面（chat.html / index.html），编译时打包无需运行时依赖
- **WebSocket** — `/ws` 端点提供实时消息推送
- **会话管理** — 基于 SQLite 的持久化会话存储，支持历史加载与自动清理
- **后台任务** — 通过 `/v1/tasks` 创建、查询、取消独立 Agent 任务
- **并行子代理** — `/v1/parallel` 支持 HTTP 层级的多子代理并发执行
- **跨域支持** — 内置 CORS，支持本地前端开发（localhost:3000 / 5173）

## 关键类型

- `AppState` — 共享应用状态，包含 Agent、会话存储、任务管理器与活动日志
- `server::run` — 启动双端口服务器的入口函数
- `create_api_router` / `create_admin_router` — API 与 Admin 路由构造器
- `PersistentSessionStore` — SQLite 持久化会话存储
- `handlers::*` — 各 API 端点的 Axum handler 集合

## 测试

```bash
cargo test -p clarity-gateway --lib
```
