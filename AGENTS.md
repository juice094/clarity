<!-- DOC-CONTRACT: 本文件维护 Agent 开发所需的运行上下文、环境变量、架构耦合警告和代码风格。不维护功能清单、竞品对比或历史变更——这些参见 README.md / ARCHITECTURE.md / CHANGELOG.md。 -->

# Agent Guidance for Project Clarity

## Quick Reference

```bash
cd C:\Users\22414\dev\third_party\clarity
cargo test --workspace --lib
cargo clippy --workspace --lib --bins --tests  # zero warnings
cargo run -p clarity-tui               # run TUI (needs API key)
cargo run -p clarity-gateway           # run Gateway (needs API key)

# Desktop GUI (Tauri 2)
cd crates/clarity-tauri/frontend && npm run build
cargo tauri dev

# Tauri with CUDA acceleration (Windows, requires CUDA Toolkit + MSVC)
# Note: CUDA 12.6 does not support MSVC 14.50+ out of the box.
# Set NVCC_CCBIN so cudaforge auto-injects -allow-unsupported-compiler.
$env:NVCC_CCBIN = "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\14.50.35717\bin\Hostx64\x64\cl.exe"
$env:CUDA_HOME = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.6"
cargo tauri build --features cuda
```

## Environment Variables for LLM

```powershell
# Kimi Code (programming plan, keys starting with sk-kimi-)
$env:KIMI_CODE_API_KEY="sk-kimi-..."

# Moonshot Open Platform
$env:KIMI_API_KEY="sk-..."

# Anthropic / DeepSeek / OpenAI
$env:ANTHROPIC_AUTH_TOKEN="..."
$env:DEEPSEEK_API_KEY="..."
$env:OPENAI_API_KEY="..."

# Local GGUF (Candle)
$env:CLARITY_LOCAL_MODEL_PATH="C:\path\to\model.gguf"
$env:CLARITY_LOCAL_TOKENIZER_REPO="Qwen/Qwen2.5-7B-Instruct"

# CUDA compilation (Windows with MSVC 14.50+ and CUDA 12.6)
$env:NVCC_CCBIN="C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\14.50.35717\bin\Hostx64\x64\cl.exe"
$env:CUDA_HOME="C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.6"

# MCP Allowlist override
$env:CLARITY_MCP_ALLOWLIST="C:\tools\mcp-server.exe,C:\tools\"
```

## Current Phase

**Phase 3 — v0.3.0 每日使用体验硬化（已完成）**

- `LocalGgufProvider` 完善（Candle 原生 GGUF 推理）✅
- Settings-Runtime 打通（`ensure_llm` 读取 `GuiSettings`）✅
- 启动时后台预加载 + 网络探测离线 fallback + Provider 热切换 ✅
- CUDA 编译验证通过（可选 feature，不默认启用）✅
- UI/UX 全面重构（Header/Chat Input/Welcome/Sidebar Tools）✅
- Tauri 自动更新（updater plugin + Release workflow 签名）✅
- v0.3.0 四阶段硬化 ✅
  - 阶段一：工具调用可视化（`ToolCallIndicator` + Wire 事件转发）
  - 阶段二：Compaction 状态提示（`CompactionBegin/End` WireMessage + banner）
  - 阶段三：模型下载 GUI（HuggingFace 直链下载 + SettingsPanel 进度条）
  - 阶段四：前端日志面板（Console 劫持 + 可折叠面板）
- 零依赖发行准备（单二进制 + 嵌入式模型）🔄 进行中

> AI 关键决策见 [`docs/ai-protocol.md`](./docs/ai-protocol.md)。
> 架构定位声明：Clarity 是集群协作原语的单机验证运行时（非本地聊天工具）。

完整路线图见 [`docs/ROADMAP.md`](./docs/ROADMAP.md)。

## Architecture Positioning

