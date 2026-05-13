# Pretext UI Theory — Why Clarity Blurs the TUI/GUI Boundary

> **Date**: 2026-05-12
> **Status**: Accepted as architectural north star
> **Scope**: All future UI work in `clarity-egui`, `clarity-tui`, and `clarity-core/src/ui`
> **Anchors**: `docs/plans/2026-05-12-pretext-ui-evolution.md` (execution plan), `docs/audits/2026-05-12-ui-design-audit.md` (Phase 0.5 audit)

This document captures the **strategic rationale** for the Pretext UI evolution.
Tactical phasing lives in the plan; this document explains *why* we accept the
investment cost.

---

## 1. Thesis

> **Blurring the TUI/GUI boundary is not an aesthetic preference — it is an
> information architecture discipline.**

Traditional GUI ≈ 30% information + 70% decoration.
Traditional TUI ≈ 100% information.
Pretext UI ≈ 90% information + 10% tasteful decoration.

The blurred boundary is a **forcing function**: if a piece of state cannot be
expressed in the TUI, it is almost certainly decoration in the GUI. Removing it
is a maintenance win, not a feature loss.

---

## 2. The Six-Dimension Advantage Matrix

### Dimension 1 — Architecture (highest leverage)

| Benefit | Mechanism | Clarity impact |
|---------|-----------|----------------|
| Single source of truth | `ViewState`/`CommandItem`/`RenderLine` shared across renderers | Business logic ×1, renderers ×N |
| Enumerable state | TUI demands tractable state machine → forces GUI to be tractable too | 33 booleans collapse to ~5 enums |
| Cross-renderer testing | `to_text()` snapshot diff replaces pixel comparison | CI time ↓80%, no flaky tests |
| Graceful degradation | GUI crash → fallback to TUI; remote sessions get first-class support | Error recovery free |

### Dimension 2 — User Experience

| Benefit | Audience |
|---------|----------|
| Keyboard-first (j/k/g/G/Enter) | Long-session users, developers |
| Unified mental model | Local GUI ↔ SSH TUI switch with zero retraining |
| Screen-reader friendly | Visually impaired users become first-class citizens free of charge |
| Low learning curve | Command palette = self-documenting affordance |

### Dimension 3 — AI Capability (Clarity's asymmetric win)

This is the dimension that makes Pretext UI **structurally aligned** with
Clarity's mission as an AI agent framework:

```
Agent reads its own UI = reads text, not OCR'd screenshots
Agent debugs itself    = reads Vec<RenderLine>, no vision model required
State -> prompt        = "Current UI:\n<attached lines>"
```

For a video editor, this dimension is irrelevant. For an AI agent framework,
it is the **core capability** that turns the UI from a black box into an
introspectable system. Without it, the agent can build UI but cannot reason
about its own UI behavior.

### Dimension 4 — Operations

| Scenario | Pretext UI behavior |
|----------|---------------------|
| SSH remote | Native (no X forwarding / VNC) |
| Low-bandwidth / edge | TUI path automatic (ratatui runs on RP2040) |
| Terminal logs | UI state = log entry = UI replay (trinity) |
| Test automation | Same fixtures validate GUI/TUI/CI |

### Dimension 5 — Engineering

| Benefit | Source |
|---------|--------|
| Compile portability | Core crate decoupled from renderer |
| Dead-code detection | One business logic copy -> static analysis trivial |
| Designer/developer collaboration | Designer edits RenderLine semantics; developers implement two renderers |
| Müller-Brockmann grid discipline | "Through structure comes freedom" — eliminates visual entropy |

### Dimension 6 — Philosophical (long horizon)

- **Anti-entropy**: text endures; aesthetic trends do not. `RenderLine` will
  still parse in 10 years; the 2026 rounded-corner fashion will not.
- **Transparency**: system behavior is readable. No black box.
- **Reversibility**: text-first state always round-trips through
  serialization.

---

## 3. Where Pretext UI Wins, Where It Loses

### Where it wins (= Clarity's profile)

- AI agent framework (content is text by nature)
- Developer tooling (audience accepts keyboard-first)
- Long sessions (10K+ messages, virtual scrolling matters)
- Remote/local hybrid usage (cloud agents + local IDE)
- Agent self-introspection requirement (core capability)

