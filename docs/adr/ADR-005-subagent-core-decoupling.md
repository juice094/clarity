---
title: ADR-005: Subagent ↔ Core Decoupling (P0 Continuation)
category: ADR
tags: [adr, agent]
---

# ADR-005: Subagent ↔ Core Decoupling (P0 Continuation)

> Status: In Progress  
> Date: 2026-05-11  
> Deciders: juice094 + Agent (architect layer)  
> Affects: `clarity-subagents`, `clarity-core`, `clarity-contract`

---

## Context

`clarity-subagents` was extracted from `clarity-core` during Sprint 13-14, but a **logical bidirectional coupling** remains:

- `clarity-core::agent::Agent` implements `SubagentOrchestrator` (contract trait) ✅
- `clarity-subagents` still imports concrete types from `clarity-core`:
  - `Agent`, `AgentConfig`, `AgentExecutor`
  - `ToolRegistry`
  - `BackgroundTaskManager` and task types
  - `ApprovalRuntime`
  - Jumpy types (`OutcomePredictor`, `JumpyState`)

This prevents `clarity-subagents` from being consumed by crates that must not depend on `clarity-core` (e.g. future standalone subagent CLI tools).

## Decision

Complete decoupling is accepted as P0 for Sprint 42, but split into **phased deliverables** to avoid a monolithic 4-8h refactor commit.

## Phases

### Phase A — Type Redirection (DONE ✅ 2026-05-11)

Migrate imports that already have contract-level equivalents.

| Type | Old Import | New Import | Files |
|------|-----------|-----------|-------|
| `AgentError` | `clarity_core::error::AgentError` | `clarity_contract::error::AgentError` | `runner.rs` |
| `ApprovalMode` | `clarity_core::approval::ApprovalMode` | `clarity_contract::ApprovalMode` | `runner.rs` |

### Phase B — AgentExecutor Trait Uplift (DONE ✅ 2026-05-11)

Move `AgentExecutor` trait definition from `clarity-core` to `clarity-contract::subagent`.

- `clarity-contract/src/subagent.rs` now owns the trait.
- `clarity-core/src/agent/executor.rs` only provides `impl AgentExecutor for Agent`.
- `clarity-subagents/src/runner.rs` consumes trait from contract.

### Phase C — ToolRegistry Trait Uplift (TODO)

**Problem**: `SubagentBuilder` and `ParallelExecutor` hold `clarity_core::registry::ToolRegistry` by value.

**Solution**: Define `ToolRegistry: Send + Sync` trait in `clarity-contract::tool` with methods needed by subagents:

```rust
pub trait ToolRegistry: Send + Sync {
    fn list_tools(&self) -> Vec<&dyn Tool>;
    fn get_tool(&self, name: &str) -> Option<&dyn Tool>;
    fn execute(&self, name: &str, args: Value) -> Result<ToolResult, ToolError>;
}
```

- Migrate `clarity-core::registry::ToolRegistry` to implement the contract trait.
- `clarity-subagents` holds `Arc<dyn ToolRegistry>` instead of concrete type.

**Risk**: `ToolRegistry` in core has ~2K lines and complex registration logic. Trait extraction must not break hot-reload or dynamic tool registration.

**Est. Effort**: 2-3 hours.

### Phase D — BackgroundTaskManager Abstraction (TODO)

**Problem**: `ParallelExecutor` and `TeamCoordinator` use `clarity_core::background::{BackgroundTaskManager, TaskSpec, TaskResult, TaskStatus, TaskPriority}`.

**Solution**: Define `TaskManager` trait + task types in `clarity-contract::subagent` (or new `clarity-contract::task` module).

```rust
pub trait TaskManager: Send + Sync {
    async fn spawn(&self, spec: TaskSpec) -> TaskId;
    async fn status(&self, id: TaskId) -> TaskStatus;
    async fn result(&self, id: TaskId) -> Option<TaskResult>;
    async fn cancel(&self, id: TaskId);
}
```

- `clarity-core::background::BackgroundTaskManager` implements `TaskManager`.
- `clarity-subagents` holds `Arc<dyn TaskManager>`.

**Est. Effort**: 1-2 hours.

### Phase E — ApprovalRuntime Trait Uplift (TODO)

**Problem**: `SubagentRunner` holds `Option<Arc<dyn ApprovalRuntime>>` from `clarity-core::approval`.

**Solution**: Move `ApprovalRuntime` trait to `clarity-contract::tool` (next to `ApprovalMode`).

**Est. Effort**: 30 minutes.

### Phase F — AgentBuilder Abstraction (TODO)

**Problem**: `SubagentBuilder::build` returns concrete `Agent`, and `SubagentRunner::build_agent` calls `agent.with_llm()`, `agent.with_approval_runtime()`, `agent.set_llm()` on the concrete type.

**Solution**: Define `AgentBuilder` trait in `clarity-contract::subagent`:

```rust
pub trait AgentBuilder: AgentExecutor {
    fn with_llm(self: Box<Self>, llm: Arc<dyn LlmProvider>) -> Box<dyn AgentBuilder>;
    fn with_approval_runtime(self: Box<Self>, runtime: Arc<dyn ApprovalRuntime>) -> Box<dyn AgentBuilder>;
    fn with_approval_mode(self: Box<Self>, mode: ApprovalMode) -> Box<dyn AgentBuilder>;
    fn set_llm(&mut self, llm: Arc<dyn LlmProvider>);
    fn build(self: Box<Self>) -> Box<dyn AgentExecutor>;
}
```

- `clarity-core::Agent` implements `AgentBuilder`.
- `SubagentBuilder::build` returns `Box<dyn AgentBuilder>`.
- `SubagentRunner::build_agent` returns `Box<dyn AgentExecutor>`.

**Risk**: High. This touches the most sensitive code path (agent construction). Requires careful review of all call sites.

**Est. Effort**: 2-3 hours.

### Phase G — Jumpy Decoupling (TODO)

**Problem**: `clarity-subagents/src/lib.rs` and `mod.rs` import `clarity_core::agent::jumpy::{OutcomePredictor, JumpyState}`.

**Solution**: Either:
- (a) Move Jumpy types to `clarity-contract::subagent`, or
- (b) Hide Jumpy behind a `Predictor` trait in contract.

**Est. Effort**: 30 minutes.

## Acceptance Criteria

`clarity-subagents/Cargo.toml` must lose its `clarity-core` dependency line:

```toml
# BEFORE (current)
[dependencies]
clarity-core = { path = "../clarity-core" }   # ← remove
clarity-contract = { path = "../clarity-contract" }
clarity-llm = { path = "../clarity-llm" }
clarity-wire = { path = "../clarity-wire" }

# AFTER (target)
[dependencies]
clarity-contract = { path = "../clarity-contract" }
clarity-llm = { path = "../clarity-llm" }
clarity-wire = { path = "../clarity-wire" }
```

## Verification After Each Phase

```bash
cargo check -p clarity-subagents
cargo test --workspace --lib   # 849 passed / 0 failed / 7 ignored
cargo clippy -p clarity-subagents -- -D warnings
```

## References

- `AGENTS.md` §Current Phase: agent ↔ subagents bidirectional coupling analysis
- `docs/SUBAGENT_PARALLEL_ANALYSIS.md`: Parallel execution design
- `crates/clarity-core/src/agent/executor.rs`: P1-2 trait abstraction (PoC scope)
