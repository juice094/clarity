---
title: Clarity v0.2.0 Release Notes
category: Release
date: 2026-05-16
tags: [release]
---

# Clarity v0.2.0 Release Notes

> **Release Date**: 2026-04-20
> **Previous**: v0.1.1
> **License**: MIT
> **Rust Version**: 1.85+

---

## What's New in v0.2.0

### New Entry Point
- **Headless CLI** (`clarity-headless`): Pure terminal agent execution for scripts and CI/CD. Supports `--prompt`, `--file`, `--output json/markdown`, 5 LLM providers, and Plan Mode.

### New Desktop GUI Features
- **Computer Use Panel**: Screenshot, click, type, scroll via GUI panel with Python bridge (pyautogui + mss)
- **Diff Viewer**: Line-based diff for code review and AI-generated changes
- **LSP Proxy Layer**: Start/manage LSP servers (rust-analyzer, etc.) via GUI, with JSON-RPC message debugging
- **Web Browser Tool**: Navigate and extract content from web pages (lightweight reqwest+scraper, zero-config)
- **Session Persistence**: JSON file-based session save/load across app restarts
- **Task Panel**: Real-time background task tracking with persistence

### Local-First Enhancements (Post-Tag)

> Note: The following features were merged to `main` after the `v0.2.0` tag was created, and are included in `v0.2.1`.

- **Local LLM Default**: `local-llm` is now the default feature for `clarity-core`; no external API required out of the box
- **Offline Auto-Fallback**: Network monitoring with automatic fallback to local provider when offline; recovery detection with provider restoration
- **Settings-Runtime Wiring**: `GuiSettings` (provider, local_model_path, network_probe_url) is read at runtime by `ensure_llm`
- **Settings Memory Cache**: Eliminates per-request disk I/O; `save_settings` validates probe URL format
- **Concurrent Load Safety**: Double-checked locking with `tokio::sync::Mutex<()>` prevents race conditions in `ensure_llm`
- **Tokenizer Auto-Detection**: Prioritizes sibling `tokenizer.json` next to model file; avoids unnecessary HuggingFace downloads
- **Startup Error Caching**: `prewarm_error` in `AppState` preserves startup LLM failures for frontend diagnostics

### New Tools
- `computer_use`: Desktop automation (screenshot, click, type, scroll)
- `web_browser`: Web page navigation and content extraction

### Security & Legal
- Removed README statement linking to leaked source
- Reframed positioning as independent Rust-native AI runtime

---

## Installation

```bash
# From git tag (recommended)
cargo install --git https://github.com/juice094/clarity --tag v0.2.0 --bin clarity-tui
cargo install --git https://github.com/juice094/clarity --tag v0.2.0 --bin clarity-gateway
cargo install --git https://github.com/juice094/clarity --tag v0.2.0 --bin clarity-headless
cargo install --git https://github.com/juice094/clarity --tag v0.2.0 --bin clarity-claw

# Or clone and install locally
git clone https://github.com/juice094/clarity.git
cd clarity
cargo install --path crates/clarity-tui
```

---

## Test Coverage

```
524 tests passed, 0 failed, 4 ignored
```

---

## Roadmap

### v0.2.1 (Current)
- **T_FTUE** — First-time user experience: launch status detection + Onboarding modal
- **T_DYNAMIC_PROMPT** — Conditional system prompt builder with approval mode injection
- **T_APPROVAL (V1)** — Rule-based risk engine for tool call approval
- **T_SETTINGS** — Provider/model hot-reload command
- **T_COMPACT** — Tier-1 fast local truncation + Tier-2 LLM summarization
- **T_PARALLEL** — Concurrent tool call execution
- **T_RELEASE/T_PACKAGE/T_UPDATE/T_SIGN** — CI release workflow + MSI/NSIS bundling + update check

### v0.3.0 (Next)
- Single-binary packaging research (`cargo-bundle` / `tauri-bundler`)
- Embedded model onboarding (guided download + progress UI)
- GUI Monaco editor integration
- Computer Use vision integration (AI reads screenshots)

---

**Full Changelog**: `git log v0.1.1..v0.2.0 --oneline`
