# Plan: Clarity Future Direction Roadmap (v0.3.0 → v0.5.0)

## Problem Statement

Clarity v0.3.0 four-stage hardening is complete, but the architectural gap between current codebase and the "cluster-as-single-node" ambition has been audited. Nine capability gaps exist across Agent runtime, Wire protocol, Session storage, Background tasks, MCP transport, and multi-process architecture.

This plan produces a phased technical roadmap to close these gaps while respecting the Hard Veto constraints (project breadth ≤ 5 core tools, no Docker/RAG/Electron, local-LLM priority, sovereignty defense).

## Target Deliverable

After approval, write `docs/FUTURE_DIRECTION.md` — a long-term technical guidance document answering "where does Clarity go from here."

## Design Approach

**Single recommendation: Four-phase incremental roadmap**

Core principle: Do not add new crates. Refactor and extend existing crates within the 6-crate boundary. Replace before adding.

| Phase | Theme | Duration | Invasiveness |
|-------|-------|----------|--------------|
| A | Infrastructure unification (quick wins) | 2 weeks | Low |
| B | Session layer consolidation | 2-3 weeks | Medium |
| C | Runtime refactoring (Hub-Worker + multi-window) | 4-6 weeks | High |
| D | Cross-device validation (Syncthing-Rust integration) | 4-6 weeks | High |

---

## Phase A: Infrastructure Unification (2 weeks)

Goal: Close low-hanging gaps with minimal code disruption. Deliver immediate value while laying groundwork for later phases.

### A1. WebSocket MCP Transport (2-3 days)
- Extend `McpTransport` enum with `WebSocket { url, headers }`
- Implement `McpClient` for WebSocket using `tokio-tungstenite`
- Convert `McpTransport` from closed enum to trait-based registry (optional, defer to Phase B if complex)
- Validation: Connect to a WebSocket MCP server and execute tool calls

### A2. Tauri ↔ BackgroundTaskManager Integration (2-3 days)
- Add `BackgroundTaskManager` to `AppState` in `clarity-tauri`
- Bridge `NotificationManager` events to Tauri event bus (`task:update`, `task:complete`)
- Replace standalone `TaskRecord` JSON store with `BackgroundTaskManager` APIs
- Validation: Create a background task from Tauri frontend, observe progress events, confirm persistence

### A3. Worker Pool Auto-scaling (2-3 days)
- Remove `_min_workers` / `_max_workers` underscore prefixes in `ScalableWorkerPool`
- Implement scale-up when queue length > threshold, scale-down when idle
- Validation: Simulate burst task load, observe worker count changes

### A4. Cross-session Memory Retrieval (2-3 days)
- Extend `clarity-memory` SQLite `session_notes` queries to support cross-session full-text search
- Add API: `search_all_sessions(query, limit)`
- Validation: Create 3 sessions, search for a term, confirm results from all sessions

---

## Phase B: Session Layer Consolidation (2-3 weeks)

Goal: Replace the dual disjoint session storage systems with a single SQLite-backed session layer. Enable Session Handoff.

### B1. Unified Session Schema (3-4 days)
- Design `sessions` table in SQLite:
  ```sql
  CREATE TABLE sessions (
      id TEXT PRIMARY KEY,
      title TEXT,
      created_at INTEGER,
      updated_at INTEGER,
      parent_session_id TEXT,  -- for Handoff lineage
      handoff_document TEXT,   -- JSON serialized
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
- Migration: Read all existing `sessions/{id}.json` and `sessions/{id}.jsonl`, import to SQLite
- Validation: All existing sessions load correctly after migration

### B2. SessionManager Abstraction (3-4 days)
- Create `SessionManager` in `clarity-core` (or extend `clarity-memory`)
- Methods: `create()`, `load()`, `save_message()`, `search()`, `handoff(source_id, target_id)`
- Replace Tauri `session.rs` JSON I/O with `SessionManager` calls
- Replace `clarity-memory` `SessionStore` JSONL with `SessionManager`
- Validation: Full feature parity with previous dual system

### B3. Session Handoff (3-5 days)
- Define `HandoffDocument` struct:
  ```rust
  struct HandoffDocument {
      session_id: Uuid,
      target_session_id: Uuid,
      context_summary: String,
      decisions: Vec<Decision>,
      pending_tasks: Vec<Task>,
      agent_state: AgentStateSnapshot,
      soul_fingerprint: String,  // SOUL.md hash
      timestamp: DateTime<Utc>,
      ttl: Duration,
  }
  ```
- Implement `session_manager.handoff(source, target)`
- On new session creation, auto-detect pending handoffs and prompt user
- Validation: Session #1 handoff → Session #2 loads context + decisions

### B4. Session Event Bus (2-3 days)
- Emit Tauri events on session mutations (`session:message_added`, `session:handoff_available`)
- Frontend listens and updates reactively
- Validation: Open two Settings panels, create session in one, other panel updates

---

## Phase C: Runtime Refactoring — Hub-Worker + Multi-Window (4-6 weeks)

Goal: Transform single-Agent single-process assumption into multi-Agent Hub-Worker scheduler. Enable multi-window as multi-node validation.

### C1. AgentInstance + AgentPool (5-7 days)
- Refactor `AgentController` from single-Agent to AgentPool manager
- New types:
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
      default_instance: Uuid,  // "Gray" anchor
  }
  ```
- `AgentPool` routes `Op::UserTurn` to appropriate instance by identity/window
- Validation: Create 2 Agent instances, send turns to both concurrently

