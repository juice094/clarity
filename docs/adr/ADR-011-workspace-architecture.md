---
title: ADR-011: Workspace Architecture — openclaw Bootstrap + Multi-Instance Extension
category: ADR
tags: [adr]
---

# ADR-011: Workspace Architecture — openclaw Bootstrap + Multi-Instance Extension

- Status: Accepted
- Deciders: juice094
- Date: 2026-05-13

## Context

The Clarity project's information architecture is migrating from a single-Agent single-Session model to a multi-Agent multi-Instance multi-Device collaboration runtime. This requires a concrete workspace persistence model.

The user direction (2026-05-13 design session): "形如 openclaw 的架构类型，但从单体 agent 实例演化为单机多实例/跨设备多实例协作场景". This locks the design to **openclaw bootstrap compatibility** (enabling workspace round-trip migration between projects) plus a **multi-instance extension layer**.

Diligence on openclaw (inspected 2026-05-13 at `~/dev/third_party/openclaw/docs/concepts/agent-workspace.md` and `agent-runtimes.md`):

- openclaw runs **a single embedded agent runtime — one agent process per Gateway**.
- Workspace at `~/.openclaw/workspace` (configurable via `agents.defaults.workspace` or `OPENCLAW_PROFILE` env).
- 7 bootstrap files loaded each session: AGENTS.md / SOUL.md / USER.md / IDENTITY.md / TOOLS.md / HEARTBEAT.md / BOOT.md, plus optional BOOTSTRAP.md (first-run ritual).
- Memory: `memory/YYYY-MM-DD.md` daily log + optional `MEMORY.md` curated long-term.
- Skill precedence (highest first): `workspace/skills` > `workspace/.agents/skills` > `~/.agents/skills` > `~/.openclaw/skills` > bundled > extraDirs.
- Metadata under `~/.openclaw/`: config, credentials, sessions, sandboxes.

This contract is well-defined and widely deployed; reusing it preserves a future migration path between projects.

The naming-convention question was resolved 2026-05-13 in favor of **role-nested** (`workspaces/<role>/<machine>-<n>/`) over flat (`workspaces/engineer-pc1-001/`) or device-nested (`workspaces/pc1/engineer-001/`). This privileges role-level operations (cross-session task orchestration within a role) over device-level operations.

## Decision