### Where it loses (not Clarity)

- Mass-market SaaS with non-developer users
- Design tools (Figma/Photoshop — content is fundamentally visual)
- Games (experience = visual immersion)
- Rich-media-first products (video, animation, drag-drop heavy)

**Conclusion**: Clarity is in the minority of products where all six
dimensions of advantage compound, and where the structural costs of blurring
are absent. This is the load-bearing claim of the strategy.

---

## 4. Borrowed Concepts (and what we keep distinct)

### From Claude (UI philosophy)

| Concept | Adoption form |
|---------|---------------|
| Slash commands (`/help`, `/clear`, `/init`) | `CommandRouter` with slash-prefix detection in Input panel |
| Inline tool-call rendering | `RenderLine::ToolCallHeader` + `ToolCallArg` (Phase 2) |
| Artifacts side panel | Workspace evolution: code blocks can promote to persistent artifact |
| Memory affordances | Per-message pinned/ephemeral indicator |
| Project context in chrome | TitleBar shows `workspace / branch / dirty count` |
| Streaming with cursor | Word-by-word with cursor token |
| Prompt-able UI | Every UI action is a `CommandId`; agent can invoke as if user |

### From TUI ecosystem (engineering rigor)

| Concept | Source | Adoption |
|---------|--------|----------|
| Constraint solver | Cassowary / ratatui kasuari | `egui_extras::StripBuilder` for chrome (Phase 1) |
| Line-based buffer | Helix / Emacs / xi-editor | `RenderLine` enum (Phase 2) |
| Baseline grid | TeX box/glue/penalty | Per-role `line_height` token |
| Box-drawing decoration | Modern TUIs (lazygit, k9s) | TUI renderer uses Unicode boxes (Phase 3) |

### What stays distinct — egui's pixel-decoration advantages

We **keep** these GUI-only affordances. They are the 10% decoration in our
"90% information + 10% decoration" ratio:

- Rounded corners on chrome (`theme.radius_*`)
- Focus rings (`theme.focus_ring`)
- Hover states with smooth color transition
- Native window controls (close/minimize/maximize)
- Window drag from TitleBar
- Drag-to-resize side panels
- File drag-drop into chat
- Image / icon rendering
- Modal dialogs with backdrop
- Mouse-driven affordances (scrub, drag-reorder)

### What stays distinct — TUI's text-purity advantages

- ANSI color and box-drawing instead of pixel decoration
- SSH-native, no X forwarding
- Pipe-able state (`clarity-tui --dump-state | jq`)
- Programmable navigation (any key sequence scriptable)
- Built-in screen-reader compatibility

---

## 5. Information vs Decoration Test

For any UI element, ask: **does this element fail gracefully when reduced to
text?**

| Element | Pretext test | Verdict |
|---------|--------------|---------|
| Message body | `"User: hello"` reads same as bubble | Information |
| Code block | Renders fine without syntax color | Information |
| Tool call | `> search(query=foo) -> 12 results` reads same as expanded card | Information |
| Hover tooltip | "Click to start Gateway" -> footer hint in TUI | Information |
| Rounded corner | Cannot reduce | Decoration (keep in GUI only) |
| Drop shadow | Cannot reduce | Decoration |
| Smooth scroll | Discrete line in TUI | Decoration (GUI bonus) |
| Avatar image | Falls back to `[A]` initials | Decoration (graceful) |

**Rule**: if reducing to text destroys meaning, the element is **decoration**
and lives in the GUI renderer only. If reducing preserves meaning, the element
is **information** and lives in `clarity-core::ui::RenderLine`.

---

## 6. Anti-Patterns to Reject

This is what we will **not** do, despite ecosystem pressure:

- Building UI as "components" that own both data and rendering (React-style coupling)
- Pixel-perfect designs that cannot be expressed as text first
- Renderer-specific state (no `egui::Memory` for business state — only for ephemeral focus / scroll position)
- Streaming text rendering that re-parses the full message every frame
- "Skinning" the TUI to look like GUI (or vice versa). Each renderer is honest about its medium.

---

## 7. References

### Academic foundation (Kimi K2.6 conversation context, 2026-05-12)

