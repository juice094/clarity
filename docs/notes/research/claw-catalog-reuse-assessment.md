# Claw Catalog Reuse Assessment

## Summary

The two Claw-related crates (`clarity-claw` and `clarity-openclaw`) are focused on
Claw / OpenClaw / KimiClaw Gateway connectivity, device identity, protocol
auto-detection, and system-tray integration. After reading every source file in
both crates, **there is no reusable code for the proposed `clarity-llm` model
catalog redesign**. None of the crates fetch model lists, cache provider
catalogs, or implement any catalog/registry abstraction that overlaps with
`ModelCatalogService`.

The only remote data fetching is WebSocket JSON-RPC / native Gateway protocol
communication for chat, tasks, threads, device registration, and role-context
sync. Local file caching is limited to device identity (`claw-device.json`),
pairing tokens (`claw-device-token.json`), OpenClaw config discovery, and
syncthing-rust role-context event files. These patterns are structurally
different from the proposed `ModelCatalogFetcher` / cache / bootstrap merge
pipeline.

---

## 1. What each crate does

### `crates/clarity-claw/`

A system-tray resident node for Clarity's internal mesh.

- **Entry point**: `src/main.rs` registers the instance as a Claw device with
  `clarity-gateway`, starts a periodic heartbeat, and runs the tray event loop.
- **Protocol**: speaks **only** the native Clarity Gateway WebSocket protocol
  (`clarity-gateway` on port 18790). It explicitly does not implement OpenClaw
  JSON-RPC.
- **Core interactions** (`src/lib.rs`):
  - `register_device` / `send_heartbeat` — device registry with the Gateway.
  - `poll_tasks` / `poll_threads` — periodic polling of task/thread state.
  - `quick_chat` / `create_remote_task` / `cancel_remote_task` /
    `create_remote_thread` — user-facing tray actions.
- **Tray UI** (`src/tray/mod.rs`): `tao` + `tray-icon` event loop, OS
  notifications, filesystem watcher on `.clarity/tasks`, native input dialogs.

### `crates/clarity-openclaw/`

A UI-agnostic library for talking to OpenClaw / KimiClaw Gateways.

- **`src/client.rs`** — persistent WebSocket JSON-RPC client. Handles token-only,
  device-paired, and token-with-device authentication; challenge signing;
  reconnect backoff; chat / history / subscription / pairing commands; and
  OpenClaw/KimiClaw streaming event parsing.
- **`src/gateway_client.rs`** — native Clarity Gateway WebSocket client
  (`WsRequest`/`WsResponse`).
- **`src/connection_manager.rs`** — protocol auto-detection that probes the
  server and dispatches to either `GatewayClient` or `ClawClient`.
- **`src/discovery.rs`** — reads `~/.kimi_openclaw/openclaw.json`,
  `~/.kimi_openclaw/devices/paired.json`, and env vars
  (`OPENCLAW_REMOTE_URL`, `OPENCLAW_REMOTE_TOKEN`, `OPENCLAW_REMOTE_NAME`) to
  produce `DeviceRecord`s.
- **`src/device.rs`** — Ed25519 device identity generation/persistence and
  paired-token save/load.
- **`src/protocol.rs`** — unified `ProtocolCommand`/`ProtocolEvent` abstraction
  over both dialects.
- **`src/types.rs`** — `DeviceInfo`, `ClawConnection`, `DeviceRecord`, etc.
- **`src/mesh/`** — distributed role-context sync via Gateway WebSocket or
  syncthing-rust, with CRDT merger and passphrase-based encryption.

---

## 2. Reusability for model catalog redesign

**Verdict: no direct reuse.**

The model catalog redesign plan (`docs/notes/plans/model-catalog-redesign.md`)
calls for:

- `ModelCatalogFetcher` trait with provider-family-specific fetchers.
- `OllamaFetcher` (`GET /api/tags` or `https://ollama.com/api/tags`).
- `OpenAiCompatibleFetcher` (`GET /v1/models`).
- Filesystem cache under `~/.clarity/catalogs/{provider_id}.json`.
- Merge priority: `models.toml` user override → cached remote → bootstrap seed.
- `ModelCatalogService` exposing `list`, `list_for_provider`, `refresh`.

None of these concerns appear in the Claw crates:

