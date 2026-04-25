# Phase 2-A: RwLock 迁移审计报告

> 日期：2026-04-23 | 审计范围：clarity-core 全部 `std::sync::(Mutex|RwLock)`

---

## 发现清单

| # | 位置 | 类型 | 用途 | 跨越 await? | 风险 |
|---|------|------|------|-------------|------|
| 1 | `agent/mod.rs:175` | `std::sync::RwLock` | `AgentInner` 运行时状态 | ❌ 否 | 低 |
| 2 | `registry.rs:35` | `std::sync::RwLock` | `ToolRegistry` 工具映射 | ❌ 否 | 低 |
| 3 | `background/mod.rs:85` | `StdRwLock<u64>` | 任务序列号生成 | ❌ 否 | 低 |
| 4 | `background/mod.rs:81` | `StdMutex<BinaryHeap>` | 任务优先级队列 | ❌ 否 | 低 |
| 5 | `background/worker.rs:75` | `std::sync::Mutex` | Worker join handles | ❌ 否 | 低 |
| 6 | `background/worker.rs:77` | `std::sync::Mutex` | Shutdown sender | ❌ 否 | 低 |

---

## 详细分析

### 1. `Agent.inner` (RwLock)
- **调用点**: `construct.rs` 中 18 处 `.read().unwrap()` / `.write().unwrap()`
- **上下文**: 全部为同步 setter/getter 方法（如 `set_approval_mode()`, `active_skill()`）
- **await 跨越**: 无。锁在方法内获取、使用、立即释放。

### 2. `ToolRegistry.tools` (RwLock)
- **调用点**: `registry.rs` 中 8 处 `.read()` / `.write()`
- **上下文**: 同步方法（`register()`, `get()`, `list_tools()`, `get_tool_schemas()`）
- **注意**: `execute()` 是 async，但它调用同步的 `self.get()`，锁不跨越 await。

### 3-4. `BackgroundTaskManager.sequence/queue`
- **调用点**: `schedule()` 和 `load_pending()` async 函数
- **锁模式**: `let seq = { let mut seq = self.sequence.write().unwrap(); ... };`
- **await 跨越**: 无。锁在块作用域内，释放后才进入 await。

### 5-6. `WorkerPool.handles/shutdown_tx`
- **调用点**: `shutdown()` async 函数
- **锁模式**: `let tx = self.shutdown_tx.lock().unwrap().take();` — 获取后立刻通过 `take()` 转移所有权，锁随即释放
- **await 跨越**: 无。

---

## 结论

**当前零 correctness bug。** 所有 `std::sync::RwLock` / `std::sync::Mutex` 的使用都没有在持有锁期间跨越 await 点。

**迁移价值**: 从 `std::sync::RwLock` 迁移到 `tokio::sync::RwLock` 属于**防御性重构**——防止未来代码变更时无意中跨越 await 导致阻塞 executor。

**迁移工作量**: 中等（~6 个结构体定义 + ~30 处调用点需从 `.unwrap()` 改为 `.await`）。

**推荐优先级**: P2（可做，但无紧急性）。
