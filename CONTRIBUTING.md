# Contributing to Clarity

Thank you for considering a contribution. This document is the single source of truth for how to build, test, and submit changes to Clarity. If something is unclear, open an issue — this document should be fixed, not circumvented.

---

## 1. Project Architecture Map

> **Rule**: Understand the crate boundary before touching the code. Violating `clarity-core`'s zero-frontend invariant is the fastest way to get a PR rejected.

```
crates/
├── clarity-core      # Agent loop, LLM providers, tools, memory, MCP, subagents
│   └── Rule: NO dependencies on any frontend or network crate.
├── clarity-memory    # BM25 + vector hybrid search, chunking, four-level compilation
├── clarity-gateway   # Axum HTTP server, Web UI, session store, channel integrations
├── clarity-tauri     # Tauri 2 Desktop GUI (React + Vite + i18n)
│   └── frontend/     # React app; communicates with Rust via Tauri invoke/commands
├── clarity-tui       # ratatui terminal interface
├── clarity-claw      # System-tray background monitor (tao + tray-icon)
├── clarity-wire      # UI↔Agent event bus (SPMC, cross-module pub/sub)
└── clarity-headless  # Headless CLI for scripts/CI
```

**Dependency direction**: `clarity-core` ← `clarity-memory` / `clarity-wire` ← all frontends (`gateway`, `tauri`, `tui`, `claw`, `headless`).

**Forbidden patterns**:
- `clarity-core` importing `tauri-apps/api` or `axum` — **never**.
- Frontend crates importing each other — use `clarity-wire` for cross-frontend communication.
- Blocking I/O in async contexts — use `tokio::task::spawn_blocking`.

---

## 2. Development Environment

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.85+ | Core language |
| Node.js | 18+ | Tauri frontend build |
| Git | any | Version control |

### One-Time Setup

```bash
# 1. Clone
git clone https://github.com/juice094/clarity.git
cd clarity

# 2. Verify Rust toolchain
rustc --version  # should be 1.85+

# 3. Install frontend dependencies (only needed for Tauri GUI)
cd crates/clarity-tauri/frontend
npm install
cd ../../..

# 4. Verify everything compiles
cargo check --workspace
```

### CUDA Support (Optional)

For NVIDIA GPU acceleration of local GGUF inference:

```powershell
# Windows
$env:NVCC_CCBIN="C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.40.33807\bin\Hostx64\x64"
cargo check -p clarity-core --features local-llm-cuda
cargo run -p clarity-tauri --features cuda
```

See [`AGENTS.md`](AGENTS.md) for full CUDA setup instructions.

---

## 3. Contribution Workflow

We use a **branch-per-change** model. All changes go through a PR to `main`.

```bash
# 1. Ensure main is up to date
git checkout main
git pull origin main

# 2. Create a feature branch
git checkout -b feat/your-feature-name

# 3. Make your changes, commit with conventional commits
git commit -m "feat(tauri): add model download progress bar"

# 4. Push and open PR
git push origin feat/your-feature-name
```

### Commit Message Convention

```
type(scope): imperative description under 72 chars

Optional body explaining why, not what.

Closes #123
```

| Type | Use when |
|------|----------|
| `feat` | New feature or capability |
| `fix` | Bug fix |
| `docs` | Documentation-only change |
| `refactor` | Code restructuring with no behavior change |
| `test` | Adding or fixing tests |
| `chore` | Maintenance (deps, CI, formatting) |
| `perf` | Performance improvement |

**Scopes**: `core`, `memory`, `gateway`, `tauri`, `tui`, `claw`, `wire`, `headless`, `ci`, `docs`.

---

## 4. Code Standards

### Rust

```bash
# Run before every commit
cargo fmt --all
cargo clippy --workspace --lib --bins --tests -- -D warnings
cargo test --workspace --lib
```

**Hard rules**:
- `cargo clippy -- -D warnings` must pass with **zero warnings**.
- `cargo test --workspace --lib` must pass with **zero failures**.
- `cargo audit` must show **zero high/critical vulnerabilities**.
- All `pub` items must have doc comments (`///`).
- `unsafe` blocks are prohibited without explicit maintainer approval.

### Frontend (React/TypeScript)

```bash
cd crates/clarity-tauri/frontend
npm run build        # must pass without TypeScript errors
```

**Rules**:
- Use `useTranslation()` for all user-facing strings. No hardcoded English or Chinese in JSX.
- Prefer functional components with hooks.
- Keep components under 300 lines; extract sub-components when exceeding.

---

## 5. Testing Requirements

| Change Type | Required Tests |
|-------------|----------------|
| New Rust module | Unit tests in `#[cfg(test)]` blocks |
| New Tauri command | Mock integration test or manual QA checklist |
| New frontend component | Visual inspection + interaction check |
| Bug fix | Regression test that fails before the fix |
| Performance change | Benchmark or latency measurement |

### Manual QA Checklist (for UI changes)

- [ ] Tauri window opens without console errors
- [ ] Settings Panel opens, saves, and persists across restart
- [ ] Language switch (中文/English) works and persists
- [ ] Offline banner appears when network is disabled
- [ ] Local model selection scans `~/models/` correctly

---

## 6. Where to Contribute

| Skill | Good First Issues | Impact |
|-------|-------------------|--------|
| Rust + AI/LLM | Local model provider improvements, memory optimization | High |
| Rust + Systems | MCP protocol extensions, sandbox research | High |
| React/TS | UI polish, onboarding flow, i18n completion | Medium |
| DevOps/CI | Release workflow, cross-platform builds | Medium |
| Documentation | Translation, tutorial writing, architecture docs | Low barrier |

Check the [GitHub Issues](https://github.com/juice094/clarity/issues) for items labeled `good first issue`.

---

## 7. Communication

- **Bug reports**: [GitHub Issues](https://github.com/juice094/clarity/issues) — use the bug template.
- **Feature requests**: Open a Discussion first. If accepted, convert to an issue.
- **Security issues**: Email `juice094@users.noreply.github.com` directly. Do not open public issues.
- **Real-time chat**: Not available. This is intentional — async, documented decisions scale better than chat history.

---

## 8. License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