| Concern | Present in Claw crates? | Notes |
|---------|------------------------|-------|
| HTTP client calls to `/api/tags`, `/v1/models`, or provider model endpoints | **No** | Only WebSocket to Gateway / OpenClaw JSON-RPC. |
| Model/provider listing or selection logic | **No** | No model IDs, no provider catalog. |
| Caching remote data to local files | Partial | Only device identity, pairing token, OpenClaw config, role-context events. |
| Fallback to hard-coded values when remote unreachable | **No** | No hard-coded model or provider lists. |
| "Catalog" / "registry" abstraction | **No** | The word "registry" appears only as "Gateway registry" for device registration. |

The closest conceptual overlap is **local file caching of remote-ish data**, but
even that is structurally different:

- Claw caches are single JSON files for identity/tokens, not TTL-based catalogs
  of provider model lists.
- Claw does not implement a fetcher trait, merge pipeline, or refresh API.

---

## 3. Relevant file paths and line numbers

Because there is **no** catalog/model-discovery code in these crates, the
relevant references are negative findings.

### No model/catalog code

- `crates/clarity-claw/src/lib.rs` — no model/provider/catalog references.
- `crates/clarity-claw/src/main.rs` — no model/provider/catalog references.
- `crates/clarity-claw/src/tray/mod.rs` — no model/provider/catalog references.
- `crates/clarity-openclaw/src/lib.rs` — no model/provider/catalog references.
- `crates/clarity-openclaw/src/client.rs` — no model/provider/catalog references.
- `crates/clarity-openclaw/src/gateway_client.rs` — no model/provider/catalog
  references.
- `crates/clarity-openclaw/src/connection_manager.rs` — no model/provider/catalog
  references.
- `crates/clarity-openclaw/src/discovery.rs` — no model/provider/catalog
  references.
- `crates/clarity-openclaw/src/device.rs` — no model/provider/catalog references.
- `crates/clarity-openclaw/src/protocol.rs` — no model/provider/catalog
  references.
- `crates/clarity-openclaw/src/types.rs` — no model/provider/catalog references.
- `crates/clarity-openclaw/src/mesh/*.rs` — no model/provider/catalog
  references.

### Grep confirmation

```text
crates/clarity-claw/src/lib.rs:304:/// Send a heartbeat to keep the device alive in the Gateway registry.
```

This is the only occurrence of "registry" in `clarity-claw`; it refers to the
Gateway's device registry, not a model registry.

### What *is* present (for structural comparison)

| File | Lines | What it does |
|------|-------|--------------|
| `crates/clarity-claw/src/lib.rs` | 42–45 | `GATEWAY_URL`, `POLL_INTERVAL_SECS` constants. |
| `crates/clarity-claw/src/lib.rs` | 75–78 | `resolve_gateway_url()` — env-var-based URL resolution. |
| `crates/clarity-claw/src/lib.rs` | 122–158 | `gateway_ws_request()` — one-shot WebSocket request helper. |
| `crates/clarity-claw/src/lib.rs` | 282–302 | `register_device()` — device registration payload. |
| `crates/clarity-claw/src/lib.rs` | 304–318 | `send_heartbeat()` — keepalive payload. |
| `crates/clarity-openclaw/src/discovery.rs` | 53–61 | `discover_openclaw_devices()` — local + remote device discovery. |
| `crates/clarity-openclaw/src/discovery.rs` | 198–214 | `resolve_openclaw_home()` — env-var + platform home resolution. |
| `crates/clarity-openclaw/src/discovery.rs` | 216–223 | `read_openclaw_config()` — read JSON config from disk. |
| `crates/clarity-openclaw/src/device.rs` | 127–131 | `device_identity_path()` — persist device identity. |
| `crates/clarity-openclaw/src/device.rs` | 185–189 | `device_token_path()` — persist paired token. |

---

## 4. Comparison: Claw approach vs. proposed `ModelCatalogService`

