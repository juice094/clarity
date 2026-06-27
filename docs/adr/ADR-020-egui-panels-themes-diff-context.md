---
title: ADR-020: S6-D Right IDE Panels, Theme System, Diff Architecture, and Context Picker
category: ADR
tags: [adr, egui, ui, themes, diff, context]
---

# ADR-020: S6-D Right IDE Panels, Theme System, Diff Architecture, and Context Picker

> Status: Accepted — implementation complete
> Date: 2026-06-28
> Deciders: juice094 + Claude
> Affects: `clarity-egui`, `clarity-wire`, `clarity-core`, `clarity-contract`
> Relates: ADR-009 (Icon Font), ADR-012 (RenderLine), ADR-016 (Three-Column Layout)

---

## 1. Context

Clarity egui frontend's right IDE rail contained 4 placeholder panels (Console, Files, Share, Templates)
showing only static labels. The diff system existed only as data types in `clarity-core::diff` with
no egui renderer. Code blocks rendered monochrome. Only 3 theme variants existed (dark/light/OLED).
No context injection mechanism existed for file/folder/terminal/web content.

## 2. Decisions

### 2.1 Right IDE Panels: Full Functionality in Place

**Decision**: Implement all 4 panels with production-quality features rather than deferring to
future sprints.

- **Console**: Ring-buffered virtualized log (5K cap), 5-level filter with count badges,
  error click→inject to chat, auto-scroll toggle, Clear button
- **Files**: Reuse existing `file_browser::render_file_tree()` with `MAX_DEPTH=6` and
  `SKIP_DIRS`; add right-click context menu (Preview/OpenInEditor/AddToChat/CopyPath);
  git status extension point via `GitStatusCache` (Option, renders silently when None)
- **Share**: Markdown/JSON/HTML export with clipboard copy and file save dialog;
  Gateway sharing stubs disabled with tooltips
- **Templates**: 5 built-in templates with one-click inject;
  `TemplateStore` with `remote_templates: Option<Vec<RemoteTemplate>>` for future marketplace

**Rationale**: Each panel has a clear role in the AI coding workflow. Extension points
use `Option` fields that render silently when `None`, enabling forward compatibility
without breaking existing UIs.

### 2.2 Diff Architecture: Widget + RenderBlock + Contract

**Decision**: Three-layer diff stack:

1. **Contract layer** (`clarity-contract::diff`): `DiffHunk`, `DiffLine` — pure data types,
   zero dependencies. Migrated from `clarity-tools::diff` (re-export preserved).

2. **Widget layer** (`widgets/diff_viewer.rs`): Unified diff with line numbers,
   color-coded backgrounds, hunk folding (collapsed by default for >6 unchanged lines),
   accept/reject buttons, delta-style 3px accent bars. `extract_diff_from_tool_result()`
   parses `_diff_preview` from tool output.

3. **RenderBlock** (`RenderBlock::Diff`): `parse_markdown()` auto-detects unified diff
   patterns (`--- a/path`, `--- path`, `--- /dev/null`). Renders inline via diff_viewer.

**Rationale**: Separating data (contract) from computation (tools) from rendering (egui)
enables TUI reuse and clean architecture boundaries.

### 2.3 Theme System: 6 Presets + Token-Driven

**Decision**: Expand from 3 themes to 6, all driven by the existing 100+ design token
system in `theme.rs`. New presets: Catppuccin Mocha, Tokyo Night, One Dark.

Each preset maps standardized community palettes to Clarity's token structure.
`info` color token added to avoid conflation between accent and informational status.
`text_dim` contrast improved (#666→#777) for WCAG AA compliance.

Theme picker in Settings → Interface renders 3 rows of 2 cards each.

**Rationale**: Developer communities have strong theme preferences. Supporting Catppuccin
(most popular Rust community theme), Tokyo Night (VS Code top-5), and One Dark (Atom
classic) covers the majority without bloat. All new themes reuse the existing token
framework — no new rendering code needed.

### 2.4 Context Picker: # Quick-Add System

**Decision**: Trigger on `#` typed in composer → popup with source type list → embedded
file browser for File/Folder, filter input for Web/Terminal → confirm → chip in composer.

Two-step flow: first click selects source type (keeps picker open), type filter value,
second click confirms. Selected items render as accent chips; click to remove.
On send, `[Context]\n{display}: {payload}\n` prefix injected into message.

`ContextItem`/`ContextSource` types with reserved variants for future `Documentation`,
`Codebase`, `GitDiff` sources.

**Rationale**: Matches Claude Code/Cursor context injection patterns. Two-step flow
avoids creating empty/meaningless context items. Reserved variants enable forward
compatibility.

### 2.5 Syntax Highlighting: syntect on Cold Path

**Decision**: Use `syntect` v5 with `regex-fancy` backend, integrated into the cold-path
`parse_markdown()` flow. 18 languages supported with short-name normalization
(`rs→Rust`, `py→Python`, `js→JavaScript`). `base16-ocean.dark` theme mapped to
egui `Color32`.

**Rationale**: Cold-path integration respects the Pretext architecture constraint
(parse once, render per frame). `syntect` is pure Rust, no system dependencies.
18 languages cover the primary AI coding use cases without excessive binary bloat.

### 2.6 Tool::format_output() Pipeline

**Decision**: Add `Tool::format_output(&self, result: &Value) -> String` to `Tool` trait
with default impl. Agent dispatch calls it server-side, result travels via
`WireMessage::ToolResult.display_result: Option<String>`. Frontend uses pre-formatted
display when available, falls back to `format_tool_output()` heuristics.

**Rationale**: Moves tool-specific formatting from frontend to tool implementation,
enabling tools to control their own display without frontend changes.

## 3. Consequences

### Positive
- All 4 right IDE panels are production-ready (was 0%)
- Diff visualization works in approval modal, inline conversations, and standalone widget
- 6 theme presets cover major developer preferences
- Context injection system matches industry standards (Claude Code, Cursor)
- Syntax highlighting visible immediately without per-frame overhead
- Tool formatting pipeline enables backend-driven UI improvements

### Negative
- `syntect` + `fancy-regex` add ~2MB to binary (acceptable trade-off)
- 6 themes increase `theme.rs` from ~1200 to ~1700 lines
- Context picker two-step flow has learning curve vs. single-click

### Neutral
- Extension points (GitStatusCache, TemplateStore, ContextSource variants) add unused
  fields — marked `#[allow(dead_code)]` until backends implement them
- Copy "Copied!" feedback uses `ctx.data()` for state — tightly scoped, no persistence needed
