---
title: ADR-012: RenderLine Enum Design — 13 Variants Covering 30 Line Patterns
category: ADR
tags: [adr, rendering]
---

# ADR-012: RenderLine Enum Design — 13 Variants Covering 30 Line Patterns

- Status: Accepted
- Deciders: juice094
- Date: 2026-05-13

## Context

The Pretext UI thesis (`docs/architecture/pretext-ui-theory.md` §1) requires every UI element to be reducible to text — the **information vs decoration test**. This forces a discrete, line-based data model for chat content rather than a pixel-continuous markdown stream. `RenderLine` is the chosen atomic data type.

A 2026-05-13 design audit produced 30 candidate line patterns from cross-referencing:

- **ClaudeCode CLI** (Anthropic): tool-call headers, thinking blocks, approval prompts, status spinners, code diffs.
- **Claude.ai web**: artifact references, streaming cursor, slash commands.
- **Kimi CLI**: document references, slash menu.
- **Cursor**: inline code suggestions.
- **openclaw**: workspace bootstrap markers, sandbox warnings.
- **Markdown**: headings, lists, quotes, code blocks.
- **Clarity-specific needs**: cross-instance mentions, token usage display, network status banners.

The risk of a naive 30-variant enum is **type-system overgrowth** (poor exhaustive-match ergonomics, brittle TUI rendering, hard cross-renderer testing). Conversely, an under-specified enum (e.g., 3 variants — Text / Code / Tool) loses the explicit structure that makes the Pretext model auditable.

The user (2026-05-13 dialogue): "我不确定一共有哪几种，以及是否是合适的安排" — explicitly delegating the variant count decision while requesting a defended rationale.

## Decision

Adopt a **13-variant enum** that absorbs 30 line patterns via two consolidation rules:

1. **Parameterize visually similar patterns through `LineRole`**. 15 patterns (user/agent/system message, error, headings H1-H6, quote, list items, file refs, mentions, plan steps, context warnings, token usage, network status, sandbox) collapse into a single `Text` variant with a `LineRole` enum parameter.
2. **Keep semantically unique patterns as distinct variants**. 12 patterns with distinct interaction semantics (tool calls, thinking, approval, status spinners, artifact refs, cross-instance refs, slash completion, streaming cursor, diff lines, dividers, empty lines, block-slot fallback) become independent variants.

### Final Enum

```rust
// clarity-core/src/ui/render_line.rs
pub enum RenderLine {
    Text { spans: Vec<Span>, role: LineRole, indent: u8 },
    CodeLine { lang: SmolStr, content: SmolStr, line_no: Option<u32>, diff: DiffKind },
    ToolCallHeader { name: SmolStr, status: ToolStatus, expanded: bool },
    ToolCallArg { key: SmolStr, value: SmolStr },
    Thinking { content: SmolStr, collapsed: bool },
    ApprovalPrompt { options: Vec<ApprovalOption> },
    StatusLine { kind: StatusKind, content: SmolStr, transient: bool },
    ArtifactRef { artifact_id: ArtifactId, summary: SmolStr },
    CrossInstanceRef {
        target_instance: InstanceId,
        target_session: Option<SessionId>,
        message: SmolStr,
    },
    SlashCompletion { command: SmolStr, description: SmolStr },
    StreamingCursor,
    Divider,
    Empty,
    BlockSlot { block_id: BlockId, line_count: u8 },
}

pub enum LineRole {
    UserMessage,
    AgentMessage,
    SystemMessage,
    ErrorMessage,
    Heading(u8),                    // 1..=6
    Quote,
    UnorderedListItem(u8),          // indent level
    OrderedListItem { num: u32, indent: u8 },
    Mention,                        // @instance / @user
    FileRef,                        // @path/to/file
    Status,
    Warning,                        // ⚠
    Note,
    TokenUsage,
    ContextCompaction,
    Sandbox,                        // openclaw sandbox warning
}

pub enum DiffKind { Normal, Added, Removed, Context }

pub enum StatusKind {
    Spinner,
    Progress { current: u32, total: u32 },
    Network,
    Compaction,
    ModelSwitch,
}

pub enum ApprovalOption {
    Yes,
    YesAndRemember,
    No { reason_required: bool },
    Custom(SmolStr),
}
```

## Pattern Coverage Map (30 → 13)

| Pattern | Variant | Notes |
|---|---|---|
| user message | `Text { role: UserMessage }` | |
| agent reply | `Text { role: AgentMessage }` | |
| system message | `Text { role: SystemMessage }` | |
| error | `Text { role: ErrorMessage }` | |
| heading h1-h6 | `Text { role: Heading(N) }` | |
| quote block | `Text { role: Quote }` | |
| unordered list | `Text { role: UnorderedListItem(indent) }` | |
| ordered list | `Text { role: OrderedListItem }` | |
| code line | `CodeLine { diff: Normal }` | |
| diff added/removed | `CodeLine { diff: Added/Removed }` | |
| tool call header | `ToolCallHeader` | |
| tool call arg | `ToolCallArg` | |
| tool call result | `Text` or `CodeLine` (post-tool) | not a unique variant |
| thinking | `Thinking` | folded by default |
| approval prompt | `ApprovalPrompt` | |
| spinner / progress | `StatusLine { kind: Spinner/Progress }` | |
| divider | `Divider` | |
| empty | `Empty` | |
| block fallback | `BlockSlot` | tables/images/full-screen Plan |
| file ref `@path` | `Text { role: FileRef }` | |
| instance mention `@inst` | `Text { role: Mention }` | |
| slash completion | `SlashCompletion` | input-panel only |
| artifact ref | `ArtifactRef` | |
| plan step | `Text { role: OrderedListItem }` | reuse |
| streaming cursor | `StreamingCursor` | |
| context compaction | `Text { role: ContextCompaction }` or `StatusLine { Compaction }` | |
| token usage | `Text { role: TokenUsage }` | |
| cross-instance ref | `CrossInstanceRef` | Clarity-specific |
| network banner | `StatusLine { kind: Network }` | |
| sandbox warning | `Text { role: Sandbox }` | openclaw-compat |

