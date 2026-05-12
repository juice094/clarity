# ADR-009: Icon Font Strategy — Standardize on `egui-phosphor` Crate

- Status: Accepted
- Deciders: juice094
- Date: 2026-05-12

## Context

The Clarity desktop UI (`clarity-egui`) renders icons via the Phosphor icon font embedded as `crates/clarity-egui/assets/fonts/Phosphor.ttf` (≈1 MB). 27 `ICON_*: &str` constants in `crates/clarity-egui/src/theme.rs` (lines 6–32) map mnemonic names to Private Use Area codepoints (`\u{E182}` ~ `\u{E946}`), with 123 call sites across the egui crate.

During the Pretext UI evolution review (2026-05-12), a Kimi conversation excerpt (`C:\Users\22414\Desktop\图标与字体参考.md`, 471 lines) raised two recommendations:

1. Migrate the icon system from Phosphor to **Lucide** for tighter alignment with the Rust minimalist philosophy.
2. Verify Lucide's availability in the Rust / egui ecosystem.

A `crates.io` survey (2026-05-12) produced the following inventory:

| Crate | Downloads | Target | Status |
|---|---|---|---|
| `lucide-icons` | 104k | iced | Active (v1.14, 2026-04) |
| `lucide-dioxus` | 40k | Dioxus | Active (v3.11, 2026-04) |
| `lucide-leptos` | 27k | Leptos | Active (v3.11, 2026-04) |
| `lucide-slint` | 5.7k | Slint | Active |
| `yew-lucide` | 109k | Yew | Stale (Apr 2025) |
| **`egui-lucide` / `lucide-egui`** | **—** | **egui** | ❌ **does not exist** |
| `egui-phosphor` | **170k** ⭐ | **egui** | Active (v0.12, 2026-03) |
| `iconflow` | 448 | egui+iced (Lucide+Phosphor+12 others) | New (v1.0, 2025-12) |
| `egui_nerdfonts` | 6.5k | egui (Nerd Font subset) | 2024-06 |

The Pretext UI architecture (see `docs/architecture/pretext-ui-theory.md`) treats icons as glyphs co-equal with text characters — a philosophy already implemented via `Theme::font_icon()` returning `FontFamily::Name("icons")`. Phosphor 1248-glyph completeness covers >90% of the 154 unique IconPark functions observed in production Kimi Web UI.

The Phase 3 TUI Parity goal (S7) requires every GUI icon to have a Unicode/ANSI fallback for ratatui rendering. Phosphor PUA codepoints (U+E000–U+F8FF) potentially conflict with Nerd Font codepoints in the same range, requiring an explicit `IconFallbackTable` mapping.

## Decision

1. **Reject** wholesale Phosphor → Lucide migration. egui ecosystem lacks an official Lucide port; building one would incur 5–8h of self-built pipeline maintenance with marginal visual benefit over Phosphor.
2. **Accept** migrating from manually-embedded `Phosphor.ttf` to the `egui-phosphor` crate (`^0.12` matching `egui = "0.31"`). This trades 30 min of integration work for:
   - Automatic coverage of the full Phosphor 1248-glyph set (currently only 27 are mapped).
   - Removal of the manually-checked-in 1 MB TTF blob from the repository.
   - Maintenance handoff to the upstream crate (170k downloads, active maintainer `amPerl`).
3. **Defer** evaluation of `iconflow` (the multi-pack unified library that includes both Lucide and Phosphor on egui). Re-evaluate when:
   - `iconflow >= v1.1` is released, **and**
   - Cumulative downloads exceed 5,000, **and**
   - `crate_size` is reduced below 4 MB (currently 8.5 MB causes binary bloat concerns).
4. **Plan for** `egui_nerdfonts` adoption during Phase 3 (S7) as a candidate strategy for the `IconFallbackTable`, conditional on whether GUI/TUI dual-end icon set unification produces engineering value.
5. The 27 existing `ICON_*` constants must be preserved as the stable internal API; the migration only changes the underlying font registration mechanism. If `egui-phosphor` exposes different codepoints than the manually-mapped values, the constants will be updated in a single atomic commit with explicit value-by-value review.

## Consequences

### Positive

