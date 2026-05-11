# ADR-008: Brain/Hands/Session 三层解耦对 Clarity 的启示

> Status: **Draft / Proposed** （尚未 Accepted，等待 ADR-006 落实经验沉淀后再决议）
> Date: 2026-05-11
> Deciders: 待人类确认
> Affects: clarity-core, clarity-tools, clarity-memory（潜在）
> Supersedes: 无
> Relates to:
>   - docs/CODE-CHANGE-PRINCIPLES.md P3（单源真相）
>   - docs/adr/ADR-006-protocol-layer-convergence.md（wire 单源化先例）
>   - docs/notes/2026-05-11-anthropic-managed-agents-mapping.md（数据基础）
>   - docs/architecture-positioning.md 五-A（定位关系）

---

## 1. Context

2026-05-11 用户通过 Kimi share 分享了 Anthropic Managed Agents 架构剖析。该架构展示了一种 "Brain（无状态推理）/ Hands（沙箱执行）/ Session（事件日志单源）" 三层物理解耦的设计哲学。

详细映射分析见 docs/notes/2026-05-11-anthropic-managed-agents-mapping.md。

### 1.1 Anthropic 三层解耦概要

```
Brain（推理）         Hands（执行）        Session（日志）
+----------+         +----------+         +----------+
| Harness  |--tool-->| Sandbox  |--event->| Session  |
| 无状态   |         | 容器隔离 |         | append-  |
| 编排器   |<--result| 可丢弃   |         | only log |
+----------+         +----------+         +----------+
     ^                                          ^
     +-------- wake(sessionId) + getSession(id) +
```

### 1.2 Clarity 当前状态

总体匹配度 ~70%（详见 mapping doc 3）。三处实质差距：

| 差距 | Anthropic | Clarity | 影响 |
|------|-----------|---------|------|
| A | Harness 无状态 | Agent 持有 registry/wire/approval 等运行时状态 | Agent 实例不可跨进程迁移 |
| B | Sandbox 容器隔离 | ToolRegistry 同进程执行 | 与 OpenClaw 同类 RCE 风险 |
| C | Session = append-only event log | messages 直接作为 LLM context | 事件无法重放，audit/time-travel 困难 |

### 1.3 关键约束

Clarity 必须保持的核心定位（参 docs/architecture-positioning.md 一）：

- Local-first：本地优先，不依赖云
- 单二进制：cargo install 即用，无外部运行时依赖
- LLM 中立：支持 7+ provider，不绑定 Anthropic
- 守护进程：唯一长生命周期，不支持 oneshot

任何借鉴方案不得违反以上约束。

---

## 2. Decision

**有选择地借鉴 Anthropic 三层解耦的哲学，但拒绝照搬实现**。

### 2.1 不照搬清单（Hard Veto）

| 项 | 拒绝理由 |
|----|----------|
| Anthropic API 兼容（managed-agents-2026-04-01） | 违反 LLM 中立原则 |
| 完全无状态 Brain（Harness 风格） | 违反长进程定位，引入不必要的序列化/反序列化开销 |
| Docker / 容器沙箱 | 违反 "无运行时依赖" |
| Session-hour 计费 | 本地免费定位 |
| FDE cloud runtime | 非技术架构问题 |

### 2.2 应借鉴的哲学（已部分落实 / 待规划）

| 项 | 状态 | 实施路径 |
|----|------|---------|
| Session 单源真相 | DONE - ADR-006 已对齐 wire 协议 | 无需新工作 |
| Brain / Hands 解耦概念 | PARTIAL（Agent 与 ToolRegistry 已分层但同进程） | M2 抽象 ToolExecutor trait（见 3.2） |
| Wake/Suspend 抽象 | PENDING - 待立项 | M1 设计 Wake/Suspend 接口（见 3.1） |
| Event Log 独立于 context | PENDING - 待立项 | M3 拆分 events + compacted_context（见 3.3） |

---

## 3. Proposed Phases

### 3.1 M1 - Wake/Suspend 抽象（中期，依赖 S3 完成）

**目标**：允许 Agent 完全从 SessionStore 重建，但日常运行不强制无状态。

**接口设计草案**（仅 API surface，非完整实现）：

```rust
pub trait Wakeable {
    async fn wake(session_id: &str, deps: &AgentDeps) -> Result<Self, WakeError>
    where Self: Sized;

    async fn suspend(&self) -> Result<SuspendedState, SuspendError>;
}

pub struct AgentDeps {
    pub registry: Arc<ToolRegistry>,
    pub llm_factory: Arc<dyn LlmFactory>,
    pub memory_store: Arc<MemoryStore>,
    pub approval_runtime: Arc<dyn ApprovalRuntime>,
}

pub struct SuspendedState {
    pub session_id: String,
    pub agent_config: AgentConfig,
    pub last_event_id: u64,
    pub serialized_state: Vec<u8>,
}
```

**前置条件**：
- S3 Settings 单源化完成（Agent 状态明朗后才能正确序列化）
- ADR-007 Turn ID 注入完成（last_event_id 锚点需要 turn_id）

**Hard veto**：
- Wake 不得变成 Agent 的唯一构造路径（保留 Agent::new 等直接构造）
- Wake 不得引入跨进程依赖（不要 RPC / IPC，只读写本地 SessionStore）

### 3.2 M2 - ToolExecutor Trait 抽象（中期，依赖本 ADR 接受）

**目标**：让 ToolRegistry 与执行后端解耦，留沙箱后端扩展点（但不强制实现沙箱）。

