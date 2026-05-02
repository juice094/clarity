# 架构解耦与代码健康计划 — Clarity Sprint 14.5

> 目标：清偿 Sprint 13 遗留的 3 项架构设计债 + 清理 AGENTS.md 过时代码，建立零债务基线。
> 当前分支：`phase2/protocol-pilot` @ `01990446`
> 预计工期：3–5 天（取决于选项）

---

## 一、问题诊断

### 问题 1：Op 枚举耦合（`clarity-core` ↔ `clarity-gateway`）

**现状**：`Op` 枚举（`crates/clarity-core/src/agent/ops.rs:7`）定义了 7 个变体，其中 `ConversationTurn` 和 `ConversationTurnSync` 是 Gateway 特有的——它们携带 OpenAI 风格的消息历史。TUI 和 stdio 服务器只使用 `UserTurn`/`Interrupt`/`Compact`/`Shutdown`。

**根因**：Gateway 的 OpenAI 兼容层需要预构建消息列表，但 `AgentController` 在 core 中，导致 Gateway 的需求通过扩展 `Op` 枚举反溯到 core。

**已有资产**：`ChatDriver` trait 已存在于 `crates/clarity-core/src/agent/driver.rs:10-59`，目的正是避免 `Op` 枚举膨胀，但**零引用**（完全未使用）。

### 问题 2：Agent Streaming 双入口（`clarity-core`）

**现状**：
- `run_streaming`（`run.rs:433`）：从 query + memory 构建消息，然后进入 loop
- `run_streaming_with_messages`（`run.rs:511`）：接收预构建消息，进入同一个 loop

**根因**：两者共享 `run_streaming_loop`，但前端的 setup（skill 发现、LLM fetch、tool schema、TurnBegin wire）和后端的 teardown（TurnEnd、Usage、memory、ticker）完全重复。`run()`/`run_with_messages_sync()` 已通过提取 `run_sync_loop()` 解决了同样的问题，streaming 路径未跟进。

**额外发现**：streaming loop 缺少 `PreDeliveryHook` 和 `SessionTerminationHook`（sync 路径通过返回状态由调用方处理，streaming 路径直接内联）。

### 问题 3：AppState 冗余（`clarity-egui` + `clarity-gateway`）

**现状**：AGENTS.md 对 AppState 的描述已过时（提及的 `tool_registry` 和 `session_manager` 属于已归档的 `clarity-tauri`）。但新的冗余被发现：

| 位置 | 冗余 | 影响 |
|------|------|------|
| `clarity-egui` AppState | `approval_runtime` + `mode_aware_approval_runtime` 并存 | `mode_aware.inner()` 已可替代 |
| `clarity-egui` AppState | `initialized: AtomicBool` | `#[allow(dead_code)]`，零引用 |
| `clarity-gateway` AppState | `agent: Arc<RwLock<Agent>>` | Agent 已有 `Arc<std::sync::RwLock<AgentInner>>`；双重锁 |
| `clarity-gateway` AppState | `active_connections: AtomicUsize` | 初始化后零引用 |

---

## 二、总体方案

三个问题并非独立——它们形成一条**由内到外的解耦链**：

```
Step 1: 统一 Agent streaming 入口（问题 2）
    ↓ 提取出纯净的 "turn loop" 抽象
Step 2: 复活 ChatDriver trait（问题 1）
    ↓ Gateway 通过 ChatDriver 构建消息 → 调用统一 loop
    ↓ Op 枚举恢复为纯生命周期变体
Step 3: 清理 AppState（问题 3）
    ↓ 移除 dead fields + 双重锁
```

### 为什么按这个顺序？

- **Step 1 必须先做**：没有统一的 loop，ChatDriver 将不知道该调用哪个入口。
- **Step 2 依赖 Step 1**：ChatDriver 的 `drive()` 方法需要一个纯净的 loop 接口。
- **Step 3 独立**：可并行或最后做，不影响前两个步骤的语义。

---

## 三、详细实施计划

### Phase A：统一 Agent Streaming Loop（1.5–2 天）

**目标**：让 `run_streaming` 和 `run_streaming_with_messages` 共享完全相同的 orchestration，消除重复。

**具体步骤**：

1. **提取 `turn_setup()` 辅助函数**（`crates/clarity-core/src/agent/run.rs`）
   - 合并 `begin_turn()`、skill 发现/激活、LLM fetch、tool schema fetch、`TurnBegin` wire emission
   - 签名：`fn turn_setup(&mut self, query_hint: &str) -> Result<TurnContext, AgentError>`
   - `TurnContext` 包含：llm、tool_schemas、skill_ctx、session_id 等 setup 产物

2. **提取 `turn_teardown()` 辅助函数**
   - 合并 `TurnEnd` wire、`Usage` emission、memory 存储、ticker 推进
   - 签名：`fn turn_teardown(&mut self, ctx: TurnContext, result: &TurnResult) -> Result<(), AgentError>`