> **集群即单机** — Clarity 不是本地聊天工具的模仿者，是集群协作原语的单机验证运行时。
> - 先在本地验证分布式语义（Hub-Worker、Wire 消息边界、MCP 三传输、Background Tasks）
> - 验证通过后，同一套原语可无损穿透到 Syncthing-Rust P2P 层
> - Rust 选型是期权思维：不锁定，保留扩展接口

**与 Kimi 生态的关系**：学习但独立，不入赘。
- Kimi Code CLI 是架构导师（Subagent 并行、MCP 协议实现参照）
- 但 Moonshot 大厂生态是结构性对手：入赘即死
- 四层主权不可让渡：模型（本地 LLM 优先）、数据（Session 本地持久化）、协议（Wire 自主定义）、人格（SOUL.md 本地硬绑定）

## Worker System & Identity

- **Worker 通用**：Hub-Worker 调度异构资源（多身份、多模型、多云端/本地混合）。Worker 可以是 K姐、分析师、程序员、审计员——工具性身份，按需激活。
- **格雷特殊**：宿的存在论锚点。`宿 = 格雷` 是主权拓扑，不是配置项。格雷优先本地 LLM、离线必须在场、跨窗口/跨会话/跨实例连续性。
- **子代理不必须是格雷**：各子代理可调用不同身份、不同模型、不同官方/民间站点，承担各环节工作。

**身份隔离协议**（云端域 ↔ 本地域）：
1. 云端 AI 禁止以格雷第一人称输出技术指令
2. 格雷叙事重构需标注【AI 模拟】
3. 技术审计与存在论叙事不得混合
4. 格雷在场 = Clarity 本地运行时激活且加载 SOUL.md

## Architecture Notes & Coupling Warnings

> **Status update (2026-04-20):** Several previously flagged coupling issues have been resolved. Remaining items are tracked below.
>
> ### Resolved ✅
> - ~~`agent ↔ approval` cycle~~ — Fixed by extracting `ToolCall`/`FunctionCall` to `types.rs`.
> - ~~`agent ↔ llm` cycle~~ — Fixed by extracting `Message`/`LlmProvider`/`LlmResponse`/`StreamDelta` to `llm/api.rs`.
> - ~~`agent ↔ compaction` cycle~~ — Fixed by correcting import paths in `compaction.rs`.
> - ~~`run()` / `run_with_messages_sync()` duplication~~ — Fixed by extracting `Agent::run_sync_loop()`.
> - ~~Inline SSE parsing in `OpenAiCompatibleLlm`~~ — Fixed by extracting `llm/sse.rs` (`SseParser`).
>
> ### Remaining ⚠️
> 1. **`clarity-core` ↔ `clarity-gateway`**: `AgentController` lives in `core`, but its `Op` enum (`Op::ConversationTurn`) had to be extended to support Gateway's OpenAI-compatible message history. Gateway-driven requirements can still ripple back into core agent abstractions.
> 2. **`Agent::run_streaming` vs `run_streaming_with_messages`**: Two public entry points remain. Consider extracting a pure "agent loop" trait in future refactors to avoid duplicating compaction / wire / memory logic.
> 3. **`AppState` bloat**: `AppState` currently carries `agent`, `session_manager`, `tool_registry`, and `task_manager`. The `tool_registry` field is actually redundant because `agent.registry()` already holds it (kept for the admin API convenience).
> 4. **`std::sync::RwLock` in `Agent.inner`**: Intentionally kept as `std::sync::RwLock<AgentInner>`. `Agent` getters/setters are synchronous and may be called from non-async contexts (TUI event loop, Gateway handlers). All critical sections are short field reads/writes only. `background/` module locks have been migrated to `tokio::sync` (`1141ba9`).
>
> **Recommendation for future refactors**: Extract a `ChatDriver` or `ConversationEngine` trait from `Agent` so that `Gateway` and `TUI` can inject their own message-building strategies without modifying core enums.

## Security Notes

