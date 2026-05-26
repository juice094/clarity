---
title: ADR-004: Replace `native-tls` with `rustls-tls` Across All Crates
category: ADR
tags: [adr]
---

# ADR-004: Replace `native-tls` with `rustls-tls` Across All Crates

- Status: Accepted
- Deciders: juice094
- Date: 2026-05-05

## Context

Dependabot alerts #22 and #23 flagged vulnerabilities in `openssl 0.10.79`:
- **CVE-2026-42327** (high severity)
- **AES key-wrap-with-padding heap buffer overflow** (moderate severity)

`cargo update` could not resolve these alerts because the vulnerabilities were in the currently latest `openssl` version available on crates.io. The `openssl` crate was pulled into the dependency tree transitively via:
- `reqwest` default features (`native-tls` backend)
- `hf-hub` default features (`ureq` + `native-tls`)
- `hyper-tls`, `tokio-native-tls`, `native-tls` itself

Because Clarity's security model requires TLS to be "never disabled" and the project follows a "本地优先 + 零依赖 + 开源" stance, waiting for upstream `openssl` patches was unacceptable. A proactive elimination of the `openssl` dependency tree was required.

## Decision

Replace all `native-tls` / `openssl` backed HTTP clients with `rustls-tls`:

1. **reqwest** (5 crates): Switch from default features to `default-features = false` + `rustls-tls`:
   - `clarity-core`
   - `clarity-gateway`
   - `clarity-mcp`
   - `clarity-claw`
   - `clarity-egui`

2. **hf-hub** (`clarity-core`): Disable default features, enable `tokio` + `rustls-tls`. This removes `ureq` and `native-tls` from the `hf-hub` subtree.

3. **local_gguf.rs**: Migrate tokenizer download from `hf_hub::api::sync::Api` (blocking, previously required by the synchronous `ureq` backend) to `hf_hub::api::tokio::Api` (async, compatible with `rustls-tls`).

4. **Cargo.lock cleanup**: After the above changes, run `cargo update` to purge `openssl`, `native-tls`, `hyper-tls`, `tokio-native-tls`, and `ureq` from the lockfile (17 packages total removed).

## Consequences

### Positive
- **Dependabot alerts #22 and #23 eliminated**.
- **`cargo tree -i openssl`** returns "did not match any packages" — the entire `openssl` dependency tree is gone.
- `rustls` is a pure-Rust TLS implementation, aligning with the "Rust 核心模块不可外包" Hard Veto and reducing C-dependency build fragility (especially on Windows where `openssl-sys` often requires Perl / NASM).
- Single-binary release packaging is simplified: no need to ship or link system OpenSSL libraries.

### Negative
- `rustls` historically had narrower platform support than OpenSSL (e.g., some embedded or exotic OS targets). This is acceptable because Clarity targets Windows/macOS/Linux desktop and server environments where `rustls` is fully supported.
- `rustls-tls` does not use the OS certificate store on all platforms by default; `rustls-platform-verifier` or `webpki-roots` is used instead. This is a behavior change but not a functional regression for Clarity's use cases (cloud LLM APIs and HuggingFace).

### Neutral
- No API surface changes; all HTTP client code continues to use `reqwest::Client` with identical method signatures.
- `cargo test --workspace --lib` passes with 800+ tests and zero failures.
- `cargo clippy --workspace --lib --tests` remains at zero errors.

## Alternatives Considered

| Alternative | Evaluation | Outcome |
|---|---|---|
| **Wait for upstream `openssl` patch** | Unacceptable: `cargo update` already could not fix; no ETA from upstream. Leaving a high-severity CVE in a local-first runtime violates security baseline. | Rejected |
| **Bump `openssl` to a newer version** | No newer compatible version existed on crates.io at the time of the alert. | Rejected |
| **Switch to `reqwest` with `native-tls-vendored`** | Would still link `openssl` statically; the underlying CVE remains present. Only hides the system dependency. | Rejected |
| **Use `hyper` + `rustls` directly, drop `reqwest`** | Would eliminate `reqwest` entirely but requires rewriting HTTP client logic across 5 crates. High risk, low reward given `reqwest`'s `rustls-tls` feature works perfectly. | Rejected |
| **`reqwest` + `rustls-tls` + `hf-hub` `tokio` + `rustls-tls`** | Drop-in replacement via Cargo feature flags. Minimal code changes (`sync::Api` → `tokio::Api` in one file). Proven by `cargo test` and `cargo tree`. | Accepted |

## Verification

```bash
cargo test --workspace --lib
# 800+ passed / 0 failed / 7 ignored

cargo tree -i openssl
# error: package ID specification `openssl` did not match any packages
```

## References

- Commit: `d27a075a` (security: remove openssl dependency entirely — Dependabot #22/#23)
- Commit: `e7303e3a` (security: bump openssl 0.10.78 → 0.10.79 — intermediate step, superseded by full removal)
- Related docs: `docs/sprint-archive.md` (Sprint 40 — "安全修复：彻底移除 openssl 依赖")
- Related docs: `docs/ARCHITECTURE.md` (Security Model: TLS layer)
