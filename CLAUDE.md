# Clarity Project Context

## Project

Clarity is a Rust-native personal AI runtime (Layer 3 infrastructure). 13 crates, AGPL-3.0, maintained by juice094.

## Architecture Invariants (Hard Veto)

- `clarity-core` has ZERO dependencies on any frontend or network crate.
- `clarity-contract` has ZERO internal dependencies.
- Frontend crates never import each other — use `clarity-wire`.
- No Docker / RAG(Qdrant) / GUI(Electron).
- Rust core modules cannot be outsourced to sub-agents without human review.

## Quality Gates (Every Commit)

```bash
cargo test --workspace --lib          # 849 passed / 0 failed / 7 ignored
cargo clippy --workspace --lib --bins --tests -- -D warnings  # zero warnings
cargo fmt --all -- --check            # zero diffs
cargo audit                           # zero high/critical
```

**Hard rules**: clippy zero warnings, test zero failures, fmt zero diffs.

## Crate Topology

```
contract  ←  {wire, memory, mcp, llm, tools}  ←  core  ←  {gateway, egui, tui, claw, headless}
                                                   ↑
                                            subagents (consumes core)
```

## Coding Standards

- All `pub` items must have doc comments.
- User-facing strings go through `i18n` (`t!("key")`).
- `unsafe` blocks require explicit maintainer approval.
- One concern per commit. Conventional commits: `feat(scope): imperative under 72 chars`.
- Keep panel render functions under 300 lines.
- Use `Frame::new()` instead of `Frame::group(ui.style())`.

## MCP Integration

This project exposes a local MCP server (`clarity-dev`) providing:
- `cargo_check` — `cargo check --workspace`
- `cargo_test` — `cargo test --workspace --lib`
- `cargo_clippy` — `cargo clippy ... -D warnings`
- `cargo_fmt_check` — `cargo fmt --all -- --check`

Claude Code in this project should prefer these tools over manual Bash when verifying code changes.

## Documentation Index

| Doc | Purpose |
|-----|---------|
| `README.md` | Project intro |
| `CONTRIBUTING.md` | Dev guide, crate boundaries |
| `AGENTS.md` | Agent dev context, sprint status |
| `docs/ARCHITECTURE.md` | Code-accurate architecture |
| `docs/architecture-positioning.md` | Project positioning, Hard Veto |
| `CHANGELOG.md` | Version history |
