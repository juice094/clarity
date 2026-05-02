# Clarity 未完成计划总览

> 生成时间：2026-04-27  
> 基线分支：`phase2/protocol-pilot` @ `d9976fe3`  
> 整合来源：ROADMAP / PROJECT_STATUS / FUTURE_DIRECTION / Sprint Plans / 解耦计划

---

## 一、已完成 Sprint

### Sprint 13 — 稳定性硬化（2026-04-27 ~ 2026-05-03）

来源：`docs/plans/2026-04-30-sprint13-stability-hardening.md`

| ID | 事项 | 优先级 | 状态 |
|----|------|--------|------|
| S13-A1 | Agent 工具失败断路器（失败即停，不无限重试） | P0 | ✅ 已完成 |
| S13-A2 | 错误消息路径脱敏（`C:\Users\...` → `~`） | P0 | ✅ 已完成 |
| S13-A3 | System Prompt 边界硬化（防内部信息泄露） | P0 | ✅ 已完成 |
| S13-B1 | Approval 状态持久化（InMemory → SQLite） | P0 | ✅ 已完成 |
| S13-B2 | Approval Request ID 一致性校验 | P0 | ✅ 已完成 |
| S13-B3 | Agent 身份统一（去除底层模型名引用） | P1 | ✅ 已完成 |
| S13-C1 | `LLMProviderSelectionPolicy` 策略抽象 | P1 | ✅ 已完成 |
| S13-C2 | 网络探测层重构（probe 只驱动 UI，不决定 provider） | P1 | ✅ 已完成 |
| S13-C3 | `ensure_llm` God Function 拆分 | P1 | ⏸️ 冻结 |

### Sprint 14.5 — 架构解耦与代码健康（2026-05-02）

来源：`docs/plans/nightcrawler-drax-atom.md`

| 事项 | 状态 | 说明 |
|------|------|------|
| Phase A：统一 Agent Streaming Loop | ✅ | 提取 `run_streaming_turn()`，消除 `run_streaming()` / `run_streaming_with_messages()` 重复编排 |
| Phase B：复活 ChatDriver + 解耦 Op 枚举 | ✅ | `ConversationChatDriver` 接入 Gateway；`Op` 恢复 5 个纯生命周期变体 |
| Phase C：清理 AppState 死字段 | ✅ | 移除 `initialized`、`active_connections`、外层 `RwLock<Agent>`、重复 `approval_runtime` |
| **P0 Bug — Agent 空响应** | ✅ | 修复 stream error fallback、tool filter 缺失、`finish_turn()` 不执行 |

**遗留问题（纳入 Sprint 15）**：
- `run_streaming_with_messages()` 不调用 `refresh_context()` → Context Convergence Phase 1
- `task_store` 孤儿问题 → 待决策

---

## 二、当前 Sprint（Sprint 15 — Context Convergence Phase 1）

| ID | 事项 | 优先级 | 状态 | 说明 |
|----|------|--------|------|------|
| S15-C1 | `refresh_context()` 统一移入 `run_streaming_turn()` | P0 | 未启动 | 确保 Gateway/egui/TUI 所有路径获取最新 Git/项目上下文 |
| S15-C2 | `SystemPromptBuilder` 消耗 `GitContext` + `ProjectMetadata` | P0 | 未启动 | 主 Agent Prompt 自动包含 Git 分支、未提交变更、项目依赖 |
| S15-C3 | Memory 检索迁移进 `SystemPromptBuilder` | P1 | 未启动 | 将 `memory_store.search()` 从 `run_streaming()` 移入 builder，统一上下文注入 |
| S15-C4 | `filter_tools_value()` 端到端验证 | P1 | 未启动 | skill 激活时 tool schema 白名单验证 |
| S15-C5 | 空响应防御机制评估 | P2 | 未启动 | 收集 1–2 周日志，决定是否添加自动重试/默认回退消息 |

---

## 三、解耦与架构健康（分发标准）

来源：`docs/plans/2026-04-27-decoupling.md`、`docs/architecture/COUPLING_AUDIT.md`

### 2.1 已完成 ✅
- `tools↔subagents` 循环依赖打破
- 4 个孤岛模块降级为 `pub(crate)`
- MCP 逻辑隔离（`feature = "mcp"`）

### 2.2 待完成

| 事项 | 优先级 | 说明 |
|------|--------|------|
| 提取 `clarity-contract` | P1 | 将 `error` + `tools` trait + `registry` 接口下沉为独立 crate，为 MCP 独立发布铺路 |
| 提取 `clarity-mcp` | P2 | 整体迁移 `mcp/` 到独立 crate，依赖 `clarity-contract` |
| `background/cron↔store` 循环 | P2 | 提取共享类型到 `background/types.rs` |
| `subagents↔agent` 循环 | P2 | `subagents` 需 `Agent` 实例；需 trait 抽象 |
| `background↔subagents` 循环 | P2 | `background` 需 `AgentTypeDefinition`；双向任务调度 |
| `llm↔agent` 伪循环 | P3 | `llm` 直接引用 `types::FunctionCall` 即可消除 |

---

## 四、egui 功能 Parity（与 core 对齐）

来源：`docs/PROJECT_STATUS.md` §3