- **Source of truth handoff**: The Phosphor font file is no longer a vendored binary artifact; the dependency manager owns its versioning.
- **Future-proofing**: Adding a new icon (e.g., `ICON_HEART`) becomes a one-line constant addition referencing an `egui_phosphor::regular::*` value instead of hex codepoint hunting.
- **Engineering rigor signal**: The decision matrix and ADR record establish a precedent — "prefer ecosystem crate over vendored binary when both are available and the crate has ≥10× usage of the vendoring effort."
- **Pretext UI alignment**: Confirms the "icons = glyphs" philosophy as an explicit, documented decision (slated for `EGUI_LAYOUT.md` RULE 11 in S2.P1.7).

### Negative

- Binary size may increase modestly (`egui-phosphor` ships all Phosphor variants by default, while the current setup only embeds Regular). Mitigated by selecting `Variant::Regular` only via `add_to_fonts(..., Variant::Regular)`.
- Adds one external dependency to `clarity-egui`'s graph, raising the workspace dependency count by 1.
- `egui-phosphor 0.12` is pinned to `egui 0.31`. Future egui upgrades will require coordinated bumps.

### Neutral

- TUI (`clarity-tui`) is not affected by this decision; it never depended on Phosphor.
- The `Theme::font_icon()` helper, `setup_fonts()` registration logic, and 123 call sites all remain semantically identical.
- The `assets/fonts/Phosphor.ttf` file may be deleted from the repository in a follow-up cleanup commit once the crate-based registration is verified in production.

## Alternatives Considered

| Alternative | Evaluation | Outcome |
|---|---|---|
| **A. Keep manual Phosphor.ttf** | Zero migration cost, but only 27/1248 icons mapped, and vendored binary remains in git history forever. | Rejected |
| **B. Switch to `egui-phosphor` crate** | 30 min cost, unlocks full Phosphor catalog, removes vendored TTF, leverages 170k-download upstream maintenance. | **Accepted** |
| **C. Adopt `iconflow` (Lucide + Phosphor dual)** | 1.5 h cost, +8.5 MB binary bloat, single-maintainer 5-month-old crate, exotic feature flag combinatorics. | Deferred to re-evaluation gate |
| **D. Build a custom `lucide-egui` pipeline** (`lucide-static` SVG → `fantasticon` → TTF) | 3+ h initial cost plus permanent maintenance debt; reproduces an existing solution (`egui-phosphor`) using a different vendor. | Rejected |
| **E. Contribute an open-source `egui-lucide` crate** | 5–8 h initial cost; community maintenance responsibility ; no immediate benefit to Clarity over option B. | Rejected (future possibility, not blocking) |

## Re-Evaluation Triggers

This decision should be revisited if **any** of the following becomes true:

1. `iconflow >= 1.1` is published with `crate_size < 4 MB` and `downloads > 5,000`.
2. `egui-phosphor` becomes unmaintained for > 18 months (last commit > 2027-09).
3. A first-party `lucide-egui` crate appears on `crates.io` with ≥ 10k downloads.
4. The Phase 3 (S7) TUI fallback work reveals that maintaining a 27-entry `IconFallbackTable` by hand is more expensive than switching everything (GUI+TUI) to Nerd Font.
5. Phosphor licensing changes (currently MIT, which is compatible with the workspace).

## References

- Survey data: `crates.io` API responses captured 2026-05-12 (Lucide, Phosphor, iconflow, egui_nerdfonts)
- Source conversation: `C:\Users\22414\Desktop\图标与字体参考.md` (Kimi-user dialogue, 471 lines)
- Architecture context: `docs/architecture/pretext-ui-theory.md` (icons-as-glyphs philosophy)
- Implementation plan: `docs/plans/2026-05-12-pretext-ui-evolution.md` (Phase 2 RenderLine, Phase 3 TUI Parity)
- Project state: `docs/plans/2026-05-12-S1-session-archive.md` (HEAD = 561696ba, 27 ICON_* constants, 123 call sites)
- Upstream crate: <https://crates.io/crates/egui-phosphor>
- Re-evaluation candidate: <https://crates.io/crates/iconflow>
- TUI fallback candidate: <https://crates.io/crates/egui_nerdfonts>
- Related ADRs: ADR-001 (egui as sole desktop stack), ADR-008 (Brain/Hands session decoupling)
