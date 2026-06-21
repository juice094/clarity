---
title: ADR-018: Session-Scoped Event Routing for Streaming Responses and Claw
category: ADR
tags: [adr, egui, claw, session, concurrency]
---

# ADR-018: Session-Scoped Event Routing for Streaming Responses and Claw

> Status: Accepted — implemented (2026-06-21)  
> Date: 2026-06-21  
> Deciders: juice094 + Agent  
> Affects: `clarity-egui`, `clarity-openclaw`  
> Relates: ADR-007 (Turn Identifier Injection), `docs/notes/deepseek-device-chat-phase.md`

---

## 1. Context

### 1.1 Problem Statement

While testing the `deepseek-device` provider, we observed a **cross-session message leak**:

1. User sends "测试" in Session 1.
2. While the model is still streaming the reply, user switches to Session 2.
3. The remaining chunks of Session 1's reply are appended to **Session 2**.
4. The web/session storage side is correct; only the egui chat pane is confused until a refresh.

A similar risk exists in **Claw**:

- `ClawEvent::History` and `ClawEvent::RoleContextSynced` were merged into `active_session_mut()`.
- If the user switches Claw sessions while a history or role-context response is in flight, the data lands in the wrong session.
- After a Gateway connection is established, subscriptions are issued for the session that happened to be active at connect time; switching sessions later leaves the UI subscribed to the old key.

### 1.2 Root Cause

The `UiEvent` variants emitted by streaming backends did not carry the identity of the session that initiated the turn:

```rust
// BEFORE
pub enum UiEvent {
    Chunk(String),
    Done,
    Error(String),
    // ...
}
```

All handlers in `crates/clarity-egui/src/handlers/chat.rs` used `session_store.active_session_mut()`:

```rust
// crates/clarity-egui/src/handlers/chat.rs (before)
if let Some(session) = app.session_store.active_session_mut() {
    session.messages.push(msg);
}
```

Because egui is single-threaded and the active session is global UI state, any event arriving after a switch was routed to the newly selected session.

Claw inherited the same pattern: events like `History(messages)` and `RoleContextSynced { role_id, events, .. }` had no `session_key`, so the handler fell back to the active session.

### 1.3 Impact

| Failure mode | Evidence | Impact |
|-------------|----------|--------|
| **Cross-session chunk leak** | `UiEvent::Chunk` appended to `active_session_mut()` | User sees another session's reply in the current pane |
| **Wrong session history merge** | `ClawEvent::History` merged into active session | Claw session A receives session B's history |
| **Role context sprayed into active session** | `ClawEvent::RoleContextSynced` merged only into active session | Multiple Claw sessions of the same role get inconsistent state |
| **Stale subscriptions after switch** | `Connected` handler subscribes to active session_key once | Switching Claw sessions does not update Gateway subscriptions |
| **UI stuck in Loading after claw error** | `claw_in_flight_session_id` not cleared on reconnect | Input remains locked even though the connection has reset |

---

## 2. Decision

**Attach the originating session identity to every streaming/turn event and route handlers by that identity, not by the active session.**

Concretely:

1. Add `session_id: String` to all `UiEvent` variants that describe a turn or stream (`Chunk`, `Done`, `Error`, `ToolStart`, `ToolResult`, `ReasoningChunk`, `Draft*`, `StatusUpdate`, `Usage`, `SessionMeta`, `Compaction*`, `PlanStep*`, `TurnStart`, `TurnEnd`, `ShellResult`).
2. Add a runtime-only `in_flight: bool` flag to `Session`. It is **not persisted**; it exists only to know which session owns the current run.
3. Keep a **global single-run lock** (`view_state.turn == Loading`) because `state.agent` is a shared instance that is not safe for concurrent `run_streaming` calls.
4. In `agent_runner.rs`, capture the active `session_id` at send time and thread it through wire events, Claw events, subagent events, and shell results.
5. In `handlers/chat.rs`, replace `active_session_mut()` with `session_store.session_mut(session_id)` for message mutations.
6. For Claw:
   - Add `session_key: Option<String>` to `ClawEvent::History` and `ClawEvent::RoleContextSynced`.
   - Route `History` to the session matching `session_key` (falling back to active for legacy responses).
   - Route `RoleContextSynced` to **all** Claw sessions matching `role_id` (or to the single session matching `session_key` when provided).
   - On `switch_to_session`, if the target is a Claw session and a Gateway WebSocket is connected, re-subscribe and re-sync role context for the new key.
   - Clear `claw_in_flight_session_id` on `ReconnectPending` and `Error`.