| Dimension | Claw crates | Proposed `ModelCatalogService` |
|-----------|-------------|--------------------------------|
| **Primary protocol** | WebSocket JSON-RPC / native Gateway WebSocket | HTTP `GET` for `/api/tags`, `/v1/models`, etc. |
| **Data fetched** | Chat messages, task/thread state, role-context events, pairing results | Model IDs, display names, capabilities, quantization, context length |
| **Cache location** | `~/.clarity/claw-device.json`, `~/.clarity/claw-device-token.json`, `~/.kimi_openclaw/` | `~/.clarity/catalogs/{provider_id}.json` |
| **Cache semantics** | Identity/token persistence; no TTL | TTL-based remote catalog cache |
| **Fetch trigger** | Continuous connection + periodic polling | Manual user refresh; no background polling |
| **Fallback strategy** | Hard-coded protocol constants (Origin, scopes, client ID) | Hard-coded bootstrap seed per provider family |
| **Merge pipeline** | None | User override → cache → bootstrap |
| **Provider abstraction** | `ClawConnection` + `ClawProtocol` enum | `ModelCatalogFetcher` trait per provider family |
| **Consumer** | System tray, Gateway, egui OpenClaw connection | Settings UI model picker, runtime router hints |
| **Overlap** | None | N/A |

---

## 5. Ponytail-style code review notes for the Claw crates

These notes are read-only observations; no source files were modified.

### `crates/clarity-claw`

#### Unnecessary complexity

- `tray/mod.rs` mixes three responsibilities in one file: tray UI event loop,
  Gateway polling, and native OS input dialogs. At 689 lines it is within the
  300-line-per-function guideline but the module itself is large. Consider
  splitting into `tray/ui.rs`, `tray/poller.rs`, and `tray/dialog.rs` if it
  grows further.
- `tray_runtime()` (lines 26–38) uses a `static Mutex<Option<Arc<Runtime>>>`. A
  single `std::sync::OnceLock<Arc<Runtime>>` would remove the `None` branch and
  the `parking_lot` dependency from this crate if not otherwise needed.
- Tooltip update logic is duplicated in `UserEvent::TaskUpdate` (lines 505–511)
  and `UserEvent::ThreadUpdate` (lines 513–521). A small helper
  `update_tooltip(&task_cache, &thread_cache)` would remove the duplication.

#### Duplicated logic

- OS notification urgency handling for Linux is repeated in `QuickInput`,
  `CreateTask`, and elsewhere. A `show_error_notification(&str)` helper would
  consolidate the `#[cfg(target_os = "linux")]` blocks.
- `prompt_input()` contains three platform-specific dialog implementations in
  one function. This is acceptable for cross-platform needs, but the function
  is long; splitting by `#[cfg]` modules would improve readability.

#### Dead code

- `tray/mod.rs` line 183: `let _soul = wire.soul_side().clone();` — the `soul`
  handle is bound but never used. The comment says "future can directly receive
  Soul pushes," so this is intentional scaffolding, but it is currently dead.
- `UserEvent::CancelTask` is emitted in the menu match arm (line 591) but there
  is no menu item that creates a `"cancel-"` id, so the branch is unreachable.
  This appears to be leftover plumbing.

#### Missing tests

- `clarity-claw` has good unit tests in `src/lib.rs` for pure helpers
  (`format_tooltip`, `classify_task_status`, deserialization, URL conversion).
- **No tests** for the tray event loop, `prompt_input`, `open_url`, or the
  filesystem watcher logic. These are UI/OS integration points and are harder
  to test, but at least `build_tray_menu()` and `thread_label()` could be unit
  tested.
- **No tests** for `gateway_ws_request()` error paths or the welcome-frame
  validation.

#### Candidates for simplification

- Replace `gateway_ws_url()` string manipulation with the `url` crate (already a
  transitive dependency via `tokio-tungstenite`) to avoid edge cases with
  trailing slashes and scheme replacements.
- `get_hostname()` could use `hostname::get()` from the `hostname` crate
  instead of env-var fallbacks, but that adds a dependency; the current env-var
  approach is fine and dependency-free.
- `tray_runtime()` → `OnceLock` as noted above.

### `crates/clarity-openclaw`

#### Unnecessary complexity

- `src/client.rs` is 1272 lines and contains multiple nested functions
  (`build_device_signature_payload`, `build_connect_req`) inside
  `run_single_connection`. Extracting these to module-level helpers would
  improve testability and reduce nesting.