## Consequences

### Positive

- **Exhaustive match enforcement**: 13 variants are tractable for `match` exhaustiveness checks. Adding a 14th forces explicit handling in every renderer.
- **AI self-introspection**: `RenderLine::to_text()` produces deterministic text representations for every variant. Agent can read its own UI as `Vec<String>`.
- **Cross-renderer parity**: TUI and GUI implementations share 13 rendering functions. Snapshot tests can compare `to_text()` byte-exact across renderers.
- **Clarity-specific differentiation**: `CrossInstanceRef` and `Mention`/`OrderedListItem` make multi-instance coordination first-class. No external system has this.
- **Streaming-friendly**: All variants are small fixed-size enums; per-line append in streaming mode is O(1) without re-parsing prior lines.

### Negative

- **`BlockSlot` is the failure escape hatch**. Heavy use signals that the line model is inadequate for the content (tables, complex HTML). Mitigation: CI metric `block_slot_count / total_lines < 5%` per fixture set.
- **`StreamingCursor` requires renderer-specific animation**. GUI gets opacity fade; TUI gets discrete character (`▎` or `█`). Renderers must stay in sync.
- **`CrossInstanceRef` depends on unbuilt infrastructure**. Pre-v0.5 (Hub-Worker absent), it must degrade gracefully — e.g., point to a local marker file in `_shared/cross-refs/`.
- **`Thinking { collapsed: bool }` adds UI state to the data model**. This was a deliberate trade — collapse state is semantically part of the line, not pure rendering. Alternative (keep collapse state in renderer Memory) was rejected for losing cross-renderer parity.

### Neutral

- 16-variant `LineRole` enum is large but each variant has a single rendering function (`text_role_style(role: LineRole) -> Style`). Growth controlled.
- `ApprovalOption` enum has 4 variants matching ClaudeCode's prompt shape. `Custom(SmolStr)` provides extensibility without enum growth.

## Implementation Phases

| Phase | Variants Implemented | Acceptance |
|---|---|---|
| **S4 (Phase 2A)** | All 13 type definitions + `to_text()` + unit tests for each variant | `cargo test -p clarity-core ui::render_line` 100% passing |
| **S5 (Phase 2B)** | 9 P0 variants fully rendered (Text, CodeLine, ToolCall×2, Thinking, Approval, Status, Divider, Empty, BlockSlot) | Visual side-by-side comparison to ClaudeCode reference screenshots |
| **S6 (Phase 2C)** | Clarity-specific variants wired (Mention, FileRef, Slash, Artifact, CrossInstance partial) | `@path` autocomplete works; clicking `@instance` jumps to local marker file |
| **v0.5+** | `CrossInstanceRef` real routing (post Hub-Worker) | Mentions deliver via `notes/mentions/inbox/` between live instances |

## Alternatives

| Option | Variant Count | Pros | Cons | Outcome |
|---|---|---|---|---|
| **Naïve** | 30 (one per pattern) | Maximal expressivity | Match ergonomics break; TUI rendering brittle | Rejected |
| **Minimal** | 3-5 (Text / Code / Tool / Block) | Tiny, easy | Loses semantic structure; defeats Pretext audit | Rejected |
| **13 (this ADR)** | 13 | Tractable, complete, parameterized via `LineRole` | `BlockSlot` is a failure mode | **Accepted** |
| **Tagged string** | 1 variant + `kind: String` | One enum value | Loses compile-time exhaustiveness, no Rust pattern matching | Rejected |

## Validation

- [ ] All 13 variants have unit tests covering `to_text()` deterministic output.
- [ ] `LineRole` exhaustive match across all renderer functions (compile-time enforced).
- [ ] CI metric: `BlockSlot` usage rate across reference fixtures < 5%.
- [ ] Snapshot test: ClaudeCode-style fixture → `Vec<RenderLine>` → TUI render → byte-compare to expected ANSI output.
- [ ] Snapshot test: same fixture → GUI render → `Message::to_text()` → byte-equal to TUI output.
- [ ] `cargo bench` shows < 16ms render of 10K line buffer (Phase 2B target).

## References

- Pretext UI theory: `docs/architecture/pretext-ui-theory.md` §1 (thesis), §5 (info-vs-decoration test)
- ClaudeCode line patterns: empirical observation, 2026-05-13 session
- openclaw bootstrap markers: ADR-011 + `~/dev/third_party/openclaw/docs/concepts/agent-workspace.md`
- Pretext UI plan: `docs/planning/plans/2026-05-12-pretext-ui-evolution.md` §5 Phase 2 (revised by this ADR)
- Companion: ADR-011 (workspace architecture, defines `notes/` subdirectories that store note variants matching `LineRole` semantics)
- Related: ADR-010 (Lucide icons — `RenderLine` icon glyphs use Lucide unicode codepoints)
