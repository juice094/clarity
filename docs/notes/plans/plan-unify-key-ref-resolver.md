# Plan: Unify `${env:VAR}` / `${file:path:field}` Key Reference Resolver

## 1. Goal

Replace the two near-identical copies of key-reference resolution with a single pure function that both `clarity-llm` and `clarity-egui` can call.

Current copies:

- `crates/clarity-llm/src/model_registry.rs:527-562`
- `crates/clarity-egui/src/provider.rs:256-288`

## 2. Proposed Location

**`crates/clarity-contract/src/key_ref.rs`**

Re-exported from `crates/clarity-contract/src/lib.rs` as `pub use key_ref::resolve_key_ref;`.

### Why `clarity-contract` and not `clarity-secrets`

| Criterion | `clarity-contract` | `clarity-secrets` |
|-----------|--------------------|-------------------|
| Both consumers already depend on it | ✅ `clarity-llm` and `clarity-egui` already depend on `clarity-contract` | ❌ `clarity-egui` does not currently depend on `clarity-secrets`; would add a new edge |
| Already has required dependencies (`dirs`, `serde_json`) | ✅ `dirs = "5.0"`, `serde_json = { workspace = true }` are already in `Cargo.toml` | ❌ Would need to add `dirs` and `serde_json` to a crate whose scope is encrypted storage |
| Semantic fit | Resolver is a cross-cutting contract/utility, not secret-specific logic | Possible, but would pull file/env resolution into the encryption crate |
| Dependency growth | Zero new dependencies | Two new dependencies + one new crate edge from egui |

`clarity-contract` is therefore the pragmatic, minimal-change home.

## 3. Proposed Function Signature

```rust
pub fn resolve_key_ref(raw: &str) -> Option<String>
```

Behavior (single source of truth):

1. Trim `raw`.
2. Return `None` if empty after trim.
3. If `raw` is `${file:path:field}`:
   - Split on `:` into `path_part` and `field`.
   - Expand leading `~/` to the user's home directory via `dirs::home_dir()`.
   - Read the file, parse as JSON, return the string value at `field`.
   - Return `None` on any error.
4. If `raw` is `${env:VAR}`:
   - Return `std::env::var(VAR).ok()`.
5. Otherwise (plain string):
   - Try `std::env::var(raw).ok()` first.
   - Fall back to returning `raw.to_string()`.

This matches the richer `clarity-llm` semantics. The `clarity-egui` copy currently returns plain strings literally without an env fallback; adopting the unified function intentionally aligns egui with the project-wide semantics requested in the task.

## 4. Files to Modify

1. `crates/clarity-contract/src/key_ref.rs` — **new file**
2. `crates/clarity-contract/src/lib.rs` — **add module + re-export**
3. `crates/clarity-contract/Cargo.toml` — **add `tempfile` dev-dependency for tests**
4. `crates/clarity-llm/src/model_registry.rs` — **remove duplicate function + update call site**
5. `crates/clarity-egui/src/provider.rs` — **remove duplicate method + update call sites**

No new runtime dependencies are required because `clarity-contract` already has `dirs` and `serde_json`; only the test-only `tempfile` dev-dependency is added.

## 5. Step-by-Step Migration Plan

### Step 1 — Create the unified resolver in `clarity-contract`

Create `crates/clarity-contract/src/key_ref.rs` with the function and unit tests.

### Step 2 — Re-export from `clarity-contract`

Add `pub mod key_ref;` and `pub use key_ref::resolve_key_ref;` to `crates/clarity-contract/src/lib.rs`.

### Step 3 — Migrate `clarity-llm`

- Delete `resolve_key_ref` from `crates/clarity-llm/src/model_registry.rs`.
- Change the call inside `resolve_api_key` from `resolve_key_ref(env_name)` to `clarity_contract::resolve_key_ref(env_name)`.
- The existing `use std::path::{Path, PathBuf};` import stays because `PathBuf` is used elsewhere in the file.

### Step 4 — Migrate `clarity-egui`

- Delete `ProviderDefinition::resolve_key_ref` from `crates/clarity-egui/src/provider.rs`.
- Replace the two call sites (`Self::resolve_key_ref(ref_str)`) with `clarity_contract::resolve_key_ref(ref_str)`.
- The existing `use std::path::PathBuf;` import stays because `PathBuf` is used elsewhere in the file.

