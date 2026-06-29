# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Clarity is a Rust-native personal AI runtime: ReAct/Plan agent loop, MCP tool ecosystem, BM25+vector memory, multi-entry (TUI/Desktop/Web/Headless/Tray/Mobile FFI). 22 active workspace crates + 1 archived (`clarity-tauri`) across 23 crate directories, AGPL-3.0, MSRV 1.85.

## Build, Test, and Lint Commands

```bash
# Full quality gate (run before any commit)
cargo test --workspace --lib
cargo clippy --workspace --lib --bins --tests -- -D warnings
cargo fmt --all -- --check
cargo audit

# Single crate
cargo test -p clarity-core
cargo clippy -p clarity-core -- -D warnings

# Single test by name filter
cargo test -p clarity-core my_test_name

# Integration tests
cargo test -p clarity-integration-tests

# With local LLM feature (Candle GGUF inference)
cargo test --workspace --lib --features local-llm

# Run entry points
cargo run -p clarity-egui                    # Desktop GUI (primary UI stack)
cargo run -p clarity-egui --features cuda    # With CUDA acceleration
cargo run -p clarity-slint                   # Desktop GUI (Slint, experimental)
cargo run -p clarity-tui                     # Terminal UI
cargo run -p clarity-gateway                 # Axum HTTP/WebSocket server
cargo run -p clarity-claw                    # System tray monitor
cargo run -p clarity-headless -- --prompt "Hello" --provider local

# Project verification script (PowerShell)
scripts/verify.ps1 --all        # Verify all crates
scripts/verify.ps1 clarity-core # Verify single crate
```

**Hard rules**: clippy zero warnings, test zero failures, fmt zero diffs.

## Architecture Invariants (Hard Veto)

- `clarity-core` has **zero** dependencies on any frontend or network crate.
- `clarity-contract` has **zero** internal dependencies; all crates build on it.
- Frontend crates **never import each other** — cross-frontend communication goes through `clarity-wire`.
- No Docker / RAG(Qdrant) / GUI(Electron).
- Rust core modules cannot be outsourced to sub-agents without human review.
- **Stores layer independence**: The 20 `stores/` modules must have **zero cross-references** to sibling stores. Stores may only depend on `crate::ui::types`, `crate::settings::GuiSettings` (data type), and `clarity_core` types. The `all_plugins()` / `mcp_plugins()` pattern (accept config directly, not the whole store) is the canonical way to avoid cross-store imports.
- **No `take()`/restore pattern**: UI config panels must borrow state in-place (`if let Some(ref mut config)`), never `take()` ownership from the store and restore it later. A panic in the render closure would permanently lose the config.

## Crate Topology

```
contract ← {wire, memory, mcp, llm, tools, channels, secrets, openclaw, rollout}
               ↑
          thread-store (→ rollout)
                                  ↓
                               core ← {gateway, egui, tui, claw, headless, mobile-core}
                                  ↑
                    {subagents, telemetry} (consumes core / cross-cutting)

# Experimental frontend that bypasses core:
slint ← {contract, wire}
```

