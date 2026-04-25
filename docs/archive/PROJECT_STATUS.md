# Clarity 项目状态报告

> 生成时间：2026-04-09（2026-04-15 诚实化更新）  
> 分支：main  
> 最近提交：Phase 4B 完成 + AgentController TUI 集成 + Phase 1 止血修复

> **2026-04-15 更新说明**：本次更新修正了此前文档中对构建状态和部分功能完成度的夸大描述，并完成了 6 项 P0 止血修复。详见 `docs/REALITY_CHECK_AND_ROADMAP_2026-04-15.md`。

---

## 1. 构建健康度

| 指标 | 状态 | 备注 |
|------|------|------|
| `cargo check --workspace` | ✅ 通过 | 全 crate 编译无错误 |
| `cargo test --workspace --lib` | ✅ 334 passed | 3 ignored（需网络/外部依赖） |
| `cargo test --workspace --examples` | ✅ 全绿 | 示例编译通过 |
| `cargo test --workspace` | ⚠️ 部分 timeout | 个别 MCP 集成测试依赖外部 `npx` 进程，可能挂起 |
| `cargo clippy --workspace` | ✅ 零警告 | 包含新引入的代码 |
| `cargo test --workspace --doc` | ✅ 全绿 | Doc-test 全部修复 |

---

## 2. 已完成功能清单

### Core — `clarity-core`
- [x] **AgentController** (`src/agent/controller.rs`)：Codex 风格事件驱动调度器，支持 `UserTurn` / `Interrupt` / `ToolApproval` / `Compact` / `Shutdown`。
  - *2026-04-15 修复*：移除了会 `panic!` 的公开 `sender()` API 陷阱。
- [x] **Agent 取消机制**：`CancellationToken` 贯穿 `run()` 与 `run_streaming()`；支持 `cancel()` 与 `reset_cancel_token()` 实现多轮取消后再生成。
- [x] **CompactionService** (`src/agent/compaction_service.rs`)： proactive 上下文压缩，按 token 阈值触发 LLM 摘要。
- [x] **ApproveForSession** (`src/approval.rs`)：交互模式下首次审批后，同 session 后续调用自动放行。
- [x] **MCP 集成** (`src/mcp/`)：支持 `mcp.json` 配置解析、stdio 启动、失败降级；示例文件已更新至新 API。
- [x] **BackgroundTaskManager** (`src/background/`)：16 个测试通过，支持优先级、调度、Worker Pool。
- [x] **Subagents** (`src/subagents/`)：LaborMarket、Store、Builder、Runner、ParallelExecutor 全链路可用。
  - *2026-04-15 修复*：修正了 `SubagentRunner::clone()` 会静默清空 `labor_market` 的数据丢失 bug。
- [x] **工具增强**：文件敏感检测、媒体嗅探（PNG/PDF）、PowerShellTool、Diff 预览内嵌 (`_diff_preview`)。

### TUI — `clarity-tui`
- [x] **AgentController 集成**：`App` 通过 `controller_tx` 发送 `Op::UserTurn` 与 `Op::Interrupt`，ESC / Ctrl+C 真正取消后台生成任务。
- [x] **Popup 系统**：`HelpPopup`、`ToolResultPopup`、`DiffPopup`（红绿 hunk 渲染）。
- [x] **AsyncSingleJob**：gitui 风格非阻塞后台任务。
- [x] **CommandBar**：底部快捷键提示栏。
- [x] **Wire 适配器**：`clarity-wire` 消息流自动转为 TUI `Event`，支持流式内容更新、工具调用弹窗。

### Gateway — `clarity-gateway`
- [x] **WebSocket 支持**：`/ws` 端点可建立长连接，与 `clarity-wire` 双向转发。
- [x] **HTTP API**：`/v1/chat/completions`、admin stats/tools、health check。
- [x] **频道集成**：Telegram、Discord、Webhook（钉钉/飞书）。
- [x] **Session 管理**：内存级 session 与会话消息追踪。