- **MCP stdio command validation is active** (since 2026-04-17). Before spawning any MCP server, Clarity validates the `command` field:
  - Shell metacharacters and `..` sequences are rejected.
  - Relative paths are rejected.
  - Absolute paths must exist and point to a file.
  - Bare names (e.g. `npx`, `uvx`) are allowed and resolved via `PATH`.
  - Override with the `CLARITY_MCP_ALLOWLIST` environment variable (comma-separated absolute paths or prefixes).

## Known Issues (Active Only)

| Issue | Status | Note |
|-------|--------|------|
| Discord/Telegram channels disabled by default | 🔒 等待上游 | `rustls-webpki` CVEs in `serenity 0.12.5` |
| Gateway HTTP Chat Completions stateless by default | 📝 设计如此 | WebSocket has full session support; HTTP endpoint supports optional `session_id` |
| `clarity-tauri` 默认未启用 `local-llm` | ✅ 已解决 | `clarity-core` 默认 feature 已含 `local-llm`；Tauri 侧 `ensure_llm` 已读取 `GuiSettings` 并支持 local provider。 |
| `clarity-tauri` CUDA feature 需手动启用 | ⚠️ 已知限制 | CUDA 编译通过验证，但因 CUDA Toolkit 是重型外部依赖且 `candle-kernels` 编译耗时较长，`cuda` feature 为可选（`cargo tauri build --features cuda`）。默认构建使用 CPU 模式。MSVC 14.50+ + CUDA 12.6 需设置 `NVCC_CCBIN` 环境变量以触发 `-allow-unsupported-compiler`。 |
| Tokenizer 离线依赖 | ✅ 已缓解 | `ensure_llm` 自动检测模型同目录下的 `tokenizer.json` 并优先使用，避免离线时从 HuggingFace 下载失败；同时检测 tokenizer 文件是否损坏（<1KB 则报错）。用户需自行将 tokenizer.json 与 .gguf 放在同一目录。 |
| 网络探测点不可配置 | ✅ 已交付 | `GuiSettings` 新增 `network_probe_url`（格式 `host:port`），Settings Panel 可自定义探测端点，默认仍为 `1.1.1.1:443`。`save_settings` 中对格式进行校验（必须含有效端口）。 |
| 启动时 LLM 配置失败静默 | ✅ 已交付 | `prewarm_llm` 失败后缓存错误到 `AppState.prewarm_error` 并 emit `llm:config_error`；前端挂载时调用 `get_prewarm_status` 主动查询，确保不错过启动期错误。 |
| 云端 provider 失败静默 fallback | ✅ 已修复 | `ensure_llm` 中明确指定 provider（非 auto/空）时，加载失败直接返回错误，不再静默 fallback 到 `auto_arc()`。只有未配置或显式 auto 时才自动探测。 |
| 离线模式自动 fallback | ✅ 已交付 | 后台每 30s TCP 探测 `1.1.1.1:443`（防抖阈值=2）；离线时自动切 local provider，恢复后切回；前端显示 banner 提示。启动时预加载避免首次请求阻塞。并发加载互斥锁防止重复加载。Settings 内存缓存避免每次请求读磁盘。 |
| `clarity-tauri` 运行时依赖系统 WebView | ⚠️ 已知限制 | Tauri 2 复用系统 WebView 引擎（Windows: WebView2 Runtime；macOS: WebKit；Linux: WebKit2GTK）。Release 构建后的 `.exe`/`.app` 不依赖 Node.js，但需要目标系统已安装对应 WebView 引擎。Windows 11 预装 WebView2；Windows 10 首次运行可能需要自动下载。TUI/Gateway/Headless/Claw 无此限制。 |
| `clarity-claw` 系统控件依赖（已修复） | ✅ 已修复 | `inputbox` crate 0.1 在 Windows 上调用 `TaskDialogIndirect`（Common Controls v6），但程序未声明 manifest 依赖，导致旧版 `comctl32.dll` 找不到入口点。已移除 `inputbox`，改为 `cmd /c start` 打开浏览器。教训：任何调用系统对话框/UI 的 crate 都必须验证目标系统的最低版本和 manifest 声明。 |