- **Sketchpad (Sutherland, 1963)** — constraint satisfaction as UI primitive
- **Cassowary (Borning et al., 1997 UIST)** — incremental linear constraints
- **Grid Systems in Graphic Design (Müller-Brockmann, 1966)** — "freedom through structure"
- **Immediate Mode GUI as State Monad (2024)** — formal proof IMGUI can host declarative sublayers

### Engineering precedent

- iOS/macOS Auto Layout (billions of devices on Cassowary)
- ratatui (Rust Cassowary port, runs on RP2040 microcontroller)
- egui_extras::StripBuilder (official egui constraint primitive)
- Helix editor (line-based buffer with incremental updates)

### Internal references

- `docs/plans/2026-05-12-pretext-ui-evolution.md` — execution plan with 5 phases
- `docs/audits/2026-05-12-ui-design-audit.md` — Phase 0.5 audit, 6-axis review
- `crates/clarity-egui/EGUI_LAYOUT.md` — layout rules including 5 traps
- `~/.config/agents/skills/egui-layout-canons/SKILL.md` — skill protocol

---

## 8. Decision Record

| Date | Decision | Source |
|------|----------|--------|
| 2026-05-12 | Adopt Pretext UI as architectural north star | This document |
| 2026-05-12 | Six-dimension advantage matrix is the value justification | §2 |
| 2026-05-12 | Clarity's profile uniquely fits Pretext UI | §3 |
| 2026-05-12 | Borrow from Claude philosophy; keep egui pixel decoration distinct | §4 |
| 2026-05-12 | Information-vs-decoration test gates new UI work | §5 |
| 2026-05-13 | Filesystem is the Agent evolution substrate (not model weights) | §9 |
| 2026-05-13 | Error Budgeting (Donaldson / NASA / Google SRE) is the methodology foundation | §10 |
| 2026-05-13 | openclaw bootstrap files are 100% adopted; multi-instance layered on top | ADR-011 |
| 2026-05-13 | RenderLine has 13 enum variants absorbing 30 line patterns via LineRole | ADR-012 |
| 2026-05-13 | Keyboard shortcuts: focus-aware routing; deep ClaudeCode-inspired bindings | ADR-013 |

---

## 9. Filesystem as Agent Evolution Substrate

This section was added 2026-05-13 after the design dialogue that produced ADR-011 (workspace architecture). It articulates *where* the Agent actually stores its evolution, and *why* the choice is architecturally load-bearing.

### 9.1 The Thesis

> **The Agent's persistent memory, learned conventions, and accumulated skill are encoded as files in a Pretext-compatible directory tree — not as fine-tuned model weights and not as opaque vector embeddings.**

Conventional AI products treat the model as the locus of intelligence:

- Provider-side fine-tuning produces a new model checkpoint.
- RLHF buries preferences inside weight updates.
- Vector databases store knowledge as black-box embeddings.

These approaches share a property hostile to Clarity's mission: **the user cannot read or audit what the Agent has learned**. The intelligence is in someone else's hardware, in someone else's format, behind someone else's API.

Pretext UI's text-first commitment forces the inverse choice:

```
Agent learning  =  filesystem evolution
Agent memory    =  text files in workspace/memory/
Agent skill     =  markdown files in workspace/skills/
Agent persona   =  files at workspace/{AGENTS,SOUL,USER,IDENTITY,TOOLS,HEARTBEAT,BOOT}.md
Agent state     =  session transcripts (JSONL) + plan YAML + artifacts/
Agent crossover =  workspaces/_shared/{facts,conventions,cross-refs}/
```

Every Agent capability has a corresponding file or directory. The user reads (and edits) the same artifacts the Agent reads. There is no privileged storage.

### 9.2 Why This Matters

| Dimension | Conventional AI | Filesystem-Substrate Agent |
|-----------|-----------------|----------------------------|
| Where learning lives | Model weights (opaque, vendor-owned) | Files (transparent, user-owned) |
| How to audit | Inspect logits / SHAP / fragile probes | Read text |
| How to backup | Vendor export API (if any) | `cp -r workspaces/ backup/` |
| How to share between agents | Vendor's RAG / RAG / fine-tune pipeline | `cp file.md other-agent/` |
| How to undo a bad lesson | Retrain (expensive, opaque) | `git revert` |
| How to sync across devices | Vendor cloud or none | Syncthing P2P |
| How to compose two agents | Federated learning (research) | Symbolic links or shared `_shared/` |