---

## 3. Implementation

### 3.1 `UiEvent` now carries `session_id`

```rust
// crates/clarity-egui/src/ui/types.rs
pub enum UiEvent {
    Chunk {
        session_id: String,
        text: String,
    },
    Done {
        session_id: String,
    },
    Error {
        session_id: String,
        message: String,
    },
    SessionMeta {
        session_id: String,
        provider_state: HashMap<String, String>,
    },
    // ... other variants follow the same pattern
}
```

### 3.2 `Session` gains a runtime `in_flight` flag

```rust
// crates/clarity-egui/src/ui/types.rs
pub struct Session {
    // ... persisted fields ...

    /// Runtime-only flag: true while this session is waiting for a streamed
    /// response. Never persisted; used to keep per-session turn state.
    pub in_flight: bool,
}
```

`SessionStore` provides a by-id lookup:

```rust
// crates/clarity-egui/src/stores/session.rs
pub fn session_mut(&mut self, id: &str) -> Option<&mut Session> {
    self.sessions.iter_mut().find(|s| s.id == id)
}
```

### 3.3 `agent_runner.rs` captures the session at send time

```rust
// crates/clarity-egui/src/services/agent_runner.rs
let session_id = self.session_store.active_session_id.clone();
if let Some(session) = self.session_store.session_mut(&session_id) {
    session.in_flight = true;
}

self.runtime.spawn(async move {
    // ... wire subscriber uses the captured id ...
    while let Some(msg) = wire_ui.recv().await {
        dispatch_wire_message(msg, &session_id, &tx_wire);
    }

    // ... all emitted events carry session_id ...
    let _ = tx.send(UiEvent::Done { session_id: session_id.clone() });
});
```

The same pattern is applied to `send_claw()`, subagent runs (`/coder`, `/explore`), and `execute_shell_direct()`.

### 3.4 Handlers target by `session_id`

```rust
// crates/clarity-egui/src/handlers/chat.rs
pub fn on_chunk(
    session_store: &mut SessionStore,
    chat_store: &mut ChatStore,
    session_id: &str,
    text: String,
) {
    // Only mutate the target session, even if it is no longer active.
    if let Some(session) = session_store.session_mut(session_id) {
        // append to session.messages ...
    }
}
```

Global UI state is reset when a run finishes regardless of which session is active, because we keep a single active run:

```rust
pub fn on_done(app: &mut crate::App, session_id: &str) {
    if let Some(session) = app.session_store.session_mut(session_id) {
        session.in_flight = false;
    }
    app.view_state.turn = TurnState::Idle;
    app.chat_store.agent_status = AgentStatus::Online;
    // ...
}
```

### 3.5 Claw event routing

```rust
// crates/clarity-egui/src/claw.rs
pub enum ClawEvent {
    History {
        session_key: Option<String>,
        messages: Vec<ClawHistoryMessage>,
    },
    RoleContextSynced {
        role_id: String,
        session_key: Option<String>,
        events: Vec<ClawContextEvent>,
        // ...
    },
}
```

`main.rs` resolves the target session(s):