1. **Adopt openclaw's 7 bootstrap files in full** (Option (a) per 2026-05-13 dialogue). Files: `AGENTS.md`, `SOUL.md`, `USER.md`, `IDENTITY.md`, `TOOLS.md`, `HEARTBEAT.md`, `BOOT.md`, plus `BOOTSTRAP.md` for first-run ritual.
2. **Role-nested instance naming** (Option B). Instance directories follow `workspaces/<role>/<machine>-<n>/` where `<role>` is the Agent persona (`engineer`, `knowledge`, `emotion`, ...), `<machine>` is a stable device identifier (`pc1`, `laptop`, `phone`), and `<n>` is a 3-digit instance counter starting at `001`.
3. **Workspace and metadata are physically separated**. `workspaces/` holds agent working files; metadata (config, credentials, sessions, sandboxes) lives under `~/.clarity/` (mirroring openclaw's `~/.openclaw/`).
4. **Clarity extensions** layered on top of openclaw:
   - `workspaces/_shared/` — cross-instance shared state (`facts/`, `conventions/`, `cross-refs/`).
   - `workspaces/_cluster/` — cluster node metadata (`peers.yaml`, sync state).
   - `workspaces/<role>/<instance>/notes/` — per-instance sticky notes with `mentions/inbox` and `mentions/outbox` subdirectories for cross-instance messaging (5 note types: todo / draft / links / sketch / mention — see ADR-012).
   - `workspaces/<role>/<instance>/workdir/` — agent working files (separates code/data from bootstrap metadata).
5. **Cluster bootstrap deferred to v0.5.0+**. `_cluster/` and Syncthing P2P sync are optional in v0.4.x; activation gated on Hub-Worker backend availability (FUTURE_DIRECTION.md Phase A-C).
6. **Skill precedence chain matches openclaw verbatim**. The skill loader extends openclaw's chain with one substitution: `~/.clarity/skills` replaces `~/.openclaw/skills` at the same precedence position.

## Workspace Layout

```
workspaces/
├── engineer/                                ← role
│   ├── pc1-001/                             ← instance (<machine>-<n>)
│   │   ├── AGENTS.md                        ← openclaw bootstrap (required)
│   │   ├── SOUL.md                          ← openclaw bootstrap (required)
│   │   ├── USER.md                          ← openclaw bootstrap (required)
│   │   ├── IDENTITY.md                      ← openclaw bootstrap (required)
│   │   ├── TOOLS.md                         ← openclaw bootstrap (required)
│   │   ├── HEARTBEAT.md                     ← openclaw bootstrap (required)
│   │   ├── BOOT.md                          ← openclaw bootstrap (required)
│   │   ├── BOOTSTRAP.md                     ← first-run ritual (auto-deleted)
│   │   ├── memory/
│   │   │   ├── YYYY-MM-DD.md                ← openclaw daily memory log
│   │   │   └── MEMORY.md                    ← openclaw curated long-term
│   │   ├── skills/                          ← openclaw skill precedence layer 1
│   │   ├── workdir/                         ← Clarity extension: agent working files
│   │   └── notes/                           ← Clarity extension: sticky notes
│   │       ├── todo/
│   │       ├── draft/
│   │       ├── links/
│   │       ├── sketch/
│   │       └── mentions/
│   │           ├── inbox/                   ← incoming from other instances
│   │           └── outbox/                  ← outgoing to other instances
│   └── pc1-002/                             ← second engineer instance
├── knowledge/
│   └── pc1-001/
├── emotion/
│   └── pc1-001/
├── _shared/                                 ← Clarity extension
│   ├── facts/
│   ├── conventions/
│   └── cross-refs/
└── _cluster/                                ← v0.5+ (Hub-Worker gated)
    └── peers.yaml

~/.clarity/                                  ← metadata (mirrors ~/.openclaw/)
├── clarity.json                             ← config
├── agents/<instanceId>/sessions/            ← session transcripts (JSONL)
├── credentials/                             ← provider/channel state
├── sandboxes/                               ← sandbox workspace overlays
└── skills/                                  ← managed skills
```

## Consequences

### Positive

- **openclaw round-trip migration**: Copying `workspaces/engineer/pc1-001/` to `~/.openclaw/workspace` yields a valid openclaw workspace (Clarity extensions `notes/`, `workdir/` are ignored, not broken). Reverse direction equally smooth.
- **Multi-instance native**: Role-level orchestration is a directory-listing operation. `ls workspaces/engineer/` returns all engineer instances.
- **Cross-instance messaging via filesystem**: A message from `engineer/pc1-001` to `knowledge/pc1-001` is a file write to `workspaces/knowledge/pc1-001/notes/mentions/inbox/<msg-id>.md`. No new IPC primitive required.
- **Syncthing-native multi-device**: The entire `workspaces/` tree is a single Syncthing target. Cross-device sync is "make the directory match across nodes", not a custom protocol.
- **Pretext UI alignment**: Every UI state corresponds to a file or directory. Agent self-introspection reads the filesystem. No black box.

### Negative

- **Schema rigidity**: Locking `<role>/<machine>-<n>/` makes future restructuring (flattening / adding a level) require migration tooling. Mitigation: top-level `workspaces/.clarity-schema.toml` with version field.
- **Bootstrap file verbosity**: 7+ required files per instance. Mitigation: `clarity setup` auto-generates minimal templates; templates can be empty (with marker comments) without disabling the agent (matches openclaw's "missing file" marker behavior).
- **Migration cost from `.clarity/`**: Existing v0.3.x sessions need a script. Mitigation: v0.4.0 backward-compatible for 2 release cycles; provide `clarity migrate-workspace`.

### Neutral

- Skill precedence chain matches openclaw with one one-for-one substitution (`~/.clarity/skills` instead of `~/.openclaw/skills`).
- `BOOTSTRAP.md` ritual preserved verbatim — deletion after completion is the operator's responsibility (same as openclaw).
- Schema version `workspaces/.clarity-schema.toml` ships with the first v0.4.0 release.

## Alternatives

| Option | Pros | Cons | Outcome |
|---|---|---|---|
| **A.** Flat `workspaces/engineer-pc1-001/` | Single layer, simple `ls` | Cross-role aggregation needs regex/sort | Rejected (B has trivial role aggregation) |
| **B.** Role-nested `workspaces/engineer/pc1-001/` | Role-level operations are directory ops | Slightly deeper traversal | **Accepted** |
| **C.** Device-nested `workspaces/pc1/engineer-001/` | Cross-device sync trivial | Cross-role on same device needs regex | Rejected (role priority wins for v0.4.0 use case) |
| **D.** Use openclaw verbatim (single workspace, no extension) | Zero conflict | Cannot express multi-instance/multi-device | Rejected (does not meet stated direction) |

## Validation

Pre-merge acceptance criteria for Phase 1.5 / S6 work adopting this schema:

- [ ] `clarity-core` exposes `WorkspaceConfig::new(root: PathBuf)` returning resolved layout with all openclaw bootstrap paths.
- [ ] `clarity setup` creates 7 bootstrap files with minimal templates against an empty `workspaces/<role>/<instance>/`.
- [ ] Manual round-trip: copy `workspaces/engineer/pc1-001/` to `~/.openclaw/workspace-clarity-test/`, run `openclaw doctor` — expects 0 errors.
- [ ] Reverse round-trip: copy `~/.openclaw/workspace/` to `workspaces/engineer/pc1-001/`, run `clarity --workspace workspaces/engineer/pc1-001 doctor` — expects 0 errors.
- [ ] Migration: `clarity migrate-workspace --from .clarity --to workspaces/engineer/pc1-001` produces a valid v0.4.0 layout from v0.3.x `.clarity/`.
- [ ] `cargo test -p clarity-core workspace::*` passes.

## References

- openclaw workspace contract: `~/dev/third_party/openclaw/docs/concepts/agent-workspace.md`
- openclaw runtime contract: `~/dev/third_party/openclaw/docs/concepts/agent-runtimes.md`
- Pretext UI theory: `docs/architecture/pretext-ui-theory.md` §9 (filesystem as Agent evolution substrate)
- Future direction: `docs/planning/FUTURE_DIRECTION.md` Phase A-D
- Companion: ADR-012 (RenderLine enum, defines note types for `notes/` subdirectories)
- Related: ADR-005 (subagent-core decoupling), ADR-006 (protocol-layer convergence)
- Syncthing-Rust: `syncthing-rust v0.2.0-beta` (TTL 2026-06-01)
