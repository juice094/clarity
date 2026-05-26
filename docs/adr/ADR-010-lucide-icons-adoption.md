---
title: ADR-010: Adopt `lucide-icons` Crate — Supersede ADR-009 Phosphor Decision
category: ADR
tags: [adr]
---

# ADR-010: Adopt `lucide-icons` Crate — Supersede ADR-009 Phosphor Decision

- Status: Accepted (supersedes the Decision section of ADR-009)
- Deciders: juice094
- Date: 2026-05-12

## Context

ADR-009 (committed 2026-05-12 as `9165be87`) accepted a switch from manually-embedded `Phosphor.ttf` to the `egui-phosphor` crate. The decision was based on an inventory which observed that **no `egui-lucide` / `lucide-egui` crate exists**, and on a cost estimate of **5–8 hours** for a self-built Lucide pipeline (Node.js `fantasticon`, manual codepoint tables, vendored TTF).

Subsequent diligence on the actual `lucide-icons` crate (downloaded, unpacked, and inspected the 1.14.0 release on 2026-05-12) revealed that this cost estimate was significantly inflated. Specifically:

- `lucide-icons` ships `LUCIDE_FONT_BYTES: &[u8]` as a **framework-agnostic** constant (`src/lib.rs:75`). It is consumable by any egui `FontData::from_static()` call without an egui-specific wrapper crate.
- The crate exposes an `Icon` enum with 1,706 variants, each with a `unicode() -> char` method and `impl From<Icon> for char` returning the codepoint. This is a **superior API** to manually-curated `ICON_* &str` constants (compile-time exhaustiveness, IDE autocomplete on icon names).
- Iced support is gated behind an `iced` feature flag (`default = []`). Enabling no features yields a pure-Rust icon-font crate with zero iced dependency, matching the egui use case exactly.
- The crate is auto-synced **daily** from upstream Lucide via GitHub Actions (the publisher `WhySoBad` maintains a release workflow). 1.14.0 was published 2026-04-30, 117 versions to date, 104k cumulative downloads.
- License is **MIT AND ISC**, compatible with the workspace AGPL-3.0-or-later.
- Crate size is **500 KB** (download), expanding to a **804 KB `lucide.ttf`** vs. our current manually-vendored **480 KB `Phosphor.ttf`**. Net binary increase ≈ 324 KB.

The user (`juice094`) expressed a strong preference for the Lucide visual aesthetic (single 1.5 px stroke, 24×24 viewBox, no weight variants) as more aligned with the Rust minimalist design philosophy than Phosphor's multi-weight catalog. With the corrected cost estimate of **75–90 minutes** (versus 5–8 hours), this preference becomes engineering-affordable.

## Decision

1. **Supersede** ADR-009 Section Decision item 2. The migration target is **`lucide-icons` 1.14 crate**, not `egui-phosphor`.
2. **Reject** `egui-phosphor` crate adoption for Clarity. Phosphor remains a valid choice for projects that prefer multi-weight icon catalogs, but the Lucide aesthetic better matches the project's design language.
3. **Adopt** `lucide-icons = "1.14"` with **no features enabled** (excludes `iced` and `serde` features). Only `LUCIDE_FONT_BYTES` and the `Icon` enum's `From<Icon> for char` are consumed.
4. **Preserve** the 27 existing `ICON_*: &'static str` constants in `crates/clarity-egui/src/theme.rs` as the stable internal API. The constant values are updated to Lucide codepoints; the 123 call sites are not modified. Internal code is free to migrate to `lucide_icons::Icon::*` directly in future commits.
5. **Defer** the removal of the manually-vendored `Phosphor.ttf` file to a follow-up cleanup commit, after this commit's `cargo test` baseline is verified. (Rationale: keeps this commit atomic and reversible.)

## Consequences

### Positive

