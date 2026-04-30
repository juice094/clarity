# RFC: `ensure_llm` 解耦为三层架构

> **Status**: Draft  
> **Author**: clarity-dev  
> **Date**: 2026-04-30  
> **Scope**: `clarity-egui` + `clarity-core`  
> **Related**: Sprint 13 Phase C (C1/C3)

---

## 1. Problem Statement

`ensure_llm()` in `clarity-egui/src/app_state.rs` is a God Function that conflates **four** distinct concerns:

| Concern | Current Location | Lines |
|---------|-----------------|-------|
| **Detection** (is LLM already bound?) | `ensure_llm` body | 10 |
| **Decision** (cloud-first vs local fallback) | `ensure_llm` body | 15 |
| **Loading** (instantiate `Arc<dyn LlmProvider>`) | `try_load_cloud` / `try_load_local` | 80 |
| **Binding** (inject into `Agent`) | `ensure_llm` body | 5 |

This coupling makes the function impossible to unit test (it requires a full `AppState` with async runtime, mutexes, and an `Agent`), and any change to loading logic risks breaking the binding invariant.

### Concrete Pain Points

1. **No unit tests for fallback strategy** — The cloud→local fallback decision is only tested manually via GUI.
2. **Binding leak** — `state.agent.set_llm()` and `state.agent.set_provider_label()` are buried inside the same function that resolves API keys and downloads model metadata.
3. **State explosion** — `ensure_llm` mutates `llm_binding`, `agent.llm`, and `agent.provider_label` in one atomic block, making rollback on partial failure impossible.
4. **Reusability** — `clarity-tui`, `clarity-headless`, and `clarity-gateway` each have their own copy of similar logic (see `gateway/src/handlers.rs:853` and `background/agent_executor.rs:125`).

---

## 2. Design Goals

1. **Testability**: The decision layer must be a pure function with zero I/O.
2. **Transparency**: The loading layer must propagate all errors without fallback swallowing.
3. **Reversibility**: The binding layer must be idempotent and reversible (unbind → rebind).
4. **No regression**: Existing `ensure_llm` callers must continue to work during migration.

---

## 3. Proposed Architecture

Split into three layers with explicit contracts:

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 1: Policy (pure, sync, testable)                     │
│  resolve_provider(settings, network, current_binding)       │
│           ↓ ProviderSelection                               │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: Loader (async, fallible, no side effects)         │
│  load_llm(selection, settings) -> Arc<dyn LlmProvider>      │
│           ↓ Arc<dyn LlmProvider>                            │
├─────────────────────────────────────────────────────────────┤
│  Layer 3: Binder (sync, idempotent)                         │
│  bind_llm(agent, backend, label)                            │
└─────────────────────────────────────────────────────────────┘
```

### 3.1 Layer 1 — Policy

```rust
// clarity-egui/src/llm_policy.rs (new file)

/// The outcome of provider selection policy.
pub enum ProviderSelection {
    /// Use the user's preferred cloud provider.
    Preferred { provider: String },
    /// Preferred failed; fallback to local.
    Fallback {
        preferred: String,
        fallback: String,
        reason: String,
    },
    /// Use a local GGUF model.
    LocalOnly { path: String },
}

/// Pure function: given current state, decide which provider to load.
///
/// # Testability
/// - No async, no I/O, no mutexes.
/// - 100% branch coverage achievable with plain `#[test]`.
pub fn resolve_provider(
    desired_provider: &str,
    network_available: bool,
    current_binding: &Option<LlmBinding>,
) -> ProviderSelection {
    // Early exit: already bound to the desired provider
    if let Some(ref b) = current_binding {
        if b.provider == desired_provider {
            return ProviderSelection::Preferred {
                provider: desired_provider.to_string(),
            };
        }
    }

    if desired_provider == "local" {
        return ProviderSelection::LocalOnly {
            path: String::new(), // resolved later by loader
        };
    }

    if !network_available {
        return ProviderSelection::Fallback {
            preferred: desired_provider.to_string(),
            fallback: "local".to_string(),
            reason: "Network offline".to_string(),
        };
    }

    ProviderSelection::Preferred {
        provider: desired_provider.to_string(),
    }
}
```

### 3.2 Layer 2 — Loader

```rust
// clarity-egui/src/llm_loader.rs (new file)

