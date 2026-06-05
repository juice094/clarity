//! # Clarity Gateway
//!
//! HTTP Gateway for Project Clarity — 提供 REST API、Web UI、WebSocket 实时通信
//! 与会话持久化的双端口服务器。
//!
//! ## 架构
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐
//! │   API Server    │     │  Admin Server   │
//! │   0.0.0.0:18790 │     │ 127.0.0.1:18800 │
//! └────────┬────────┘     └────────┬────────┘
//!          │                       │
//!          ▼                       ▼
//! ┌──────────────────────────────────────┐
//! │           Axum Router                │
//! │  /v1/chat/completions  (OpenAI 兼容) │
//! │  /v1/tasks             (后台任务 CRUD)│
//! │  /v1/parallel          (并行子代理)   │
//! │  /ws                   (WebSocket)   │
//! │  /api/files/*          (文件操作)     │
//! │  /api/*                (Admin 管理)   │
//! └──────────────────────────────────────┘
//!          │
//!          ▼
//! ┌──────────────────────────────────────┐
//! │           AppState (Arc)             │
//! │  ├─ Agent (RwLock)                   │
//! │  ├─ PersistentSessionStore (SQLite)  │
//! │  ├─ BackgroundTaskManager            │
//! │  └─ ActivityLogger                   │
//! └──────────────────────────────────────┘
//! ```
//!
//! ## 主要模块
//!
//! - `server` — 双端口服务器启动、`AppState`、路由构造器 (`create_api_router` / `create_admin_router`)
//! - `handlers` — 所有 Axum handler，包括聊天补全、任务管理、Admin 配置、文件操作
//! - `session_store` — SQLite 持久化会话存储，支持历史加载、追加与过期清理
//! - `ws` — WebSocket handler，提供实时消息推送
//! - `channels` — 第三方通道集成（Discord、Telegram、Webhook）
//! - `session` — 会话 ID 与入口类型定义
//!
//! ## API 路由概览
//!
//! ### 公共 API (`:18790`)
//! | 方法 | 路径 | 说明 |
//! |------|------|------|
//! | GET  | `/health` | 健康检查 |
//! | POST | `/v1/chat/completions` | OpenAI 兼容的流式/非流式聊天 |
//! | GET/POST | `/v1/tasks` | 列出 / 创建后台任务 |
//! | GET/DELETE | `/v1/tasks/:id` | 查询 / 取消任务 |
//! | POST | `/v1/parallel` | 并行子代理执行 |
//! | GET  | `/api/files/*` | 文件树、读取、写入、Glob |
//! | GET  | `/ws` | WebSocket 连接 |
//!
//! ### Admin API (`:18800`，本地回环)
//! | 方法 | 路径 | 说明 |
//! |------|------|------|
//! | GET  | `/api/stats` | 运行统计 |
//! | GET  | `/api/tools` | 工具列表 |
//! | GET  | `/api/models` | 可用模型 |
//! | GET/POST | `/api/provider` | 切换 Provider |
//! | GET/POST | `/api/approval-mode` | 审批模式管理 |
//! | GET/POST | `/api/config` | 配置读写 |
//! | GET/DELETE | `/api/sessions/:id` | 会话管理 |

pub mod channels;
pub mod handlers;
pub mod health;
pub mod server;
pub mod session;
pub mod session_store;
pub mod ws;