3. **改造 `run_streaming_loop`**
   - 当前：接收 `messages`，内部执行 setup + loop + teardown
   - 目标：接收 `messages` + `TurnContext`（由调用方通过 `turn_setup()` 准备），只负责 LLM stream + tool dispatch
   - teardown 移出 loop，由调用方在 stream 结束后调用

4. **改造两个入口**：
   - `run_streaming`：调用 `turn_setup()` → `run_streaming_loop(...)` → `turn_teardown()`
   - `run_streaming_with_messages`：调用 `turn_setup()` → `run_streaming_loop(...)` → `turn_teardown()`
   - 两者现在只有消息构建的差异，orchestration 完全一致。

5. **补齐缺失的 Hook**：
   - 在 `turn_teardown()` 中加入 `PreDeliveryHook` 和 `SessionTerminationHook`（当前 streaming 路径缺失）。

6. **验证**：
   - `cargo test --workspace --lib` 必须全绿
   - `cargo test -p clarity-core` 中的 streaming 测试必须通过

**风险**：中等。改动触及 Agent 核心执行路径，但测试覆盖率高（256+ tests in core），且 `run_sync_loop()` 已证明该模式可行。

### Phase B：复活 ChatDriver，解耦 Op 枚举（1.5–2 天）

**目标**：将 `Op` 恢复为纯生命周期变体，Gateway 的 OpenAI 兼容层通过 `ChatDriver` 注入。

**具体步骤**：

1. **审视现有 `ChatDriver` trait**（`driver.rs:10-59`）
   - 当前定义是否匹配需求？如不匹配，调整接口。
   - 预期接口：
     ```rust
     pub trait ChatDriver: Send + Sync {
         fn build_messages(&self, prompt: &str, system_prompt: Option<&str>) -> Vec<Message>;
         // 可选：driver 可以携带自己的消息历史管理
     }
     ```

2. **创建 `ConversationChatDriver`**（`clarity-gateway` 中）
   - 持有 OpenAI 风格的消息历史
   - 实现 `ChatDriver::build_messages`，将历史转换为 `clarity-core::Message` 列表

3. **修改 `AgentController`**
   - 当前：`Op::ConversationTurn { messages }` → 调用 `agent.run_streaming_with_messages(messages)`
   - 目标：Controller 不再关心消息如何构建。Gateway 层直接持有 `ChatDriver`，在发送 `Op::UserTurn { prompt }` 之前，先通过 `driver.build_messages()` 构建消息，然后调用 `agent.run_streaming_with_messages()`。
   - **或者更彻底**：让 Controller 完全不知道 `ConversationTurn`，Gateway handler 直接调用 `agent.run_streaming_with_messages()` 而不经过 Controller。

4. **缩小 `Op` 枚举**
   - 移除 `ConversationTurn` 和 `ConversationTurnSync`
   - 剩余变体：`UserTurn`、`Interrupt`、`ToolApproval`、`Compact`、`Shutdown`

5. **更新 Gateway handlers**
   - `handlers.rs:196-198`：不再构造 `Op::ConversationTurn`，而是直接调用 Agent（或发送 `UserTurn` + 通过 ChatDriver 构建消息）
   - `ws.rs` 中如有类似用法，一并更新

6. **验证**：
   - `cargo test -p clarity-gateway` 全绿
   - Gateway HTTP integration tests 通过
   - `Op` 枚举的 match 表达式编译检查确保无遗漏

**风险**：中等。改动 Gateway ↔ core 边界，但 integration tests 覆盖全面。

### Phase C：清理 AppState（0.5–1 天）

**目标**：移除 dead fields 和双重锁，可独立执行。

**具体步骤**：

1. **移除 `initialized: AtomicBool`**（`clarity-egui/src/app_state.rs:35`）
   - 零引用，直接删除字段 + 构造器初始化

2. **移除 `active_connections: AtomicUsize`**（`clarity-gateway/src/server.rs:37`）
   - 确认 `src/` 下零引用后删除

3. **统一 `approval_runtime`**（`clarity-egui`）
   - `panels/approval.rs:5`：`app.state.approval_runtime.list_pending()` → `app.state.mode_aware_approval_runtime.inner().list_pending()`
   - `handlers/mod.rs:71`：`app.state.approval_runtime.clone()` → `app.state.mode_aware_approval_runtime.inner().clone()`
   - 删除 `AppState.approval_runtime` 字段

4. **去除 Gateway Agent 外层 `RwLock`**（`clarity-gateway`）
   - `server.rs:32`：`Arc<RwLock<Agent>>` → `Arc<Agent>`
   - 所有 handler 中 `state.agent.read().await.clone()` → `state.agent.clone()`
   - 所有 handler 中 `state.agent.write().await.set_xxx()` → `state.agent.set_xxx()`（Agent 内部已是 `std::sync::RwLock`）