| Crate | Role |
|-------|------|
| `clarity-contract` | Shared traits + reliability types: `RetryConfig`, `ExponentialBackoff`, `RestartConfig`, `ConnectionState`, `HeartbeatConfig`, `ConnectionMetrics`, `RetentionPolicy`; identity types: `User`, `Team`, `Organization`, `TeamPolicy`, `PermissionPolicy`, `LlmProvider`, `Tool` |
| `clarity-wire` | UI ↔ Agent event bus (SPMC) + `WireEventBuffer` ring buffer for reconnection replay; cross-frontend communication channel |
| `clarity-memory` | SQLite + BM25 + vector search, chunking, four-level compaction |
| `clarity-mcp` | MCP client — stdio / SSE / HTTP / WebSocket transports |
| `clarity-llm` | LLM provider abstraction + registry + `RacingProvider` (parallel LLM race) + built-in providers + Candle GGUF local inference |
| `clarity-secrets` | ChaCha20-Poly1305 encrypted secret storage (`enc2:`) |
| `clarity-tools` | Built-in tools: file, shell, web, devkit, task, team, etc. |
| `clarity-channels` | External channel abstraction; WeChat iLink implemented; Discord/Slack/Telegram disabled by default pending `rustls-webpki` fix; Webhook enabled |
| `clarity-subagents` | Sub-agent executor + parallel scheduler |
| `clarity-thread-store` | Thread persistence abstraction; depends on `clarity-rollout` |
| `clarity-rollout` | JSONL rollout persistence for threads (API design inspired by OpenAI Codex) |
| `clarity-openclaw` | OpenClaw/KimiClaw Gateway WebSocket client, device identity, discovery |
| `clarity-telemetry` | WideEvent + SQLite/GreptimeDB backends + tracing Layer |
| `clarity-core` | Agent loop (ReAct/Plan), `Supervisor` (task supervision), `ToolOrchestrator` (concurrency control), `ToolResultCache`, `WorkspaceDiff`, `CompactionCache`, Approval, Skill, MCP integration, Background Tasks |
| `clarity-gateway` | Axum HTTP/WebSocket server, shared `Wire` for cross-connection event replay, `GET /health/metrics` endpoint, `Subscribe` message for reconnection catch-up, Web IDE, session store |
| `clarity-egui` | Desktop GUI — eframe/egui, pure Rust, zero Web dependencies |
| `clarity-tui` | ratatui terminal interface |
| `clarity-claw` | System-tray background monitor — Gateway WebSocket client, `GatewayWatchdog` (self-healing restart), `NetMonitor` (network change detection), reconnection loop with `RetryConfig` backoff, mesh merger with version vector conflict resolution |
| `clarity-headless` | Headless CLI for scripts / CI |
| `clarity-mobile-core` | Mobile FFI core — UniFFI bridge for Android/iOS Runtime/events/config/memory |
| `clarity-slint` | Experimental Slint desktop GUI — alternative to clarity-egui; depends on `clarity-contract` + `clarity-wire` only |
| `clarity-anthropic-proxy` | Anthropic Messages API gateway; protocol adapter lives in `clarity-llm::anthropic` |

`clarity-tauri` is archived; do not modify.

## Coding Standards

- All `pub` items must have `///` doc comments.
- User-facing strings go through `i18n` (`t!("key")`); no hardcoded English or Chinese in UI code.
- New `unwrap()` / `expect()` outside lock scenarios require `// SAFE: <invariant>` comment.
- Prefer `?` propagation for JSON parsing, path operations, string parsing.
- `unsafe` blocks require explicit maintainer approval and safety documentation.
- No `TODO` / `FIXME` / `XXX` in code; migrate to GitHub Issues or `docs/notes/`.
- Conventional commits: `feat(scope): imperative under 72 chars`.
- Keep egui panel render functions under 300 lines; use `Frame::new()` instead of `Frame::group(ui.style())`.

## Egui Error Handling Patterns

### No `take()`/restore for UI config state

```rust
// WRONG — panic in the closure permanently loses config:
let mut config_opt = app.store.config.take();
// ... render UI ...
app.store.config = config_opt;

// RIGHT — borrow in-place, no null window:
if let Some(ref mut config) = app.store.config {
    render_editor(ui, config, &theme);
}
```

### `render_safe()` = React Error Boundary

Every panel in `render_layout_shell()` is wrapped in `render_safe(name, |app, ctx| ...)` which catches panics via `catch_unwind`. If a panel panics: it renders blank for that frame, an error toast appears, and the app continues running.

### Gateway offline path

When Gateway is unreachable:
- Gateway startup failure → logged, app continues with `gateway_manager = None`
- Gateway task HTTP client → `reqwest::Client::builder().timeout(10s)`, falls back to local `TaskStore`
- Gateway health poll → 5s timeout, sets `GatewayStatus::Offline`
- Agent WebSocket → agent returns error, caught by caller

All reqwest client construction has a timeout fallback chain:
```rust
reqwest::Client::builder()
    .timeout(Duration::from_secs(N))
    .build()
    .unwrap_or_else(|e| {
        tracing::warn!("Client build failed: {}", e);
        reqwest::Client::builder()
            .timeout(Duration::from_secs(N))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    });
```

### Secret decryption error distinction

`ProviderDefinition::resolve_password()` returns `Result<Option<String>, String>`:
- `Ok(None)` → no password stored (legitimate)
- `Ok(Some(p))` → successfully decrypted
- `Err(msg)` → secret store unavailable or decryption failed (corrupted key file)

Callers must handle the error case distinctly from the "no password" case.

## Cross-Layer Change Checklist

Modifying these types requires syncing all callers:

| Type | Check these callers |
|------|---------------------|
| `AgentController`, `Op` enum | `clarity-tui`, `clarity-gateway`, `tests/integration` |
| `WireMessage` | All frontends that pattern-match or construct it |
| `ViewCommand`, `UserAction` | egui `protocol_renderer.rs`, TUI `protocol_renderer.rs`, Gateway `ws.rs`, `clarity-mobile-core` FFI mapping |

Adding a new LLM provider (see AGENTS.md §New Provider Checklist):
1. `crates/clarity-llm/src/model_registry.rs` — add `ProtocolType` if a new protocol adapter is needed; ensure `build_provider_from_registry` can construct it
2. `crates/clarity-llm/src/registry_table.rs` — add one row of canonical family defaults (protocol, base_url, auth style, default model)
3. `crates/clarity-core/src/view_models/settings.rs` — `get_available_models()` fallback list (temporary; will be derived from registry)
4. Run full test + clippy gate.

**Note**: `LlmFactory` is frozen — do not add new match branches there. New OpenAI-compatible providers should only need a `models.toml` entry.

## Platform-Specific Rules

- **Linux eframe**: Any crate using `eframe` with `default-features = false` must explicitly enable `"x11"` (and optionally `"wayland"`). Disabling defaults removes all window backends and triggers `compile_error!("platform not supported")`.
- **Windows shell tools**: `BashTool` is excluded via `#[cfg(target_os = "...")]`; only `PowerShellTool` registers on Windows.
- **Linux-only APIs**: `notify-rust::Notification::urgency()` must be wrapped in `#[cfg(target_os = "linux")]`.

## CI / Cache Troubleshooting

If CI fails with `cannot find module or crate 'clarity_core'` while local passes:
- `rust-cache` target directory may be stale.
- Diagnostic: insert `cargo clean` in CI (one-time).
- Fix: bump `rust-cache` key/prefix-key to force a miss; do not repeatedly push no-op commits.

## MCP Integration

This project exposes a local MCP server (`clarity-dev`) providing:
- `cargo_check` — `cargo check --workspace`
- `cargo_test` — `cargo test --workspace --lib`
- `cargo_clippy` — `cargo clippy ... -D warnings`
- `cargo_fmt_check` — `cargo fmt --all -- --check`

Prefer these tools over manual Bash when verifying code changes.

## Environment Variables for Development

```powershell
# LLM providers (env-var fallback; preferred: models.toml)
$env:KIMI_CODE_API_KEY="sk-kimi-..."
$env:KIMI_API_KEY="sk-..."
$env:ANTHROPIC_AUTH_TOKEN="..."
$env:DEEPSEEK_API_KEY="..."
$env:OPENAI_API_KEY="..."

# Local GGUF (Candle)
$env:CLARITY_LOCAL_MODEL_PATH="C:\path\to\model.gguf"
$env:CLARITY_LOCAL_TOKENIZER_REPO="Qwen/Qwen2.5-7B-Instruct"

# Provider registry configuration (preferred)
$env:CLARITY_MODELS_CONFIG="C:\path\to\models.toml"

# Gateway runtime overrides
$env:CLARITY_MAX_CONTEXT_TOKENS="1000000"
$env:CLARITY_APPROVAL_MODE="yolo"   # interactive | smart | plan | yolo

# CUDA compilation (Windows with MSVC 14.50+ and CUDA 12.6)
$env:NVCC_CCBIN="C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.50.35717\bin\Hostx64\x64\cl.exe"
$env:CUDA_HOME="C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.6"

# MCP allowlist override
$env:CLARITY_MCP_ALLOWLIST="C:\tools\mcp-server.exe,C:\tools\"
```

### Multi-Provider Configuration (`models.toml`)

Clarity prefers a registry-driven provider configuration via `models.toml`:

```toml
[providers.deepseek]
protocol = "open_ai_chat"
base_url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"

[providers.moonshot]
protocol = "open_ai_chat"
base_url = "https://api.moonshot.cn/v1"
api_key_env = "KIMI_API_KEY"

[[models]]
alias = "deepseek-v4-pro"
provider = "deepseek"
model_id = "deepseek-v4-pro"
# Encrypted key (preferred). Use `cargo run -p clarity-secrets --example encrypt_key`.
api_key = "enc2:..."
tags = ["cheap", "coding", "long-context"]

[[models]]
alias = "kimi-k2"
provider = "moonshot"
model_id = "kimi-k2.6"
tags = ["coding"]
pricing = { input_per_1m = 0.5, output_per_1m = 0.5 }

[[models]]
alias = "router"
provider = "router"
model_id = "router:cheap"
```

