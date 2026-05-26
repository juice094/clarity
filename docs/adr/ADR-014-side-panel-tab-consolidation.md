---
title: ADR-014: Right Side-Panel Consolidation — From Multi-Drawer to Single Tab
category: ADR
tags: [adr]
---

# ADR-014: Right Side-Panel Consolidation — From Multi-Drawer to Single Tab

- Status: Accepted
- Deciders: juice094
- Date: 2026-05-13
- Supersedes (in part): P1.5.1 placement of `Skill` / `Mcp` in `SidePanel`

## Context

The S3 (Phase 1.5) state machine migration is replacing 50 boolean flags with a typed `ViewState` aggregate. P1.5.4 was scheduled to migrate four panel booleans — `task_panel_open`, `team_panel_open`, `skill_panel_open`, `mcp_panel_open` (plus `dashboard_panel_open` discovered during exploration) — into `view_state.right: Option<SidePanel>`.

Exploration during P1.5.4 surfaced two semantic mismatches between the existing production code and the P1.5.1 data model:

### Mismatch 1: Multi-drawer parallelism vs single-tab exclusivity

`main.rs:805-854` reveals that **three right-side panels can be open simultaneously**:

```rust
content_w = current_width - sidebar_w - workspace_w - dashboard_w - team_w - task_w;
//                                                    ^^^^^^^^^^^   ^^^^^^   ^^^^^^
//                                                    three columns that can co-exist
```

Each panel reserves its own horizontal column (`size_panel_right`). When the viewport becomes too narrow, the responsive logic closes them **one at a time, by priority** (`dashboard → team → task`), confirming that multi-drawer state is a "wide-screen perk" rather than a core interaction model.

A direct migration to `Option<SidePanel>` (single-exclusive) would therefore be a **behavior regression**, not a refactor.

### Mismatch 2: `Skill` / `Mcp` are Modals, not Side Panels

P1.5.1 (commit `682b303c`) placed `Skill` and `Mcp` into `SidePanel`, on the assumption they were floating side panels. Reading `panels/skill.rs` and `panels/mcp.rs` shows they are **100% modal** in behavior:

| Behavior | Skill | Mcp | Modal indicator |
|---|---|---|---|
| Full-screen scrim (70% black overlay) | OK rgba(0,0,0,180) | OK same | yes |
| Outside-click closes | OK | OK | yes |
| `Esc` closes | OK | OK | yes |
| Centered fixed position | OK CENTER_CENTER | OK same | yes |
| Non-resizable, non-movable | OK resizable(false) | OK same | yes |
| Background input blocked | OK scrim absorbs | OK same | yes |

These are not "floating side panels". They are bona-fide modals and belong in `ModalType`, not `SidePanel`.

## Decision

This ADR resolves both mismatches together. Two coordinated decisions:

### Decision 1: Right-side multi-drawer collapses into a single Tab

The three right-anchored business panels (`Dashboard`, `Team`, `Task`) are unified under `view_state.right: Option<SidePanel>` with **mutual exclusion**: at most one of them is open at any time. The user cannot have Team and Task open simultaneously.

**Behavior change** (documented and intentional):

- Before: Team + Task could co-exist on a wide monitor.
- After: Selecting Team auto-closes Task (and vice versa). Selecting nothing closes all.

**Rationale for the change**:

1. The "wide-screen perk" parallelism was always degraded to single-panel on narrow screens (responsive logic in `main.rs:805-854`). The asymmetric model added complexity without core value.
2. Future D-form-factor work (ADR-011 §Decision 4) targets a right-side Tab bar with `SSH / Workspace / Settings` choices. Mutual exclusion now is a *direct progression* toward that target, not a detour.
3. Pretext UI Theory §1 ("90% information + 10% decoration") prefers a single high-attention context over visually-competing parallel views.

### Decision 2: `Skill` / `Mcp` relocate from `SidePanel` to `ModalType`

P1.5.1's `SidePanel` and `ModalType` definitions are revised:

```diff
 pub enum SidePanel {
     Sidebar,
     Workspace,
     Team,
     Task,
+    Dashboard,         // newly added (was missed in P1.5.1)
-    Skill,             // moved to ModalType
-    Mcp,               // moved to ModalType
     PreviewDrawer,
     SubAgentProgress,
 }

 pub enum ModalType {
     Approval,
     Snapshot,
     Login,
     TaskCreate,
     TaskView,
     TeamCreate,
     CronCreate,
     SubAgentView,
     AddProvider,
     KimiCodeLogin,
+    Skill,             // moved from SidePanel
+    Mcp,               // moved from SidePanel
 }
```

`SidePanel` ends at **7 variants** (was 8 in P1.5.1, two removed and one added).
`ModalType` ends at **12 variants** (was 10 in P1.5.1, two added).

### Decision 3: World model is binary

After this ADR, the right-side UI surface has exactly two categories:

| Category | Behavior | Data model |
|---|---|---|
| **Tab** (right-anchored Side Panel) | Mutually exclusive, allocates a column | `view_state.right: Option<SidePanel>` |
| **Modal** | Blocks background, single at a time, full-screen scrim | `view_state.modal: Option<ModalType>` |

There is **no** third category ("floating panel"). What was previously believed to be floating (`Skill` / `Mcp`) is a Modal.

### Decision 4: Legacy "floating windows" are transition artifacts, not a third category

A secondary discovery during P1.5.3 exploration (2026-05-13) revealed that five `ModalType` variants — `TaskCreate`, `TaskView`, `TeamCreate`, `CronCreate`, `SubAgentView` — are **not true modals**. They are implemented as `egui::Window::new().anchor(CENTER_CENTER)` **without a full-screen scrim**, meaning they do **not** block background input and can theoretically coexist with each other or with true modals.

These "floating windows" are **MVP-era temporary UI** — features that had no dedicated panel space in the old layout, so they were crammed into pop-up windows. Under the Pretext UI / D-form-factor direction (ADR-011), they are **not a permanent third category**; they will be absorbed into the right-side Tab content as sub-views:

| Legacy floating window | Future home (right-side Tab) |
|---|---|
| TaskCreate / TaskView | **Task Tab** — inline create / detail views |
| TeamCreate | **Team Tab** — inline create view |
| CronCreate | **Dashboard Tab** — scheduling sub-page |
| SubAgentView | **SubAgentProgress Tab** or Dashboard widget |

**Therefore**:

1. These five variants remain in `ModalType` for the moment (to avoid breaking P1.5.1 commit `682b303c` tests), but they are **not migrated** into `view_state.modal` during S3.
2. They continue to be controlled by their legacy boolean flags until S6–S8, when the D-form-factor refactor provides them a proper Tab home.
3. `view_state.modal` is reserved for **true blocking modals** (`Approval`, `Snapshot`, `Login`, `Skill`, `Mcp`, `AddProvider`, `KimiCodeLogin`).
4. P1.5.3 (modal booleans → `ModalType`) is **skipped** — there are no true modal booleans left to migrate.

This decision aligns with the user's observation: *"弹窗可能是旧设计的相关产物，新设计为侧边栏"*.

## Consequences

### Positive

- `view_state.right: Option<SidePanel>` directly matches production semantics. No new aggregate types needed.
- ~50 lines of multi-drawer responsive logic in `main.rs:805-854` become deletable.
- The four `*_panel_open` boolean writers across `task_store`, `team_store`, `ui_store`, `mcp_store` consolidate into a single `view_state.right` field, simplifying the audit's "50 booleans" count.
- Future D-form-factor (right-side `SSH / Workspace / Settings` Tab bar) flows naturally from this model — `Option<SidePanel>` is already the right shape.
- World model becomes binary (Tab + Modal), removing the murky "floating panel" category and the question "where does Skill go?".
- Removes a P1.5.1 design defect (Skill/Mcp misclassification) before it propagates further into S4-S9.

### Negative

