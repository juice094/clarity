---
title: Clarity V2 线程与迁移
category: Development
date: 2026-06-15
tags: [threads, rollout, migration, headless]
---

# Clarity V2 线程与迁移

Clarity v0.3.x 引入了 **Thread / Rollout** 作为新的持久化对话模型，逐步替代旧的 V1 `sessions.db` 方案。

---

## 1. 核心概念

| 概念 | 说明 |
|------|------|
| **Thread** | 一个持久对话单元，拥有唯一 UUID（`thread_id`）。 |
| **Rollout** | 线程的追加式 JSONL 事件日志，位于 `.clarity/sessions/rollout-<thread_id>.jsonl`。 |
| **State DB** | SQLite 元数据索引，位于 `.clarity/state.db`，用于快速列出、搜索线程。 |
| **Session** | 在线程模型中，`session_id` 通常与根 `thread_id` 相同。 |

---

## 2. 存储布局

```text
.clarity/
├── state.db                     # 线程元数据索引
└── sessions/
    ├── rollout-<thread_id>.jsonl  # 线程事件日志
    └── ...
```

---

## 3. Gateway API

详见 [`API_CONTRACT.md`](./API_CONTRACT.md#210-v2-线程threads--rollouts)。

常用端点：

```http
POST /api/v2/threads
GET  /api/v2/threads
GET  /api/v2/threads/:id?include_history=true
POST /api/v2/threads/:id/chat          # SSE 流式对话
POST /api/v2/threads/:id/fork
```

---

## 4. Headless CLI

`clarity-headless` 提供线程管理子命令：

```bash
# 列出最近 20 个线程
clarity-headless threads list

# 创建线程
clarity-headless threads create --title "My chat"

# 查看线程（含历史）
clarity-headless threads show <thread_id> --history

# 继续对话
clarity-headless threads resume <thread_id> --prompt "Explain this"

# Fork 线程
clarity-headless threads fork <thread_id>
clarity-headless threads fork <thread_id> --before-user 3

# 归档 / 删除
clarity-headless threads archive <thread_id>
clarity-headless threads delete <thread_id>

# 使用非默认的 Clarity home
clarity-headless threads --clarity-home /path/to/home list
```

### 4.1 迁移旧会话

启用 `session-migration` feature：

```bash
cargo run -p clarity-headless --features session-migration -- \
  threads migrate .clarity/sessions.db --clarity-home .clarity
```

该命令会读取 V1 `sessions.db`，为每个旧 session 创建一个 V2 线程，并把消息写入对应的 rollout 文件。V1 数据库不会被删除，可反复尝试。

---

## 5. 托盘集成（Claw）

Clarity Claw 系统托盘会轮询 `/api/v2/threads`，在右键菜单中显示 **Recent Threads**。点击条目会打开 `chat.html?thread_id=...`；选择 **New Chat** 会创建新线程并打开。

---

## 6. 从 V1 迁移到 V2

### 6.1 何时需要迁移

- 你之前运行过 Clarity v0.2.x，存在 `.clarity/sessions.db`。
- 你想在 v0.3.x 的 Web UI / Headless CLI / Claw 中继续访问旧对话。

### 6.2 迁移步骤

1. 备份 `.clarity` 目录。
2. 使用 Headless 迁移工具：

   ```bash
   cargo run -p clarity-headless --features session-migration -- \
     threads migrate .clarity/sessions.db
   ```

3. 检查输出中的 `sessions_migrated` 和 `errors`。
4. 启动 Gateway，访问 `chat.html`，左侧线程列表应出现旧对话。

### 6.3 编程式迁移

```rust
use clarity_core::session::thread_migration::ThreadMigrator;
use clarity_thread_store::RolloutConfig;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RolloutConfig {
        clarity_home: PathBuf::from(".clarity"),
        sqlite_home: PathBuf::from(".clarity"),
        cwd: PathBuf::from("."),
        model_provider_id: "default".to_string(),
        generate_memories: false,
    };

    let migrator = ThreadMigrator::new(".clarity/sessions.db", config)?;
    let report = migrator.migrate().await?;
    println!("{:?}", report);
    Ok(())
}
```

---

## 7. 故障排查

| 现象 | 原因 | 解决 |
|------|------|------|
| 线程列表为空 | `state.db` 未初始化或线程创建失败 | 检查 Gateway / Headless 日志，确认 `.clarity` 目录可写。 |
| 迁移报错 `invalid session id` | V1 `session_id` 不是合法 UUID | 工具会自动跳过；如需保留，可手动映射 UUID。 |
| 旧历史加载不全 | rollout 中的 `Other` 类型消息不映射为 LLM 消息 | 这是预期行为，V1 消息以保留载荷形式存入 rollout。 |