### Step 5 — Add / update tests

- Add comprehensive unit tests in `clarity-contract/src/key_ref.rs`.
- Verify the existing `clarity-egui` provider tests still pass (they do: `${env:TEST_FAKE_KEY}` is not set, and `sk-mykey` is returned literally because no env var has that name).

### Step 6 — Verify

Run the verification commands in §8.

## 6. Concrete `Edit` Blocks

These blocks are ready for the parent agent to apply verbatim.

### Block A — Create `crates/clarity-contract/src/key_ref.rs`

Create a new file with this exact content:

```rust
//! Key-reference resolution utilities.
//!
//! Supports `${env:VAR}`, `${file:path:field}`, and plain-string fallbacks.

#![cfg_attr(test, allow(unsafe_code))]

use std::path::PathBuf;

/// Expand a key reference string.
///
/// Supported syntax:
/// - `${file:path:field}` — read `field` from JSON file at `path` (`~` is expanded).
/// - `${env:VAR}` — read environment variable `VAR`.
/// - plain string — treated as an env-var name for backward compat, or returned as-is.
pub fn resolve_key_ref(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    // ${file:path:field}
    if let Some(inner) = raw
        .strip_prefix("${file:")
        .and_then(|s| s.strip_suffix('}'))
    {
        let (path_part, field) = inner.split_once(':')?;

        let path = if path_part.starts_with("~/") {
            dirs::home_dir()
                .map(|h| h.join(&path_part[2..]))
                .unwrap_or_else(|| PathBuf::from(path_part))
        } else {
            PathBuf::from(path_part)
        };
        let content = std::fs::read_to_string(&path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        return json
            .get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }

    // ${env:VAR}
    if let Some(var) = raw.strip_prefix("${env:").and_then(|s| s.strip_suffix('}')) {
        return std::env::var(var).ok();
    }

    // Try env var, fall back to literal
    std::env::var(raw).ok().or_else(|| Some(raw.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn empty_returns_none() {
        assert_eq!(resolve_key_ref(""), None);
        assert_eq!(resolve_key_ref("   "), None);
    }

    #[test]
    fn env_ref_when_set() {
        // SAFE: test-only env var setup; no concurrent reads of this name.
        unsafe { std::env::set_var("RESOLVE_TEST_KEY", "secret123") };
        assert_eq!(resolve_key_ref("${env:RESOLVE_TEST_KEY}"), Some("secret123".into()));
        // SAFE: test-only env var cleanup.
        unsafe { std::env::remove_var("RESOLVE_TEST_KEY") };
    }

    #[test]
    fn env_ref_when_missing() {
        // Use a name that is extremely unlikely to exist.
        assert_eq!(resolve_key_ref("${env:RESOLVE_TEST_MISSING_XYZ}"), None);
    }

    #[test]
    fn file_ref_absolute_path() {
        let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
        write!(tmp, r#"{{"api_key": "file-secret"}}"#).unwrap();
        let path = tmp.path().to_string_lossy();
        assert_eq!(
            resolve_key_ref(&format!("${{file:{path}:api_key}}")),
            Some("file-secret".into())
        );
    }

    #[test]
    fn file_ref_missing_field_returns_none() {
        let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
        write!(tmp, r#"{{"other": "x"}}"#).unwrap();
        let path = tmp.path().to_string_lossy();
        assert_eq!(resolve_key_ref(&format!("${{file:{path}:api_key}}")), None);
    }

    #[test]
    fn plain_string_literal() {
        assert_eq!(resolve_key_ref("sk-mykey"), Some("sk-mykey".into()));
    }

    #[test]
    fn plain_string_env_fallback() {
        // SAFE: test-only env var setup; no concurrent reads of this name.
        unsafe { std::env::set_var("RESOLVE_PLAIN_FALLBACK", "plain-secret") };
        assert_eq!(resolve_key_ref("RESOLVE_PLAIN_FALLBACK"), Some("plain-secret".into()));
        // SAFE: test-only env var cleanup.
        unsafe { std::env::remove_var("RESOLVE_PLAIN_FALLBACK") };
    }
}
```

> **Note:** `tempfile` is not currently a dev-dependency of `clarity-contract`. Block G adds it.

