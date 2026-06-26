# Plan: Remove dead `capable()` capability stub from `AdaptiveModelRouter`

## 1. Recommended option

**Option B: Delete the dead code.**

Remove the `capable()` stub, the unused `requires_reasoning` / `requires_vision` / `requires_tools`
fields on `TaskDescriptor`, their builder methods, the phantom capability-filter step in the
module docs, and the empty `.filter()` call in `route()`. Add a `// ponytail:` marker documenting
the deferred path.

## 2. Why Option B (Ponytail justification)

- **No real consumer.** A workspace-wide search shows the only reads of
  `requires_reasoning`, `requires_vision`, and `requires_tools` are the `TaskDescriptor`
  field definitions and their own builder methods. No caller sets or checks them.
  `capable()` is only invoked from `route()` and always returns `true`.
- **The real router already exists.** Production capability/vision/tool/reasoning routing
  happens in `clarity-llm/src/runtime_router.rs`, which uses model tags and `RouterHint`.
  `AdaptiveModelRouter` in `clarity-core` is not wired to actual provider selection, so
  implementing capability filtering here would be abstraction theater.
- **Option A would be cross-crate bloat.** `clarity-contract/src/llm.rs::ProviderCapabilities`
  does not even have a `reasoning` flag today. Adding real filtering would require either
  (a) extending the contract type for a speculative use case, or (b) leaving
  `TaskDescriptor::reasoning()` as a lie. Both violate YAGNI and P2 (delete优先).
- **Honesty.** The module-level doc comment currently shows a "Filter: capability" pipeline
  step that does not exist. Removing it makes the code and docs consistent.

## 3. Files and line ranges to modify

Single file:

- `crates/clarity-core/src/adaptive/router.rs`
  - Lines 10–28: module-level routing pipeline diagram
  - Lines 75–82: `requires_*` fields in `TaskDescriptor`
  - Lines 109–125: `reasoning()`, `vision()`, `tools()` builder methods
  - Lines 411–415: `route()` algorithm doc comment
  - Lines 428–433: `route()` iterator chain
  - Lines 460–465: `capable()` method body

## 4. Concrete `Edit` blocks

### Edit 1 — Simplify the pipeline diagram and add the ponytail marker

```rust
//! ## Routing pipeline
//!
//! ```text
//! TaskDescriptor
//!      │
//!      ▼
//! ┌─────────────────┐
//! │ Filter: health  │  — exclude providers below quality / error-rate thresholds
//! └─────────────────┘
//!      │
//!      ▼
//! ┌─────────────────┐
//! │ Score: weighted │  — combine latency / quality / cost / user-preference scores
//! │ composite         │
//! └─────────────────┘
//!      │
//!      ▼
//! ┌─────────────────┐
//! │ Rank + select   │  — pick top provider, emit ModelRoute event for feedback
//! └─────────────────┘
//! ```
//!
//! // ponytail: capability routing deferred. The production router in
//! `clarity-llm/src/runtime_router.rs` already handles vision/tool/reasoning
//! hints via model tags. If `AdaptiveModelRouter` is later wired to real
//! provider selection, re-introduce a `capabilities: ProviderCapabilities`
//! field on `ProviderProfile` and a task-requirements filter here.
```

### Edit 2 — Remove dead capability fields from `TaskDescriptor`

```rust
/// Description of a task submitted for routing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskDescriptor {
    /// Human-readable task classification.
    pub task_type: TaskType,

    /// Estimated token count for the prompt.
    pub estimated_input_tokens: usize,

    /// Maximum acceptable latency in milliseconds.
    pub max_latency_ms: Option<u64>,

    /// Maximum acceptable cost in USD.
    pub max_cost_usd: Option<f64>,

    /// Minimum quality threshold (0.0 - 1.0).
    pub min_quality: f64,
}
```

### Edit 3 — Remove unused builder methods

Delete the entire block:

```rust
    /// Require reasoning capability.
    pub fn reasoning(mut self) -> Self {
        self.requires_reasoning = true;
        self
    }

    /// Require vision capability.
    pub fn vision(mut self) -> Self {
        self.requires_vision = true;
        self
    }

    /// Require tool use.
    pub fn tools(mut self) -> Self {
        self.requires_tools = true;
        self
    }
```

### Edit 4 — Update `route()` algorithm doc comment

```rust
    /// Route a task to the best available provider.
    ///
    /// # Algorithm
    ///
    /// 1. Filter out providers that don't meet health thresholds.
    /// 2. Score remaining providers using weighted composite fitness.
    /// 3. Return the highest-scoring provider.
```

### Edit 5 — Drop the `capable()` filter from `route()`

```rust
        let mut candidates: Vec<(&String, f64)> = profiles
            .iter()
            .filter(|(_, profile)| self.is_healthy(profile))
            .map(|(id, profile)| (id, profile.fitness_score(task, &weights)))
            .collect();
```

### Edit 6 — Remove `capable()` and replace with the ponytail marker

```rust
    // ponytail: capability routing deferred. The production router in
    // clarity-llm/src/runtime_router.rs already handles vision/tool/reasoning
    // hints via model tags. If this adaptive router is later wired to real
    // provider selection, add a `capabilities: ProviderCapabilities` field to
    // `ProviderProfile` and restore a capability filter here.
```

## 5. Tests to update / add

**No existing test needs to change.** The removed fields and methods were dead code and
were not exercised by the current test suite.

Existing tests that continue to cover the real behavior:

- `test_provider_profile_ewma_latency`
- `test_provider_profile_error_rate`
- `test_router_basic`
- `test_router_excludes_unhealthy`
- `test_router_no_providers`

**Optional addition** (can be skipped; include only if the reviewer wants explicit
documentation of the deleted surface):

```rust
#[test]
fn test_task_descriptor_has_no_capability_fields() {
    // Regression guard: ensures the deleted requires_* fields do not return
    // via Default or construction helpers.
    let task = TaskDescriptor::default();
    assert_eq!(task.task_type, TaskType::General);
    assert_eq!(task.estimated_input_tokens, 0);
    assert!(task.max_latency_ms.is_none());
    assert!(task.max_cost_usd.is_none());
    assert_eq!(task.min_quality, 0.0);
}
```

## 6. Risk level

**Low.**

- The change is confined to `crates/clarity-core/src/adaptive/router.rs`.
- `AdaptiveModelRouter`, `TaskDescriptor`, and the adaptive `ProviderProfile` are not
  referenced anywhere else in the workspace (the `ProviderProfile` match in
  `clarity-mobile-core` is a separate mobile-specific type).
- `TaskDescriptor` does not derive `Serialize`/`Deserialize`, so there is no saved-state
  compatibility concern.
- The production provider-selection path lives in `clarity-llm/src/runtime_router.rs` and
  is untouched.

## 7. Verification commands

Run after applying the edits:

```bash
# Fast compile check for the affected crate
cargo check -p clarity-core --lib

# Run the router unit tests
cargo test -p clarity-core --lib adaptive::router

# Ensure no new warnings / lint failures
cargo clippy -p clarity-core --lib -- -D warnings

# Full lib baseline (matches project test strategy)
cargo test --workspace --lib --exclude clarity-slint

# Formatting
cargo fmt --all -- --check
```

## 8. What was explicitly not done

- No changes to `clarity-contract/src/llm.rs::ProviderCapabilities`.
- No new capability fields added to `ProviderProfile`.
- No changes to `clarity-llm/src/runtime_router.rs`.
