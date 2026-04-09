# Changelog

All notable changes to Project Clarity will be documented in this file.

## [Unreleased] - 2026-04-09

### Added

- **`clarity-memory` 集成到 `clarity-core`**
  - `PersistentMemoryStore` 从占位符升级为真实实现
  - 底层使用 `clarity_memory::MemoryStore`（SQLite + FTS5）
  - 支持 `store`、`retrieve`、`search`、`get_all`、`clear`、`count` 全生命周期操作
  - 集成测试已覆盖记忆持久化链路

- **Gateway WebSocket 集成**
  - Gateway 新增 `/ws` WebSocket endpoint，支持实时流式消息推送
  - 实现 Agent Wire 与 WebSocket 的双向桥接
  - 新增 3 个 WebSocket 集成测试全部通过

- **TUI Wire 适配器**
  - TUI 应用层完成 Wire 绑定：`Agent::with_wire(Arc<Wire>)`
  - 实现 WireMessage → TUI Event 的实时适配与渲染
  - 支持行级滚动，优化长消息的可读性

- **后台任务系统骨架**
  - 新增 `BackgroundTaskManager`、`TaskScheduler`、`WorkerPool`
  - 支持任务优先级队列、并发控制（信号量）、状态通知
  - 任务状态机：`Pending → Running → Completed / Failed / Cancelled`
  - 待完成：Gateway/TUI 集成活化、Wire File 持久化通信

### Changed

- **全工作区 clippy 清理**
  - 修复所有 crate 的编译警告
  - 消除大量未使用导入、冗余生命周期、不必要的可变绑定
  - 当前剩余 3 个警告（均为未使用变量/可变修饰符，已在计划中修复）

### Testing

- **跨模块集成测试扩充**
  - 新增 `core_wire` 集成测试（2 个）
  - 新增 `gateway_http` 集成测试（4 个）
  - 新增 `memory_persistence` 集成测试（1 个）
  - 全工作区测试总数达到 **~380+**，全部通过

### Documentation

- **Updated KIMI_CLI_COMPARISON.md**
  - 更新模块对比矩阵：PersistentMemoryStore、Gateway WebSocket、记忆系统标记为 ✅ 已完成
  - 更新立即行动项为当前计划（BackgroundTaskManager、MCP `mcp.json`、Git 上下文、工具安全增强）
  - 新增 Tier S/A/B/C 参照性矩阵

- **Added THIRD_PARTY_INTEGRATION_ROADMAP.md**
  - 第三方项目关系与集成路线图
  - 短期/中期/长期规划与决策日志

- **Updated README.md**
  - 测试数量更新为 ~380+
  - 项目状态表更新（clarity-memory ✅、Gateway WebSocket ✅、BackgroundTaskManager 🔄、MCP config 🔄）
  - 已知限制更新，移除已修复项

- **Updated docs/README.md**
  - 新增 `THIRD_PARTY_INTEGRATION_ROADMAP.md` 索引
  - 更新项目状态速览

### Code Status (Verified)

| 指标 | 数值 | 验证命令 |
|------|------|----------|
| 编译 | ✅ | `cargo check --workspace` |
| 测试 | ~380+ passed | `cargo test --workspace --lib --tests` |
| 警告 | 3 | `cargo clippy --workspace` |
| 代码规模 | ~750 KB，91 个 Rust 文件 | PowerShell 统计 |

## [0.1.1] - 2026-04-03

### Added