Alias-level overrides (`api_key`, `api_key_env`, `base_url`, `extra`, `headers`, `pricing`, `tags`) take precedence over the provider family defaults. Encrypted keys use the `enc2:` prefix and are decrypted by the project-level secret store (`.clarity/secrets.key`).

`fallback_aliases` declares a failover chain. Gateway wraps the active alias and its fallbacks in `clarity_llm::ReliableProvider`, which provides exponential-backoff retries, `Retry-After` honouring, context-window truncation retry, empty-completion re-roll, and chain failover.

`router:<hint>` aliases are resolved at request time by `clarity_llm::runtime_router`. Supported hints: `cheap`, `coding`, `vision`, `tools`, `fast`, or an explicit alias name.

Search order:
1. `$env:CLARITY_MODELS_CONFIG`
2. `./.clarity/models.toml`
3. `~/.config/clarity/models.toml`
4. Built-in env-var fallback

> **Gray Migration note**: Stage A/B/C complete. Provider configuration uses per-alias `models.toml` with encrypted secrets (`clarity-secrets`), canonical family defaults live in `clarity-llm::registry_table`, Gateway wraps active providers with `clarity_llm::ReliableProvider` for retry/fallback, and `clarity_llm::runtime_router` provides hint-based alias routing (`router:cheap`, `router:coding`, etc.). A unified `clarity_llm::auth::OAuthService` exposes `/api/auth/device` and `/api/auth/poll` for headless OAuth device flows. Plaintext provider keys have been removed from `dev/clarity/.clarity/gateway.env`; that file now only holds non-secret runtime toggles (WeChat, Gray workspace, etc.).

## Current State & Limitations

- **Multi-entry frontends are stable**: egui desktop (primary), ratatui TUI, Axum Gateway/Web IDE, headless CLI, claw tray node.
- **Pretext layout is stable** in egui; three-rail shell (left icon rail / center stage / right tool rail) with layout diagnostics (`Ctrl+Shift+L`).
- **Provider/Secret stack is stable**: `models.toml` per-alias config, `enc2:` secrets, `ReliableProvider` failover, `runtime_router` alias routing.
- **Thread/Session persistence**: JSON-based session files in `sessions/`, auto-saved every 0.5s when dirty (crash recovery window ≤ 0.5s). Save failures surface as toast notifications. Stuck turns auto-reset after 5 min.
- **Identity layer (Phase 6)**: `User`, `Team`, `Organization` types in `clarity-contract::identity`; identity flows through AgentConfig → TurnContext → lifecycle events → rollout logs; SQLite-backed in SessionStoreV2 (`users`, `teams`, `team_members`, `organizations`, `org_members` tables).
- **Permissions (Phase 8)**: `TeamPolicy` + `PermissionPolicy` in `clarity-contract`; `policy_engine::authorize()` in `clarity-core::approval`; `AgentConfig.team_policy` + `AgentConfig.member_role`; integrated into `execute_tool_call()` for Allow/Deny/RequireApproval decisions; `team_policies` and `device_identities` tables in SessionStoreV2.
- **Mobile FFI core exists** (`clarity-mobile-core`) but full Android/iOS UIs are still in the roadmap.
- **External channels** are limited: only WeChat iLink (`chkit`) is implemented; Discord/Slack/Telegram are disabled by default pending a `rustls-webpki` fix. Clarity is not a multi-channel inbox product.
- **Cross-device sync (Phase 7)**: Sync data types (`ClawContextEvent`, `SyncRequest`, `SyncResponse`) in `clarity-contract`; `RoleContextStore` in gateway; device↔user binding in SessionStoreV2.
- **Slint frontend** is experimental and excluded from default CI.
- **`clarity-tauri` is archived** and excluded from the workspace; do not modify.
- **`clarity-openclaw` is archived** (merged into `clarity-claw`); do not modify.
- **Legacy panels** (`panels/legacy/`) removed; MCP and Skill overlays promoted to `panels/` directly.
- **Egui test coverage**: 347 tests (0 failures), up from 259. Key areas with coverage: `wire_dispatcher` (11), `gateway_task_client` (8), `gateway_manager` (4), `chat` handlers (18), `session` persistence (14), `provider` (29), `truncate` (11), `system` handlers (6), `subagent` handlers (9), `navigation_tree` layout (4). Still untested: `gateway_poller`, `agent_runner`, `claw_events`, `message_actions`, `task_service`, `tray`, most `panels/` render functions.
- **Library target**: `src/lib.rs` exposes `test_util` via `#[cfg(test)] mod test_util;` so `cargo test --lib` runs 3 shared-test-helper smoke tests. The binary target (`main.rs`) holds all rendering modules. Cross-crate consumers should use `clarity-contract` or `clarity-wire` for shared types.
- **Layout invariants**: Left rail is NEVER auto-collapsed by window width (only manual toggle). During animation, `effective_left_rail_width()` must be used for all layout consumers (panel width, `compute_metrics()`, `render_main_stage_border()`, `render_status_bar()`). The navigation tree footer uses `allocate_new_ui` with `top_down` (separator at top) + inner `bottom_up` (avatar pinned to bottom).

