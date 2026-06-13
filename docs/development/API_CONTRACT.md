---
title: Clarity Gateway API 契约
category: Contract
date: 2026-05-16
tags: [contract]
---

# Clarity Gateway API 契约

> 版本：v0.3.1+ | 关联：[`ARCHITECTURE.md`](../architecture/ARCHITECTURE.md) · [`THREAT_MODEL.md`](../security/THREAT_MODEL.md)

---

## 1. 服务端口

| 端口 | 用途 | 绑定地址 | 认证 |
|------|------|---------|------|
| `18790` | 公开 API + Web UI | `0.0.0.0` | 无（公开端点）或 Session Cookie |
| `18800` | Admin API | `127.0.0.1` | Bearer Token（`CLARITY_GATEWAY_ADMIN_TOKEN`） |

---

## 2. HTTP API 端点

### 2.1 健康检查

```http
GET /health
```

**响应**：`200 OK` + JSON `{ "status": "ok" }`

### 2.2 OpenAI 兼容聊天补全

```http
POST /v1/chat/completions
Content-Type: application/json

{
  "model": "gpt-4",
  "messages": [{"role": "user", "content": "Hello"}],
  "stream": true
}
```

**响应**：SSE stream (`text/event-stream`)
- `data: {"choices":[{"delta":{"content":"..."}}]}`
- `data: [DONE]`

### 2.3 后台任务

```http
POST /v1/tasks
GET /v1/tasks
GET /v1/tasks/:id
DELETE /v1/tasks/:id
```

### 2.4 并行子代理

```http
POST /v1/parallel
GET /v1/parallel/:id
```

### 2.5 文件操作

```http
GET /api/files/tree?path=<dir>
GET /api/files/read?path=<file>
POST /api/files/write
GET /api/files/glob?pattern=<glob>
```

### 2.6 Provider 切换

```http
POST /api/provider
Authorization: Bearer <admin-token>

{
  "provider": "openai",
  "model": "gpt-4"
}
```

### 2.7 MCP 服务器管理

```http
GET /api/mcp/servers
GET /api/mcp/servers/:name
POST /api/mcp/servers/:name
DELETE /api/mcp/servers/:name
```

### 2.8 Cron 任务

```http
GET /api/cron/tasks
POST /api/cron/tasks
DELETE /api/cron/tasks/:id
```

### 2.9 记忆搜索

```http
POST /api/search

{
  "query": "Rust lifetime",
  "top_k": 5
}
```

---

## 3. WebSocket 协议

### 3.1 连接

```http
GET /ws
```

升级至 WebSocket，用于实时事件推送。

### 3.2 消息格式

```json
{
  "type": "event",
  "event": "agent.chunk",
  "payload": {
    "session_id": "...",
    "content": "..."
  }
}
```

### 3.3 事件类型

| 事件 | 方向 | 说明 |
|------|------|------|
| `agent.chunk` | Server → Client | SSE 流式内容片段 |
| `agent.done` | Server → Client | Agent 循环完成 |
| `agent.error` | Server → Client | 执行错误 |
| `tool.executed` | Server → Client | 工具执行结果 |
| `plan.step_begin` | Server → Client | 计划步骤开始 |
| `plan.step_end` | Server → Client | 计划步骤完成 |
| `compaction.begin` | Server → Client | 上下文压缩开始 |
| `compaction.end` | Server → Client | 上下文压缩完成 |
| `ping` | Bidirectional | 心跳 |

---

## 4. 认证

### 4.1 Admin 端口 (18800)

所有端点要求 HTTP Header：

```http
Authorization: Bearer <CLARITY_GATEWAY_ADMIN_TOKEN>
```

Token 不匹配返回 `401 Unauthorized`。

### 4.2 公开端口 (18790)

- `/health` — 无需认证
- `/v1/chat/completions` — 无需认证（由 core 内部管理 API Key）
- 其余端点 — 无强制认证（v0.3.x 为本地/内网场景设计）

---

## 5. 错误响应

统一 JSON 格式：

```json
{
  "error": {
    "code": "path_traversal",
    "message": "Path escapes working directory"
  }
}
```

| HTTP 状态 | 错误码 | 说明 |
|-----------|--------|------|
| `400` | `invalid_request` | 请求参数错误 |
| `401` | `unauthorized` | Admin Token 无效或缺失 |
| `403` | `path_traversal` | 文件路径超出工作目录 |
| `404` | `not_found` | 资源不存在 |
| `500` | `internal_error` | 服务器内部错误 |
| `502` | `provider_error` | LLM Provider 请求失败 |
| `503` | `service_unavailable` | Gateway 尚未就绪 |

---

## 6. 静态文件服务

公开端口同时提供前端静态文件：

| 路径 | 文件 |
|------|------|
| `/` | `chat.html` |
| `/index.html` | `index.html`（Admin 面板） |

Admin 端口提供：

| 路径 | 文件 |
|------|------|
| `/` | `index.html` |

---

## 7. 版本与兼容性

- **当前版本**：v0.3.1
- **OpenAI 兼容层**：`/v1/chat/completions` 遵循 OpenAI Chat Completions API 格式（流式 SSE）
- **破坏性变更预期**：v0.4.0 前 Admin API 可能调整；WebSocket 事件格式保持向后兼容
