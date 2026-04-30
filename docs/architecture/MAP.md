# Clarity 架构地图 · 总览

> 北极星：替代 Kimi CLI（MVP）
> 范围：仅 clarity（devbase 为对侧团队责任，本侧通过 MCP 接口使用）
> 更新触发：模块新增/删除、trait 签名变更、crate 依赖变更

---

## 快速路由

| 你要做什么 | 先读哪一层 |
|-----------|-----------|
| 改代码前确认影响范围 | [影响层](map-impact.md) |
| 新增模块/拆包/调整依赖 | [拓扑层](map-topology.md) + [契约层](map-contracts.md) |
| 跑测试/回滚/验证 | [验证层](map-verification.md) |
| 找已存在但未激活的能力 | [能力孤岛](map-islands.md) |
| 改 Agent / LLM / 审批流程 | 本文件 §3 关键契约 |

---

## 1. 拓扑速查（精简版）

```
┌─────────────────────────────────────────────────────────────────┐
│  前端层（用户接触）                                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │ clarity-egui│  │ clarity-tui │  │ clarity-gateway (HTTP)  │  │
│  │  主力 GUI    │  │  备用 TUI   │  │  Web API / WebSocket    │  │
│  └──────┬──────┘  └──────┬──────┘  └───────────┬─────────────┘  │
└─────────┼────────────────┼─────────────────────┼────────────────┘
          │                │                     │
          └────────────────┴──────────┬──────────┘
                                      │
┌─────────────────────────────────────┼─────────────────────────────┐
│  协议层                              │                             │
│  ┌──────────────────────────────────┘                             │
│  │ clarity-wire: WireMessage / Wire / ViewCommand / UserAction   │
│  └───────────────────────────────────────────────────────────────┘
└───────────────────────────────────────────────────────────────────┘
                                      │
┌─────────────────────────────────────┼─────────────────────────────┐
│  核心层（clarity-core）              │                             │
│  ┌──────────────────────────────────┘                             │
│  │ agent/        → Agent, AgentController, Op, Plan              │
│  │ approval/     → ApprovalRuntime, ModeAwareApprovalRuntime     │
│  │ tools/        → 16 个内置工具                                 │
│  │ llm/          → LlmProvider, ModelRegistry                    │
│  │ subagents/    → SubAgent spawn, Team, Token                   │
│  │ background/   → Cron, Worker, Task scheduler                  │
│  │ mcp/          → MCP client, registry                          │
│  │ skills/       → Skill discovery, registry, loader             │
│  │ memory/       → In-memory store (clarity-core 内)             │
│  │ compaction/   → Conversation compaction                       │
│  │ notifications/→ Broadcast channels                            │
│  │ view_models/  → Settings, Session VM                          │
│  └───────────────────────────────────────────────────────────────┘
└───────────────────────────────────────────────────────────────────┘
                                      │
┌─────────────────────────────────────┼─────────────────────────────┐
│  基础设施层                          │                             │
│  ┌──────────────────────────────────┘                             │
│  │ clarity-memory → SQLite + BM25 + CosineIndex (独立 crate)     │
│  └───────────────────────────────────────────────────────────────┘
└───────────────────────────────────────────────────────────────────┘
```

完整拓扑见 [拓扑层](map-topology.md)。

---

## 2. 北极星对齐（Kimi CLI Parity）

| Kimi CLI 功能 | clarity 对应模块 | 状态 | 差距 |
|--------------|-----------------|------|------|
| 多会话切换 | `clarity-core::agent` + `clarity-egui::session` | ✅ | — |
| 代码读写编辑 | `tools::file` (read/write/edit) | ✅ | — |
| Shell 执行 | `tools::shell` | ✅ | Windows PowerShell |
| Web 搜索 | `tools::web` | ✅ | — |
| Plan 执行 | `agent::plan` + `execution.rs` | ✅ | — |
| 审批系统 | `approval` (Interactive/Yolo/Plan/Smart) | ✅ | Smart 模式新增 |
| Streaming | `Wire` + `AgentController` | ✅ | — |
| Settings / 模型选择 | `view_models::settings` + `llm::model_registry` | ✅ | 增量保存 |
| 子代理 | `subagents` | ✅ | 缺 egui UI |
| 背景任务 | `background` | ✅ | 缺 egui UI |
| Skill 系统 | `skills` | ✅ | 缺 egui UI |
| 能力发现 | `capability` | 🔄 | Sprint 10 D3 |
| MCP 集成 | `mcp` | ✅ | 38 tools |
| 长程记忆 | `clarity-memory` | ⚠️ | session 级，未接 devbase |
| UI 单元测试 | — | ❌ | 渲染测试仍缺口 |

