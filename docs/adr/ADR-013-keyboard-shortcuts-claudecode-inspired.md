---
title: ADR-013: Keyboard Shortcuts — ClaudeCode-Inspired with Focus-Aware Routing
category: ADR
tags: [adr]
---

# ADR-013: Keyboard Shortcuts — ClaudeCode-Inspired with Focus-Aware Routing

- Status: Accepted
- Deciders: juice094
- Date: 2026-05-13

## Context

Clarity's existing shortcut system (per `PROJECT_STATUS.md` and `egui-layout-canons` SKILL §RULE 6) ships an MVP set: `Ctrl+N` new session, `Ctrl+Enter` send, `Ctrl+K` clear, `Ctrl+Shift+P` command palette, `Ctrl+.` toggle, `Ctrl+Shift+T` terminal. The full Vim-keybinding engine is frozen (T_SHORTCUTS, awaiting v0.5+).

The 2026-05-13 dialogue produced two related decisions:

1. **`Ctrl+S` scope change**: From the originally-proposed global "Save router" (would write back focused file regardless of which tab has focus, including SSH transport) → **local to right-panel Workspace tab only**. The user selected option (c): "保存 Workspace 预览中的文件（只在右栏 Workspace 有焦点时生效）".
2. **Shortcut strategy direction**: "另外快捷方式可深度参考 ClaudeCode 进行相关扩展" — keyboard shortcut design should deeply reference ClaudeCode CLI conventions.

This ADR codifies both decisions before S3 (Phase 1.5 State Machine Migration) so the focus state machine being designed accommodates focus-aware shortcut routing from day one.

The original "global Save router" framing in `ADR-011` Decision item 4 referenced the four right-panel operation buttons (Refresh / Save / Download / Close). That framing is **revised by this ADR**: `Save` remains a Workspace-tab button, but its keyboard accelerator `Ctrl+S` is **focus-scoped** rather than globally routed.

## Decision

### 1. Focus-Aware Routing Strategy

Keyboard events route through a **focus-aware dispatcher** that resolves bindings based on the currently focused panel:

```
KeyEvent
  ↓
FocusState (which panel + which widget has focus)
  ↓
ShortcutRegistry::resolve(key, focus_scope)
  ↓
CommandId
  ↓
CommandRouter (existing infrastructure from S1 P0.5.C)
```

**Scope hierarchy** (most specific wins):

| Scope | Active when | Example |
|---|---|---|
| `Widget` | A specific widget has focus | Approval `1/2/3` only when prompt is shown |
| `Panel` | A panel has focus | `Ctrl+S` only in Workspace tab; `j/k` only in chat stream |
| `Modal` | A modal is open | `Esc` cancels modal |
| `App` | No specific focus | `Ctrl+Shift+P` command palette |
| `OS` | Window-level | `Ctrl+Q` quit, `F11` fullscreen |

### 2. ClaudeCode Shortcut Adoption (Direct)

These bindings are adopted verbatim from ClaudeCode CLI:

| Binding | Scope | Action | ClaudeCode parity |
|---|---|---|---|
| `/` | App (in input panel) | Open slash command menu | ✅ Identical |
| `@` | App (in input panel) | File reference autocomplete | ✅ Identical |
| `Esc` | Modal / Input | Cancel current input / dismiss prompt | ✅ Identical |
| `Esc Esc` | App | Interrupt agent execution (double-tap detection within 500ms) | ✅ Identical |
| `Ctrl+R` | Panel (chat stream) | Expand collapsed tool call / thinking block at cursor | ✅ Identical |
| `Tab` / `Shift+Tab` | Widget (approval prompt) | Cycle approval options | ✅ Identical |
| `1` / `2` / `3` | Widget (approval prompt) | Select option directly | ✅ Identical |
| `Up Arrow` | Panel (input panel) | Previous message in history | ✅ Identical |
| `Down Arrow` | Panel (input panel) | Next message in history | ✅ Identical |
| `Ctrl+L` | Panel (chat stream) | Clear chat area (visual; preserves transcript) | ✅ Identical |
| `\ Enter` | Widget (input panel) | Multi-line input continuation | ✅ Identical |
| `?` | App (top-level) | Show keyboard shortcut overlay | ✅ Identical |
| `Ctrl+C` (1×) | App | Cancel current input (clear input panel) | ✅ Identical |
| `Ctrl+C` (2× within 1s) | App | Quit (egui: minimize to tray) | ✅ Identical |
| `Ctrl+D` (empty input) | Widget (input panel) | Quit (egui: same as Ctrl+C×2) | ✅ Identical |

### 3. ClaudeCode-Adapted (Modified for GUI)

These ClaudeCode bindings are adapted because the egui GUI context differs from a terminal:

