---
title: Agent State Machine Design
category: Document
date: 2026-05-16
tags: [document, agent]
---

# Agent State Machine Design

> **Status**: Design document for Wave 3 (E-1 ~ E-4)  
> **Target**: `crates/clarity-core/src/agent/`  
> **Impact**: `clarity-core`, `clarity-tui`, `clarity-gateway`

---

## 1. Problem Statement

The `Agent` struct currently manages runtime lifecycle through implicit conventions rather than explicit state. This leads to:

1. **Overlapping runs** — Two concurrent calls to `Agent::run()` on the same clone are not rejected; the second call silently operates on a stale or reset `cancel_token`.
2. **Late error detection** — `llm` is `Option<Arc<dyn LlmProvider>>`; every entry point (`run`, `run_streaming`, `run_with_messages_sync`) repeats the same `ok_or_else(|| AgentError::Llm("No LLM provider configured"))` check deep in the call stack.
3. **Token freshness boilerplate** — `AgentController` manually calls `reset_cancel_token()` before every turn. Direct callers of `Agent::run()` get no help and must know this invariant.
4. **Session usage never resets** — `session_usage` accumulates forever across turns and clones, making per-turn token reporting unreliable.
5. **Skill bleed across turns** — `active_skill` is set globally with no turn boundary; a skill selected for turn N silently affects turn N+1.

---

## 2. Current Architecture

```
┌─────────────────────────────────────────────┐
│  AgentController                            │
│  ├─ ControllerState::Idle                   │
│  ├─ ControllerState::Running(handle)        │
│  └─ manually calls reset_cancel_token()     │
└─────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────┐
│  Agent (raw struct)                         │
│  ├─ cancel_token: CancellationToken         │
│  ├─ llm: Arc<RwLock<Option<Arc<dyn LlmProvider>>>>
│  ├─ session_usage: Arc<Mutex<TokenUsage>>   │
│  ├─ active_skill: Arc<RwLock<Option<String>>>
│  └─ ... other fields                        │
└─────────────────────────────────────────────┘
```

The `Agent` has no opinion about whether it is idle, running, or stalled. The controller maintains a parallel state machine (`ControllerState`) to compensate, but direct callers (Gateway per-request `AgentController`, TUI via `Op::UserTurn`) bypass this protection.

---

## 3. Proposed State Machine

### 3.1 Core Enum

```rust
use tokio_util::sync::CancellationToken;

/// Lifecycle state of an Agent instance.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentState {
    /// No LLM configured. `run()` is illegal.
    Unconfigured,
    /// Ready to accept a turn. `cancel_token` is guaranteed fresh.
    Idle,
    /// A turn is currently in progress on this (or a cloned) Agent.
    Running {
        /// Snapshot of the turn's token, used by `cancel()`.
        cancel_token: CancellationToken,
    },
    /// Previous turn was cancelled or the inner task panicked.
    /// Requires explicit reset (or implicit reset on next `run()` attempt).
    Stalled,
}
```

### 3.2 State Transitions

```
                    set_llm(provider)
         ┌─────────────────────────────────────┐
         │                                     │
         ▼                                     │
   ┌──────────────┐      run(query)            │
   │ Unconfigured │ ─────────────────► ┌───────┴──────┐
   └──────────────┘                    │              │
         ▲                             │   Running    │
         │         clear_llm()         │              │
         └─────────────────────────────┤              │
                                       └───────┬──────┘
                                               │
              ┌────────────────────────────────┘
              │ natural completion
              ▼
   ┌──────────────────────────────────────────┐
   │                  Idle                     │
   └──────────────────────────────────────────┘
         ▲                          │
         │ reset()                  │ cancel()
         └──────────────────────────┤
                                    ▼
                              ┌──────────┐
                              │  Stalled │
                              └──────────┘
                                    │
                                    │ reset() / implicit on run()
                                    ▼
                                  Idle
```

| Transition | Trigger | Action |
|---|---|---|
| `Unconfigured → Idle` | `set_llm()` | Store LLM, reset usage, set state = Idle |
| `Idle → Running` | `run()` / `run_streaming()` | Atomically swap state; generate fresh `cancel_token`; reset `session_usage`; capture `active_skill` snapshot |
| `Running → Idle` | Turn completes naturally | Set state = Idle |
| `Running → Stalled` | `cancel()` / panic | Set state = Stalled |
| `Stalled → Idle` | `reset()` or next `run()` | Reset token, clear stale skill, set state = Idle |
| `Idle → Unconfigured` | `clear_llm()` | Remove LLM, set state = Unconfigured |

