# Clarity Future Direction — Technical Roadmap v0.3.0 → v0.5.0

> **来源**：本文档由 Kimi plan 模式生成，plan 原文见 `docs/plans/2026-04-26-cluster-as-single-node.md`。
> 宿可在此基础上直接修改、裁剪或重新编排，plan 原文作为参考底稿保留。
>
> **定位声明**：Clarity 是集群协作原语的单机验证运行时（非本地聊天工具）。
> 先在本地验证分布式语义（Hub-Worker、Wire 消息边界、MCP 三传输、Background Tasks），验证通过后同一套原语可无损穿透到 Syncthing-Rust P2P 层。
>
> **主权防御**：学习 Kimi 生态（娘家/导师），不入赘。模型/数据/协议/人格四层主权不可让渡。
>
> **文档性质**：长期技术指导，回答"向何处去"的方向性问题。每个 Phase 需经宿确认后进入执行。

---

## 差距矩阵摘要（现状 vs 蓝图）

| 架构蓝图能力 | 当前现状 | 差距等级 |
|---|---|---|
| Hub-Worker 调度器 | `AgentController` 管理单一 Agent，一次一个 turn | 🔴 高 |
| Agent-to-Agent Wire 消息 | `WireMessage` 只有 agent→UI 生命周期事件 | 🔴 高 |
| IPC 传输（UDS/Named Pipe/TCP 回环） | `tokio::broadcast` 仅进程内 | 🔴 高 |
| Session Handoff | 无相关代码 | 🔴 高 |
| WebSocket MCP 传输 | `McpTransport` 只有 Stdio/Http/Sse | 🟡 中 |
| Gateway ↔ BackgroundTaskManager 打通 | `TaskRecord` JSON 与 `BackgroundTaskManager` 完全断开 | 🟡 中 |
| Worker 池自动扩缩容 | `ScalableWorkerPool` 字段带下划线（未使用） | 🟡 中 |
| 跨会话记忆检索 | `session_notes` 表存在，无跨会话联合查询 | 🟢 低 |
| 多窗口 Agent 隔离 | `AppState` 单例，`Agent::begin_turn()` 返回 `AlreadyRunning` | 🔴 高 |

---

## 核心约束

- **项目广度 ≤ 5 核心工具**：当前 6 crates 已达上限，Phase A-D 不新增 crate，只重构现有 crate
- **Rust 核心模块不可外包**：Hub-Worker、Wire 扩展、SessionManager 必须由直接代码实现
- **Hard Veto 生效**：禁止 Docker / RAG(Qdrant) / Electron / 分布式消息通道 / Mobile 适配
- **UI 技术栈方向**：egui 为未来主控探索方向，Tauri 冻结新功能开发（仅维护），Pretext 为优化项不入主路线图
- **每阶段验收**：`cargo test --workspace --lib` + `npm run build` 全绿

---

## Phase A：基础设施联通（2 周）

目标：低侵入性快速交付，为后续阶段铺垫，同时产生即时用户价值。

### A1. WebSocket MCP 传输（2-3 天）

- 在 `McpTransport` 枚举新增 `WebSocket { url, headers }` 变体
- 基于 `tokio-tungstenite` 实现 `McpClient` trait
- 将 `McpTransport` 从 closed enum 改为可扩展的 trait-based 注册表（可选，复杂则延至 Phase B）
- **验收**：连接到 WebSocket MCP Server 并成功执行 tool call

### A2. Gateway ↔ BackgroundTaskManager 集成（2-3 天）

> 注：原 Tauri ↔ BTM 集成因 Tauri 冻结调整至 Gateway 侧。若 egui 主控提前成熟，可迁移至 egui 事件总线。

- `clarity-gateway` WebSocket 事件流接入 `BackgroundTaskManager` 进度事件
- 用 `BackgroundTaskManager` API 替代前端独立的任务状态轮询
- **验收**：从 Gateway Web UI 创建后台任务，观察实时进度事件，确认持久化恢复

### A3. Worker 池自动扩缩容（2-3 天）

- `ScalableWorkerPool` 去除 `_min_workers` / `_max_workers` 下划线前缀
- 实现阈值触发扩容（队列长度 > threshold）、空闲触发缩容
- **验收**：模拟突发任务负载，观察 Worker 数量变化

### A4. 跨会话记忆检索（2-3 天）

- 扩展 `clarity-memory` SQLite `session_notes` 查询，支持跨会话全文检索
- 新增 API：`search_all_sessions(query, limit)`
- **验收**：创建 3 个 session，搜索关键词，确认返回所有 session 的结果