### C2. Identity Routing (3-4 days)
- Define `Identity` enum: `Gray`, `Kimi`, `Analyst`, `Programmer`, `Auditor`, `Custom(String)`
- Define `ModelSpec`: `Local { model_id }` | `Remote { provider, model }` | `Hybrid { ... }`
- `AgentInstance` binds identity + model_spec at creation
- Hub routing strategy: `ByTask`, `ByCapability`, `ByIdentity`, `GrayDirect`
- Validation: Route "code review" task to Programmer instance, "data analysis" to Analyst instance

### C3. Wire Protocol Extension for Inter-Agent Messaging (4-5 days)
- Extend `WireMessage`:
  ```rust
  AgentMessage {
      from: Uuid,
      to: Uuid,
      payload: MessagePayload,
  }
  AgentStateSnapshot { instance_id, state_json }
  ```
- Add `MessageEnvelope` with routing metadata
- Validation: Instance A sends message to Instance B, B receives and responds

### C4. IPC Transport Layer (4-6 days)
- Implement `Transport::Ipc` in `clarity-wire`
- Cross-platform: TCP 127.0.0.1 as universal fallback, UDS (Linux/macOS) and Named Pipe (Windows) as optimizations
- Key requirement: Message format identical to TCP穿透 — cluster semantics validated today on loopback, extended tomorrow to P2P
- Validation: Two processes communicate via Wire over TCP loopback, message boundaries correct

### C5. Multi-Window State Model (3-5 days)
- Refactor `AppState`:
  ```rust
  pub struct AppState {
      agent_pool: Arc<RwLock<AgentPool>>,  // replaces single Agent
      // ... other fields
  }
  ```
- Each Tauri window gets a `window_id`; `AgentPool` routes by window
- Background tasks broadcast to all windows via Tauri event bus
- Validation: Open 2 Tauri windows, each chats independently, background task progress visible in both

### C6. Gray Anchor Hard-binding (2-3 days)
- `AgentPool::default_instance` always points to Identity::Gray
- Gray instance: always local-LLM, offline-capable, auto-created on startup
- SOUL.md hash checked on startup; mismatch logs warning
- Validation: Disconnect network, confirm Gray instance still responds

---

## Phase D: Cross-Device Validation — Syncthing-Rust Integration (4-6 weeks)

Goal: Extend validated cluster semantics from single-device to multi-device via Syncthing-Rust P2P layer.

### D1. Device Identity & Discovery (1 week)
- Syncthing-Rust device certificate as Clarity device identity
- Local device registry: `devices` table in SQLite (device_id, label, last_seen, trust_level)
- Validation: Two Clarity instances discover each other on LAN

### D2. Session CRDT Synchronization (2 weeks)
- Integrate CRDT library (Loro or crdt-kit) for session message merging
- Conflict resolution: last-writer-wins for metadata, append-only for messages
- Sync trigger: Syncthing file watcher detects remote session update
- Validation: Device A adds message to Session #1, Device B sees it within sync interval

### D3. Agent State Migration (1-2 weeks)
- Serialize `AgentInstance` state (not full memory, just turn-level context) to portable format
- Transmit via Syncthing-Rust encrypted channel
- Validation: Start turn on Device A, migrate to Device B, continue seamlessly

### D4. P2P Wire Protocol (1 week)
- Wire `Transport::P2P` variant using Syncthing-Rust TLS channels
- Device-to-device AgentMessage routing
- Validation: Device A's Agent α sends message to Device B's Agent β

---

## Technical Choices & Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| IPC primary transport | TCP 127.0.0.1 | Universal, no platform-specific code; UDS/Named Pipe as future optimization |
| Session storage unification | SQLite single source | Replaces JSON+JSONL duality; FTS5 for search; WAL for concurrency |
| CRDT library | Loro (Rust core) | Mature, delta sync, WASM-ready if frontend needs it later |
| Agent pool concurrency | `tokio::sync::RwLock<HashMap>` | Simple, sufficient for single-node; distributed lock deferred to Phase D |
| Multi-process vs multi-thread | Multi-thread first | Tauri backend is single-process multi-thread; multi-process (IPC) validated as extension |

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Phase C refactoring breaks existing Tauri/Gateway/TUI | High | Maintain `AgentController` backward-compat API; `AgentPool` wraps it |
| Session migration loses data | High | Migration tool with dry-run + backup; validation suite before deleting JSON |
| Project breadth exceeds 5-core limit | High | Phase A-D do not add crates; only refactor existing 6 crates |
| BackgroundTaskManager integration destabilizes | Medium | Feature-flag integration; fallback to old task system if errors |
| CRDT sync performance poor | Medium | Benchmark early; fallback to simple last-writer-wins if CRDT overhead > 50ms |

## Success Criteria

- Phase A: All 4 quick-wins merged, zero regression in `cargo test --workspace --lib`
- Phase B: Single `SessionManager` API; all sessions in SQLite; Handoff functional
- Phase C: 2+ Agent instances run concurrently; multi-window chat works; IPC loopback validated
- Phase D: Two Clarity devices sync sessions; Agent state migrates across devices

## Resource Constraints

- No new crates (refactor within existing 6)
- Rust core modules implemented directly (no subagent outsourcing)
- Each phase ends with `cargo test --workspace --lib` + `npm run build` green
- Phase A and B can parallel with v0.3.x patch releases; Phase C and D target v0.4.0/v0.5.0
