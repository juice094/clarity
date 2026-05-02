# Agent 空响应 Bug 修复与后续计划

> 日期：2026-05-02  
> 分支：`fix/agent-empty-response` → `phase2/protocol-pilot`  
> 基线 commit：`93a0214c`（Sprint 14.5 完成）  
> 修复 commit：`b74bc79f` + `d5d3fa36`

---

## 一、问题报告

用户通过 UI 截图反馈：Agent 返回空响应。测试基线（438 passed / 0 failed）全部通过，说明 bug 在集成/UI 路径，不在单元测试覆盖范围内。

---

## 二、根因分析（三处修复）

### 根因 1：Stream error 导致空响应（Critical）

**位置**：`crates/clarity-core/src/agent/run.rs:run_streaming_loop()`

**问题**：`llm.stream()` 中途报错时，旧代码执行：
1. `accumulated.clear()`
2. `tool_calls.clear()`
3. `break` 出 `while` 循环
4. **但仍设置** `turn_response = Some(LlmResponse { content: "", ... })`

这导致代码错误地把空响应当作有效的流式响应，跳过 `complete()` fallback，直接返回空字符串。

**修复**：引入 `stream_ok` 标志。仅当 stream 完整成功时才设置 `turn_response`。报错时 `turn_response` 保持 `None`，自然落入 `llm.complete()` fallback。

```rust
// 修复前（bug）
Err(e) => {
    accumulated.clear();
    tool_calls.clear();
    break;  // break 后仍执行 turn_response = Some(...)
}

// 修复后
Err(e) => {
    stream_ok = false;
    break;  // stream_ok=false 阻止 turn_response 设置
}
```

### 根因 2：Tool filter 缺失

**位置**：`crates/clarity-core/src/agent/run.rs:run_streaming_turn()`

**问题**：`run_streaming_turn()` 直接调用 `self.registry.get_tool_schemas()?`，而不是 `self.filter_tools_value(&self.registry.get_tool_schemas()?)`。当 skill 激活且有 tool whitelist 时，这会把全量工具描述发给 LLM，可能导致 LLM 忽略系统指令或输出异常内容。

**修复**：恢复 `filter_tools_value()` 调用。

**注意**：`run()` 和 `run_with_messages_sync()` 早已使用 `filter_tools_value()`，只有 Sprint 14.5 提取 `run_streaming_turn()` 时遗漏了这一点。

### 根因 3：`finish_turn()` 不执行

**位置**：`crates/clarity-core/src/agent/run.rs:run_streaming_turn()`

**问题**：
```rust
let (final_response, completed) = self.run_streaming_loop(...).await?;
self.finish_turn();  // ? 提前返回时不会执行
```

当 `run_streaming_loop()` 返回 Err（如 API 错误、取消、工具执行 fatal 错误），`?` 立即向上传播，`finish_turn()` 不会执行。Agent 状态卡在 `Running`，后续输入被 `begin_turn()` 的 "Agent is already running" 检查阻塞。

**修复**：
```rust
let loop_result = self.run_streaming_loop(...).await;
self.finish_turn();  // 无论成败都执行
let (final_response, completed) = loop_result?;
```

---

## 三、探索中发现但未修复的问题

### 发现 1：`run_streaming_with_messages()` 不调用 `refresh_context()`

**位置**：`run_streaming_with_messages()` → `run_streaming_turn()`

**问题**：`run_streaming()` 在调用 `run_streaming_turn()` 之前执行了 `self.refresh_context().await`，缓存 Git 上下文和项目元数据到 `AgentInner`。但 `run_streaming_with_messages()`（Gateway/ChatDriver 路径）直接调用 `run_streaming_turn()`，跳过了 `refresh_context()`。

**影响**：Gateway 驱动的 turn 使用 stale 的 Git 上下文和项目元数据。如果用户在 Gateway 会话期间切换了 Git 分支或修改了项目文件，Agent 的 System Prompt 不会反映这些变更。

**修复方案**：将 `refresh_context()` 移入 `run_streaming_turn()` 内部（而非仅在 `run_streaming()` 中调用）。这样所有 streaming 路径（egui、Gateway、TUI）统一获取最新上下文。

**纳入计划**：Context Convergence Phase 1（Sprint 15 优先项）。

### 发现 2：Stream 空内容无防御机制

**问题**：当 `llm.stream()` 成功打开但立即关闭（无任何 chunks），或 LLM 在最终 round 返回空内容时，`run_streaming_loop()` 会返回 `Ok(("", true))`。UI 端 `on_chunk` 从未被调用，不会创建任何 Agent 消息，但 `TurnEnd` 会触发 `on_done`，用户看到"Agent 没有回复"。

**当前状态**：已添加诊断日志（`warn!` 当 `final_response.is_empty()`），可在运行时识别。暂不添加强制防御（如返回默认错误消息），因为某些场景下 LLM 返回空内容是正常的（如纯工具调用 round 后无总结文本）。

**后续决策**：收集 1–2 周运行日志后，根据 `Empty final response` 的触发频率决定是否添加防御性回退（如 "The model returned an empty response, please retry."）。

### 发现 3：`SystemPromptBuilder` 未消耗 `GitContext` 和 `ProjectMetadata`

**位置**：`crates/clarity-core/src/agent/prompt.rs`

**问题**：`refresh_context()` 缓存了 `git_context` 和 `project_metadata` 到 `AgentInner`，但 `SystemPromptBuilder` 的 `build_system_prompt()` 没有消耗这两个组件。它们被标记为 `dead_code`。

**修复方案**：在 `SystemPromptBuilder` 中添加 `GitContext` 和 `ProjectMetadata` 组件，使主 Agent 的 System Prompt 自动包含 Git 分支、未提交变更、项目依赖等感知信息。

**纳入计划**：Context Convergence Phase 1。

---

## 四、验证结果

```bash
cargo test --workspace --lib
# test result: 438 passed / 0 failed / 6 ignored

cargo check --workspace --lib
# 0 warnings
```

---

## 五、后续计划（纳入 Sprint 15）

### 高优先级：Context Convergence Phase 1（1.5–2.5 天）

| 任务 | 说明 | 关联发现 |
|------|------|---------|
| `refresh_context()` 统一 | 移入 `run_streaming_turn()`，确保所有路径获取最新上下文 | 发现 1 |
| `SystemPromptBuilder` 扩展 | 添加 `GitContext` + `ProjectMetadata` 组件消耗 | 发现 3 |
| Memory 检索迁移 | 将 `memory_store.search()` 从 `run_streaming()` 移入 `SystemPromptBuilder` | AGENTS.md Capability Islands |
| `filter_tools_value()` 回归测试 | 验证 skill 激活时 tool schema 正确过滤（已修复，需端到端验证） | 根因 2 |

### 中优先级：空响应防御机制（0.5 天，待定）

- 收集 1–2 周运行日志中 `Empty final response` 触发频率
- 若频率 > 5%，添加防御性回退消息或自动重试机制

### 低优先级：诊断日志清理（0.25 天）

- 空响应 bug 确认修复后，将 `debug!`/`warn!` 日志降级或移除，避免日志噪声

---

## 六、验收标准

- [ ] egui 实测：发送 10 次不同 query，无空响应现象
- [ ] Gateway 实测：通过 WebSocket 发送 query，Agent 正确返回内容
- [ ] `RUST_LOG=debug` 日志中无 `Stream error` + `Empty final response` 连续出现
- [ ] Context Convergence Phase 1 完成后，`run_streaming_with_messages()` 路径的 System Prompt 包含 Git 上下文