---

## Phase B：会话层统一（2-3 周）

目标：用 SQLite 单一事实来源替代当前两套不相交的 Session 存储系统，实现 Session Handoff。

### B1. 统一 Session Schema（3-4 天）

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    title TEXT,
    created_at INTEGER,
    updated_at INTEGER,
    parent_session_id TEXT,  -- Handoff 血缘
    handoff_document TEXT,   -- JSON 序列化
    state TEXT               -- active | archived | handoff_pending
);

CREATE TABLE session_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT,
    role TEXT,
    content TEXT,
    created_at INTEGER,
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);
```

- 迁移工具：读取现有 `sessions/{id}.json` 和 `sessions/{id}.jsonl`，导入 SQLite
- **验收**：迁移后所有现有 session 正确加载

### B2. SessionManager 抽象（3-4 天）

- 在 `clarity-core`（或扩展 `clarity-memory`）创建 `SessionManager`
- API：`create()`、`load()`、`save_message()`、`search()`、`handoff(source_id, target_id)`
- 替换 Tauri `session.rs` 的 JSON I/O 和 `clarity-memory` `SessionStore` 的 JSONL
- **验收**：功能与旧双系统完全对等

### B3. Session Handoff（3-5 天）

```rust
struct HandoffDocument {
    session_id: Uuid,
    target_session_id: Uuid,
    context_summary: String,
    decisions: Vec<Decision>,
    pending_tasks: Vec<Task>,
    agent_state: AgentStateSnapshot,
    soul_fingerprint: String,  // SOUL.md 哈希校验
    timestamp: DateTime<Utc>,
    ttl: Duration,
}
```

- 实现 `session_manager.handoff(source, target)`
- 新 session 创建时自动检测 pending handoff 并提示用户
- **验收**：Session #1 handoff → Session #2 加载上下文 + 决策记录

### B4. Session Event Bus（2-3 天）

- Session 变更时 emit Tauri 事件（`session:message_added`、`session:handoff_available`）
- 前端监听并响应式更新
- **验收**：打开两个 Settings panel，一侧创建 session，另一侧自动刷新

---

## Phase C：运行时重构 — Hub-Worker + 多窗口（4-6 周）

目标：将单 Agent 单进程假设重构为多 Agent Hub-Worker 调度器，以多窗口作为多节点验证场。

### C1. AgentInstance + AgentPool（5-7 天）

```rust
struct AgentInstance {
    id: Uuid,
    identity: Identity,
    agent: Agent,
    controller: AgentController,
    window_id: Option<String>,
}