## Egui Frontend Architecture (S6 Phase E)

Three-rail shell: left nav tree, center chat stage, right IDE rail. 347 unit tests, 6 theme presets, palette-derived design tokens, animation-driven panel transitions.

### Egui Layout System

Egui's layout model is **declaration-order-dependent space partitioning**, not CSS absolute positioning. Panels declared first consume their space first:

```
TopBottomPanel::top("titlebar")    → 32px (full width)
SidePanel::left("nav_tree")        → animated 210px↔36px (full height below titlebar)
SidePanel::right("ide_panel")      → user-resizable 180–400px
TopBottomPanel::bottom("status")   → 24px (declared first = bottom-most slot)
TopBottomPanel::bottom("input")    → natural height (declared second = above status bar)
CentralPanel::default()            → REMAINING space
```

Z-order (bottom to top): `LayerId::background()` (main stage fill, border stroke) → Panels (Middle layer) → `LayerId::new(Order::Tooltip, ...)` (border stroke above right-rail cover) → Foreground Areas (toasts) → Foreground Areas (modal scrim + Window) → Command palette Window → `LayerId::new(Order::Foreground, "theme_transition_overlay")`.

Key egui primitives used in clarity-egui:
- `ui.allocate_new_ui(UiBuilder::new().max_rect(rect).layout(...), |ui| ...)` — child Ui constrained to exact rectangle, used for nav footer and scroll area partitioning
- `Frame::Prepared` API (`begin()` → modify fill/stroke → `end()`) — dynamic coloring after content inspection, used in turn renderer for error/warning accents
- `ui.put(rect, widget)` — place widget at exact rect bypassing flow layout
- `Sense::click()`, `Sense::drag()`, `Sense::hover()` — interaction sensitivity (does NOT affect rect size)
- `UiBuilder::new()` — child Ui with custom max_rect, layout, id_salt

### Panel Declaration Order

The order in `render_layout_shell()` is load-bearing:
1. `render_main_stage_border()` — background painter only (no panel)
2. `render_titlebar()` — TopBottomPanel::top
3. `render_left_rail()` — SidePanel::left (animated width)
4. `render_right_rail()` — SidePanel::right (animated open/close + native resize line cover on Foreground layer)
5. `render_status_bar()` — TopBottomPanel::bottom (first = bottom slot)
6. `render_input_panel()` — TopBottomPanel::bottom (second = above status bar)
7. `render_main_stage()` — CentralPanel (remaining space)
8. Overlays: skill, mcp, toasts, modal scrim + window, onboarding

Each panel is independently panic-guarded via `render_safe(name, |app, ctx| ...)` which wraps in `catch_unwind`. A panic in one panel shows an error toast but leaves other panels intact.

### Test Infrastructure

`src/test_util.rs` provides shared test helpers (available via `crate::test_util` in any `#[cfg(test)]` module):

| Export | Purpose |
|--------|---------|
| `with_temp_dir(name, closure)` | Create isolated temp dir → run closure → RAII cleanup |
| `with_temp_sessions_dir(name, closure)` | Like `with_temp_dir` + auto-create `sessions/` subdir |
| `TempDirGuard` | RAII struct that `remove_dir_all` on drop |

`crate::session::save_session_to_path(session, path)` is `#[cfg(test)] pub(crate)` — writes a Session to an explicit path for roundtrip integration tests. Any test module can import it via `use crate::session::save_session_to_path`.

