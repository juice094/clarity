---
type: log
title: Session Log
description: Decision log for Clarity agent-assisted sessions.
timestamp: 2026-06-26T11:28:50Z
---

# Session Log

## 2026-06-26

### Project direction clarified

- The project is not a thesis. Its long-term purpose is to build a
  **local-first, distributed AI identity runtime**.
- Core motivation: solve cross-device / cross-instance continuity problems
  (session isolation, role-play continuity, cross-day memory inheritance).
- Guiding metaphor: a *Misaka Network*-style federation where multiple
  avatars share one personality + memory + social-relation carrier.

### Compute source inventory

- Documented the full LLM/provider layer in
  `docs/notes/research/provider-compute-source.md`.
- Compute sources: local Candle GGUF, external local processes (Ollama,
  llama-server), and cloud APIs (OpenAI-compatible, Anthropic, Kimi,
  DeepSeek, Ollama remote).
- DeepSeek mobile-device access is a private workaround; it must not be
  committed or documented as an official project capability.

### OKF adoption

- Adopted Google Open Knowledge Format (OKF) v0.1 as the knowledge-storage
  format for the project.
- Enhanced the existing `docs/okf/clarity-worktree/` bundle with OKF-recommended
  frontmatter fields (`title`, `description`, `tags`, `timestamp`).
- Added `docs/okf/clarity-worktree/log.md` for bundle-level changes.

### Ponytail optimization round 1

Identified three low-to-medium-risk simplifications for `clarity-llm` /
`clarity-core`:

1. **Delete `AdaptiveModelRouter::capable()` stub** — dead code; real
   capability routing lives in `clarity-llm/src/runtime_router.rs`.
2. **Unify `${env:VAR}` / `${file:path:field}` resolver** — duplicated
   between `clarity-llm/src/model_registry.rs` and
   `clarity-egui/src/provider.rs`; move to `clarity-contract`.
3. **Derive `model_listing` fallback from `registry_table`** — remove the
   duplicated hard-coded model catalog.

Detailed plans live in `docs/notes/plans/`.

### Model catalog architecture decision

- Confirmed that hard-coded model lists are only acceptable as an offline
  bootstrap, not as the primary discovery mechanism.
- Researched mature implementations: Ollama `/api/tags`, OpenAI `/v1/models`,
  LM Studio `/api/v1/models`, OpenClaw's fetch-then-fallback pattern.
- Decided on a three-layer catalog architecture:
  1. User override (`models.toml`)
  2. Cached remote catalog (`~/.clarity/catalogs/`)
  3. Minimal hard-coded bootstrap seed (`registry_table::known_models`)
- Documented the redesign in
  `docs/notes/plans/model-catalog-redesign.md`.

### Notes organization

- Created `docs/notes/research/` for research notes.
- Created `docs/notes/plans/` for implementation plans.
- This file (`docs/notes/session-log.md`) serves as the running decision log.
