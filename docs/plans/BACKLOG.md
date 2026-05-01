# Clarity 未完成计划总览

> 生成时间：2026-04-27  
> 基线分支：`phase2/protocol-pilot` @ `d9976fe3`  
> 整合来源：ROADMAP / PROJECT_STATUS / FUTURE_DIRECTION / Sprint Plans / 解耦计划

---

## 一、当前 sprint（Sprint 13：稳定性硬化）

来源：`docs/plans/2026-04-30-sprint13-stability-hardening.md`

| ID | 事项 | 优先级 | 状态 |
|----|------|--------|------|
| S13-A1 | Agent 工具失败断路器（失败即停，不无限重试） | P0 | 未启动 |
| S13-A2 | 错误消息路径脱敏（`C:\Users\...` → `~`） | P0 | 未启动 |
| S13-A3 | System Prompt 边界硬化（防内部信息泄露） | P0 | 未启动 |
| S13-B1 | Approval 状态持久化（InMemory → SQLite） | P0 | 未启动 |
| S13-B2 | Approval Request ID 一致性校验 | P0 | 未启动 |
| S13-B3 | Agent 身份统一（去除底层模型名引用） | P1 | 未启动 |
| S13-C1 | `LLMProviderSelectionPolicy` 策略抽象 | P1 | 未启动 |
| S13-C2 | 网络探测层重构（probe 只驱动 UI，不决定 provider） | P1 | 未启动 |
| S13-C3 | `ensure_llm` God Function 拆分 | P1 | 未启动 |

---

## 二、解耦与架构健康（分发标准）

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

## 三、egui 功能 Parity（与 core 对齐）

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

## 四、核心架构演进（Future Direction）

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

## 五、技术债务

| 债务项 | 严重度 | 说明 | 计划 |
|--------|--------|------|------|
| egui 零测试 | 🔴 重大 | 当前 0 tests，违反 `test_governance.md` | Sprint 13+ 注入 ≥20 纯逻辑测试 |
| `unwrap()` 密度 | 🟡 中 | 171 总量 / ~39 真实风险 | 冻结新增，渐进清理（目标 ≤150） |
| cargo audit 上游漏洞 | 🟡 低 | 3 处 Tauri 间接依赖（已归档） | 等上游更新，不主动投入 |
| 文档过时 | 🟢 低 | 持续维护 | 每次重大重构后同步 |

---

## 六、冻结项（6 个月内不启动）

| ID | 事项 | 冻结原因 | 解除条件 |
|----|------|----------|----------|
| T_APPROVAL_V2 | AI 分类器混合审批 | 需大模型标注数据，ROI 不明确 | v0.5.0 后且有标注数据集 |
| T_SHORTCUTS | 全局快捷键系统 | egui 跨平台快捷键不成熟 | egui 官方快捷键 API 稳定 |
| T_MOBILE | Mobile 适配 | Hard Veto 禁止 | 项目广度约束解除且 v1.0 后 |
| T_PLUGIN_SDK | Plugin SDK / Sandbox | 需 WASM/IPC 沙箱基础设施 | v0.6.0 后 |
| T_VOICE | 语音输入/输出 | 与"零依赖"冲突 | 本地语音识别方案验证通过 |

---

## 七、验收命令（任何变更后必执行）

```bash
cargo test --workspace --lib          # 577 passed, 0 failed
cargo clippy --workspace -- -D warnings  # 0 warnings
cargo fmt --all -- --check            # 0 diff
cargo doc --workspace --no-deps       # 0 doc warnings
cargo audit                           # 0 RUSTSEC
```

---

*本文件随开发进度持续更新。下次审视：Sprint 13 结束时。*