- `src/connection_manager.rs` translates between `GatewayResponse` /
  `ClawResponse` and `ProtocolEvent` with two nearly identical polling/drain
  loops (`run_gateway_manager`, `run_openclaw_manager`). A shared
  `spawn_bridge` helper could consolidate the 10 ms sleep + drain pattern.
- `extract_claw_text()` (lines 485–527) checks many JSON shapes. This is
  necessary for interop, but it is a good candidate for a small set of
  provider-specific extractors once the shape set stabilizes.

#### Duplicated logic

- Backoff computation exists in both `client.rs` (`next_backoff`, lines
  356–360) and `gateway_client.rs` (`next_backoff`, lines 288–294). The two
  implementations differ slightly (one doubles a `Duration`, the other uses
  `2usize.saturating_pow`). A shared `clarity-openclaw::util` module or the
  existing `clarity-contract` crate could host a single exponential-backoff
  helper.
- Origin derivation logic (`app://kimi-desktop` for localhost) appears in
  `client.rs` (lines 432–443). It is protocol-specific, but if other OpenClaw
  clients appear, this should be a helper.

#### Dead code

- Multiple `#[allow(dead_code)]` annotations indicate unfinished UI wiring:
  - `client.rs` line 40: `ClawAuth::DevicePaired`.
  - `client.rs` line 179: `ClawClient::connect_with_device`.
  - `client.rs` line 285: `ClawClient::request_pairing`.
  - `client.rs` line 309: `ClawClient::send_raw_request`.
  - `client.rs` line 327: `ClawClient::try_recv`.
  - `connection_manager.rs` line 226: `ProtocolEvent::Unsupported` may be emitted
    but is not handled by consumers in this crate.
- `device.rs` line 17: `DeviceIdentityFile` has `#[allow(dead_code)]` on the
  struct, but its fields are used for persistence. The annotation may be stale.

#### Missing tests

- `client.rs` has tests for `extract_agent_event` and `next_backoff`, but no
  tests for `extract_messages`, `extract_session_message`,
  `compute_stream_delta`, `extract_pairing_result`, or `parse_message_list`.
- `connection_manager.rs` has no tests for `probe_protocol` or the manager
  dispatch logic.
- `discovery.rs` has **no tests** for `discover_openclaw_devices`,
  `resolve_openclaw_home`, or `read_openclaw_config`.
- `gateway_client.rs` has good protocol serialization/deserialization tests.
- `mesh/merger.rs` and `mesh/crypto.rs` have good unit tests.
- `mesh/syncthing_transport.rs` has async tests but they use real temp dirs and
  the real `SyncService`; they are integration-style and may be flaky in
  parallel test runs.

#### Candidates for simplification

- The `RoleContextTransport::subscribe()` method returns a closed channel as a
  fallback in `GatewaySyncTransport`, `SyncthingTransport`, and `NullTransport`.
  This is awkward; returning `Option<UnboundedReceiver<RoleContextId>>` or a
  dedicated `Subscriber` handle would remove the "closed channel fallback"
  pattern.
- `detected_protocol` in `connection_manager.rs` could be extended to also
  detect model-catalog endpoints, but that would violate the crate's scope.
  Better to keep catalog concerns out of this crate entirely.
- `discovery.rs` hard-codes `ws://127.0.0.1:18679` for the local OpenClaw
  Gateway. A constant or config default would be cleaner.

---

## 6. Recommendation

Do **not** attempt to reuse code from `clarity-claw` or `clarity-openclaw` for
the model catalog redesign. Implement the catalog inside `clarity-llm` as
planned:

1. Add `crates/clarity-llm/src/catalog/` with `ModelCatalogFetcher`,
   `ModelCatalogEntry`, `CatalogCache`, and `ModelCatalogService`.
2. Keep provider-specific fetchers (`OllamaFetcher`, `OpenAiCompatibleFetcher`,
   `NullFetcher`) in `clarity-llm` where they have access to provider config and
   API keys.
3. Use `~/.clarity/catalogs/{provider_id}.json` for the cache.
4. Reduce `registry_table::FamilyDefaults::known_models` to the minimal
   bootstrap seed.

If future work wants to expose model catalog refresh from a Claw node or the
Gateway, the Gateway can depend on `clarity-llm`'s `ModelCatalogService`
directly; `clarity-claw` should continue to speak only Gateway WebSocket and
not know about catalog internals.
