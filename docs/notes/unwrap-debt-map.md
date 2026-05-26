---
title: Unwrap/Expect Debt Map
category: Note
date: 2026-05-16
tags: [note]
---

# Unwrap/Expect Debt Map

> Last updated: 2026-05-11  
> Total (production code): ~468  
> Total (including tests): ~1397

---

## Debt Categories

### Category A: Hardcoded Regex (SAFE — annotate only)

~120 occurrences across workspace.  
Pattern: `Regex::new(r"...").unwrap()` in constructors or methods.

**Rationale**: regex literal is validated at compile/dev-time; syntax error would fail tests immediately.

**Hotspot files**:
- `crates/clarity-memory/src/extractor.rs` — 12
- `crates/clarity-tools/src/web.rs` — 10
- `crates/clarity-tools/src/web_browser.rs` — 5
- `crates/clarity-core/src/agent/compaction_service.rs` — 4

**Action**: Add `// SAFE: hardcoded regex` comment. No code change needed.

---

### Category B: Test Code (IGNORE)

~929 occurrences in `#[cfg(test)]`, `mod tests`, `tests/` integration tests, and `#[test]` functions.

**Action**: None. Test unwraps are acceptable per project convention.

---

### Category C: Startup/Infallible Paths (LOW risk)

~50 occurrences. Patterns:
- `tokio::runtime::Runtime::new().unwrap()` — only in `main()` or test setup
- `tracing_subscriber::fmt::init()` — infallible in practice
- `HeaderValue::parse("http://localhost:3000").unwrap()` — hardcoded valid value
- `std::env::var("CARGO_PKG_VERSION").unwrap()` — always present at compile time

**Hotspot files**:
- `crates/clarity-gateway/src/server.rs` — 8 (startup path)
- `crates/clarity-egui/src/main.rs` — 5 (startup path)
- `crates/clarity-headless/src/main.rs` — 3 (startup path)

**Action**: Replace `.unwrap()` with `.expect("context")` where missing. Already mostly using `expect`.

---

### Category D: Runtime I/O — MEDIUM risk (audit target)

~40 occurrences. Patterns:
- `fs::read_to_string(path).unwrap()` — file may not exist
- `serde_json::from_slice(&body).unwrap()` — external payload may be malformed
- `Command::new(...).output().unwrap()` — subprocess may fail
- `mpsc::channel().unwrap()` / `Mutex::new().unwrap()` — allocation failure (rare)

**Hotspot files**:
- `crates/clarity-core/src/background/store.rs` — 15
- `crates/clarity-gateway/src/session_store.rs` — 12
- `crates/clarity-core/src/tools/file.rs` — 10
- `crates/clarity-memory/src/store.rs` — 8

**Action**: Migrate to `?` or `match` with proper `AgentError`/`ToolError` propagation.

---

### Category E: Lock/Async — LOW-MEDIUM risk

~25 occurrences. Patterns:
- `std::sync::Mutex::lock().unwrap()` — poison risk (parking_lot migration eliminated most)
- `tokio::sync::mpsc::Sender::send(...).unwrap()` — channel closed risk
- `Arc::try_unwrap().unwrap()` — reference count risk

**Hotspot files**:
- `crates/clarity-core/src/approval/mod.rs` — 8
- `crates/clarity-subagents/src/runner.rs` — 6
- `crates/clarity-wire/src/lib.rs` — 5

**Action**:
- `Mutex::lock().unwrap()` → `parking_lot::Mutex` (already migrated in Sprint 40)
- Channel sends → `.map_err(|_| AgentError::ChannelClosed)`

---

## Target State

| Metric | Current | Target | Sprint |
|--------|---------|--------|--------|
| Total production unwrap | ~468 | <200 | 42-43 |
| Category D (I/O) | ~40 | 0 | 42 |
| Category E (Lock/Async) | ~25 | <10 | 42 |
| Category A (Regex) annotated | 0 | 120 | 42 |
| `expect()` without context | ~30 | 0 | 42 |

## Next Actions (Priority Order)

1. **P0**: Fix Category D I/O unwraps in `background/store.rs` and `session_store.rs`.
2. **P1**: Annotate Category A regex unwraps with `// SAFE:`.
3. **P1**: Replace bare `.unwrap()` with `.expect("context")` in Category C startup paths.
4. **P2**: Audit `approval/mod.rs` channel sends for graceful degradation.
