# ADR-007: Turn Identifier Injection into `WireMessage`

> Status: Accepted — Phase A implemented (2026-05-18)  
> Date: 2026-05-17  
> Deciders: juice094 + Agent (architect layer)  
> Affects: `clarity-wire`, `clarity-core`, `clarity-egui`, `clarity-tui`, `clarity-gateway`  
> Relates to: ADR-006 (Protocol Layer Convergence)  
> Blocks: ADR-006 Phase B (EventBus removal — turn-level correlation required)

---

## 1. Context

### 1.1 Problem Statement

`WireMessage` (Gen-1 protocol per ADR-006) carries 14 variants that describe the full agent turn lifecycle (`TurnBegin`, `StepBegin`, `ContentPart`, `ToolCall`, `ToolResult`, `TurnEnd`, ...). **None of these variants carries a turn identifier.**

This creates three concrete failures:

| Failure mode | Evidence | Impact |
|-------------|----------|--------|
| **Cross-turn event interleaving** | `agent_runner.rs` creates a fresh `Wire::new()` per turn; the old `recv()` task may still emit buffered `ContentPart` chunks after `TurnEnd` while the new turn has already fired `TurnBegin` | UI displays tool calls from turn N-1 under turn N's header |
| **Gateway session reconstruction** | Gateway WebSocket streams `WireMessage` JSON to browser; without `turn_id`, the browser cannot group messages into turns for the "AgentTurn aggregation" view | All turns appear as one continuous chat |
| **Background task replay** | Background tasks serialize `WireMessage` to disk for later replay; replay loses turn boundaries | `TaskResult` shows a flat message list with no turn grouping |

### 1.2 Root Cause

The `Agent::run_turn` method (and its `execute_jumpy_mode` / `execute_plan_mode` variants) does **not** assign a unique identifier to the turn it is about to execute:

```rust
// crates/clarity-core/src/agent/construct.rs:766
self.send_wire_message(WireMessage::TurnBegin {
    user_input: query.to_string(),
    // No turn_id — the UI must infer boundaries from TurnBegin/TurnEnd pairs.
});
```

Pair-based boundary inference (`TurnBegin` ... `TurnEnd`) is fragile because:
- A panic or cancellation may skip `TurnEnd`, leaving the UI in "open turn" state.
- Multiple producers (soul + subagent + plan step) may share the same `Wire`, and their `TurnBegin` events nest or overlap.

### 1.3 Prior Art

ADR-006 §2 already mandates this change:

> "`WireMessage` (Gen-1) — **保留**，加入 `turn_id: u64` 字段（破坏性变更，ADR-007 单独决议）"

This ADR fulfills that mandate with concrete design and migration details.

---

## 2. Decision

**Add a `turn_id: String` field to every `WireMessage` variant.**

### 2.1 Why `String` instead of `u64`

ADR-006 proposed `u64`. After reviewing the codebase, `String` is preferred for these reasons:

1. **UUID v4 is already a project dependency** (used in `session_id`, `tool_call.id`, `plan.id`). Re-using `uuid::Uuid` avoids introducing a separate sequencing mechanism.
2. **Turn IDs are not ordered** — consumers must not assume `turn_id` is monotonic. UUID removes the temptation.
3. **Gateway / Web consumers** receive JSON; UUID strings are idiomatic in web protocols.
4. **Subagent turns** may originate from a different process (future Gateway distributed mode); a central u64 counter would require coordination. UUID is coordination-free.

### 2.2 Field Placement

Every `WireMessage` variant receives `turn_id` as its **first** field. This is a breaking serde change (field order matters for untagged/tuple variants), but because `WireMessage` uses `#[serde(tag = "type", rename_all = "snake_case")]` (externally tagged), field order is irrelevant — the change is backward-compatible for JSON consumers that ignore unknown fields.