### 3.3 Error Model

```rust
#[derive(Debug, thiserror::Error)]
pub enum AgentRunError {
    #[error("Agent is not configured with an LLM")]
    Unconfigured,
    #[error("Agent is already running a turn")]
    AlreadyRunning,
    #[error("Agent is in a stalled state; call reset() first")]
    Stalled,
}
```

---

## 4. Struct Refactoring

### 4.1 Agent Fields

Replace the free-floating `cancel_token` and `session_usage` with a unified runtime state.

```rust
#[derive(Clone)]
pub struct Agent {
    // --- Immutable configuration (setup-time only) ---
    registry: ToolRegistry,
    config: AgentConfig,
    memory_store: Option<Arc<dyn MemoryStore>>,
    memory_ticker: Option<MemoryTicker>,
    wire: Option<Arc<Wire>>,
    approval_runtime: Option<Arc<dyn ApprovalRuntime>>,
    approval_mode: ApprovalMode,
    compaction_config: CompactionConfig,
    max_context_tokens: usize,
    compaction_service: Option<CompactionService>,
    skill_registry: Option<SkillRegistry>,

    // --- Shared mutable runtime state ---
    inner: Arc<std::sync::RwLock<AgentInner>>,
}

struct AgentInner {
    state: AgentState,
    llm: Option<Arc<dyn LlmProvider>>,
    session_usage: TokenUsage,
    active_skill: Option<String>,
    file_prompt_cache: Option<String>,
}
```

**Key changes:**
- `llm` is lifted out of the outer `Arc<RwLock<Option<...>>>` into `AgentInner`.
- `cancel_token` is no longer a persistent field; it is created on-demand inside `Running`.
- `session_usage` is reset on every `Idle → Running` transition.
- `active_skill` is snapshotted at turn start so that subsequent `set_active_skill()` calls do not affect the in-flight turn.

### 4.2 Backward Compatibility

The public API surface is preserved:

| Old API | New Behavior |
|---|---|
| `Agent::with_llm()` | Sets `llm` and transitions `Unconfigured → Idle` |
| `Agent::set_llm()` | Same, plus notifies clones via `Arc<RwLock<AgentInner>>` |
| `Agent::clear_llm()` | Sets `llm = None`, transitions to `Unconfigured` |
| `Agent::cancel()` | No-op unless `Running`; transitions to `Stalled` |
| `Agent::reset_cancel_token()` | **Deprecated** — token lifecycle is managed automatically |
| `Agent::run(query)` | Returns `Err(AgentRunError::*)` immediately if state is illegal |

---

## 5. Caller Impact Analysis

### 5.1 AgentController (`clarity-core/src/agent/controller.rs`)

**Current:**
```rust
// Controller manually manages token + state
self.agent.reset_cancel_token();
let handle = tokio::spawn(async move { agent.run(query).await });
self.state = ControllerState::Running(handle);
```

**After:**
```rust
// Agent itself guarantees token freshness and rejects overlapping calls
let handle = tokio::spawn(async move { agent.run(query).await });
// run() atomically checks state; if already Running, returns Err immediately
self.state = ControllerState::Running(handle);
```

**Benefit:** Controller no longer needs `reset_cancel_token()`. Overlapping `Op::UserTurn` while one is in-flight will be rejected at the Agent level rather than leaking a background task.

### 5.2 TUI (`clarity-tui/src/app.rs`)

**Current:**
- TUI maintains `is_generating: bool` that shadows Agent state.
- If the agent errors out, TUI must manually call `handle_error()` to clear the flag.

**After:**
- TUI can query `agent.state()` to derive UI mode.
- `is_generating` becomes a pure function of `agent.state() == AgentState::Running`.
- No risk of flag/state desync on error paths.

### 5.3 Gateway (`clarity-gateway/src/handlers.rs`)

**Current:**
- Gateway creates a brand-new `AgentController` per HTTP request, sidestepping lifecycle issues.
- Hot-swaps the global LLM via `state.agent.read().await.set_llm(...)` while other requests hold clones.

