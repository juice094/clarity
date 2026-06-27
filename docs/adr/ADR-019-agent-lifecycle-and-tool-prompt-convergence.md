---
title: ADR-019: Agent Lifecycle State Machine, Tool Prompt Convergence, and YOLO Guardrails
category: ADR
tags: [adr, core, agent, tools, yolo, lifecycle]
---

# ADR-019: Agent Lifecycle State Machine, Tool Prompt Convergence, and YOLO Guardrails

> Status: Accepted — design complete, implementation in progress  
> Date: 2026-06-27  
> Deciders: juice094 + Agent  
> Affects: `clarity-core`, `clarity-llm`  
> Relates: ADR-006 (Protocol Layer Convergence), ADR-011 (Workspace Architecture), `docs/architecture/protocol-layer.md`

---

## 1. Context

### 1.1 Problem Statement

During a tool-exposure test we observed two recurring failure modes in `clarity-core`:

1. **Tool prompt repeated exposure**: every ReAct iteration inside a single turn re-serializes the full JSON Schema of all enabled tools and sends it to the LLM. There is no in-turn caching, versioning, or summarization.
2. **YOLO mode lacks early stopping / convergence**: when `ApprovalMode::Yolo` is active the agent bypasses most approvals, but the only stop conditions are `max_iterations`, global budget, exact-output hash collision (`LoopDetector`), or hard cancellation. There is no guardrail for "the LLM keeps calling semantically equivalent tools without making progress".

Both problems increase token cost, latency, and the risk of runaway loops.

### 1.2 Root Cause

#### Tool prompt repeated exposure

The tool schema is injected at two places:

- `agent/prompt.rs` builds a natural-language `- name: description` block for the static system prompt.
- `agent/run/loop_trait.rs` calls `filter_tools_value(registry.get_tool_schemas())` at the start of every turn and passes the resulting `serde_json::Value` into `run_loop_iterations()`. On every iteration the full value is forwarded to `llm.complete()` or `llm.stream()`.

There is no stable hash or snapshot of the working tool set, so the schema is re-serialized on each LLM call even when nothing has changed.

#### YOLO lack of convergence

`LoopDetector` (in `agent/loop_detector.rs`) only checks for **exactly identical** outputs or arguments using `DefaultHasher`. It cannot detect:

- the same tool being called repeatedly with trivially different arguments,
- a sequence of calls that explores the same information space without new results,
- LLM responses that become templated or repetitive.

`ApprovalMode::Yolo` short-circuits `wait_for_response()` to `Approve`, but no semantic guardrail asks the user for clarification when progress stalls.

---

## 2. Decision

We will introduce three coordinated changes in `clarity-core`:

1. **`ToolPromptManager`** — a per-turn snapshot of the working tool schema with stable hashing and incremental updates. It eliminates repeated full-schema serialization inside a turn.
2. **`LoopDetector` v2 + `YOLOGuardrails`** — semantic/sequential stagnation detection that can turn a runaway loop into an `ask_user` break instead of waiting for budget exhaustion.
3. **`AgentLifecycle` state machine** (Phase 4) — an explicit state enum that replaces implicit booleans and makes the message loop easier to reason about, test, and extend.

These changes stay within `clarity-core`. `clarity-wire` and frontends consume the same events; only the internal loop behavior changes.

---

## 3. Detailed Design

### 3.1 `ToolPromptManager`

Responsibility: own the LLM-facing tool description for the current turn.

```rust
pub(crate) struct ToolPromptManager {
    /// Stable hash of the current working tool set.
    schema_hash: u64,
    /// Pre-serialized tools value for the LLM request.
    tools_value: serde_json::Value,
}

impl ToolPromptManager {
    /// Build from the registry/schema once per turn.
    pub fn new(tools: &serde_json::Value) -> Self;

    /// Return the current tools value; cheap on every iteration.
    pub fn tools_value(&self) -> &serde_json::Value;

    /// Incrementally remove a tool after circuit-breaker failure.
    pub fn filter_tool(&mut self, name: &str) -> bool;

    /// Check whether the underlying schema has drifted.
    pub fn is_stale(&self, tools: &serde_json::Value) -> bool;
}
```