```rust
pub enum WireMessage {
    TurnBegin {
        turn_id: String,
        user_input: String,
    },
    StepBegin {
        turn_id: String,
        tool_name: String,
    },
    ContentPart {
        turn_id: String,
        text: String,
    },
    // ... every variant follows the same pattern
}
```

### 2.3 Turn ID Lifecycle

```text
┌──────────────┐     generate UUID      ┌─────────────────┐
│  Agent::run  │ ──────────────────────▶ │   TurnContext   │
│  (entry)     │                         │   (turn_id)     │
└──────────────┘                         └─────────────────┘
        │                                          │
        │ send WireMessage::TurnBegin { turn_id }  │
        ▼                                          │
┌──────────────────────────────────────────────────┐
│  All subsequent WireMessage variants in this     │
│  turn carry the same turn_id.                    │
│  TurnEnd { turn_id } closes the boundary.        │
└──────────────────────────────────────────────────┘
```

Rules:
- `turn_id` is generated **once per `Agent::run` / `run_streaming` call** at the entry point.
- It is stored in `TurnContext` (new field) so that every `send_wire_message` call can reference it.
- Subagents spawned within a turn receive the **parent turn_id** (they do not mint their own) — subagent output is part of the parent turn.
- Plan mode: each plan step is a separate turn with its own `turn_id` (PlanStepBegin already has `step_id`; `turn_id` is orthogonal).

---

## 3. Impact Analysis

### 3.1 `clarity-wire`

| Change | File | Effort |
|--------|------|--------|
| Add `turn_id: String` to all 14 variants | `src/lib.rs` | Low |
| Update `WireMessage` tests (roundtrip serde) | `src/lib.rs` (tests) | Low |
| Update doc examples | `src/lib.rs` | Low |

### 3.2 `clarity-core`

| Change | File | Effort |
|--------|------|--------|
| Add `turn_id: String` to `TurnContext` | `src/agent/turn_context.rs` | Low |
| Generate UUID in `Agent::run` / `run_streaming` / `execute_plan_mode` | `src/agent/run.rs`, `src/agent/execution.rs` | Medium |
| Thread `turn_id` through all `send_wire_message` calls | `src/agent/construct.rs` | Medium |
| Update `Agent` unit tests | `src/agent/tests.rs` | Low |

### 3.3 `clarity-egui`

| Change | File | Effort | Pause-safe? |
|--------|------|--------|-------------|
| Thread `turn_id` through `agent_runner.rs` message translation | `src/services/agent_runner.rs` | Low | Yes |
| Update `Message` / `Session` structs to store `turn_id` | `src/ui/types.rs` | Medium | Yes |
| Group chat messages by `turn_id` for AgentTurn aggregation | `src/render_line.rs` or `src/chat.rs` | High | **No — touches render path** |
| Update `Snapshot` persistence to include `turn_id` | `src/stores/snapshot.rs` | Low | Yes |

**Note**: The high-effort render-path change is intentionally deferred to post-pause. The ADR mandates that `turn_id` is **stored** immediately, but **consumed** (grouping UI) can be incremental.

### 3.4 `clarity-tui`

| Change | File | Effort |
|--------|------|--------|
| Thread `turn_id` through `protocol_renderer.rs` | `src/protocol_renderer.rs` | Low |
| Store `turn_id` on `Message` equivalent | `src/ui.rs` | Low |

### 3.5 `clarity-gateway`

| Change | File | Effort |
|--------|------|--------|
| Include `turn_id` in WebSocket JSON payload (no-op — `WireMessage` serde handles it) | `src/ws.rs` | None |
| Update browser-side JS to group by `turn_id` | `static/app.js` | Medium |

---

## 4. Migration Path

### 4.1 Phase A — Wire + Core (本 ADR 同 PR 落地)

1. **Add `turn_id` to `WireMessage`** with a **default value** for backward compatibility during the transition.

   ```rust
   // Temporary: use `#[serde(default = "default_turn_id")]`
   // This allows old consumers (tests, gateway JS) to deser messages
   // that lack turn_id without crashing.
   ```

2. **Generate and thread `turn_id` in `clarity-core`**.
3. **Update all unit tests** to supply a dummy `turn_id`.
4. **Remove `#[serde(default)]`** in a follow-up PR once all consumers are updated.