---

## 3. 关键契约（改前必读）

### 3.1 Agent 核心接口

```rust
// crates/clarity-core/src/agent/mod.rs
pub struct Agent { /* ... */ }
impl Agent {
    pub fn new(config: AgentConfig) -> Self;
    pub fn set_llm(&self, llm: Arc<dyn LlmProvider>);
    pub fn unset_llm(&self);                          // Sprint 13 Tech Debt 新增
    pub fn set_approval_mode(&self, mode: ApprovalMode);
    pub async fn run(&self, input: &str) -> Result<Vec<Message>, AgentError>;
    pub async fn run_streaming(&self, input: &str, wire: &Wire) -> Result<(), AgentError>;
}
```

**变更影响**：改 `Agent` 构造或 `run` 签名 → 影响 `clarity-egui`, `clarity-gateway`, `clarity-tui`, 集成测试。

### 3.2 审批运行时契约

```rust
// crates/clarity-core/src/approval/mod.rs
#[async_trait]
pub trait ApprovalRuntime: Send + Sync {
    async fn create_request(&self, ...) -> Result<String, AgentError>;
    async fn wait_for_response(&self, request_id: &str) -> Result<ApprovalResponse, AgentError>;
    async fn resolve(&self, request_id: &str, response: ApprovalResponse) -> Result<(), AgentError>;
    fn list_pending(&self) -> Vec<ApprovalRequest>;   // Sprint 13 从 concrete 提升为 trait
}
```

**实现者**：
- `InMemoryApprovalRuntime` — 内存存储，UI 查询用
- `ModeAwareApprovalRuntime<R>` — 包裹层，Smart / Yolo / Plan 逻辑

**变更影响**：改 trait → 两处实现 + `clarity-egui::panels::approval` + `clarity-gateway::handlers::admin`。

### 3.3 Wire 协议契约

```rust
// crates/clarity-wire/src/lib.rs
pub enum WireMessage {
    Text { role: TextRole, content: String },
    ToolCallBegin { name: String, args: String },
    ToolCallEnd { name: String, result: String },
    CompactionBegin, CompactionEnd,
    PlanStepBegin { step_id: String, tool_name: String },
    PlanStepEnd { step_id: String, status: String },
    // ... 增删变体需同步 egui/gateway/tui
}
```

**变更影响**：增删 `WireMessage` 变体 → `clarity-egui::process_events` + `clarity-gateway::ws` + `clarity-tui::render`。

### 3.4 LLM 绑定三层契约（Sprint 13 Tech Debt）

```rust
// crates/clarity-egui/src/llm_policy.rs   → 纯策略，sync，testable
pub fn resolve_provider(...) -> ProviderSelection;

// crates/clarity-egui/src/llm_loader.rs   → 异步加载
pub async fn load_llm(...) -> Result<(Arc<dyn LlmProvider>, Option<LlmBinding>), EguiError>;

// crates/clarity-egui/src/llm_binder.rs   → 同步绑定
pub fn bind_llm(agent: &Agent, llm: Arc<dyn LlmProvider>, label: &str);
pub fn unbind_llm(agent: &Agent);
```

**变更影响**：改 `resolve_provider` 分支 → 影响 `llm_policy` 5 个单元测试；改 `load_llm` → 影响 `app_state::ensure_llm`。

完整契约见 [契约层](map-contracts.md)。

---

## 4. 安全撤退路径（全局）

```bash
cd C:\Users\22414\dev\third_party\clarity

# 1. 改代码前标记锚点
git stash push -m "WIP: <模块名>"

# 2. 改完后验证基线
cargo test --workspace --lib
cargo clippy --workspace --lib --bins --tests -- -D warnings
cargo fmt --all -- --check

# 3. 任一失败 → 撤退
git stash pop   # 或 git checkout -- <file>

# 4. 全绿后提交
git add -A
git -c user.email="160722440+juice094@users.noreply.github.com" -c user.name="juice094" commit -m "..."
```

详细验证矩阵见 [验证层](map-verification.md)。

---

*本文件由 AI 会话维护，人类开发者可直接编辑。拓扑/契约/影响变更需同步到对应图层文件。*
