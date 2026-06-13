---
title: ADR-002: Replace `std::sync::Mutex`/`RwLock` with `parking_lot` Across 6 Crates
category: ADR
tags: [adr]
---

# ADR-002: Replace `std::sync::Mutex`/`RwLock` with `parking_lot` Across 6 Crates

- Status: Accepted
- Deciders: juice094
- Date: 2026-05-02

## Context

The Clarity workspace contained approximately **154 instances** of `lock().unwrap()` / `read().unwrap()` / `write().unwrap()` on `std::sync::Mutex` and `std::sync::RwLock` across production code. These unwraps were a significant category of the overall unwrap debt (~1,069 non-test unwraps at v0.3.0 baseline per `docs/ARCHITECTURE.md`).

While many of these lock unwraps were theoretically safe (poisoning was unlikely in short critical sections), they:
1. Inflated the unwrap count tracked as a code-health metric.
2. Added noise to security audits and clippy risk scans.
3. Represented a paper-cut risk: if any thread panicked while holding a lock, subsequent lock attempts would panic via `unwrap()`.

`parking_lot` provides `Mutex` and `RwLock` implementations that are:
- Smaller and faster than `std::sync` equivalents.
- **Poison-free**: `lock()` returns the guard directly, no `Result` wrapping.
- Compatible with `std::sync` APIs (drop-in replacement for most usages).

## Decision

Migrate all eligible `std::sync::Mutex` and `std::sync::RwLock` usages to `parking_lot::Mutex` and `parking_lot::RwLock` in the following 6 crates:

- `clarity-core`
- `clarity-memory`
- `clarity-gateway`
- `clarity-claw`
- `clarity-wire`
- `tests/integration`

**Exceptions** (intentionally retained as `std::sync::Mutex`):
- `clarity-core/src/approval/mod.rs` — relies on `LockResult` poison semantics for `PersistingApprovalRuntime` safety.
- `clarity-core/src/tools/web_browser.rs` — relies on `LockResult` poison semantics.

Add `parking_lot = "0.12"` to the `Cargo.toml` of each affected crate.

## Consequences

### Positive
- **~154 lock unwraps eliminated**, reducing production unwrap density by ~14%.
- Lock operations are now infallible at the call site; no `.unwrap()` or `?` needed.
- `parking_lot` locks are more compact (1 byte vs. 24+ bytes) and faster (no kernel syscalls on uncontended paths).
- Cleaner diffs in future refactors: lock sites read as `let guard = mutex.lock();` instead of `let guard = mutex.lock().unwrap();`.

### Negative
- New external dependency (`parking_lot 0.12`) added to 5 crates. This is a well-maintained, widely-audited crate (used by `tokio`, `rayon`, `dashmap`), so supply-chain risk is low.
- `parking_lot::RwLock` does not support `into_inner()` or `get_mut()` in all the same ways as `std::sync::RwLock`; a handful of sites required minor restructuring (e.g., using `Arc::get_mut` outside the lock).

### Neutral
- `tokio::sync::RwLock` / `tokio::sync::Mutex` in async contexts (e.g., `background/` module) were already migrated in a prior refactor and were not touched in this pass.
- The two poison-retention sites (`approval/mod.rs`, `tools/web_browser.rs`) remain exactly as before; no behavior change.

## Alternatives Considered

| Alternative | Evaluation | Outcome |
|---|---|---|
| **Keep `std::sync` and annotate each unwrap with `// SAFE:`** | Would preserve zero new dependencies but leave 154 unwraps on the books, failing the Sprint 40 goal of runtime robustness deepening. | Rejected |
| **Migrate to `tokio::sync` everywhere** | `tokio::sync` locks are async-only and heavier; many lock sites are in synchronous getter/setter methods called from TUI event loops and Gateway HTTP handlers. | Rejected |
| **Custom infallible lock wrapper around `std::sync`** | Would avoid a dependency but reinvents `parking_lot` poorly. | Rejected |
| **`parking_lot` drop-in replacement** | Zero API friction for most sites, eliminates unwraps entirely, and improves performance. | Accepted |

## References

- Commit: `5e827983`
- Related docs: `docs/ARCHITECTURE.md` (Code Health Metrics: unwrap count baseline)
- Related docs: `docs/planning/sprint-archive.md` (Sprint 40 delivery notes)