### Block B — Update `crates/clarity-contract/src/lib.rs`

Add the module declaration and re-export after the existing `pub mod` block.

**old_string:**
```rust
pub mod capability;
pub mod claw_context;
pub mod error;
pub mod federation;
pub mod llm;
pub mod reliable_provider;
pub mod rollout;
pub mod subagent;
pub mod thread;
pub mod tool;
```

**new_string:**
```rust
pub mod capability;
pub mod claw_context;
pub mod error;
pub mod federation;
pub mod key_ref;
pub mod llm;
pub mod reliable_provider;
pub mod rollout;
pub mod subagent;
pub mod thread;
pub mod tool;

pub use key_ref::resolve_key_ref;
```

### Block C — Remove duplicate from `crates/clarity-llm/src/model_registry.rs`

Remove the standalone `resolve_key_ref` function (lines 521-562).

**old_string:**
```rust

/// Expand a key reference string.
///
/// Supported syntax:
/// - `${file:path:field}` — read `field` from JSON file at `path` (`~` is expanded).
/// - `${env:VAR}` — read environment variable `VAR`.
/// - plain string — treated as an env-var name for backward compat, or returned as-is.
pub fn resolve_key_ref(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    // ${file:path:field}
    if let Some(inner) = raw
        .strip_prefix("${file:")
        .and_then(|s| s.strip_suffix('}'))
    {
        let (path_part, field) = inner.split_once(':')?;

        let path = if path_part.starts_with("~/") {
            dirs::home_dir()
                .map(|h| h.join(&path_part[2..]))
                .unwrap_or_else(|| PathBuf::from(path_part))
        } else {
            PathBuf::from(path_part)
        };
        let content = std::fs::read_to_string(&path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        return json
            .get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }

    // ${env:VAR}
    if let Some(var) = raw.strip_prefix("${env:").and_then(|s| s.strip_suffix('}')) {
        return std::env::var(var).ok();
    }

    // Try env var, fall back to literal
    std::env::var(raw).ok().or_else(|| Some(raw.to_string()))
}

/// Resolve an API key from a hierarchy of sources.
```

**new_string:**
```rust
/// Resolve an API key from a hierarchy of sources.
```

### Block D — Update the call site in `crates/clarity-llm/src/model_registry.rs`

**old_string:**
```rust
    let env_name = alias_api_key_env.or(provider_api_key_env)?;
    resolve_key_ref(env_name)
}
```

**new_string:**
```rust
    let env_name = alias_api_key_env.or(provider_api_key_env)?;
    clarity_contract::resolve_key_ref(env_name)
}
```

### Block E — Remove duplicate method from `crates/clarity-egui/src/provider.rs`

Delete the private `resolve_key_ref` method (lines 255-288).

**old_string:**
```rust

    /// Resolve a key reference string (env var, file field, or literal).
    fn resolve_key_ref(ref_str: &str) -> Option<String> {
        // ${env:VAR}
        if let Some(env_var) = ref_str
            .strip_prefix("${env:")
            .and_then(|s| s.strip_suffix('}'))
        {
            return std::env::var(env_var).ok();
        }

        // ${file:path:field}
        if let Some(inner) = ref_str
            .strip_prefix("${file:")
            .and_then(|s| s.strip_suffix('}'))
        {
            let (path_part, field) = inner.split_once(':')?;

            let path = if path_part.starts_with("~/") {
                dirs::home_dir()
                    .map(|h| h.join(&path_part[2..]))
                    .unwrap_or_else(|| std::path::PathBuf::from(path_part))
            } else {
                std::path::PathBuf::from(path_part)
            };
            let content = std::fs::read_to_string(&path).ok()?;
            let json: serde_json::Value = serde_json::from_str(&content).ok()?;
            return json
                .get(field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        Some(ref_str.to_string())
    }

    /// Full display name: falls back to `id` if `display_name` is empty.
```

**new_string:**
```rust
    /// Full display name: falls back to `id` if `display_name` is empty.
```

### Block F — Update call sites in `crates/clarity-egui/src/provider.rs`

There are two `Self::resolve_key_ref(ref_str)` calls (lines 237 and 252). Provide surrounding context so each replacement is unambiguous.

**First call site (OAuth static override):**