#### LLM Provider System
- **Anthropic Protocol Support**: Added `AnthropicLlm` provider for Claude Code compatibility
  - Supports `/v1/messages` endpoint
  - Compatible with Kimi Code (`https://api.kimi.com/coding`)
  - Uses `x-api-key` authentication header
  - Environment variables: `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `ANTHROPIC_MODEL`
  
- **Auto-Detection**: Added `LlmFactory::auto()` for automatic provider detection
  - Priority: ANTHROPIC → KIMI → DEEPSEEK → OPENAI
  - Returns descriptive error if no provider configured

#### Bug Fixes
- **TUI Unicode Support**: Fixed multi-byte UTF-8 character handling in input pane
  - Fixed `insert_char()` to use byte index conversion
  - Fixed `delete_char()` to handle character boundaries
  - Fixed `move_cursor_right()` to use character count
  - Fixed `delete_char_forward()` to handle UTF-8 correctly
  - Fixed `render()` to calculate display width correctly

- **TUI Input Repeat**: Fixed duplicate character input
  - Only processes `KeyEventKind::Press`, ignores `KeyRepeat`
  - Fixed `End` key to use character count instead of byte length

- **Streaming Fallback**: When streaming not supported, falls back to `complete()` with simulated streaming
  - Character-by-character output for smooth display
  - Optional delay for visual effect

### Changed

- **Kimi Code Integration**: Updated to work with Kimi Code API
  - User-Agent header: `claude-code/0.1.0 (Claude Code)`
  - Base URL handling: automatically adds `/v1` if missing
  
- **HTTP Client**: Added proper headers for Kimi Code compatibility
  - `Authorization: Bearer <token>`
  - `User-Agent: claude-code/0.1.0 (Claude Code)`
  - `Content-Type: application/json`

## [0.1.0] - 2026-04-03

### Added

#### Core Framework
- **Agent System**: ReAct loop implementation with tool execution
- **Tool Registry**: 8 built-in tools
  - `file_read` - Read file contents
  - `file_write` - Write to files
  - `file_edit` - String replacement in files
  - `glob` - File pattern matching
  - `grep` - Content search
  - `bash` - Shell command execution
  - `web_search` - DuckDuckGo search
  - `web_fetch` - Web page content extraction

#### LLM Providers
- **OpenAI Compatible**: Generic provider for OpenAI-compatible APIs
- **Kimi (Moonshot)**: Native Kimi API support
- **DeepSeek**: DeepSeek API support

#### Memory System
- **Persistent Storage**: SQLite-based memory store
- **Vector Search**: TF-IDF implementation
- **Memory Ticker**: Periodic memory consolidation

#### TUI Application
- **Interactive Terminal UI**: Ratatui-based interface
- **Real-time Streaming**: Live response display
- **Chat History**: Scrollable conversation view

#### Gateway Service
- **HTTP API**: OpenAI-compatible `/v1/chat/completions`
- **WebSocket**: Real-time streaming support
- **Admin UI**: Web interface on port 18800
- **Multi-channel**: Telegram, Discord, Webhook support (skeleton)

### Known Issues
- HybridStore tests have timeout issues (functionality works)
- MCP (Model Context Protocol) is skeleton implementation
- Some examples removed due to API changes

---

## Testing Status (Updated 2026-04-04)

| Component | Status | Notes |
|-----------|--------|-------|
| Compilation | ✅ Pass | `cargo check --workspace` |
| Unit Tests | ✅ Pass | 169 tests (core: 104, memory: 57, wire: 8) |
| TUI UTF-8 | ✅ Fixed | Chinese input working |
| Kimi Code API | ✅ Verified | Anthropic protocol |
| Streaming | ✅ Working | With fallback |
| Wire Communication | ✅ Pass | 8 tests |
| Approval System | ✅ Pass | Interactive/Yolo/Plan modes |
| Compaction | ✅ Pass | Context compression |

## Migration Guide

### From 0.1.0 to 0.1.1

No breaking changes. To use new Anthropic protocol:

```bash
# Instead of
export KIMI_API_KEY="sk-xxx"
export KIMI_BASE_URL="https://api.kimi.com/coding"

# Can now use (Claude Code compatible)
export ANTHROPIC_AUTH_TOKEN="sk-xxx"
export ANTHROPIC_BASE_URL="https://api.kimi.com/coding"
export ANTHROPIC_MODEL="kimi-k2-turbo-preview"
```

Both configurations work; the system will auto-detect.