```rust
let target_id = session_key
    .as_deref()
    .and_then(|key| self.claw_session_id_by_key(key))
    .unwrap_or_else(|| self.session_store.active_session_id.clone());
if let Some(session) = self.session_store.session_mut(&target_id) {
    // merge history
}

let target_ids: Vec<String> = session_key
    .as_deref()
    .and_then(|key| self.claw_session_id_by_key(key))
    .map(|id| vec![id])
    .unwrap_or_else(|| self.claw_session_ids_by_role(&role_id));
for id in target_ids {
    // merge role context
}
```

### 3.6 Re-subscribe on session switch

```rust
// crates/clarity-egui/src/app_logic.rs
if let crate::ui::types::SessionContext::Claw { role, session_key, .. } = &session.context {
    if let Some(ref ws) = self.claw_ws {
        let is_openclaw = self.active_claw_protocol()
            == Some(crate::claw::ClawProtocol::OpenClawJsonRpc);
        if !is_openclaw {
            ws.subscribe_session(session_key);
            ws.subscribe_messages(session_key);
            ws.get_history(session_key);
        }
    }
    self.maybe_sync_claw_role_context(role);
}
```

---

## 4. Consequences

### 4.1 Positive

- **No more cross-session message leaks.** A stream's chunks always append to the session that initiated it.
- **Safe session switching during streaming.** The user can switch away from a loading session; the UI remains consistent and the response is persisted in the correct session.
- **Claw history/role-context no longer depends on active session timing.** Responses are routed by `session_key` or `role_id`.
- **Claw session switches update Gateway subscriptions.** The backend always streams the conversation the user is actually looking at.

### 4.2 Trade-offs and Constraints

- **Global single-run lock remains.** `view_state.turn` is still `Loading` while any session streams. This prevents concurrent `Agent::run_streaming` calls on the shared `state.agent`. A future ADR can explore per-session agent instances or a queue-based scheduler if concurrent turns are desired.
- **Event variant signatures changed.** Any new streaming/turn event must include `session_id` (or `session_key` for Claw) and be routed by identity from the start.
- **`ProtocolEvent` does not yet carry `session_key`.** We added the field at the `ClawEvent` layer and fall back to active-session/role-based routing. If `clarity-openclaw` later includes `session_key` in `History`/`RoleContextSynced` responses, the egui layer will use it automatically.

### 4.3 Future Work

- Propagate `session_key` through `clarity-openclaw::ProtocolEvent` so history/role-context responses can be matched exactly.
- Consider per-session agent instances or a turn queue if the product wants true concurrent turns across sessions.
- Extend the same identity-routing pattern to Gateway WebSocket clients (`clarity-gateway`) and TUI.

---

## 5. Checklist for Future Streaming Features

When adding a new backend event that produces session/chat state:

- [ ] Does the event variant carry `session_id` (or `session_key` for Claw)?
- [ ] Does the producer capture the originating session/key at send time?
- [ ] Does the handler use `session_store.session_mut(id)` instead of `active_session_mut()`?
- [ ] If the event mutates transient UI state (`draft_status`, `status_message`, `agent_status`), is it guarded by `active_session_id == session_id`?
- [ ] Does the handler save the target session even when it is not active?
- [ ] Are error/reconnect paths clearing any per-session in-flight markers?

---

## 6. Affected Files

- `crates/clarity-egui/src/ui/types.rs`
- `crates/clarity-egui/src/stores/session.rs`
- `crates/clarity-egui/src/stores/chat.rs`
- `crates/clarity-egui/src/session.rs`
- `crates/clarity-egui/src/services/agent_runner.rs`
- `crates/clarity-egui/src/services/wire_dispatcher.rs`
- `crates/clarity-egui/src/handlers/chat.rs`
- `crates/clarity-egui/src/handlers/mod.rs`
- `crates/clarity-egui/src/app_logic.rs`
- `crates/clarity-egui/src/main.rs`
- `crates/clarity-egui/src/claw.rs`
- `crates/clarity-egui/src/panels/modals/task_create.rs`