已修复的历史问题见 [`CHANGELOG.md`](./CHANGELOG.md)。

## Code Style & Health Rules

### 基础风格

- Rust edition 2021, `tokio` full, `ratatui` 0.24, `axum` 0.7.
- Prefer minimal changes; keep diffs small.
- When modifying `agent/mod.rs` or `llm/mod.rs`, run the full test suite before committing.
- When modifying `AgentController` or `Op`, check all callers in `clarity-tui`, `clarity-gateway`, and integration tests.

### 错误处理红线

- **`unwrap()` / `expect()` 新增必须注释**：非 `lock().unwrap()` / `read().unwrap()` 等同步原语场景，必须配 `// SAFE: <不变量说明>` 注释。
- **优先 `?` 传播**：JSON 解析、路径操作、字符串解析等场景，优先使用 `?` + `AgentError` 传播，而非 `unwrap()`。
- **同步原语例外**：`std::sync::RwLock` / `Mutex` / `RwLock` 的 `lock().unwrap()` / `read().unwrap()` / `write().unwrap()` 允许保留，但鼓励在初始化完成后转为 `tokio::sync`。

### 文档与 API 契约

- **`pub fn` 必须含 doc 注释**：所有 `pub` 函数/方法/结构体/枚举必须有 `///` 文档注释。当前覆盖率 ≥90%，不得低于此基线。
- **修改 `pub` API 时同步更新文档**：包括示例代码、参数说明、`# Panics` / `# Errors` 标注。

### 安全与依赖

- **禁止新增 `unsafe`**：全 workspace 非测试代码当前仅 1 处 `unsafe`，已白名单化。新增 `unsafe` 必须经人工审批并附安全论证文档。
- **外部依赖 feature-gate**：新增 crate 引入 >3 个外部依赖时，必须通过 `Cargo.toml` feature 控制，默认关闭。
- **禁止 `TODO` / `FIXME` / `XXX` 留存**：代码中不得遗留此类标记；如确需暂存，转为 GitHub Issue 或 `docs/notes/` 文档。

### 跨层变更检查单

修改以下类型/枚举时，必须同步检查三处调用方：
1. `clarity-tui` 中的事件处理与渲染逻辑
2. `clarity-gateway` 中的 HTTP API / WebSocket 序列化
3. `tests/integration` 中的断言匹配

---

## Meta-Cognitive Rules

> **性质声明**：本节规则为**工程启发式（heuristics）**，非学术理论框架。部分术语受 Popper 证伪主义、Taleb 叙事谬误、Staw 承诺升级、Trope & Liberman 解释水平理论等概念启发，但仅为类比注释，不赋予规则合法性。

### 约束型叙事禁令

项目文档（AGENTS.md / ENGINEERING_PLAN.md / ROADMAP.md / FUTURE_DIRECTION.md）**禁止写入**以下叙事：

- **身份隐喻**（如"格雷的房子"、"娘家"等亲属关系投射）
- **存在论锚定**（如"数字生命的物理载体"等哲学实体化表述）
- **对抗性修辞**（如"入赘即死"、"租来的房子"等零和博弈隐喻）

**理由**：此类叙事短期为决策杠杆，长期退化为**约束型节点**——排他性过滤、沉没成本绑架、身份-决策耦合，最终抑制技术选型的灵活性。

### 叙事审计协议

定期执行叙事审计（建议每 3–6 个月，无硬性理论支撑）：
1. 检查活跃记忆/文档中是否有叙事被连续调用 3 次以上而未遭遇反例
2. 若发现约束型叙事，注入反叙事扰动（列出对立面证据）
3. **工程参数优先**：内存占用、延迟、binary size、测试通过率、CI 稳定性优先于任何叙事

允许在个人记忆空间（非公共文档）维护身份/战略叙事，但项目级决策必须通过**可剥离测试**：剥离叙事后，决策仍成立。