- **Visual aesthetic alignment**: The 27 icon glyphs shift from Phosphor's multi-weight system to Lucide's uniform 1.5 px stroke, producing a visually crisper and more "Rust-idiomatic" UI surface. This is the user's stated preference and the primary motivation.
- **Catalog expansion**: 1,706 icons available (vs. Phosphor's 1,248), giving headroom for future icon additions (e.g., S2-S9 may surface new semantic needs).
- **Type-safe future migration path**: New code may use `Icon::Settings.unicode()` directly, gaining compile-time guarantees that a misspelled icon name cannot compile. Legacy `ICON_*` constants remain available during the transition.
- **Upstream maintenance**: Daily auto-sync from `lucide-icons/lucide` means new icons appear in `Icon` enum within 24 hours of release.
- **Architecture preserved**: The "icons-as-glyphs" Pretext UI philosophy (planned RULE 11 in `EGUI_LAYOUT.md`) remains intact — only the underlying font bytes change.

### Negative

- **Binary size**: +324 KB (Lucide TTF 804 KB minus Phosphor TTF 480 KB). Mitigation: insignificant on desktop targets; if pressed, the crate could be replaced with a subsetted custom TTF.
- **27 codepoint values change**: Although the `ICON_*` names are preserved, every constant hex codepoint differs from Phosphor's. Any downstream consumers that hardcode Phosphor codepoints (none observed in current workspace) would break.
- **Visual semantic shifts**: The 27 icons render with subtly different glyphs (e.g., `ICON_STOP` shifts from Phosphor's filled square to Lucide's `CircleStop` (circle with square inside)). All 27 glyphs are inspected case-by-case in the implementation commit and judged visually equivalent or improved.
- **Sub-pixel rendering risk at small sizes**: Lucide's 1.5 px stroke at 12-14 px font sizes (`text_xs` / `text_sm`) may render with approximately 0.75-0.94 px anti-aliased lines. Mitigation: the implementation commit includes visual spot-checks at small sizes; if quality is unacceptable, fallback options include using `egui::pixels_per_point` upscaling or restricting small-icon use sites.

### Neutral

- The font registration mechanism in `setup_fonts()` is structurally identical: `egui::FontData::from_static(...)` then `FontFamily::Name("icons")` stack. Only the byte source changes.
- TUI (`clarity-tui`) is not affected; ratatui never uses Lucide.
- The `IconFallbackTable` planned for S7 must map `IconId` enum variants (when defined in S4) to Unicode-standard fallback chars (e.g., `IconId::Settings` → U+2699 gear). Lucide codepoints are private-use-area and not displayable in terminals, but this was equally true for Phosphor — the fallback table strategy is unchanged.

## Alternatives Considered (Updated)

| Alternative | Evaluation | Outcome |
|---|---|---|
| **A.** Continue with manually-embedded `Phosphor.ttf` | Status quo; no Lucide aesthetic benefit. | Rejected (insufficient progress) |
| **B.** Adopt `egui-phosphor` crate (ADR-009 original decision) | 30 min cost, Phosphor catalog, upstream maintenance. Rejected because user prefers Lucide. | **Superseded** by this ADR |
| **C.** Adopt `lucide-icons` crate | 75-90 min cost, Lucide catalog, upstream maintenance, framework-agnostic font bytes. | **Accepted** |
| **D.** Adopt `iconflow` (multi-pack Lucide + Phosphor) | 1.5 h cost, +8.5 MB binary, single-author/5-month-old. | Deferred (re-evaluate after iconflow v1.1+) |
| **E.** Self-built Lucide pipeline (`fantasticon` + Node.js) | 5-8 h initial + permanent maintenance debt; violates "cargo-only toolchain" (ADR-001). | Rejected |
| **F.** Contribute open-source `egui-lucide` crate | 5-8 h initial + permanent community responsibility; no immediate Clarity benefit over option C. | Rejected (Clarity team focus) |

## Re-Evaluation Triggers

This decision should be revisited if **any** of the following becomes true:

1. `lucide-icons` becomes unmaintained for greater than 18 months (last commit > 2027-09).
2. The Phase 3 (S7) TUI fallback work reveals that maintaining a 27-entry `IconFallbackTable` by hand is more expensive than switching everything (GUI+TUI) to Nerd Font.
3. Lucide upstream changes license away from MIT/ISC.
4. Sub-pixel rendering quality at 12-14 px font sizes is unacceptable in production usage and `egui::pixels_per_point` upscaling is insufficient.
5. A first-party `egui-lucide` crate appears on `crates.io` with greater than 10k downloads and a more ergonomic egui-native API.

## Validation

Pre-commit verification required:

- [ ] `cargo check -p clarity-egui` produces 0 warnings.
- [ ] `cargo test -p clarity-egui` passes 66/66 tests.
- [ ] All 27 `ICON_*` constants are updated to the verified Lucide codepoints (full mapping table archived in the commit message).
- [ ] `setup_fonts()` registers `lucide_icons::LUCIDE_FONT_BYTES` under font_data key `"lucide"`, added to `FontFamily::Name("icons")` stack.
- [ ] Manual visual inspection of 5-7 key call sites (TitleBar buttons, Send button, Settings panel) confirms glyphs render correctly.

## References

- Source crate (verified 2026-05-12): <https://crates.io/crates/lucide-icons/1.14.0>
- Source repository: <https://github.com/WhySoBad/lucide-icons-rs>
- Inspected source: `lucide-icons-1.14.0/src/lib.rs` (LUCIDE_FONT_BYTES line 75), `src/icon.rs` (Icon enum, 10324 lines, 1706 variants)
- User-provided diligence: `C:\Users\22414\Desktop\lucide 图标库.md` (third-party verification of API claims)
- Superseded ADR: `docs/adr/ADR-009-icon-font-strategy.md` (Decision section item 2)
- Related: ADR-001 (egui as sole desktop stack; cargo-only toolchain constraint), ADR-009 (icon font strategy first pass)
- Implementation plan: `docs/plans/2026-05-12-pretext-ui-evolution.md` (Phase 1 / S2)
