---
title: ADR-001: Deprecate `clarity-tauri` and Make `clarity-egui` the Sole Primary Desktop Stack
category: ADR
tags: [adr, egui, tauri, ui]
---

# ADR-001: Deprecate `clarity-tauri` and Make `clarity-egui` the Sole Primary Desktop Stack

- Status: Accepted
- Deciders: juice094
- Date: 2026-04-28

## Context

Clarity initially adopted **Tauri 2** (`clarity-tauri`) as the desktop GUI stack (React 18 + Vite frontend, Rust backend). Sprint 1–2 delivered a functional Tauri-based GUI with Chat Panel, Session Sidebar, Task Panel, Settings Panel, and Theme System.

However, the following issues emerged during Sprint 12–14:

1. **Cross-platform build fragility**: Tauri's WebView2 dependency on Windows and WebKit2GTK on Linux introduced CI failures and platform-specific packaging overhead (`.msi` signing, macOS notarization, Android SDK complexity).
2. **Frontend stack divergence**: Maintaining a TypeScript/React frontend inside a Rust workspace violated the "Rust core module不可外包" constraint and doubled the toolchain surface (Node.js + npm + Vite vs. pure `cargo`).
3. **Mobile scope overrun**: Tauri 2's iOS/Android promise tempted expansion into mobile, which triggered the **Hard Veto** on project breadth (> 5 core tools) and Mobile adaptation.
4. **egui maturity**: `clarity-egui` (eframe/egui, pure Rust) reached feature parity in Sprint 14 with 71 `.rs` files, covering Chat, Settings, Plan visualization, Subagent progress, Cron/Team panels, Glassmorphism theming, and single-binary packaging — all without a JS build step.
5. **Zero-dependency alignment**: The Phase 2 roadmap goal of "单二进制 + 嵌入式模型" is incompatible with Tauri's WebView runtime requirement.

`docs/ROADMAP.md` Phase 1 was explicitly annotated as archived: "以下基于 `clarity-tauri`（React+Vite）的实现已废弃归档。v0.4.0 起全部功能由 `clarity-egui`（eframe/egui，纯 Rust）承接。"

## Decision

1. **Freeze `clarity-tauri`**: Stop all new feature development. The crate and its frontend code are archived (moved out of the active workspace in commit `899d8f92`).
2. **Promote `clarity-egui` to sole primary desktop stack**: All desktop GUI capabilities (Chat, Settings, Plan tracking, Subagent visualization, Sidebar tools, Onboarding, etc.) are owned by `clarity-egui`.
3. **Update documentation and CI**: Remove Tauri-specific build steps from CI workflows, README, and `AGENTS.md` quick-reference. Gateway HTTP API and TUI remain as alternative entry points.

## Consequences

### Positive
- Single toolchain (`cargo` only) for the entire desktop GUI surface.
- Eliminates Node.js/npm/Vite dependency, aligning with the "零依赖发行" goal.
- Single-binary `.exe` / `.msi` packaging is fully under Rust control (no WebView2 runtime prerequisite).
- Prevents project breadth creep into Mobile (Hard Veto compliance).
- `clarity-egui` compiles and clippy-checks with the rest of the workspace; no cross-language linting gap.

### Negative
- Tauri-specific capabilities (auto-updater plugin, deep links, push notifications, biometric approval) are lost unless reimplemented natively in Rust.
- Rich text editing and complex layouts that are trivial in React require more manual egui code.
- Existing `clarity-tauri` code is frozen but not deleted; it incurs no compile cost because it was moved out of the workspace.

### Neutral
- Gateway Web UI (Axum + static files) and TUI (ratatui) remain unchanged as independent presentation layers.
- `clarity-egui` retains the same `clarity-core` + `clarity-wire` dependency graph that `clarity-tauri` used.

## Alternatives Considered

| Alternative | Evaluation | Outcome |
|---|---|---|
| **Keep Tauri 2, drop egui** | Would retain WebView rendering and React ecosystem, but violates zero-dependency and Rust-purity goals. Mobile temptation remains. | Rejected |
| **Maintain both stacks in parallel** | Would double every UI feature (React + egui), violating project-breadth constraints and stretching limited maintenance bandwidth. | Rejected |
| **Dioxus** | Evaluated in `docs/tech_stack_decision_ui.md`. Pure Rust RSX, but ecosystem immaturity and no production mobile cases made it inferior to egui at this stage. | Rejected |
| **egui as sole stack** | Matches all constraints: pure Rust, single binary, no extra runtime, compiles on all three CI platforms without special casing. | Accepted |

## References

- Commit: `899d8f92` (chore: archive `clarity-tauri` out of active workspace)
- Commit: `78dbbb72` (fix: remove archived tauri reference from benchmarks)
- Related docs: `docs/ROADMAP.md` (Phase 1 archive annotation, Sprint 14 egui maturity)
- Related docs: `docs/FUTURE_DIRECTION.md` ("UI 技术栈方向：egui 为唯一主力栈；Tauri 废弃归档")
- Related docs: `docs/ARCHITECTURE.md` (Crate Topology, Desktop GUI section)
- Related docs: `docs/tech_stack_decision_ui.md` (original Tauri 2 selection rationale and reversal context)