| Binding | Scope | Clarity Action | Difference from ClaudeCode |
|---|---|---|---|
| `Ctrl+T` | App | Cycle next session tab | ClaudeCode has no tabs (single session per process) |
| `Ctrl+Shift+T` | App | Reopen last closed session | egui-specific |
| `Ctrl+W` | App | Close current session tab | egui-specific |
| `Ctrl+N` | App | New session within current persona | Aligns with existing MVP shortcut |
| `Ctrl+1`..`Ctrl+9` | App | Direct-jump to session N | egui-specific multi-session |
| `Ctrl+Shift+1`..`Ctrl+Shift+9` | App | Direct-jump to persona N | Clarity-specific multi-persona |

### 4. Focus-Scoped Bindings (Critical: where ClaudeCode is a CLI, Clarity is GUI)

These bindings exist ONLY in specific focus scopes — they do not fire globally:

| Binding | Scope (focus REQUIRED) | Action |
|---|---|---|
| **`Ctrl+S`** | **Right-panel Workspace tab** | **Save the file currently in preview to local fs.** Does NOT fire globally. (2026-05-13 user direction option c.) |
| `j` / `k` | Chat stream panel | Line navigation (RenderLine cursor down/up) |
| `g` / `G` | Chat stream panel | Jump to top / bottom |
| `Enter` | Chat stream panel | Activate selected line (open ArtifactRef / follow CrossInstanceRef / expand ToolCallHeader) |
| `v` / `V` | Chat stream panel | Line selection mode (multi-line copy) |
| `Ctrl+F` | Chat stream panel | Search within chat |
| `Ctrl+F` | Right-panel Workspace tab | Search file content |
| `Ctrl+P` | App | File picker (opens Workspace tab and focuses search) |
| `Ctrl+B` | App | Toggle left panel visibility |
| `Ctrl+J` | App | Toggle bottom Status Bar floating panels (Equipment region) |
| `m` | Chat stream panel | Pin/unpin selected message (Mention LineRole tag) |
| `/` (TUI mode only) | App | Slash menu — `clarity-tui` keyboard-driven equivalent |

### 5. Clarity-Specific Extensions (Not in ClaudeCode)

These bindings address Clarity-unique surfaces that ClaudeCode has no equivalent for:

| Binding | Scope | Action | Why Clarity-Specific |
|---|---|---|---|
| `Ctrl+Shift+P` | App | Command Palette (existing, unchanged) | egui Pretext architecture |
| `Ctrl+Shift+O` | App | Open Orchestrate dashboard (cross-session view) | Multi-session orchestration (no CCode equivalent) |
| `Ctrl+Shift+C` | App | Open Cluster view (Settings → Cluster) | Multi-device topology (no CCode equivalent) |
| `Ctrl+Shift+M` | App | Compose a Mention to another instance | Cross-instance messaging (no CCode equivalent) |
| `Alt+Left` / `Alt+Right` | App | Cycle right-panel tabs (SSH / Workspace / Settings) | egui tab D form factor |
| `Alt+1` / `Alt+2` / `Alt+3` | App | Direct-jump to SSH / Workspace / Settings tab | egui tab D form factor |

### 6. Modifier Policy

To avoid binding conflicts and make routing predictable:

| Modifier | Domain |
|---|---|
| `Ctrl+` | App-level actions (new session, save, command palette, search) |
| `Ctrl+Shift+` | Global app actions (palette, persona switch, orchestrate, cluster) |
| `Alt+` | Panel/tab navigation |
| `Cmd+` (macOS) | OS window actions |
| (no modifier) | Focus-scoped — only fires when the right panel/widget has focus |

### 7. Conflict Resolution

When multiple bindings match a key event:

1. **Scope specificity wins**: `Widget` > `Panel` > `Modal` > `App` > `OS`.
2. **Most recently registered wins** if same scope (handler order is LIFO).
3. **Input mode wins over navigation mode**: If a text-input widget has focus and the binding has no modifier, the input gets the keystroke. (E.g., `j` typed in the input panel inserts a `j`, does NOT trigger line navigation.)

This last rule is **critical** — it prevents `j/k/g/G` line navigation from interfering with text input. The chat stream's line navigation only activates when the chat stream itself has focus (e.g., user clicked on a message or pressed `Esc` to leave the input panel).

## Consequences

### Positive

- **ClaudeCode users transfer their muscle memory instantly**: 14 direct-adoption bindings means a user moving from ClaudeCode CLI to Clarity GUI does not relearn the core interaction model.
- **Focus-aware routing is unambiguous**: `Ctrl+S` cannot accidentally save a file when the user thinks they are in a chat conversation. The hierarchy is explicit.
- **TUI parity is preserved**: All ClaudeCode-adopted bindings work in `clarity-tui` (which is a TUI of the same family). Snapshot tests can validate parity.
- **Extension path is clear**: New Clarity-specific surfaces get bindings in the `Alt+` or `Ctrl+Shift+` namespaces, which ClaudeCode does not use.

### Negative