### Memory — `clarity-memory`
- [x] **持久化存储**：SQLite / JSONL / Hybrid Backend。
- [x] **MemoryTicker**：周期性触发记忆总结与归档。
  - *限制*：当前触发后仅打印日志，实际的记忆整合/归档动作尚未接入（P1）。
- [x] **向量化**：TF-IDF / Cosine Similarity（`sqlite-vec` 语义检索为 roadmap 项）。

---

## 3. 关键文件变更（本次提交范围）

```
M  Cargo.toml / Cargo.lock
M  README.md / CHANGELOG.md / PROJECT_REPORT.md
M  crates/clarity-core/src/agent/mod.rs
M  crates/clarity-core/src/approval.rs
M  crates/clarity-core/src/error.rs
M  crates/clarity-core/src/tools/file.rs
M  crates/clarity-core/src/tools/shell.rs
M  crates/clarity-core/src/tools/web.rs
M  crates/clarity-core/examples/mcp_demo.rs
M  crates/clarity-core/examples/mcp_filesystem_demo.rs
A  crates/clarity-core/src/agent/compaction_service.rs
A  crates/clarity-core/src/agent/controller.rs
A  crates/clarity-core/src/agent/ops.rs
A  crates/clarity-core/src/background/
A  crates/clarity-core/src/mcp/
A  crates/clarity-core/src/notifications/
A  crates/clarity-core/src/skill/
A  crates/clarity-core/tests/mcp_*.rs
M  crates/clarity-tui/src/app.rs
M  crates/clarity-tui/src/main.rs
M  crates/clarity-tui/src/ui.rs
A  crates/clarity-tui/src/async_job.rs
A  crates/clarity-tui/src/command_bar.rs
A  crates/clarity-tui/src/diff.rs
A  crates/clarity-tui/src/popup.rs
A  crates/clarity-tui/src/popups/
A  crates/clarity-tui/src/wire_adapter.rs
M  crates/clarity-gateway/src/ws.rs
A  crates/clarity-gateway/tests/
M  crates/clarity-memory/src/store.rs
M  crates/clarity-wire/src/lib.rs
A  docs/PHASE_REPORT_2026-04-09.md
A  docs/THIRD_PARTY_INTEGRATION_ROADMAP.md
A  IMPLEMENTATION_SUMMARY.md
A  tests/integration/...
```

---

## 4. 如何运行

### 4.1 TUI（用户交互页面）

```bash
# 设置任一 LLM API Key
$env:ANTHROPIC_API_KEY="sk-..."
# 或 $env:KIMI_API_KEY="..."
# 或 $env:DEEPSEEK_API_KEY="..."
# 或 $env:OPENAI_API_KEY="..."

cargo run --bin clarity-tui
```

**默认快捷键：**
- `i` / `Enter` — 进入输入模式
- `Esc` — 退出输入模式 / 关闭弹窗
- `Ctrl+C` — 停止生成（若正在生成）
- `/stop` — 停止生成
- `?` — 打开帮助弹窗
- `q` — 退出程序（Normal 模式下）
- `j/k` 或 `↑/↓` — 滚动聊天记录

### 4.2 Gateway（服务端）

```bash
cargo run --bin clarity-gateway
```

默认监听 `0.0.0.0:3000`：
- WebSocket: `ws://localhost:3000/ws`
- Chat API: `POST /v1/chat/completions`
- Health: `GET /health`

---

## 5. 待测试与反馈清单

以下是本次新增/改动较大、最需要你手动验证的功能点。请在本地运行后逐项勾选并反馈：

