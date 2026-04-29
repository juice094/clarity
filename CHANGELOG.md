# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Sprint 9 Phase 1 — API Key 安全注入 + Settings 增量保存** — `GuiSettings::resolve_api_key()` 支持 `${env:VAR_NAME}` 语法，运行时解析环境变量，避免 API Key 明文落盘。`save()` 改为增量 merge（`merge_json` 递归合并），只写入变更字段，保留未知配置，规避 OpenClaw 式全配置覆盖风险。UI 输入框 placeholder 提示环境变量语法。
- **Sprint 9 Phase 2 — ModelRegistry 动态接入 egui** — `get_available_models()` 从 `ModelRegistry::load()` 动态读取 provider/model 列表，registry 结果与硬编码 fallback **合并**（registry 优先，缺位补充）。`ensure_llm` 非 local provider 创建时优先尝试 `ModelRegistry`（支持 `models.toml` 自定义 provider），失败 fallback 到 `LlmFactory`。新增 `build_provider_from_registry_with_key()`，允许 UI 传入的 API key 覆盖环境变量。

### Changed

- `clarity-egui` 测试基线从 0 提升至 26（`app_state`/`settings`/`theme` 纯逻辑测试），UI 渲染测试仍为缺口。

## [0.3.1] — 2026-04-27

### Added

- **嵌入式模型自动下载** — `clarity-core` 新增 `model_download.rs`（HuggingFace 直链流式下载 + 进度回调）。预配置 Qwen2.5-1.5B-Instruct-GGUF（~1.0 GB）。`clarity-egui` 新增首次启动引导覆盖层 `onboarding.rs`：三选项（输入 API Key / 下载本地模型 / 稍后配置），下载完成后自动配置 provider=local 并 reload LLM。
- **unwrap/expect 全面审计** — 全 workspace 非测试代码 171 处 unwrap/expect 分级审计。5 处高风险已修复（subagents/store、gateway/webhook、memory/embedding×2、tui/main）。中风险 9 处确认全部已有 `// SAFE:` 注释或属低风险类别（Regex::new、锁、duration_since）。新增 `docs/unwrap-debt-map.md` 作为持续维护的债务地图。

### Changed

- **Release CI 适配 egui** — `.github/workflows/release.yml` 移除 Node.js / Tauri CLI / 前端构建步骤，改为 `cargo build --release -p clarity-egui` + 自签名 + Release 上传。`.cargo/audit.toml` 清理已失效的 Tauri 相关忽略项，注释当前 transitive deps 的 unmaintained 原因。
- **benchmark 脚本硬化** — 修复 `Measure-Memory` 哈希表数组处理 bug（`Measure-Object` 对 `[hashtable[]]` 失效 → 改用 `ForEach-Object` 提取属性）；支持 `-Profile release` 覆盖；新增 `clarity-egui` startup/memory 测试；移除已归档的 `clarity-tauri` 编译基准。
- **Settings 模型选择体验修复** — Model 字段从自由文本 `TextEdit` 改为 `ComboBox`，provider 切换时自动联动更新模型；`load()` 解析失败时记录 warning 并备份损坏文件为 `.bak`；`default_with_env()` 改为 provider 匹配的环境变量选择；`ensure_llm` 新增断网自动 fallback 到 local provider。
- **Mutex 硬化** — `AppState` 和 `main.rs` 中 `std::sync::Mutex` → `parking_lot::Mutex`，消除 12 处 `lock().unwrap()` panic 风险。
- **App::update() 拆分** — 550 行 monolithic `update()` 拆分为 6 个独立渲染方法（`render_sidebar`, `render_task_panel`, `render_chat_area`, `render_settings_panel`, `render_mcp_panel`, `render_toasts`），符合 Pretext hot-path 原则。

### Removed

- `clarity-tauri` 完全归档 — 前端代码、package-lock.json、workspace 引用全部删除；Rust 源码移至外部归档目录。Dependabot 安全报警从 3 个降至 0 个。当前主力桌面 GUI 为 `clarity-egui`（eframe/egui，纯 Rust，零 web 依赖）。
- `clarity-egui` 中未使用的 `anyhow` 依赖。