**接口设计草案**：

```rust
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(
        &self,
        tool: &dyn Tool,
        args: Value,
        ctx: ToolContext,
    ) -> Result<ToolResult, ToolError>;

    fn isolation_level(&self) -> IsolationLevel;
}

pub enum IsolationLevel {
    None,           // 同进程，无隔离（当前 InProcessExecutor）
    Process,        // 子进程隔离
    Wasm,           // WASM sandbox 隔离
    Container,      // 容器隔离（如可用）
}
```

**Hard veto**：
- 默认实现 InProcessExecutor 必须永远可用（不依赖外部依赖）
- 添加 ContainerExecutor 不得引入 Docker/runc 强依赖（feature flag 控制）
- 不得违反 "单二进制" 定位

### 3.3 M3 - Event Log 模型拆分（长期，依赖 M1 + ADR-007）

**目标**：把 Session 内容分为 append-only 事件流 + 派生的 LLM context。

**数据模型草案**：

```rust
pub struct SessionV2 {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub events: Vec<SessionEvent>,
    pub compacted_context: Vec<Message>,
}

pub struct SessionEvent {
    pub event_id: u64,
    pub turn_id: u64,
    pub timestamp: DateTime<Utc>,
    pub kind: SessionEventKind,
}

pub enum SessionEventKind {
    UserInput { text: String },
    AgentResponse { text: String, model: String, tokens: usize },
    ToolCall { name: String, args: Value },
    ToolResult { name: String, result: Value, success: bool },
    PlanGenerated { plan: Plan },
    Compaction { input_count: usize, output_summary: String },
}
```

**Hard veto**：
- 不得引入 message-only API 的破坏性变更（current API 继续工作）
- compacted_context 必须可从 events 重新派生（避免双源真相）

### 3.4 Phase 顺序

```
ADR-006 (完成)
   |
   v
ADR-007 Turn ID 注入 (待立项) ---- M3 依赖
   |                                |
   v                                v
S3 Settings 单源化 (进行中)     M1 Wake/Suspend 设计 (依赖 S3)
   |                                |
   v                                v
ADR-008 (本) Accepted          M3 Event Log 拆分 (依赖 M1+ADR-007)
   |
   v
M2 ToolExecutor trait (与 M1 可并行)
```

---

## 4. Consequences

### 4.1 Positive

- 架构一致性：Brain / Hands / Session 解耦哲学与 ADR-006 单源真相同源
- 未来扩展点：M2 ToolExecutor 为可选沙箱后端铺路
- 可观测性：M3 Event Log 为 audit / time-travel debugging 铺路
- 可移植性：M1 Wake/Suspend 让 Agent 状态可从 SessionStore 重建

### 4.2 Negative

- 复杂度增加：三个 M 阶段共增加 ~3000 行抽象代码（估算）
- 学习曲线：新贡献者需要理解三层解耦概念
- 过度设计风险：如未来用户场景未需要 wake 或沙箱，则 M1/M2 抽象成本浪费

### 4.3 Neutral

- LLM 中立性保持
- 单二进制定位保持
- 本地优先定位保持

---

## 5. Alternatives Considered

### 5.1 完全拒绝借鉴

Pros: 零工作量，保持现状。
Cons: 与 OpenClaw 同类安全风险持续暴露；audit / time-travel 能力缺失。
结论: 部分拒绝（不要 Docker / cloud）+ 部分借鉴（抽象接口）。

### 5.2 完全照搬 Anthropic

Pros: 与 Anthropic 生态最佳兼容。
Cons: 违反 Clarity 全部核心约束（LLM 中立、单二进制、本地优先）。
结论: Hard veto（见 2.1）。

### 5.3 仅引入 ToolExecutor，跳过 Wake/Event Log

Pros: 工作量最小，安全收益最大。
Cons: 错过 audit / time-travel / 跨进程 Agent 迁移等长期价值。
结论: 部分采纳作为 Phase 1（M2 先行），但保留 M1/M3 在 backlog。

---

## 6. Verification Criteria

ADR-008 视为完整落地的判据：

- [ ] 本 ADR 接受（status: Draft -> Accepted）
- [ ] M1 设计 RFC：Wake/Suspend 接口设计完整 + Hard veto 明确
- [ ] M2 实施完成：ToolExecutor trait + InProcessExecutor 默认实现 + 至少 1 个备选实现（wasm 或 process）
- [ ] M3 数据模型 RFC：SessionV2 / SessionEvent 设计 + 与现有 messages 的迁移路径
- [ ] OpenClaw 安全风险评估：与 OpenClaw 学术分析对照，Clarity 防御深度有书面评估

---

## 7. References

- Anthropic Engineering Blog: https://www.anthropic.com/engineering/managed-agents
- Claude Docs: https://platform.claude.com/docs/en/managed-agents/overview
- arxiv 2603.12644v1 "A Case Study of OpenClaw"
- docs/CODE-CHANGE-PRINCIPLES.md P3 单源真相
- docs/adr/ADR-006-protocol-layer-convergence.md（wire 单源化先例）
- docs/notes/2026-05-11-anthropic-managed-agents-mapping.md（详细映射分析）
- docs/architecture-positioning.md 五-A（定位关系）

---

## 8. Revision Log

| 日期 | 变更 | 提议者 |
|------|------|--------|
| 2026-05-11 | 1.0 Draft 起草 | 主会话（基于 Kimi 对话分析） |