- **More state in the focus dispatcher**: The `ShortcutRegistry::resolve(key, focus_scope)` function adds complexity over a flat binding map. Mitigation: covered by S3 (State Machine Migration) since focus state is part of the typed `ViewState` enum anyway.
- **Documentation cost**: 30+ bindings × 5 scopes = lots of cells to keep accurate. Mitigation: the `?` help overlay generates from the same `ShortcutRegistry`, ensuring docs cannot drift from runtime.
- **Approval `1/2/3` may shadow text input in TUI**: If approval prompt and input panel both have focus simultaneously (which should not happen but TUI focus is single-channel), input may break. Mitigation: approval prompt grabs exclusive focus.

### Neutral

- **`Ctrl+S` revised from "global router" (ADR-011 Decision item 4 framing in plan.md §6.2) to "Workspace tab focus only"**. ADR-011's workspace structure is unchanged; only the keyboard binding scope is narrowed.
- **The existing 6 MVP bindings** (`Ctrl+N`, `Ctrl+Enter`, `Ctrl+K`, `Ctrl+Shift+P`, `Ctrl+.`, `Ctrl+Shift+T`) are absorbed verbatim into this scheme. No backward-incompatible changes.
- **Vim keybindings remain frozen** (T_SHORTCUTS per PROJECT_STATUS.md §10). The `j/k/g/G` line navigation here is **not** Vim-mode — it is ClaudeCode-style chat stream navigation, scope-limited to the chat panel.

## Implementation Phases

| Phase | Scope | Acceptance |
|---|---|---|
| **S3 (Phase 1.5)** | Focus state enum in `ViewState`; `ShortcutRegistry::resolve()` skeleton | `cargo test -p clarity-core ui::shortcuts` validates focus-aware routing |
| **S4 (Phase 2A)** | Approval, slash, file-ref bindings on RenderLine variants | Approval `1/2/3` works on ApprovalPrompt RenderLine |
| **S5 (Phase 2B)** | Chat stream navigation bindings (`j/k/g/G/Enter/v/V/Ctrl+R/Ctrl+L`) | Line cursor moves correctly; expansion works |
| **S6 (Phase 2C)** | Right-panel `Ctrl+S` Workspace-focus-only; `Alt+1/2/3` tab cycle; `Ctrl+P` file picker | `Ctrl+S` does NOT fire when chat stream has focus |
| **S7 (Phase 3A)** | TUI parity — same bindings work in `clarity-tui` | Snapshot test: same key event → same CommandId in both renderers |
| **S8 (Phase 3B)** | Clarity-specific extensions (`Ctrl+Shift+O/C/M`) | Orchestrate / Cluster / Mention dashboards open via keyboard |

## Alternatives

| Option | Description | Outcome |
|---|---|---|
| **A.** Global Ctrl+S router (original ADR-011 framing) | One Ctrl+S routes to any saveable surface regardless of focus | Rejected per 2026-05-13 option (c) |
| **B.** Focus-scoped Ctrl+S (this ADR) | Ctrl+S only fires when Workspace tab has focus | **Accepted** |
| **C.** No Ctrl+S; only the right-panel `Save` button | Drop the keyboard binding entirely | Rejected (degrades keyboard-first UX) |
| **D.** Vim-mode global binding `:w` | Vim-style command line | Rejected (T_SHORTCUTS frozen; out of scope) |

## Validation

- [ ] `cargo test -p clarity-core ui::shortcuts` covers focus-scoped routing for all 30+ bindings.
- [ ] `?` help overlay renders all bindings grouped by scope.
- [ ] Pressing `j` in the chat input panel inserts the character `j` (does NOT trigger navigation).
- [ ] Pressing `Esc` to exit input panel and then `j` triggers line navigation.
- [ ] Pressing `Ctrl+S` while chat panel has focus is a no-op (or shows a hint that it is Workspace-only); pressing `Ctrl+S` in Workspace tab saves the previewed file.
- [ ] `Esc Esc` within 500ms interrupts active agent turn; outside 500ms is two separate Esc presses.
- [ ] Snapshot parity test: same key sequence produces same `CommandId` in GUI (`clarity-egui`) and TUI (`clarity-tui`).

## References

- ClaudeCode CLI keyboard reference: empirical observation of `claude` CLI ≤ 2.1.88 (2026-04-30 source map leak); also <https://docs.anthropic.com/claude-code/keyboard-shortcuts> (if upstream publishes).
- Existing Clarity shortcuts: `PROJECT_STATUS.md` §6 Sprint 12 closure (MVP set); `~/.config/agents/skills/egui-layout-canons/SKILL.md`.
- Companion ADR-011 §Decision item 4: "Save = global Ctrl+S router" framing is revised by this ADR (Save remains a button; keyboard accelerator is focus-scoped).
- Companion ADR-012: RenderLine variants ToolCallHeader / ApprovalPrompt / Thinking define focus-scoped bindings target widgets.
- Pretext UI plan: `docs/planning/plans/2026-05-12-pretext-ui-evolution.md` §6.2 (P3B.6 description updated by companion commit).
- Vim mode freeze: `PROJECT_STATUS.md` §10 T_SHORTCUTS (Vim engine deferred to v0.5+).