### Fixed

- **onboarding 本地模型检测** — `should_show_onboarding()` 新增 `default_model_dir()` 下 `.gguf` 扫描，避免手动放置模型后仍强制显示引导覆盖层。

## [0.3.0] — 2026-04-26

### Added

- **T_FTUE** — First-time user experience with launch status detection (`get_launch_status` command) and `OnboardingModal` React component with i18n support (en/zh).
- **T_DYNAMIC_PROMPT** — `SystemPromptBuilder` with declarative `PromptComponent` enum for conditional prompt assembly (approval mode notices, offline notices, template variables).
- **T_APPROVAL (V1)** — Rule-based risk engine (`RuleEngine` with `RiskLevel`: Auto/Low/Medium/High). Integrated into `execute_tool_call` to reduce unnecessary approval popups in Interactive mode.
- **T_SETTINGS** — `reload_llm` Tauri command for provider/model hot-swap without restart.
- **T_COMPACT** — Two-tier compaction: Tier-1 fast local truncation of old assistant text (no LLM call) + Tier-2 LLM summarization.
- **T_PARALLEL** — Concurrent tool call execution via `futures::future::join_all` in both sync and streaming loops.
- **T_RELEASE/T_PACKAGE/T_UPDATE/T_SIGN** — GitHub Actions release workflow (tag-triggered), MSI/NSIS bundling, auto-update check (GitHub API + SemVer), and Windows self-signed certificate signing.
- **v0.3.0 Daily Hardening (4 stages)** — Tool call visualization, compaction status banner, HuggingFace model download GUI, frontend log panel.

### Changed

- **Approval Module Restructure** — `approval.rs` split into `approval/mod.rs` + `approval/rules.rs`.
- **Interactive Mode Refinement** — Low-risk tools (`file_read`, `web_search`) now auto-approve in Interactive mode; only High/Medium/forced-approval tools prompt.

### Fixed

- **Tauri Build Paths** — `tauri.conf.json` `frontendDist` corrected from `../frontend/dist` to `frontend/dist`; `beforeBuildCommand`/`beforeDevCommand` switched to `npm run build`/`npm run dev` (Tauri CLI executes these in `frontend/` directory on Windows).
- **CI Release Workflow** — Added `working-directory: crates/clarity-tauri` to the Tauri build step so `cargo tauri build` locates `tauri.conf.json` correctly in GitHub Actions.
- **FTUE Settings Reload** — `SettingsPanel` now calls `reload_llm` after `save_settings` succeeds, ensuring the LLM binding is re-created with new provider/model configuration without restart.
- **GUI API Key Input** — `SettingsPanel` now displays a password input field for non-local providers. `LlmFactory::create_with_key()` creates cloud providers (OpenAI/Anthropic/Kimi/DeepSeek/Ollama) with an explicit API key, bypassing environment variables. `ensure_llm` prioritizes `GuiSettings.api_key`, falls back to env vars, and surfaces a clear error if neither is set. `ftue::configured` now checks for `api_key` presence. Clarity is now usable without manual environment configuration.

## [0.2.1] — 2026-04-25

### Added

