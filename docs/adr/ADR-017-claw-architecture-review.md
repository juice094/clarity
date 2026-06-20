---
title: ADR-017: Claw Architecture Review — ZeroClaw, OpenClaw, and Multi-Mode Sessions
category: ADR
tags: [adr, claw, openclaw, zeroclaw, session, architecture]
---

# ADR-017: Claw Architecture Review — ZeroClaw, OpenClaw, and Multi-Mode Sessions

- Status: Proposed / Under Discussion
- Deciders: juice094
- Date: 2026-06-20
- Related: ADR-006 (protocol layer convergence), ADR-011 (workspace architecture), ADR-016 (pretext three-column layout)

## Context

Clarity has two overlapping "claw" concepts:

- **ZeroClaw**: the local `clarity-claw` system-tray daemon that talks to the local `clarity-gateway` (port `18790`). It is discoverable as a local bot instance (`zeroclaw-local`).
- **OpenClaw**: a compatibility client for external OpenClaw / KimiClaw Gateways (port `18789` by default). It can be local or remote and supports token, token+device, and device-paired authentication.

Both paths currently share the same `clarity_openclaw::ClawClient` WebSocket client and the same `clarity-egui` integration surface (`claw_ws`, `active_bot_id`, bot bar, right-rail Claw panels). However, they speak different protocols, have different session models, and are wired into the local `SessionStore` in different ways. The recent Ponytail audit flagged `ClawClient` as over-engineered because it spawns its own OS thread and tokio runtime, but that runtime issue cannot be decided in isolation: it depends on what claw is supposed to be in the overall architecture.

This ADR reviews the current state, clarifies the user-facing purpose of claw, explores what multi-mode sessions should mean, and proposes a path forward.

## Current State

### 1. ZeroClaw

`crates/clarity-claw` is a system-tray daemon. Its responsibilities today:

- Register itself with the local `clarity-gateway` (`POST /api/v1/claw/devices`).
- Poll Gateway for tasks and threads.
- Provide OS notifications and a tray menu.
- **Not wired to egui via WebSocket.** The federation runtime (`Coordinator`, `CoreNode`, `FederalAgentSession`) exists in `clarity-claw/src/coordinator/` and `src/runtime/` but is not connected to the tray loop or any I/O path.

`clarity-egui` discovers ZeroClaw in `src/claw.rs::discover_zeroclaw()` by always injecting a bot at `127.0.0.1:18790` with status `Online`, without probing whether the daemon or Gateway is alive.

When the user selects the ZeroClaw bot, `clarity-egui` opens a WebSocket with `clarity_openclaw::ClawClient`. This is the first protocol mismatch: `ClawClient` speaks the OpenClaw JSON-RPC protocol (`type: req`, `method: connect/chat.send`), while `clarity-gateway /ws` expects the Clarity-native protocol (`type: chat/ping/get_history`). So the ZeroClaw WebSocket path is currently broken at the protocol level.

### 2. OpenClaw

`crates/clarity-openclaw` is a UI-agnostic client. Its public API (`src/client.rs`):

- `connect(url, token)` / `connect_with_device(...)` / `connect_with_remote_device(...)`
- `send_message(session_key, text)` → Gateway method `chat.send`
- `send_session_message(key, text)` → Gateway method `sessions.send`
- `fetch_history`, `subscribe_session`, `subscribe_messages`, `request_pairing`, `send_raw_request`, `drain`

The client spawns a background `std::thread` and creates a fresh `tokio::runtime::Runtime` inside that thread. Commands cross from the UI thread via `std::sync::mpsc`; the background thread bridges them to the async WebSocket loop with `tokio::task::spawn_blocking` and a blocking `recv()`.

Pairing flow (`clarity-egui/src/main.rs`):

1. Load or generate an Ed25519 `DeviceIdentity`.
2. Connect with admin token and call `request_pairing()`.
3. User approves on the Gateway.
4. Save the returned device token to the matching `OpenClawConnection` and to `~/.clarity/claw-device-token.json`.

