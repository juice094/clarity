# 架构地图 · 契约层

> 用途：改接口前确认契约边界，避免 breaking change 扩散
> 更新触发：trait 方法签名变更、enum 变体增删、struct 字段变更

---

## 契约等级

| 等级 | 定义 | 变更规则 |
|------|------|---------|
| 🔴 P0 — 核心契约 | 跨 crate 使用的 trait / enum | 禁止 breaking change；只能新增，旧接口保留 deprecated |
| 🟡 P1 — 内部契约 | 单 crate 内跨模块接口 | 允许 breaking change，但需同步修改所有实现者 |
| 🟢 P2 — 实现细节 | 私有 struct / 函数 | 自由变更 |

---

## P0 核心契约（跨 crate）

### C0.1 Agent 构造与运行

```rust
// crates/clarity-core/src/agent/mod.rs
pub struct Agent { /* opaque */ }

impl Agent {
    pub fn new(config: AgentConfig) -> Self;
    pub fn set_llm(&self, llm: Arc<dyn LlmProvider>);
    pub fn unset_llm(&self);                          // v0.3.1+
    pub fn set_provider_label(&self, label: String);  // v0.3.0+
    pub fn set_approval_mode(&self, mode: ApprovalMode);
    pub fn set_skills(&self, skills: Vec<Skill>);
    pub fn run(&self, input: &str) -> impl Future<Output = Result<Vec<Message>, AgentError>>;
    pub fn run_streaming(&self, input: &str, wire: &Wire) -> impl Future<Output = Result<(), AgentError>>;
}
```

**调用方**：`clarity-egui::app_logic::send`, `clarity-gateway::handlers`, `clarity-tui::main`, 集成测试

**变更规则**：
- 增删 `pub fn` → 必须检查所有调用方编译
- 改 `run` / `run_streaming` 签名 → breaking change，需大版本号或新增方法

---

### C0.2 LlmProvider Trait

```rust
// crates/clarity-core/src/llm/mod.rs
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: ChatCompletionRequest) -> Result<LlmResponse, LlmError>;
    async fn complete_stream(&self, request: ChatCompletionRequest, tx: mpsc::Sender<StreamDelta>) -> Result<(), LlmError>;
}
```

**实现者**：`DeepSeekProvider`, `OllamaProvider`, `LocalGgufProvider`, `OpenAiProvider`, `ReasonerProvider`, `MockLlm`

**变更规则**：增删方法 → 6 个实现者必须同步更新。

---

### C0.3 ApprovalRuntime Trait

```rust
// crates/clarity-core/src/approval/mod.rs
#[async_trait]
pub trait ApprovalRuntime: Send + Sync {
    async fn create_request(&self, tool_call: &ToolCall, source: ApprovalSource, description: Option<String>, diff_preview: Option<String>) -> Result<String, AgentError>;
    async fn wait_for_response(&self, request_id: &str) -> Result<ApprovalResponse, AgentError>;
    async fn resolve(&self, request_id: &str, response: ApprovalResponse) -> Result<(), AgentError>;
    fn list_pending(&self) -> Vec<ApprovalRequest>;
}
```

**实现者**：`InMemoryApprovalRuntime`, `ModeAwareApprovalRuntime<InMemoryApprovalRuntime>`

**变更规则**：trait 方法签名变更 → 全 workspace 编译失败，强制同步。

---

### C0.4 WireMessage Enum

```rust
// crates/clarity-wire/src/lib.rs
pub enum WireMessage {
    Text { role: TextRole, content: String },
    ToolCallBegin { name: String, args: String },
    ToolCallEnd { name: String, result: String },
    ToolCallError { name: String, error: String },
    CompactionBegin,
    CompactionEnd,
    PlanStepBegin { step_id: String, tool_name: String, description: String },
    PlanStepEnd { step_id: String, status: PlanStepStatus },
    Loading { active: bool },
    Done,
}
```

**序列化方**：`clarity-core::agent::driver`（发送）
**反序列化方**：`clarity-egui::process_events`, `clarity-gateway::ws`, `clarity-tui::render`

**变更规则**：
- 新增变体 → 安全（前端忽略未知变体需确认）
- 删除变体 → breaking change，需同步所有反序列化方
- 改字段 → breaking change

---

### C0.5 Tool Trait

```rust
// crates/clarity-core/src/tools/mod.rs
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    async fn execute(&self, args: Value, context: &ToolContext) -> Result<Value, ToolError>;
}
```

**实现者**：16 个内置工具（file, shell, web, ...）+ MCP 动态工具

**变更规则**：`execute` 签名变更 → 全工具实现需更新。

---

## P1 内部契约（clarity-core 内跨模块）

### C1.1 AgentController

```rust
// crates/clarity-core/src/agent/controller.rs
pub struct AgentController;
impl AgentController {
    pub fn new(agent: Agent, wire: Wire) -> Self;
    pub async fn start(&self, input: String) -> Result<(), AgentError>;
    pub fn stop(&self);
    pub fn interrupt(&self) -> Result<(), AgentError>;
}
```

**调用方**：`egui::app_logic`, `gateway::handlers`, `tui::main`

### C1.2 Op Enum

```rust
// crates/clarity-core/src/agent/ops.rs
pub enum Op {
    Think(String),
    ToolCall(ToolCall),
    Respond(String),
    // 增删变体需同步 driver.rs + execution.rs
}
```

### C1.3 AgentConfig

```rust
// crates/clarity-core/src/agent/config.rs
pub struct AgentConfig {
    pub system_prompt: Option<String>,
    pub max_turns: usize,
    pub approval_mode: ApprovalMode,
    pub compaction_config: CompactionConfig,
    // 新增字段需同步 construct.rs + egui settings
}
```

---

## P2 实现细节（自由变更）

以下结构变更**不需要**跨模块同步，但建议保留单元测试：

- `InMemoryApprovalRuntime` 内部 HashMap 结构
- `App::update()` 内的 UI 布局代码
- `Theme` 配色定义
- `OnboardingModal` 步骤流程
- 具体 LLM Provider 的 HTTP 请求体构造

---

## Breaking Change 检查单

修改 P0 契约前必须确认：

- [ ] 是否新增而非修改？（优先 `#[deprecated]` 旧接口 + 新增新接口）
- [ ] 所有实现者是否已更新？（trait / enum 匹配）
- [ ] 所有调用方是否已更新？（编译检查）
- [ ] 序列化格式是否兼容？（WireMessage / JSON schema）
- [ ] 测试是否覆盖？（单元测试 + integration test）

---

*本文件由 AI 会话维护。契约变更需同步版本号和变更日志。*