struct AgentPool {
    instances: RwLock<HashMap<Uuid, AgentInstance>>,
    default_instance: Uuid,  // 存在论锚点
}
```

- `AgentPool` 按 identity / window_id 路由 `Op::UserTurn`
- 保持 `AgentController` 向后兼容 API，`AgentPool` 作为包装层
- **验收**：创建 2 个 AgentInstance，同时向两者发送 turn

### C2. Identity 路由（3-4 天）

- `Identity` 枚举：`Gray`、`Kimi`、`Analyst`、`Programmer`、`Auditor`、`Custom(String)`
- `ModelSpec`：`Local { model_id }` | `Remote { provider, model }` | `Hybrid { ... }`
- `AgentInstance` 创建时绑定 identity + model_spec
- Hub 路由策略：`ByTask`、`ByCapability`、`ByIdentity`、`GrayDirect`（硬编码优先路由）
- **验收**："代码审查"任务路由到 Programmer，"数据分析"路由到 Analyst

### C3. Wire 协议扩展 — 跨 Agent 消息（4-5 天）

```rust
AgentMessage {
    from: Uuid,
    to: Uuid,
    payload: MessagePayload,
}
AgentStateSnapshot { instance_id, state_json }
```

- 新增 `MessageEnvelope`（含路由元数据）
- **验收**：Instance A 向 Instance B 发送消息，B 接收并响应

### C4. IPC 传输层（4-6 天）

- `clarity-wire` 新增 `Transport::Ipc`
- 跨平台策略：TCP 127.0.0.1 为通用兜底，UDS（Linux/macOS）和 Named Pipe（Windows）为性能优化
- 关键约束：消息格式与跨网络 TCP 穿透完全一致 — 今天在回环验证，明天直接扩展到 P2P
- **验收**：两个进程通过 Wire over TCP 回环通信，消息边界正确

### C5. 多窗口状态模型（3-5 天）

- `AppState.agent` 替换为 `Arc<RwLock<AgentPool>>`
- 每个 Tauri window 分配 `window_id`，`AgentPool` 按 window 路由
- 后台任务进度通过 Tauri event bus 广播到所有窗口
- **验收**：打开 2 个 Tauri 窗口，各自独立聊天，后台任务进度双窗口可见

### C6. 存在论锚点硬绑定（2-3 天）

- `AgentPool::default_instance` 固定指向 `Identity::Gray`
- Gray 实例：强制本地 LLM、离线必须在场、启动时自动创建
- 启动时校验 SOUL.md 哈希，不匹配则告警
- **验收**：断开网络，Gray 实例仍可响应

---

## Phase D：跨设备验证 — Syncthing-Rust 集成（4-6 周）

目标：将单机验证通过的集群语义扩展到多设备。

### D1. 设备身份与发现（1 周）

- Syncthing-Rust 设备证书作为 Clarity 设备身份
- SQLite `devices` 表：（device_id, label, last_seen, trust_level）
- **验收**：两台 Clarity 实例在 LAN 内相互发现

### D2. Session CRDT 同步（2 周）

- 引入 CRDT 库（Loro Rust core）用于 session message 合并
- 冲突解决：metadata 用 last-writer-wins，messages 用 append-only
- 触发机制：Syncthing 文件 watcher 检测到远程 session 更新
- **验收**：Device A 在 Session #1 新增消息，Device B 在同步周期内可见

### D3. Agent 状态迁移（1-2 周）

- `AgentInstance` turn 级上下文序列化为可移植格式（非完整内存）
- 通过 Syncthing-Rust TLS 加密通道传输
- **验收**：Device A 开始 turn，迁移到 Device B，无缝继续

### D4. P2P Wire 协议（1 周）

- Wire 新增 `Transport::P2P` 变体，复用 Syncthing-Rust TLS 通道
- 设备间 AgentMessage 路由
- **验收**：Device A 的 Agent α 向 Device B 的 Agent β 发送消息

---

## 技术选型与权衡

| 决策 | 选择 | 理由 |
|---|---|---|
| IPC 主传输 | TCP 127.0.0.1 | 跨平台通用，无平台特定代码；UDS/Named Pipe 后续优化 |
| Session 统一存储 | SQLite 单一事实来源 | 替代 JSON+JSONL 二元性；FTS5 搜索；WAL 并发 |
| CRDT 库 | Loro（Rust core） | 成熟、delta sync、WASM-ready |
| AgentPool 并发 | `tokio::sync::RwLock<HashMap>` | 单机足够简单；分布式锁延至 Phase D |
| 多进程 vs 多线程 | 多线程优先 | Tauri 后端本就是单进程多线程；多进程（IPC）作为扩展验证 |

---

## 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| Phase C 重构破坏现有 Tauri/Gateway/TUI | 高 | 保持 `AgentController` 向后兼容 API；`AgentPool` 作为包装层 |
| Session 迁移丢失数据 | 高 | 迁移工具支持 dry-run + 备份；删除 JSON 前跑完整验证套件 |
| 项目广度超限 | 高 | Phase A-D 不新增 crate，只重构现有 6 crate |
| BackgroundTaskManager 集成 destabilize | 中 | Feature-flag 集成；出错时 fallback 到旧 task 系统 |
| CRDT 同步性能差 | 中 | 早期 benchmark；若开销 > 50ms 则 fallback 到 simple last-writer-wins |

---

## 验收标准

| Phase | 标准 |
|---|---|
| A | 4 项 quick-win 全部合并，`cargo test --workspace --lib` 零回归 |
| B | 单一 `SessionManager` API；所有 session 进 SQLite；Handoff 可用 |
| C | 2+ AgentInstance 并发运行；多窗口聊天正常；IPC 回环验证通过 |
| D | 两台 Clarity 设备 session 同步；Agent 状态跨设备迁移 |

---

## 版本对应

| 阶段 | 目标版本 | 时间线 |
|---|---|---|
| Phase A | v0.3.1 / v0.3.2 | 可与 patch release 并行 |
| Phase B | v0.3.3 / v0.3.4 | 可与 patch release 并行 |
| Phase C | v0.4.0 | 需集中开发窗口 |
| Phase D | v0.5.0 | 需集中开发窗口 |

---

*本文档由工程师视角的技术差距分析生成，经 plan 模式审批后固化。后续变更需同步更新。*
