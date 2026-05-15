# ADR-015: EndpointDescriptor Abstraction — Unified Contract for Personas, Sites, and Frontends

- Status: Accepted
- Deciders: juice094
- Date: 2026-05-15
- Affects: `clarity-core::endpoint`, future `openteam-core`, S8 Persona switcher UI

## Context

Three distinct subsystems are growing concurrently:

1. **Clarity Personas** (S8 P3B.1): Gray, Analyst, Programmer — each binds an LLM
   provider + system prompt + capability set. The UI needs a switcher.

2. **OpenTeam-Core Sites** (Phase 2, separate repo): ChatGPT, Claude, Gemini,
   DeepSeek — each binds a DOM selector + injection script + capability set.
   The UI needs a switcher.

3. **Frontend Adapters** (existing): TUI, GUI, Headless — each binds a render
   path + capability set.

All three share the **same conceptual shape**: identity + display metadata +
capabilities + a dispatch kind. Without a unified contract, each subsystem
invents its own switcher widget, its own capability enum, its own registry
type — leading to triplicated UI code and divergent serde schemas.

The 2026-05-15 cross-session audit (clarity-2026-05-15-plan + openteam-re-2026-05-15
merge) made the isomorphism explicit: a Persona is just an in-process endpoint,
a Site is just a browser-mediated endpoint, a Frontend is just a render
endpoint. The right place for the shared contract is `clarity-core::endpoint`.

## Decision

Adopt a **single descriptor schema** for all endpoint kinds, plus a minimal
in-memory registry:

```rust
// crates/clarity-core/src/endpoint.rs

pub const ENDPOINT_SCHEMA_VERSION: u8 = 1;

pub enum EndpointCapability {
    Chat, Coding, Analysis, Browse, Vision, ToolUse, Planning,
}

pub enum EndpointKind {
    LocalLlm, RemoteLlm, BrowserSite, Frontend, McpTool,
}

pub struct EndpointDescriptor {
    pub schema_version: u8,
    pub id: SmolStr,
    pub display_name: SmolStr,
    pub description: SmolStr,
    pub kind: EndpointKind,
    pub capabilities: Vec<EndpointCapability>,
    pub icon: Option<SmolStr>,        // Lucide icon name
    pub accent: Option<SmolStr>,      // hex "#RRGGBB"
}

pub struct EndpointRegistry { /* Vec<EndpointDescriptor>; LIFO override */ }

pub fn default_clarity_personas() -> EndpointRegistry { ... }
```

### Why these specific fields

- `schema_version` — migration safety; bump triggers ADR.
- `id` — kebab-case stable identifier for routing.
- `display_name` / `description` — UI rendering.
- `kind` — disambiguates "which subsystem owns dispatch".
- `capabilities` — drives UI gating ("disable Browse panel when active endpoint
  doesn't support it").
- `icon` / `accent` — optional styling, frontends fall back to first letter.

### Non-goals

- This module **does not dispatch traffic**. It only describes. Dispatch
  remains owned by:
  - `clarity-llm` for LLM personas
  - `openteam-core::sites` (future) for browser sites
  - `clarity-egui` / `clarity-tui` for frontends

- Persona configuration files (TOML in `personality/domain.rs`) remain the
  authoring source; `EndpointDescriptor` is the in-memory projection consumed
  by UI and routers.

## Consequences

### Positive

1. **Single switcher widget** — GUI persona switcher and OpenTeam site
   switcher render from the same `&EndpointRegistry` reference.
2. **Capability-driven UX gating** — `descriptor.supports(Coding)` triggers
   the IDE-style panel automatically; no hard-coded persona-name checks.
3. **Cross-process telemetry** — `serde_json::to_string(&descriptor)` produces
   a stable wire format for debug overlays and remote inspection.
4. **OpenTeam-Core bootstrapping** — Phase 2 work can start immediately
   because the contract surface is frozen.

### Negative

1. **Schema lock-in** — adding a new `EndpointKind` requires bumping
   `ENDPOINT_SCHEMA_VERSION` and writing an ADR. This is intentional
   friction to prevent ad-hoc growth.
2. **Indirection cost** — a thin layer between "I have a Persona enum" and
   "I have a typed descriptor". Mitigated by keeping the module ~300 LOC.

### Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Two crates fork the schema | `clarity-contract` re-exports the type if cross-crate use grows |
| Capability list explodes | Hard cap at 8 variants; new capabilities require ADR |
| Frontend renderers diverge on capability semantics | Document each `EndpointCapability` variant inline; 7 unit tests in `endpoint.rs` |

## Validation

Six unit tests in `crates/clarity-core/src/endpoint.rs`:

- `default_personas_has_three_entries` — registry seeds correctly
- `register_overwrites_same_id` — LIFO semantics
- `capability_filter_works` — `with_capability()` enumerates only matching entries
- `browser_site_factory_sets_browse_capability` — OpenTeam path tested
- `serde_roundtrip_preserves_all_fields` — wire format stable
- `empty_registry_returns_none_on_get` — edge case

`cargo test -p clarity-core --lib endpoint` → 6 passed.

## Related

- ADR-011: Workspace architecture (3-tier panel layout)
- ADR-014: Side panel tab consolidation (Tab D — Persona switcher lives in
  Top Bar, NOT here)
- `crates/clarity-core/src/personality/domain.rs` — TOML authoring source
- `dev/openteam-core/` (external) — Phase 2 site adapter consumer