**old_string:**
```rust
        // OAuth path: static key takes precedence, then token store
        if self.auth_type == AuthType::OAuth {
            if !ref_str.is_empty() {
                return Self::resolve_key_ref(ref_str);
            }
```

**new_string:**
```rust
        // OAuth path: static key takes precedence, then token store
        if self.auth_type == AuthType::OAuth {
            if !ref_str.is_empty() {
                return clarity_contract::resolve_key_ref(ref_str);
            }
```

**Second call site (ApiKey / None path):**

**old_string:**
```rust
        // ApiKey / None path
        if ref_str.is_empty() {
            return None;
        }
        Self::resolve_key_ref(ref_str)
    }
```

**new_string:**
```rust
        // ApiKey / None path
        if ref_str.is_empty() {
            return None;
        }
        clarity_contract::resolve_key_ref(ref_str)
    }
```

### Block G — Add `tempfile` dev-dependency to `crates/clarity-contract/Cargo.toml`

The new unit tests in `key_ref.rs` use `tempfile::NamedTempFile`. `clarity-contract` does not currently have `[dev-dependencies]`, so add the section.

**old_string:**
```toml
[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
async-trait = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
tokio = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
tracing = { workspace = true }
parking_lot = "0.12"
dirs = "5.0"
```

**new_string:**
```toml
[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
async-trait = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
tokio = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
tracing = { workspace = true }
parking_lot = "0.12"
dirs = "5.0"

[dev-dependencies]
tempfile = "3"
```

## 7. Tests to Add or Update

### New tests in `clarity-contract/src/key_ref.rs`

| Test | Scenario |
|------|----------|
| `empty_returns_none` | Empty/whitespace input returns `None` |
| `env_ref_when_set` | `${env:VAR}` resolves to the env value |
| `env_ref_when_missing` | `${env:VAR}` returns `None` when unset |
| `file_ref_absolute_path` | `${file:/abs/path:field}` reads JSON field |
| `file_ref_missing_field_returns_none` | Missing JSON field returns `None` |
| `plain_string_literal` | Plain string returns literal |
| `plain_string_env_fallback` | Plain string matching an env var returns the env value |

> Optionally add `file_ref_home_expansion` if the test runner has a reliable home directory. Keep it simple to avoid CI flakiness.

### Existing tests to keep

- `clarity-egui/src/provider.rs::test_api_key_env_ref`
- `clarity-egui/src/provider.rs::test_api_key_literal`

Both should continue to pass after migration because the env var in the env-ref test is unset and the literal string does not collide with any env var.

## 8. Risk Level and Verification Commands

### Risk Level

**Low to Medium.**

- **Low** because the change is a mechanical move of pure code; no async, no state, no trait changes.
- **Medium** because of one intentional semantic alignment:
  - `clarity-egui` previously returned plain strings literally (no env fallback).
  - The unified resolver tries `std::env::var(plain)` first, then falls back to the literal.
  - This means an egui provider config whose `api_key_ref` happens to match an env var name will now resolve to that env var. This is the behavior requested in the task, but it is a change from the egui copy.

### Mitigations

- The two existing egui tests cover the common cases (unset `${env:...}` and a literal key).
- New contract-level unit tests cover the full matrix of supported syntaxes.
- Only one new dev-dependency is added (`tempfile` in `clarity-contract`), which is already used elsewhere in the workspace.
- Test-only `unsafe` for env-var manipulation is localized to `key_ref.rs` and explicitly allowed only under `#[cfg(test)]`.

### Verification commands

Run after applying the edits:

```bash
# 1. Check formatting
cargo fmt --all -- --check

# 2. Check clarity-contract compiles
cargo check -p clarity-contract

# 3. Check clarity-llm compiles
cargo check -p clarity-llm

# 4. Check clarity-egui compiles
cargo check -p clarity-egui

# 5. Run the new unit tests
cargo test -p clarity-contract --lib key_ref

# 6. Run affected crate tests
cargo test -p clarity-llm --lib model_registry
cargo test -p clarity-egui --lib provider

# 7. Full workspace sanity (excluding experimental clarity-slint)
cargo test --workspace --lib --exclude clarity-slint
```

Block G already adds the required `tempfile` dev-dependency, so no separate check is needed.