/// Async loader: given a selection, produce a live LLM backend.
///
/// # Transparency
/// - All errors are propagated verbatim.
/// - No fallback logic (that's Layer 1's job).
pub async fn load_llm(
    selection: ProviderSelection,
    settings: &GuiSettings,
) -> Result<Arc<dyn LlmProvider>, EguiError> {
    match selection {
        ProviderSelection::Preferred { provider } |
        ProviderSelection::Fallback { preferred: provider, .. } => {
            try_load_cloud(&provider, settings).await
        }
        ProviderSelection::LocalOnly { .. } => {
            try_load_local(settings).await
        }
    }
}
```

`try_load_cloud` and `try_load_local` move from `app_state.rs` into this module with **no functional changes**.

### 3.3 Layer 3 — Binder

```rust
// clarity-egui/src/llm_binder.rs (new file)

/// Idempotent binder: attach a loaded backend to an Agent.
///
/// # Reversibility
/// - Can be followed by `unbind_llm(agent)` to detach.
/// - Safe to call multiple times (replaces previous binding).
pub fn bind_llm(
    agent: &clarity_core::Agent,
    backend: Arc<dyn LlmProvider>,
    label: &str,
) {
    agent.set_llm(backend);
    agent.set_provider_label(label);
}

/// Detach the current LLM from the agent.
pub fn unbind_llm(agent: &clarity_core::Agent) {
    // Agent currently has no "unset_llm" API; Phase C will add it
    // or we can use a sentinel (e.g. MockLlm) as a placeholder.
}
```

---

## 4. Migration Path

### Phase C1: Add new APIs (Day 1-2)

1. Create `llm_policy.rs`, `llm_loader.rs`, `llm_binder.rs`.
2. Move `try_load_cloud` / `try_load_local` to `llm_loader.rs`.
3. Add unit tests for `resolve_provider` (all branches).

### Phase C2: Migrate `ensure_llm` (Day 3)

Replace `ensure_llm` body with the three-layer call:

```rust
pub async fn ensure_llm(state: &AppState) -> Result<(), EguiError> {
    let settings = { state.cached_settings.lock().clone() };
    apply_profile_overlay(&mut settings);

    let network = state.network_available.load(Ordering::Relaxed);
    let binding = state.llm_binding.lock().clone();

    let selection = resolve_provider(&settings.provider, network, &binding);

    let _guard = state.llm_load_lock.lock().await;

    // Re-check after acquiring lock
    let binding = state.llm_binding.lock().clone();
    if matches!(selection, ProviderSelection::Preferred { ref provider } if binding_matches(&binding, provider, "")) {
        return Ok(());
    }

    let backend = load_llm(selection, &settings).await?;
    bind_llm(&state.agent, backend, &format!("{}:{}", settings.provider, settings.model));

    // Update binding record
    *state.llm_binding.lock() = Some(LlmBinding {
        provider: settings.provider,
        local_model_path: String::new(),
    });

    Ok(())
}
```

### Phase C3: Deprecate old `ensure_llm` (Day 4)

Mark old `ensure_llm` as `#[deprecated]` and migrate callers:
- `clarity-gateway/src/handlers.rs` (2 sites)
- `clarity-core/src/background/agent_executor.rs` (1 site)
- `clarity-core/src/subagents/runner.rs` (1 site)

### Phase C4: Remove old `ensure_llm` (Sprint 14)

After all callers migrated, delete the monolithic function.

---

## 5. Testing Strategy

| Layer | Test Type | Coverage Target |
|-------|-----------|-----------------|
| Policy | Unit (`#[test]`) | All branches: preferred / fallback / local / already-bound |
| Loader | Async unit + mock registry | Cloud success, cloud failure, local success, local missing file |
| Binder | Unit with MockLlm | Bind, re-bind, unbind |
| Integration | `cargo test --workspace --lib` | Full `ensure_llm` round-trip |

---

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `Agent` lacks `unset_llm` | Add `Agent::unset_llm()` in `clarity-core` as part of C3 |
| Gateway/Headless/TUI have divergent loading logic | Audit each site first; unify into `llm_loader.rs` gradually |
| `llm_load_lock` semantics change | Lock moves to `ensure_llm` wrapper, not into any layer; no change |

---

## 7. Acceptance Criteria

- [ ] `resolve_provider` has ≥ 90% branch coverage
- [ ] `cargo clippy --workspace --lib --bins --tests -- -D warnings` passes
- [ ] `cargo test --workspace --lib` passes (574 passed)
- [ ] GUI smoke test: switch provider → load → chat → switch back (no regression)

---

*End of RFC Draft*