The asymmetric advantage is **composition without orchestration**. Multi-Agent systems become file-system relationships:

- Agent A's output is Agent B's input → just write to `workspaces/B/inbox/`.
- Two agents share a fact → both point to `workspaces/_shared/facts/redis-cache-policy.md`.
- A device-local agent learns something → Syncthing replicates the file across the cluster.

No bus, no broker, no schema, no API. The filesystem **is** the protocol.

### 9.3 The Pretext UI Connection

The 90% / 10% information-vs-decoration ratio is not a stylistic preference — it is a **consequence** of the filesystem-substrate thesis:

- If knowledge is text in files, the UI primarily renders text.
- If decisions are markdown, the UI primarily reads markdown.
- If conversation is line-based, the UI primarily lists lines.
- The 10% decoration is reserved for the irreducibly visual: focus rings, animation cues, mouse affordances. None of these store knowledge.

When Pretext UI shows the Agent's state, it is **literally** showing the filesystem — formatted, but not transformed. The user could `cat` the same files and get the same information.

### 9.4 Practical Consequences (Already Locked In)

| Decision | Locked by |
|----------|-----------|
| Workspace contract = openclaw 7 bootstrap files | ADR-011 (2026-05-13) |
| Workspace naming = role-nested (`<role>/<machine>-<n>/`) | ADR-011 §Decision item 2 |
| Cross-instance messaging via `notes/mentions/{inbox,outbox}/` files | ADR-011 §Decision item 4 |
| Cluster topology metadata at `workspaces/_cluster/peers.yaml` | ADR-011 §Decision item 4 |
| Daily memory at `memory/YYYY-MM-DD.md` (openclaw contract) | ADR-011 §Decision item 1 |
| RenderLine variants must be `to_text()`-serializable | ADR-012 §Decision (data model thesis) |

### 9.5 Anti-Patterns This Rejects

- **Hidden caches**: An Agent state that lives only in process memory or in a SQLite blob without a filesystem mirror is invisible to the user. Rejected.
- **Encrypted opaque memory**: Encrypting Agent files prevents `cat`/`grep` audit. If encryption is required (e.g., secrets), they live in `~/.clarity/credentials/`, NOT in `workspaces/`. Same boundary as openclaw.
- **Vendor-locked memory APIs**: Memory provided through a vendor SDK that does not round-trip to disk is a non-starter. Memory plugins must serialize state to user-visible files.
- **Model fine-tuning as a substitute for filesystem**: Fine-tuned models are vendor-owned and inscrutable. Filesystem is the substrate; the model is a renderer.

---

## 10. Error Budgeting as Methodology Foundation

This section was added 2026-05-13 to formalize the methodology framework implicit in the multi-session execution plan, audit gates, and CI baselines. The framework is **Error Budgeting**, a mature engineering discipline with deep academic and industrial roots.

### 10.1 The Methodology in One Sentence

> **Every UI / Agent / system design decision lives within an explicit error budget that allocates failure tolerance across subsystems, and every phase gate spends or returns budget against measurable error probes.**

### 10.2 Academic and Industrial Lineage

| Era | Contributor | Contribution |
|-----|-------------|--------------|
| 1980s | Donaldson | Error budgeting in precision machine design |
| 1990s | Thompson & Fix, Slocum | Roll-down / roll-up / iterate methodology for ultra-precision machine tools and CMMs |
| 2000s | ESA, NASA | *Pointing Error Engineering Handbook*; knowledge vs. performance error separation |
| 2010s | Google SRE | Error Budget = `1 - SLO`; trade reliability for release velocity |
| Core math | RSS composition | `E_total = sqrt(sum(e_i^2))` for independent error sources |

The methodology is not about predicting errors; it is about **using budgeted tolerance as a trade-off currency**. "Where should we invest engineering effort to maximally satisfy total constraints?" is the canonical question.

### 10.3 Application to Clarity

