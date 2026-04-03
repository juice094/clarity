# Changelog

All notable changes to Project Clarity will be documented in this file.

## [Unreleased] - 2026-04-04

### Added

- **Subagent Runner** (`crates/clarity-core/src/subagents/runner.rs`)
  - 完整子代理执行器实现，参考 Kimi CLI 架构
  - 支持前台执行和恢复执行模式
  - 执行上下文管理（持久化对话历史）
  - Git 上下文自动收集（分支、提交、未提交更改）
  - 输出收集和摘要生成
  - 完整的错误处理（MaxStepsReached、Cancelled 等）
  - 18 个单元测试覆盖核心逻辑
  - API 设计参考 `std::process::Command` 构建器模式

- **SubagentManager** (`crates/clarity-core/src/subagents/mod.rs`)
  - 高级接口整合所有子代理功能
  - 简化子代理生命周期管理

### Documentation

- **Updated README.md**: 根据实际代码状态更新
  - 修正代码规模为 ~645 KB（68 个 Rust 文件）
  - 更新测试数量为 180+ 个
  - 明确标注 PersistentMemoryStore 为占位符实现
  - 更新子代理 Runner 状态为 ✅ 已实现
  - 更新功能完成度矩阵

- **Updated PROJECT_REPORT.md**: 技术验证报告 v3.0
  - 更新所有统计数据以匹配实际代码
  - 补充 clarity-wire 和审批/压缩功能的测试状态
  - 修正已知限制列表
  - 更新后续推进路线

- **Added KIMI_CLI_COMPARISON.md**: 横向对比分析
  - 与 Kimi CLI 的详细功能对比
  - 参考价值和实现建议

### Code Status (Verified)

| 指标 | 数值 | 验证命令 |
|------|------|----------|
| 编译 | ✅ | `cargo check --workspace` |
| 测试 | 180+ passed | `cargo test --workspace --lib` |
| 警告 | 3 | `cargo clippy --workspace` |
| 代码规模 | ~650 KB | PowerShell 统计 |

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
