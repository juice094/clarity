# Contributing to Clarity

Thank you for considering a contribution. This document is the single source of truth for how to build, test, and submit changes to Clarity. If something is unclear, open an issue — this document should be fixed, not circumvented.

---

## 1. Project Architecture Map

> **Rule**: Understand the crate boundary before touching the code. Violating `clarity-core`'s zero-frontend invariant is the fastest way to get a PR rejected.

```
crates/
├── clarity-contract   # Shared trait/types contract: LlmProvider, Tool,
│                      # AgentError, FederationMessage.
│                      # Rule: ZERO internal dependencies.
├── clarity-wire       # UI ↔ Agent event bus (SPMC) + ViewCommand channel.
├── clarity-memory     # BM25 + vector hybrid search, chunking, compaction.
├── clarity-mcp        # MCP client — stdio / SSE / HTTP / WebSocket transports.
├── clarity-llm        # LLM provider abstraction + built-in providers + Candle GGUF.
├── clarity-tools      # Built-in tool library (file / shell / web / devkit / …).
├── clarity-secrets    # ChaCha20-Poly1305 encrypted secret storage.
├── clarity-channels   # External message channels (Discord/Slack/Telegram/Webhook/WeChat).
├── clarity-subagents  # Sub-agent executor + parallel scheduler.
├── clarity-core       # Agent loop (ReAct/Plan), Approval, Skill, MCP integration.
│                      # Rule: NO dependencies on any frontend or network crate.
├── clarity-telemetry  # WideEvent + metrics + traces + config audit.
├── clarity-gateway    # Axum HTTP/WebSocket server, Web UI, session store.
├── clarity-egui       # Desktop GUI (eframe/egui) — primary UI stack, pure Rust.
├── clarity-tui        # ratatui terminal interface.
├── clarity-claw       # System-tray background monitor (tao + tray-icon).
├── clarity-headless   # Headless CLI for scripts / CI.
├── clarity-slint      # Experimental Slint frontend (excluded from default CI).
└── clarity-tauri      # Archived frontend (excluded from workspace).
```

### clarity-egui Module Map

Key directories for contributors working on the desktop GUI:

| Directory | Purpose |
|-----------|---------|
| `stores/` | Zustand-style state (ConsoleStore, FilesStore, ChatStore, SessionStore, UiStore) |
| `panels/right_ide_panel/` | Console (filtered log), Files (dir tree+context menu), Share (export), Templates (prompts) |
| `widgets/` | diff_viewer, context_picker (# picker), svg_image, command_palette, interactive_row |
| `ui/` | markdown parser/renderer, syntax_highlight (syntect 18-lang), message renderer |
| `handlers/` | UiEvent dispatch (chat, session, settings, task, cron, subagent, team) |
| `theme.rs` | 100+ design tokens, 6 presets (Dark/Light/OLED/Catppuccin/TokyoNight/OneDark) |

**Dependency direction**

```
contract ← {wire, memory, mcp, llm, tools, channels} ← core ← {gateway, egui, tui, claw, headless}
                                                          ↑
                                                    subagents (consumes core)
                                                    telemetry (cross-cutting)
```

**Forbidden patterns**:
- `clarity-core` importing `eframe`/`egui` or `axum` — **never**.
- `clarity-contract` importing any other internal crate — **never**.
- Frontend crates importing each other — use `clarity-wire` for cross-frontend communication.
- Blocking I/O in async contexts — use `tokio::task::spawn_blocking`.

---

## 2. Development Environment

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.94+ | Core language |
| Git | any | Version control |
| ~~Node.js~~ | ~~20+~~ | ~~Gateway Web UI build~~ — **deprecated**. Tauri frontend archived; Gateway serves pre-built static assets. Node.js no longer required. |

### One-Time Setup

```bash
# 1. Clone
git clone https://github.com/juice094/clarity.git
cd clarity

# 2. Verify Rust toolchain
rustc --version  # should be 1.94+ (matches MSRV in CI; Cargo.toml rust-version = 1.85)

# 3. Verify everything compiles
cargo check --workspace
```

### CUDA Support (Optional)

For NVIDIA GPU acceleration of local GGUF inference:

```powershell
# Windows
$env:NVCC_CCBIN="C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.40.33807\bin\Hostx64\x64"
cargo check -p clarity-core --features local-llm-cuda
cargo run -p clarity-egui --features cuda
```

See [`docs/development/setup.md`](docs/development/setup.md) for full build commands and CUDA setup.

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
git commit -m "feat(egui): add model download progress bar"

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

**Scopes**: `core`, `memory`, `gateway`, `egui`, `tui`, `claw`, `wire`, `headless`, `ci`, `docs`.

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

### Frontend (egui)

```bash
cargo check -p clarity-egui
cargo clippy -p clarity-egui -- -D warnings
```

**Rules**:
- All user-facing strings must go through `i18n` (`t!("key")`). No hardcoded English or Chinese.
- Use `Frame::new()` instead of `Frame::group(ui.style())` for consistent theming.
- Keep panel render functions under 300 lines; extract sub-components when exceeding.
- Prefer `ScrollArea` with `AlwaysHidden` scrollbar for clean glassmorphism look.
- Modal dialogs must use `Frame::window` + `radius_lg` + dimmer overlay + Escape/click-outside to close.

---

## 5. Testing Requirements

| Change Type | Required Tests |
|-------------|----------------|
| New Rust module | Unit tests in `#[cfg(test)]` blocks |
| New egui panel/component | Manual QA checklist or visual regression check |
| New frontend component | Visual inspection + interaction check |
| Bug fix | Regression test that fails before the fix |
| Performance change | Benchmark or latency measurement |

### Manual QA Checklist (for UI changes)

- [ ] egui window opens without panic or layout overflow
- [ ] Settings popup opens, saves, and persists across restart
- [ ] Language switch (中文/English) works and persists
- [ ] Offline banner appears when network is disabled
- [ ] Local model selection scans `~/models/` correctly
- [ ] Tab bar renders full-width with correct active state
- [ ] Sidebar global toolbar buttons (Online/Token/Lang/Skills/MCP/Settings) all functional

---

## 6. Where to Contribute

| Skill | Good First Issues | Impact |
|-------|-------------------|--------|
| Rust + AI/LLM | Local model provider improvements, memory optimization | High |
| Rust + Systems | MCP protocol extensions, sandbox research | High |
| egui/Rust UI | UI polish, glassmorphism refinements, i18n completion | Medium |
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

## 8. Sprint Discipline & Agent Collaboration

This project uses AI agents for rapid iteration. If you are an agent contributor, follow these rules:

### Commit Hygiene
- **One concern per commit**. Do not mix feature code, test fixes, and documentation in a single commit.
- **Commit message format**: `<type>(<scope>): <imperative summary>` — e.g. `feat(agent): add LoopDetector`.
- **Verify before commit**: `cargo test --workspace --lib -- --test-threads=1` must pass.

### Sub-Agent Isolation
- Each sub-agent task must `git status` before committing to ensure only its own files are staged.
- If a sub-agent finds uncommitted changes from another task, it must `git stash` them, complete its work, commit, then `git stash pop`.
- Never force-push to `main` without human approval.

### Architecture Health Check (Before Every Major Refactor)
1. Can this module be extracted as an independent crate in half a day?
2. Can you write a 50-word README explaining why an external project would use it?
3. Can you name 2 existing alternatives?
4. Do unit tests pass without depending on other modules' side effects?

If any answer is "no", the module is too coupled. Refactor first, feature second.

---

## 9. Further Reading

| Document | Purpose |
|----------|---------|
| [`AGENTS.md`](AGENTS.md) | Agent development context, environment variables, architecture invariants |
| [`docs/development/setup.md`](docs/development/setup.md) | Build, test, lint, and run commands |
| [`docs/development/CODE-CHANGE-PRINCIPLES.md`](docs/development/CODE-CHANGE-PRINCIPLES.md) | PR self-checklist (P1–P7) |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Code-accurate architecture reference |
| [`docs/planning/ROADMAP.md`](docs/planning/ROADMAP.md) | Future direction and milestones |
| [`SECURITY.md`](SECURITY.md) | Security policy and vulnerability reporting |

## 10. Contributor License Agreement (CLA)

By submitting a pull request or otherwise contributing code, documentation, or other materials to Clarity, you agree to the following:

1. **You grant juice094** a perpetual, worldwide, non-exclusive, royalty-free, irrevocable license to use, reproduce, modify, display, perform, sublicense, and distribute your contributions as part of the Clarity project under the [GNU Affero General Public License v3.0](LICENSE) (or any later version).
2. **You represent that** each of your contributions is your original creation and that you have the right to grant the above license. If your employer has rights to intellectual property that you create, you represent that you have received permission to make contributions on behalf of that employer, or that your employer has waived such rights.
3. **You understand** that this CLA does not change your rights to use your own contributions for any other purpose.

## 10. License

By contributing, you agree that your contributions will be licensed under the [GNU Affero General Public License v3.0](LICENSE) (or any later version).