Discovery (`crates/clarity-openclaw/src/discovery.rs`) reads local KimiClaw config (`~/.kimi_openclaw/openclaw.json`), paired devices, and env vars `OPENCLAW_REMOTE_URL` / `OPENCLAW_REMOTE_TOKEN`.

### 3. Session Model

Local sessions live in `crates/clarity-egui/src/stores/session.rs::SessionStore`. A session has:

- `id`, `title`, `category`
- `context: SessionContext { Chat, Project { project_id, has_workspace }, Claw { device_id } }`
- `lifecycle`, `archived`, `messages`

`SessionStore::active_category` is initialized to `"engineering"` and is never changed by the current UI. It is used only to title new sessions and to infer an initial `SessionContext`.

When a Claw bot is active, `agent_runner.rs::send_claw()` sends the user message over `claw_ws` to a hard-coded Gateway session key `"agent:main:main"`. Replies are drained every frame and merged into whichever local session happens to be active. History is fetched once on connect and merged by content deduplication. There is no 1:1 mapping between a local `Session` and a Gateway/OpenClaw session.

### 4. Work / Chat Context (after recent cleanup)

The Work/Chat toggle was removed in the previous refactor. What remains:

- `AppView::Work` in `clarity_core::ui` still exists and dispatches to `panels/work/mod.rs`, but there is no UI entry point to reach it.
- `WorkTemplate` is just a `{ name, prompt }` shortcut; clicking it creates a normal session with a pre-filled composer.
- `SessionContext` (`Chat`/`Project`/`Claw`) is the real current axis, but it is often inferred from `category`/`title` heuristics in `bot_bar.rs` rather than set explicitly.
- The right-rail now uses `RightRailPanel` selected by the bot bar based on `SessionContext`.

## Problem Statement

1. **Claw is two different things wearing one UI coat.** ZeroClaw and OpenClaw have different protocols, different discovery mechanisms, and different session semantics, but they are presented to the user as interchangeable "bots" in the same list.
2. **The session model is unclear.** A local `Session` can forward messages to a Gateway session, but the relationship is implicit (`"agent:main:main"`), not owned by the session data.
3. **The runtime bridge is a symptom, not the root cause.** `ClawClient` needs its own thread/runtime because it was designed as a self-contained black box. If it were a normal async client owned by `clarity-egui`, it would just use `app.runtime`.
4. **Work/Chat mode is unresolved.** We removed the toggle, but the concept of "work session" vs "chat session" still surfaces in `AppView::Work`, `WorkTemplate`, `SessionContext::Project`, and the bot-bar right-rail panels.

## Multi-Mode Sessions: What Should We Expect?

Before refactoring the runtime, we need to decide what "multi-mode" means for the user. Here are the candidate models and their implications.

### Model A: Session Context is the Mode

A session has a `SessionContext` that determines its behavior and tooling:

- `Chat`: normal local/agent conversation.
- `Project`: tied to a project directory; right rail shows Files, Console, Knowledge.
- `Claw`: tied to a remote Gateway/device; right rail shows Settings, Workspace, Terminal, WebBridge.

Implications:
- Mode is a property of the session, not a global UI toggle.
- Switching sessions switches mode.
- Work templates become "create a Project-context session from this prompt".
- `AppView::Work` can be removed; "work" is just a Project session.

### Model B: Global Mode Switch

The UI has a persistent "Work / Chat" switch that changes the available tools and default new-session behavior, independent of the active session.

Implications:
- Re-introduces a global mode state (but simpler than `NavContext`: just a `UiStore` enum).
- Same session can be viewed in Chat mode or Work mode.
- More complex state management; can lead to the "floating multi-select" class of UI bugs we just fixed.

### Model C: Mode as a View Lens

Mode is not state at all; it is a layout lens. The user can open the same session in a chat view, a project dashboard view, or a claw terminal view.

Implications:
- Richer UI, but significantly more complexity.
- Requires a clean separation between session data and view state.
- Probably overkill for the current scope.

### Recommendation

**Adopt Model A: `SessionContext` is the mode.** It matches the data model that already exists after the cleanup, eliminates the orphaned `AppView::Work`, and avoids the global-mode UI bugs we just removed. The left sidebar should be a neutral navigation tree; mode is determined by which session you open.