### 4.2 Phase B — Frontend Storage (parallel session 收敛后)

1. `clarity-egui`: add `turn_id: String` to `Message` struct; persist it in session JSON.
2. `clarity-tui`: same for TUI message type.
3. `clarity-gateway`: update JS to group by `turn_id`.

### 4.3 Phase C — UI Consumption (长期)

1. AgentTurn aggregation view uses `turn_id` instead of `TurnBegin`/`TurnEnd` heuristics.
2. Background task replay reconstructs turn boundaries accurately.
3. Remove `TurnBegin`/`TurnEnd` pair-based inference code.

---

## 5. Rejected Alternatives

### RA-1: Keep `u64` counter in `AgentInner`

- **Rejected because**: requires `Mutex<u64>` or atomic; subagent/distributed mode would need a centralized counter. UUID is coordination-free.

### RA-2: Add `turn_id` only to `TurnBegin` and `TurnEnd`

- **Rejected because**: does not solve cross-turn interleaving. A consumer that joins the stream mid-turn has no way to know which turn the current `ContentPart` belongs to.

### RA-3: Use `session_id` + `turn_sequence` composite key

- **Rejected because**: adds complexity (consumers must parse composite keys); `session_id` is already present on `AgentConfig` but not on `WireMessage`. A single UUID is simpler.

### RA-4: Put `turn_id` on `Wire` instead of `WireMessage`

- **Rejected because**: `Wire` is a broadcast channel; multiple turns may share the same `Wire` instance (especially in Gateway multi-session mode). The ID must be per-message, not per-channel.

---

## 6. Consequences

### Positive

- Turn-level correlation becomes explicit and reliable.
- UI can render "AgentTurn cards" without fragile `TurnBegin`/`TurnEnd` heuristics.
- Background task replay preserves turn boundaries.
- Gateway WebSocket consumers can reconstruct session history accurately.

### Negative

- **Breaking change** for any external consumer of `WireMessage` JSON (e.g., custom WebSocket clients). Mitigation: serde default + changelog notice.
- **Slight payload increase**: +36 bytes per `WireMessage` (UUID string). At ~100 messages/turn, this is ~3.6 KB/turn — negligible.
- **All test fixtures** that construct `WireMessage` literals must be updated.

### Risks

| Risk | Mitigation |
|------|------------|
| Parallel session touches `WireMessage` | ADR-006 已标记 `WireMessage` 为保留类型；若 parallel session 正修改 `WireMessage` 变体，merge 冲突需人工解决。建议：本 PR 在 parallel session 收敛后 rebase。 |
| Gateway JS deser fails on missing `turn_id` | Phase A 使用 `#[serde(default)]` 兜底；JS 侧可忽略该字段直到 Phase B。 |

---

## 7. Implementation Checklist

- [x] `clarity-wire`: add `turn_id: String` to all `WireMessage` variants + serde default
- [x] `clarity-wire`: update unit tests (roundtrip, merging, broadcast)
- [x] `clarity-core`: add `turn_id` to `TurnContext`
- [x] `clarity-core`: generate UUID in `Agent::begin_turn`
- [x] `clarity-core`: thread `turn_id` through all `send_wire_message` calls
- [x] `clarity-core`: update agent tests
- [x] `clarity-egui`: thread `turn_id` through `agent_runner.rs`
- [ ] `clarity-egui`: add `turn_id` to `Message` / session persistence (Phase B)
- [x] `clarity-tui`: thread `turn_id` through `wire_adapter.rs`
- [x] `clarity-gateway`: verify WebSocket JSON carries `turn_id`
- [x] `clarity-claw`: update tray notification patterns
- [ ] Remove `#[serde(default)]` once all consumers updated (Phase C)

---

*End of ADR-007*