| 功能 | core 状态 | egui 状态 | 差距 |
|------|-----------|-----------|------|
| 后台任务创建/取消 | ✅ | ❌ | 🔴 高 |
| 子代理进度面板 | ✅ | ❌ | 🔴 高 |
| Cron 调度 UI | ✅ | ❌ | 🔴 高 |
| 团队协调 UI | ✅ | ❌ | 🔴 高 |
| 记忆提取/搜索面板 | ✅ | ❌ | 🟡 中 |
| 模型下载 GUI | ❌ | ❌ | 🟡 中 |
| 日志/Console 面板 | ❌ | ❌ | 🟡 中 |
| LSP 集成面板 | ✅ (core) | ❌ | 🟢 低 |
| Token 权限可视化 | ✅ (backend) | ❌ | 🟢 低 |
| 快捷键系统 | ❌ | ❌ | ⏸️ 冻结 |
| 搜索增强 (Command Palette) | ❌ | ❌ | ⏸️ 冻结 |

---

## 五、核心架构演进（Future Direction）

来源：`docs/FUTURE_DIRECTION.md`

### Phase A：基础设施联通（2 周）

| 事项 | 优先级 | 说明 |
|------|--------|------|
| WebSocket MCP 传输 | P1 | `McpTransport` 新增 `WebSocket { url, headers }` 变体 |
| Gateway ↔ BackgroundTaskManager 集成 | P1 | Gateway WebSocket 事件流接入 BTM 进度 |
| Worker 池自动扩缩容 | P1 | `ScalableWorkerPool` 去除下划线前缀，实现阈值触发扩缩 |
| 跨会话记忆检索 | P2 | `search_all_sessions(query, limit)` 跨 session 全文检索 |

### Phase B：会话层统一（2–3 周）

| 事项 | 优先级 | 说明 |
|------|--------|------|
| 统一 Session Schema（SQLite） | P1 | 替代 JSON+JSONL 双系统；含 parent_session_id / handoff_document |
| SessionManager 抽象 | P1 | `create/load/save_message/search/handoff` 统一 API |
| Session Handoff | P1 | 会话间上下文迁移：`HandoffDocument` 含 decisions / pending_tasks / agent_state |
| Session Event Bus | P2 | `session:message_added`、`session:handoff_available` 事件 |

### Phase C：运行时重构 — Hub-Worker + 多窗口（4–6 周）

| 事项 | 优先级 | 说明 |
|------|--------|------|
| AgentInstance + AgentPool | P1 | 包装 `AgentController`，支持多实例并发 |
| Identity 路由 | P1 | `Gray/Kimi/Analyst/Programmer` 等身份 + 按任务路由 |
| Wire 协议扩展（跨 Agent 消息） | P1 | `AgentMessage { from, to, payload }` + `AgentStateSnapshot` |
| IPC 传输层 | P1 | `Transport::Ipc`（TCP 回环 + UDS + Named Pipe） |
| 多窗口状态模型 | P1 | `AppState.agent` → `Arc<RwLock<AgentPool>>` |
| 存在论锚点硬绑定 | P2 | `AgentPool::default_instance` 固定指向 `Identity::Gray` |

### Phase D：跨设备验证 — Syncthing-Rust（4–6 周）

| 事项 | 优先级 | 说明 |
|------|--------|------|
| 设备身份与发现 | P1 | 基于 Syncthing 设备证书 |
| Session CRDT 同步 | P1 | Loro Rust core，messages append-only |
| Agent 状态迁移 | P1 | turn 级上下文序列化，跨设备传输 |
| P2P Wire 协议 | P2 | `Transport::P2P`，复用 Syncthing TLS 通道 |

---

## 六、技术债务

| 债务项 | 严重度 | 说明 | 计划 |
|--------|--------|------|------|
| egui 零测试 | 🔴 重大 | 当前 0 tests，违反 `test_governance.md` | Sprint 13+ 注入 ≥20 纯逻辑测试 |
| Agent 空响应防御 | 🟡 中 | stream 空内容/LLM 返回空无自动回退 | 收集日志后决策（S15-C5） |
| `unwrap()` 密度 | 🟡 中 | 171 总量 / ~39 真实风险 | 冻结新增，渐进清理（目标 ≤150） |
| cargo audit 上游漏洞 | 🟡 低 | 3 处 Tauri 间接依赖（已归档） | 等上游更新，不主动投入 |
| 文档过时 | 🟢 低 | 持续维护 | 每次重大重构后同步 |

---

## 七、冻结项（6 个月内不启动）

| ID | 事项 | 冻结原因 | 解除条件 |
|----|------|----------|----------|
| T_APPROVAL_V2 | AI 分类器混合审批 | 需大模型标注数据，ROI 不明确 | v0.5.0 后且有标注数据集 |
| T_SHORTCUTS | 全局快捷键系统 | egui 跨平台快捷键不成熟 | egui 官方快捷键 API 稳定 |
| T_MOBILE | Mobile 适配 | Hard Veto 禁止 | 项目广度约束解除且 v1.0 后 |
| T_PLUGIN_SDK | Plugin SDK / Sandbox | 需 WASM/IPC 沙箱基础设施 | v0.6.0 后 |
| T_VOICE | 语音输入/输出 | 与"零依赖"冲突 | 本地语音识别方案验证通过 |

---

## 八、验收命令（任何变更后必执行）

```bash
cargo test --workspace --lib          # 438 passed, 0 failed, 6 ignored
cargo clippy --workspace -- -D warnings  # 0 warnings
cargo fmt --all -- --check            # 0 diff
cargo doc --workspace --no-deps       # 0 doc warnings
cargo audit                           # 0 RUSTSEC
```

---

*本文件随开发进度持续更新。下次审视：Sprint 15 结束时。*