Clarity already operates within this framework, but the methodology was implicit before this section. The mapping:

| Clarity Practice | Error-Budgeting Equivalent |
|------------------|---------------------------|
| 49h Pretext UI Phase 2-3 total budget | Top-level total budget `E_total` |
| Per-session hour allocation (S3=6h, S4-S6=18h, S7-S9=16h) | Subsystem budget allocation |
| Decision gates G1-G4 in plan | Probe checkpoints |
| `cargo test --workspace` 0 failure | Acceptance-criterion probe |
| `cargo clippy -D warnings` 0 warning | Acceptance-criterion probe |
| `BlockSlot` usage < 5% (ADR-012 metric) | Quality probe at runtime |
| Roll-back path (feature flag, ADR supersede) | Budget return mechanism |
| `cargo bench` 60fps @ 10K lines | Performance budget |

### 10.4 The Numerical-Integration Analogy

The 2026-05-13 materials presented an extension of error budgeting using numerical integration as the analogy:

| Numerical Method | Engineering Practice | Clarity Instance |
|------------------|---------------------|------------------|
| Composite quadrature | Atomic Design / modular phases | S1-S9 atomic commits per phase |
| Gauss quadrature | Critical-path node prioritization | P0 acceptance criteria gating each phase |
| Richardson extrapolation | Multi-perspective review | Audit + Skill + ADR + plan cross-check |
| Adaptive step size | Probe-driven refinement | Decision gates G1-G4 |
| Runge's phenomenon | High-order extrapolation failure | Single-shot 49h sprint = guaranteed runge collapse |

The numerical analogy is **not** a new methodology — it is a heuristic vocabulary that makes the trade-offs intuitive. The load-bearing framework remains Error Budgeting.

### 10.5 Operational Rules

These are mandatory for all future Clarity work touching the Pretext UI surface or Agent runtime:

1. **Every plan section declares its budget** in hours / sessions / commit count.
2. **Every acceptance criterion is a probe** with a measurable threshold.
3. **Every probe failure returns budget** (declared rollback path) rather than consuming it silently.
4. **Every ADR records the decision context** so future budget reallocation can be made against documented prior state.
5. **No phase commits without an explicit probe**. Even refactor commits must specify a probe (cargo check / cargo test / visual diff).

### 10.6 What This Replaces

This methodology replaces (more accurately: *names*) several practices that were previously implicit:

- "Best-effort coding without acceptance criteria" → replaced by **probe-based gates**.
- "We will refactor when we have time" → replaced by **return budget to its origin or accept the debt explicitly in an ADR**.
- "Estimated 500 lines" → replaced by **declared budget with confidence band, e.g. 1500-2500 lines + maintenance**.
- "It works on my machine" → replaced by **CI probes + cross-renderer parity probes + benchmark probes**.

### 10.7 Practical Probes Inventory (Current State 2026-05-13)

| Probe | Threshold | Source |
|-------|-----------|--------|
| `cargo check --workspace` | 0 errors | Pre-commit |
| `cargo clippy --workspace --lib --bins --tests -- -D warnings` | 0 warnings | Pre-commit |
| `cargo test --workspace --lib` | 0 failures | CI gate |
| `cargo audit --deny unsound --deny yanked` | 0 high/critical | CI gate |
| `cargo fmt --all -- --check` | 0 diffs | CI gate |
| `cargo bench` (Phase 2B+) | 60fps @ 10K lines | Phase 2B acceptance |
| `BlockSlot` rate (Phase 2C+) | < 5% of total rendered lines | ADR-012 |
| Cross-renderer `to_text()` byte-equality | byte-exact | Phase 3A snapshot test |
| Workspace round-trip with openclaw | 0 `doctor` errors | ADR-011 |

### 10.8 References

- Donaldson (1980s): Error budgeting in precision machine design.
- Slocum (1992): *Precision Machine Design*, Chapter 2 (error budget methodology).
- ESA *Pointing Error Engineering Handbook* (2000s, regularly updated).
- Google SRE Book (Beyer et al., 2016): Chapter 3 (Embracing Risk) and Chapter 4 (Service Level Objectives).
- Wing (2006): *Computational Thinking* — the meta-framework that allows this cross-domain methodology import.
