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
488 tests passed, 0 failed, 3 ignored
```

---

## Roadmap

### v0.3.0 (Next)
- GUI Monaco editor integration
- Ollama model list auto-discovery
- Computer Use vision integration (AI reads screenshots)
- Enhanced local-LLM onboarding

---

**Full Changelog**: `git log v0.1.1..v0.2.0 --oneline`