Known fragility areas when modifying layout code:
- Changing panel declaration order breaks the space-partitioning contract
- Modifying `effective_left_rail_width()` without updating all consumers causes misalignment
- The `render_main_stage_border()` painter uses absolute screen coordinates; misalignment with panel rects creates visible gaps
- Navigation tree footer separator positioning depends on `top_down` layout cursor behavior

### Theme System
Six theme presets + OS auto-detection: `Theme::system()` reads Windows registry / macOS defaults / Linux gsettings for dark/light preference on first launch. Light themes use soft ambient shadows (α×0.17 vs. dark). Theme switches animate with a 250ms cubic-ease-out fade overlay. All colors are palette-derived from 16 base hex values + 2 font names — adding a new theme is ~20 lines.

| Preset | Accent | Base |
|--------|--------|------|
| Dark (default) | #1a88ff | #121212 |
| Light | #c98a5e | #f0f1f6 |
| OLED | #1a88ff | #000000 |
| Catppuccin Mocha | #CBA6F7 | #1E1E2E |
| Tokyo Night | #7AA2F7 | #1A1B26 |
| One Dark | #61AFEF | #282C34 |

### Key Widgets
| Widget | File | Purpose |
|--------|------|---------|
| `DiffViewer` | `widgets/diff_viewer.rs` | Unified diff with line numbers, syntax coloring, hunk folding, accept/reject |
| `ContextPicker` | `widgets/context_picker.rs` | `#` quick-add popup with source list, embedded file browser |
| `CommandPalette` | `widgets/command_palette.rs` | Fuzzy-searchable command surface (Ctrl+Shift+P) |
| `FindBar` | `main.rs:render_find_bar()` | Ctrl+F find-in-session with match count, prev/next, close |
| `StatusBar` | `main.rs:render_status_bar()` | Bottom bar: git branch (⎇), agent status (●), model name |
| `Skeleton` | `design_system.rs:skeleton()` | Pulsing loading placeholder for async content |

### New State Stores
| Store | File | Purpose |
|-------|------|---------|
| `ConsoleStore` | `stores/console.rs` | Ring-buffered task log (5K cap) with level filters |
| `FilesStore` | `stores/files.rs` | Workspace root, expanded dirs, recent files |
| `ChatStore.find_*` | `stores/chat.rs` | Find-in-session query, matches, current index, cached query |

### Right IDE Panels
- **Console** — filterable virtualized log, error click→inject to chat
- **Files** — recursive dir tree, right-click context menu
- **Share** — Markdown/JSON/HTML export, clipboard/file save
- **Templates** — 5 built-in templates, one-click inject
- **ClawSettings / Workspace / Terminal / WebBridge** — remote device panels
- **KnowledgeBase** — OKF bundle browser
- Right rail is user-resizable (180–400px), width persists in settings. Close/open animated with cubic-ease-out.

### Context System (# Quick-Add)
Type `#` in the composer → ContextPicker popup → select source. Chips render above composer; on send, `[Context]` prefix is injected.

### Syntax Highlighting
`syntect` v5 (regex-fancy), 18 languages. Language identifiers normalized from short forms. Theme: base16-ocean.dark → Color32.

### Keyboard Shortcuts
| Shortcut | Action |
|----------|--------|
| Ctrl+N | New session |
| Ctrl+Enter | Send message |
| Ctrl+C | Stop generation |
| Escape | Close modal / find bar / panel |
| Ctrl+F | Find in session |
| Ctrl+Shift+P | Command palette |
| Ctrl+K | Focus chat input |
| Ctrl+B | Toggle sidebar |
| Ctrl+` | Toggle Console panel |
| Ctrl+Shift+F | Toggle Files panel |
| Ctrl+Shift+S | Toggle Share panel |
| Ctrl+/ | Keyboard shortcuts reference |
| Ctrl+Plus/Minus | Font scale |

## Documentation Index

| File | What it contains |
|------|-----------------|
| `AGENTS.md` | Current sprint status, coupling warnings, capability islands, security notes |
| `CONTRIBUTING.md` | Full contributor guide, PR workflow, manual QA checklist |
| `docs/ARCHITECTURE.md` | Code-accurate architecture: crate details, data flows, extension points |
| `docs/architecture/architecture-positioning.md` | Project positioning, Hard Veto, relationship to external projects |
| `docs/planning/ROADMAP.md` | Phase roadmap (Phase 1→2→3) |
| `CHANGELOG.md` | Version history |