- `Agent` holds an `Option<ToolPromptManager>`.
- `prepare_sync_turn` / `setup_turn` / `run_streaming_turn` initialize it once.
- `run_loop_iterations` calls `tools_value()` on each iteration instead of cloning/re-serializing the full schema.
- Circuit-breaker removal mutates the manager, not the raw `serde_json::Value`.

### 3.2 `LoopDetector` v2 and `YOLOGuardrails`

Responsibility: detect stagnation and decide when to break into `ask_user`.

`LoopDetector` keeps its existing exact-match checks and gains:

- `ConsecutiveSameTool` — count of consecutive calls to the same tool name.
- `ResultDiversity` — Jaccard-like similarity of the last K tool outputs (using stdlib hashing, no new NLP dependency).
- `ResponseEntropy` — simple check for repeated assistant content.

`YOLOGuardrails` is a configuration + runtime check object:

```rust
pub struct YoloGuardrails {
    max_tool_calls_per_turn: usize,
    max_consecutive_same_tool: usize,
    stagnation_window: usize,
    stagnation_threshold: f64,
}

impl YoloGuardrails {
    pub fn check(&self, detector: &LoopDetector, state: &GuardrailState) -> GuardrailOutcome;
}

pub enum GuardrailOutcome {
    Ok,
    Warning(String),
    AskUser { question: String },
    Stop { reason: String },
}
```

- Guardrails are **not** tied only to `ApprovalMode::Yolo`; they are a safety net under any auto-execution mode.
- `AskUser` is converted into `DispatchOutcome::Break { final_response: question, is_error: false }`, reusing the existing `ask_user` wait path.

### 3.3 `AgentLifecycle` State Machine (Phase 4)

```rust
pub enum AgentState {
    Idle,
    Planning,
    AwaitingTools,
    Synthesizing,
    AwaitingUser,
    Interrupted,
    Complete,
    Error,
}
```

This replaces implicit flags such as `cancel_token.is_cancelled()`, `ask_user_question.is_some()`, and `completed` booleans. The exact migration plan is in Phase 4.

---

## 4. Consequences

### Positive

- Lower per-turn token cost and latency.
- Fewer runaway loops in YOLO / auto modes.
- Easier to test and debug the agent loop after state-machine introduction.
- Clearer boundaries between prompt construction, loop control, and approval.

### Negative / Risks

- A too-aggressive `LoopDetector` may break legitimate repeated calls. Mitigation: conservative defaults, warning before break, user-configurable thresholds.
- `ToolPromptManager` must stay consistent with circuit-breaker filtering. Mitigation: make filtering go through the manager exclusively.
- `AgentLoop` sync/streaming split adds friction to the refactor. Mitigation: keep the trait surface small and share helper functions.

---

## 5. Implementation Phases

| Phase | Focus | Approx. Duration | Blocked By |
|---|---|---|---|
| 0 | ADR + interface drafts (this document + `crates/clarity-core/design/agent-lifecycle-rfc.md`) | 1 week | — |
| 1 | Code-style cleanup: consolidate `ToolRegistry` factories, unify allow-list filtering, add `// ponytail:` markers | 1 week | Phase 0 review |
| 2 | Implement `ToolPromptManager` and remove per-iteration full-schema serialization | 1–2 weeks | Phase 0, Phase 1 |
| 3 | Implement `LoopDetector` v2 + `YOLOGuardrails` with tests | 2 weeks | Phase 2 |
| 4 | Introduce `AgentLifecycle` state machine and simplify `AgentLoop` trait | 2 weeks | Phase 3 |

Phases 0 and 1 may run partially in parallel. Phases 2 and 3 are sequential because guardrails rely on stable tool-call history from the prompt manager.

---

## 6. References

- `crates/clarity-core/src/agent/prompt.rs`
- `crates/clarity-core/src/agent/run/loop_trait.rs`
- `crates/clarity-core/src/agent/loop_detector.rs`
- `crates/clarity-core/src/agent/execution.rs`
- `crates/clarity-core/src/approval/mod.rs`
- `crates/clarity-core/src/registry.rs`
- `crates/clarity-core/design/agent-lifecycle-rfc.md` (companion interface draft)