- **Local GGUF Inference (Default)** — `LocalGgufProvider` using Candle for zero-dependency native GGUF model loading. Supports Qwen2/Qwen2.5/DeepSeek-R1-Distill-Qwen architectures with auto chat-template detection and streaming via `tokio::mpsc`. Now enabled by default via `clarity-core/default = ["local-llm"]`.
- **Offline Auto-Fallback** — Startup `prewarm_llm` with background network monitoring (TCP probe every 30s). Offline mode automatically falls back to local provider; reconnects to cloud provider when network recovers. Frontend shows `llm:fallback` banner.
- **Settings-Runtime Integration** — `ensure_llm` reads `GuiSettings` provider and `local_model_path` at runtime. Settings cache eliminates per-request disk I/O. `save_settings` validates `network_probe_url` format.
- **Headless CLI Local Provider** — `clarity-headless --provider local` with `CLARITY_LOCAL_MODEL_PATH` and `CLARITY_LOCAL_TOKENIZER_REPO` env vars.
- **Model Path Resolution** — `resolve_local_model_path()` replaces all hardcoded paths; uses `CLARITY_LOCAL_MODEL_PATH` env var or auto-scans `~/models/`.
- **Settings Panel Local Model Config** — `clarity-tauri` Settings Panel now supports selecting `Local (GGUF)` provider with auto-scanned `.gguf` models from `~/models/` and `CLARITY_LOCAL_MODEL_PATH`. Path persisted in `gui-settings.json`.
- **Tokenizer Auto-Detection** — `ensure_llm` auto-detects `tokenizer.json` in model directory; avoids HuggingFace download when local file exists. Files < 1024 bytes are rejected as corrupted.
- **Startup Error Caching** — `AppState.prewarm_error` caches startup LLM load errors for frontend inspection via `get_prewarm_status`.

### Changed

- **Kalosm Deprecation** — Old `KalosmProvider` stubbed to redirect users to `LocalGgufProvider`. `ProtocolType::KalosmLocal` now builds `LocalGgufProvider`.

### Fixed

- **Concurrent LLM Load Race** — `tokio::sync::Mutex<()>` double-checked locking prevents concurrent duplicate model loading in `ensure_llm`.
- **Explicit Provider Failure** — When user explicitly selects a provider and it fails, error is returned directly instead of silently falling back to `auto_arc`.
- **LlmBinding.is_fallback Dead Code** — Removed unused field.

### Security

- **Network Probe URL Validation** — `save_settings` validates probe URL contains valid port (1–65535), rejecting malformed endpoints.

## [0.1.2] — 2026-04-24

### Added

- **Memory Depth Integration** — `clarity-core` now uses `clarity-memory::SharedMemoryTicker` exclusively. Gateway `create_agent()` wires up `PersistentMemoryStore` + `SharedMemoryTicker` with a 5-turn default trigger. `MemoryCompiler` four-level pipeline (today → week → longterm → facts) runs automatically on ticker callback via `LlmProviderBridge`.
- **Slack Channel** — New `SlackChannel` implementing the `Channel` trait. Supports `chat.postMessage` with automatic 4000-char chunking, Slack Events API challenge verification, and HMAC-SHA256 signature validation (`verify_signature`). Enabled via `SLACK_ENABLED` / `SLACK_BOT_TOKEN` / `SLACK_APP_TOKEN` env vars.
- **TOML Config System** — `Config::load()` reads three-layer TOML (defaults → `~/.config/clarity/config.toml` → `.clarity.toml`). `export_to_env()` writes profile credentials to provider-specific env vars only when not already set. Integrated into both Gateway and TUI `create_agent()`.

### Fixed

- **Documentation Drift** — `PROJECT_STATUS.md` and `tools_roadmap.md` corrected 6 falsely-claimed limitations that were already implemented.
- **Send Safety** — `clarity-memory::CompilationFuture` and `CompileCallback` tightened with `+ Send` bounds to compile inside `tokio::spawn` async blocks.

## [0.1.1] — 2026-04-23

### Security

- **Directory Traversal Fix** — `resolve_path()` in `clarity-core` now validates that resolved paths stay within the working directory, preventing `..` escapes and absolute-path traversal.
- **Gateway Path Sanitization** — `sanitize_path()` in `clarity-gateway` restricts all file API access to the current working directory prefix after `canonicalize()`.

### Added

- **Crate Documentation** — README + `AGENTS.md` for all 6 crates.
- **Gateway Handler Tests** — 8 mock integration tests covering `health_check`, `file_tree`, `file_read`, `admin_tools`, `admin_get_approval_mode`, `admin_set_approval_mode`.
- **Claw & TUI Lib Split** — Binary crates now expose testable `lib.rs` modules with 6 tests each.
- **CI Hardening** — `cargo audit` and `cargo tarpaulin` jobs added to GitHub Actions.
- **Hybrid Store Tests** — 5 new unit tests for `HybridStore` cache behavior; 2 previously-ignored integration tests re-enabled.