**After:**
- Safe because `set_llm()` updates `AgentInner` behind `RwLock`; all clones see the new state.
- Per-request controller creation remains valid and safe.
- `admin_switch_provider` handler no longer needs to worry about overlapping requests because each gets its own controller.

---

## 6. Implementation Roadmap (Wave 3)

### E-1: Introduce `AgentState` + atomic transitions
**Scope:** `agent/mod.rs`, `agent/run.rs`, `agent/construct.rs`
**Work:**
1. Define `AgentState` enum and `AgentRunError`.
2. Introduce `AgentInner` struct.
3. Refactor `Agent` fields to use `inner: Arc<RwLock<AgentInner>>`.
4. Implement `run()` entry guard: check state, atomically transition `Idle → Running`, generate fresh `cancel_token`.
5. Return `AgentRunError` on illegal states.
6. Adapt `run_streaming` and `run_with_messages_sync` with the same guard.

**Tests:**
- `test_run_rejects_unconfigured_agent`
- `test_run_rejects_already_running_agent`
- `test_run_transitions_idle_to_running`
- `test_completion_transitions_running_to_idle`
- `test_cancel_transitions_running_to_stalled`

### E-2: `cancel_token` / `session_usage` lifecycle cleanup
**Scope:** `agent/run.rs`, `agent/construct.rs`, `agent/controller.rs`
**Work:**
1. Remove `cancel_token` field from `Agent`.
2. Remove `reset_cancel_token()` method.
3. Create `cancel_token` inside `Idle → Running` transition.
4. Reset `session_usage` on every `Idle → Running` transition.
5. Remove `reset_cancel_token()` call from `AgentController`.

**Tests:**
- `test_session_usage_resets_between_turns`
- `test_cancel_token_is_fresh_every_turn`

### E-3: `llm: Option<...>` → state expression
**Scope:** `agent/mod.rs`, `agent/construct.rs`
**Work:**
1. Move `llm` into `AgentInner`.
2. `with_llm()` / `set_llm()` transitions `Unconfigured ↔ Idle`.
3. `clear_llm()` transitions to `Unconfigured`.
4. Remove late `ok_or_else` checks inside `run_sync_loop`; the entry guard already guarantees `llm.is_some()`.

**Tests:**
- `test_set_llm_transitions_from_unconfigured`
- `test_clear_llm_transitions_to_unconfigured`
- `test_cloned_agent_sees_llm_update`

### E-4: Controller / TUI / Gateway adaptation
**Scope:** `agent/controller.rs`, `clarity-tui/src/app.rs`, `clarity-gateway/src/handlers.rs`
**Work:**
1. **Controller:** Remove manual `reset_cancel_token()` and `is_generating` shadow flag. Use `agent.state()` for state queries.
2. **TUI:** Replace `is_generating` bool with `matches!(agent.state(), AgentState::Running)`.
3. **Gateway:** No functional changes needed; verify per-request controller creation still works.

**Tests:**
- `test_controller_rejects_overlapping_turn`
- `test_tui_derives_generating_from_state`
- Full workspace test suite: `cargo test --workspace --lib`

---

## 7. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| `Arc<RwLock<AgentInner>>` adds lock contention | `AgentInner` is only accessed at turn boundaries, not inside the hot loop. Contention is negligible. |
| Breaking external callers who rely on `reset_cancel_token()` | API is deprecated with `#[deprecated]` first, then removed in a follow-up. Currently no external crate calls it directly. |
| State machine panics if `run()` is called while `Running` | Return `Err(AgentRunError::AlreadyRunning)` — never panic. |
| `cancel()` when `Idle` or `Stalled` | No-op or return `Err(CancelNotApplicable)`. Callers (Controller) already guard this. |

---

## 8. Success Criteria

- [x] `Agent::run()` rejects overlapping calls with a typed error.
- [x] `Agent::run()` rejects unconfigured agents with a typed error (not deep stack `Llm` error).
- [x] `reset_cancel_token()` is removed from public API.
- [x] `session_usage` resets on every new turn.
- [x] `active_skill` is snapshotted at turn start.
- [x] `cargo test --workspace --lib` passes (342 tests).
- [x] No manual state synchronization in TUI or Controller.