### 5.1 TUI 交互（高优先级）
- [ ] **启动测试**：配置 API Key 后，`cargo run --bin clarity-tui` 能否正常启动并显示欢迎语？
- [ ] **输入与生成**：输入任意问题后按 Enter，是否能收到流式回复？
- [ ] **取消生成**：在生成过程中按 `Esc` 或 `Ctrl+C` 或输入 `/stop`，生成是否立即停止？停止后能否立即发起新一轮对话？
- [ ] **Diff 弹窗**：让 Agent 修改一个文件（例如 `"请写一个 hello.txt 并写入 Hello World"`），TUI 是否会弹出 `DiffPopup` 显示红绿差异？能否用 `↑/↓` 滚动并 `q/Esc` 关闭？
- [ ] **Help 弹窗**：Normal 模式下按 `?`，是否弹出帮助？按 `Esc/q` 能否关闭？
- [ ] **多轮对话**：连续进行 3-4 轮对话，确认上下文没有丢失或错乱。

### 5.2 工具与审批（高优先级）
- [ ] **敏感文件审批**：在非 Yolo 模式下请求读取 `~/.ssh/config` 或类似敏感路径，TUI 是否会弹出审批提示？（目前 TUI 的审批机制通过 Wire 的 `ApprovalRequired` 消息实现，需确认弹窗或系统消息正常。）
- [ ] **ApproveForSession**：首次审批时若选择 "Approve for this session"，同一会话的第二次同类请求是否不再弹审批？
- [ ] **Yolo 模式**：若从代码层面设置 `ApprovalMode::Yolo`，工具调用是否完全自动执行？

### 5.3 Gateway & Wire（中优先级）
- [ ] **Gateway WebSocket**：启动 Gateway 后，用任意 WebSocket 客户端连接 `ws://localhost:3000/ws`，发送聊天消息后能否收到流式响应？
- [ ] **Gateway 会话历史**：发送多轮消息后，调用相关 API 查看历史记录是否完整？

### 5.4 MCP（中优先级）
- [ ] **MCP Filesystem Demo**：确保已安装 `npx`，运行 `cargo run --example mcp_filesystem_demo -- "C:\Users\YourName"`，是否正常连接 MCP 服务器并列出工具？

### 5.5 稳定性（低优先级 / 长期观察）
- [ ] **长时间运行**：让 TUI 保持运行并进行 10+ 轮对话，观察是否出现 panic 或内存泄漏。
- [ ] **Gateway 压力**：并发 5-10 个 WebSocket 连接，观察 Gateway 是否稳定。

---

## 6. 已知限制 & Roadmap

| 限制 | 说明 | 预计优先级 |
|------|------|-----------|
| MCP SSE Transport | `SseMcpClientStub` 是占位实现，使用 HTTP POST 而非真正的 SSE 长连接 | P1 |
| BackgroundTaskManager 真实 Agent 任务 | 当前仅支持 Bash/占位任务，缺少真正的 `AgentTask` | P1 |
| AgentController 流式输出 | `UserTurn` 通过 Controller 执行时丢弃流式输出，TUI 与 Gateway 路径尚未统一 | P1 |
| MemoryTicker 实际动作 | 触发后仅打印日志，未执行记忆整合/归档 | P1 |
| Gateway Session 持久化 | Session 仅存内存，重启后丢失 | P2 |
| Vector Search (sqlite-vec) | `search_similar` 使用 TF-IDF，非语义向量检索 | P2 |
| Slack 频道集成 | Roadmap 中标记为 P0，尚未实现 | P2 |
| 统一配置系统 | TUI/Gateway 仍依赖环境变量，缺少 TOML 配置层 | P3 |

---

## 7. 快速反馈模板

如果你发现任何问题，请按以下格式回复：

```markdown
### 问题摘要
一句话描述问题。

### 复现步骤
1. ...
2. ...

### 期望行为
...

### 实际行为
...

### 环境信息
- API Key 类型: (ANTHROPIC / KIMI / DEEPSEEK / OPENAI)
- 操作系统: Windows 11 / macOS / Linux
- Rust 版本: `rustc --version`
```