### Fixed

- **Clippy Clean** — Zero warnings across the entire workspace (`-D warnings`).
- **File Cleanup** — Removed untracked artifacts.

## [0.1.0] — 2026-04-21

### Added

- **Plan Mode** — LLM generates a structured JSON plan (`Plan` / `PlanStep`) before execution. `Agent::run()` in Plan mode bypasses the standard ReAct loop and executes steps in batch. TUI commands `/plan` and `/execute`.
- **Parallel Subagents** — `Agent::run_parallel()` executes multiple `RunSpec` concurrently via `BackgroundTaskManager`. TUI `/parallel` command, Gateway `POST /v1/parallel`, and Web UI parallel execution panel.
- **Background Tasks** — `BackgroundTaskManager` with agent scheduler loop, priority queue, and persistent store. Gateway `POST/GET/DELETE /v1/tasks`. claw system-tray app monitors `.clarity/tasks/` in real-time via `notify` + OS notifications.
- **MCP SSE Transport** — Full MCP-over-SSE protocol implementation with endpoint discovery, relative URL resolution, and reconnect loop. `McpManager::from_config()` now honors `transport: "stdio" | "http" | "sse"` from `mcp.json`.
- **Web UI Task Panel** — Click the task badge to open a task list modal with refresh, cancel, and detail view.
- **TUI Parallel Results** — Polished boxed layout with aligned columns for success/failure/aggregation.
- **E2E Validation** — Verified end-to-end with kimi-code: chat completions, background tasks, and parallel subagents all pass.

### Fixed

- Plan mode no longer triggers per-tool interactive approval in `execution.rs` (plan-level vetting happens in `run()`).
- `cancel_on_error` in parallel execution now actually cancels remaining tasks via `task_manager.cancel()`.

## [0.0.9] — 2026-04-20

### Added

- **Skill System** — Markdown+YAML orchestration layer. `SkillLoader` parses frontmatter + body. `SkillRegistry` with keyword search. `Agent` injects skill context into system prompt and enforces tool whitelists. TUI `/skill list` and `/skill use <id>`.
- **BM25 + Hybrid Search** — `SqliteStore::search_similar` uses FTS5 recall + in-memory BM25 re-ranking. Replaces O(n) full-table scan.
- **RAG Chunking** — `Chunker` with configurable size, overlap, separator, and source tracking.
- **Gateway Session Store** — SQLite-backed session persistence (CRUD, message append, request counting, expiration cleanup).
- **SSE Parsing Extraction** — Dedicated `SseParser` in `llm/sse.rs` for OpenAI-style streaming deltas, reasoning_content, and tool call deltas.

### Fixed

- Cyclic dependency decoupling: `ToolCall`/`FunctionCall` → `types.rs`; `Message`/`LlmProvider` → `llm/api.rs`.
- `run()` / `run_with_messages_sync()` duplication eliminated via `Agent::run_sync_loop()`.
- Gateway SSE now forwards `tool_calls` deltas as structured events (`ToolCallStart`, `ToolResult`, `StepBegin`).

## [0.0.8] — 2026-04-17

### Added

- **Security Hardening** — `validate_mcp_command()` rejects shell metacharacters, relative paths, and non-existent absolute paths before spawning MCP stdio servers. Override via `CLARITY_MCP_ALLOWLIST`.
- **MCP Auto-loading** — Gateway startup loads `~/.config/clarity/mcp.json` automatically.
- **Stream-first LLM Architecture** — `Agent::run_streaming()` calls `llm.stream()` first, falls back to `complete()` only when streaming is unavailable.

### Fixed

- Tool-calling pipeline in Gateway chat: `get_tool_descriptions()` parses `ToolRegistry::get_tool_schemas()` correctly. `OpenAiCompatibleLlm` forwards `tool_calls` and `tool_call_id`. Kimi `thinking` mode disabled to prevent 400 on multi-round tool calls.
- `cargo audit` reports 0 vulnerabilities after updating `rustls-webpki`, `rand`, and 40+ transitive deps.