- User-visible behavior change: Team + Task can no longer be open simultaneously on wide monitors. Documented in CHANGELOG.md as an intentional UX simplification.
- 36 grep-located references to `*_panel_open` booleans require migration (P1.5.4d step).
- P1.5.1 unit tests touching `SidePanel::Skill` / `SidePanel::Mcp` need updating (~2 tests).

### Neutral

- The four `*_panel_open` fields in stores are not removed in P1.5.4 — they become **read-only mirrors** of `view_state.right`, following the same pattern as `TurnState::from_legacy` (P1.5.5). Final removal happens in P1.5.2 (bridge reversal).
- `Skill` and `Mcp` rendering logic in `panels/skill.rs` / `panels/mcp.rs` is unchanged — only their state-flag location moves.

## Alternatives Considered

### Alternative A: Multi-drawer model with `right_columns: PanelColumns`

Introduce a new aggregate struct that bundles three booleans (`dashboard`, `team`, `task`) plus a separate `Option<SidePanel>` for the future D-form Tab. Each business panel stays independent.

**Rejected because**:
- Adds a third data structure (`PanelColumns`) that becomes obsolete the moment D-form-factor lands. We'd refactor twice.
- Preserves the "wide-screen perk" parallelism that the responsive logic already degrades on narrow screens — i.e., we'd be defending a half-supported feature.
- Doesn't fix the `Skill` / `Mcp` misclassification.

### Alternative B: Generalize `SidePanel` into a set

Make `view_state.right: SmallVec<[SidePanel; 4]>` or `HashSet<SidePanel>` to truly support multi-selection.

**Rejected because**:
- All P1.5.1 commit-`682b303c` tests assume `Option<SidePanel>` — wholesale rewrite.
- Multi-selection at the data layer would force every consumer (renderers, shortcut handlers, restoration logic) to handle subset state, multiplying complexity.
- Contradicts ADR-011's D-form-factor direction (single Tab).

### Alternative C: Defer to S6

Skip P1.5.4 entirely in S3, push the panel migration to S6 (when D-form-factor lands).

**Rejected because**:
- Leaves four panel booleans unmigrated, breaking the "50 booleans -> typed enums" goal of Phase 1.5.
- Forces P1.5.x to ship with a known design defect (Skill/Mcp in SidePanel).
- The migration cost is roughly the same whether done now or in S6; doing it now reduces S6 scope.

## Validation

This ADR is satisfied when, after P1.5.4 completes:

1. `view_state.right: Option<SidePanel>` is the authoritative writer for which right-anchored business panel is open.
2. `view_state.modal: Option<ModalType>` includes `Skill` and `Mcp`; legacy booleans `skill_panel_open` / `mcp_panel_open` are read-only mirrors.
3. `main.rs:805-854` multi-panel responsive collapse is reduced to single-panel close-on-narrow.
4. Selecting Team via UI auto-closes Task and vice versa (verified by manual test).
5. `Skill` / `Mcp` still render with full-screen scrim and Esc-close behavior (no rendering regression).
6. `cargo test -p clarity-core --lib ui::` passes including updated SidePanel/ModalType variant tests.
7. `cargo clippy --workspace --bins --tests -- -D warnings` is clean.

## References

- `docs/architecture/pretext-ui-theory.md` §1 (information vs decoration)
- `docs/adr/ADR-011-workspace-architecture.md` §Decision 4 (D-form-factor target)
- `docs/adr/ADR-012-renderline-enum-design.md` (parent data-model thesis)
- `docs/plans/2026-05-12-pretext-ui-evolution.md` §S3 P1.5.4
- Commit `682b303c` (P1.5.1 SidePanel/ModalType definitions being revised)
- `crates/clarity-egui/src/main.rs:805-854` (responsive collapse logic to simplify)
- `crates/clarity-egui/src/panels/skill.rs:1-80` (modal-behavior evidence for Skill)
- `crates/clarity-egui/src/panels/mcp.rs:1-80` (modal-behavior evidence for Mcp)