5. **更新 AGENTS.md**
   - 移除过时的 AppState bloat 描述
   - 添加 Phase C 完成后的新状态

**风险**：低。Phase C 都是局部变更，编译器会强制检查所有引用点。

---

## 四、选项（三种投入级别）

### 选项 A：完整解耦（推荐，3.5–5 天）

执行 **Phase A + B + C** 全部内容。

- **收益最大**：Agent 核心获得统一的 turn 抽象，Gateway 彻底解耦，`Op` 恢复纯净，AppState 瘦身。
- **适合时机**：当前 Sprint 刚结束，没有紧急 feature deadline，适合投入完整解耦。
- **验收标准**：
  1. `Op` 枚举仅剩 5 个生命周期变体，无 Gateway 特有变体
  2. `ChatDriver` 被至少一个实现引用（Gateway）
  3. `run_streaming` 和 `run_streaming_with_messages` 的 orchestration 代码行数差异 < 5 行
  4. AppState 字段数减少（egui: 10→8，gateway: 7→6）
  5. `cargo test --workspace --lib` 全绿

### 选项 B：精简解耦（2–3 天）

执行 **Phase A + C**，跳过 Phase B（Op 枚举 / ChatDriver）。

- **收益**：Agent 核心统一，AppState 瘦身。Op 枚举的 Gateway 耦合暂时保留。
- **适合时机**：如果 Gateway OpenAI 兼容层近期有 feature 变更，Phase B 可能与其冲突，推迟更安全。
- **验收标准**：
  1. `run_streaming` / `run_streaming_with_messages` orchestration 统一
  2. AppState 字段数减少
  3. `cargo test --workspace --lib` 全绿

### 选项 C：最小清扫（0.5–1 天）

仅执行 **Phase C**（AppState 清理）。

- **收益**：快速降低代码异味，编译器消除 dead code。
- **适合时机**：如果有紧急 feature 需要立即启动，先做最小清扫保持基线清洁。
- **验收标准**：
  1. `clippy --workspace` 零新增 warning
  2. AppState dead fields 移除
  3. `cargo test --workspace --lib` 全绿

---

## 五、风险矩阵

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| Phase A 改变 streaming 时序，导致 UI 事件顺序变化 | 中 | 高 | 严格依赖现有 wire event 测试；改动前后对比 `TurnBegin`/`TurnEnd`/`Usage` 事件序列 |
| Phase B 的 ChatDriver 接口与 Gateway 历史管理不兼容 | 低 | 中 | 先在 Gateway 中实现 `ConversationChatDriver`，保留原有历史管理逻辑，仅替换消息构建 |
| Phase C 的 Gateway Agent 去锁导致并发竞态 | 低 | 中 | `Agent` 内部的 `std::sync::RwLock` 已保护所有可变状态；外层 `Arc` 保证线程安全。Integration tests 验证并发场景。 |
| 编译时间膨胀 | 低 | 低 | 改动集中在 core + gateway，不涉及 egui（最重 crate）。预估增量编译 < 2min。 |

---

## 六、需要用户决策的问题

1. **选项选择**：A（完整，3.5–5 天）/ B（精简，2–3 天）/ C（最小，0.5–1 天）？
2. **ChatDriver 范围**：是否希望 TUI 和 egui 也实现 `ChatDriver`（统一所有前端的消息构建策略），还是仅 Gateway 使用？
3. **Phase C 的 `task_store` 孤儿问题**：egui 的 `TaskStore` 只有存储没有执行，是否保留（等待后续 BackgroundTaskManager 接入）还是移除？

---

## 七、执行记录

| 日期 | 动作 | 结果 |
|------|------|------|
| 2026-05-02 | 选项 A 确认，启动 Phase A/B/C | 由目标会话完成核心编码（`d7a40c79`） |
| 2026-05-02 | 清理编译警告（unused import）+ 文档同步 | 当前会话收尾 |

**验收状态（选项 A）**：
- [x] `Op` 枚举仅剩 5 个生命周期变体，无 Gateway 特有变体
- [x] `ChatDriver` 被 Gateway 实现引用（`ConversationChatDriver`）
- [x] `run_streaming` 和 `run_streaming_with_messages` 的 orchestration 代码行数差异 < 5 行
- [x] AppState 字段数减少（egui: 移除 `initialized`；gateway: 移除 `active_connections` + 外层 `RwLock`）
- [x] `cargo test --workspace --lib` = 438 passed / 0 failed / 6 ignored
- [x] `cargo check --workspace --lib` 0 warnings

**遗留问题**：
- `task_store` 孤儿问题未处理，保留至后续 Sprint 决策