## Options for the Claw Runtime

Given Model A, we can evaluate the runtime refactor options.

### Option 1: Minimal Refactor — Async Client on Shared Runtime

- Convert `ClawClient` to use `tokio::sync::mpsc` and run its WebSocket task on a provided `tokio::runtime::Handle`.
- Keep the public API mostly synchronous-looking for egui (`connect` returns a handle; `drain()` polls).
- Remove the OS thread and internal tokio runtime.

Pros:
- Removes the over-engineered bridge.
- Low risk if we keep egui call sites similar.
- Works regardless of whether ZeroClaw and OpenClaw keep sharing one client.

Cons:
- Does not fix the ZeroClaw/OpenClaw protocol mismatch.
- Does not fix the hard-coded `"agent:main:main"` session key.

### Option 2: Protocol Split — Separate ZeroClaw and OpenClaw Clients

- Give ZeroClaw its own lightweight client that speaks the Clarity-native Gateway WebSocket protocol.
- Keep OpenClaw client only for external OpenClaw/KimiClaw Gateways.
- Both clients run on `app.runtime`.

Pros:
- Correctly models the two different protocols.
- Allows ZeroClaw to use `WireMessage`/`FederationMessage` in the future.

Cons:
- More code than Option 1 in the short term.
- Requires defining the Clarity-native Gateway WebSocket protocol more precisely.

### Option 3: Unified Abstraction — `ClawTransport` Trait

- Define a `ClawTransport` trait with `send(session_id, message)` and `subscribe() -> Stream<Event>`.
- Implement `OpenClawTransport` (JSON-RPC) and `ZeroClawTransport` (native Gateway WS).
- `ClawClient` becomes a wrapper around a boxed transport.

Pros:
- Clean architecture for supporting more transports later.
- Hides protocol differences from egui.

Cons:
- Adds an abstraction layer that currently has only two implementations.
- Ponytail would flag this as YAGNI until a third transport is needed.

### Recommendation

**Start with Option 1 (minimal async refactor).** It directly addresses the audit finding without expanding scope. Then, **only after** the ZeroClaw protocol path is validated and the session-mapping problem is solved, decide whether to split into Option 2. Avoid Option 3 until there is a third transport on the roadmap.

## Concrete Next Steps

1. **Remove dead mode artifacts.**
   - Delete `AppView::Work` and `panels/work/mod.rs` (or move its useful pieces into `RightRailPanel::ClawWorkspace`).
   - Remove `SessionStore::active_category` zombie; new sessions default to `SessionContext::Chat` unless explicitly created as Project/Claw.

2. **Do not refactor `ClawClient` runtime yet.**
   - First fix the protocol mismatch for ZeroClaw or explicitly decide that ZeroClaw in egui should not use WebSocket at all (e.g., use Gateway HTTP polling like the tray daemon does).

3. **Introduce explicit session-to-device binding.**
   - When a session is created with `SessionContext::Claw { device_id }`, store the device id and the Gateway session key.
   - The send path should use that key, not `"agent:main:main"`.

4. **Revisit after ADR-016 Pretext layout is further along.**
   - The left sidebar and right rail are being redesigned. Claw device selection and claw-specific tools should fit into the new three-column layout before we invest in a deep transport refactor.

## Consequences

- **Positive**: We avoid a premature runtime refactor that would make the ZeroClaw/OpenClaw protocol mismatch harder to see.
- **Positive**: We align "mode" with `SessionContext`, which simplifies the mental model and removes the orphaned Work view.
- **Negative**: `ClawClient` keeps its self-owned thread/runtime a bit longer. This is acceptable technical debt while the larger architecture is being decided.
- **Negative**: Some claw right-rail panels remain placeholders until the architecture is finalized.

## Notes

- The OpenClaw examples (`crates/clarity-egui/examples/openclaw_*.rs`) currently rely on `send_raw_request` and `Reply.method`. Any future client refactor must either preserve these escape hatches or update the examples.
- `clarity-contract/src/federation.rs` defines `FederationMessage` and `FederationNode`. This is the long-term shape for distributed/agentic collaboration, but it is not yet wired to either ZeroClaw or OpenClaw.
