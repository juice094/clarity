# Clarity 未完成计划总览

> 生成时间：2026-05-05  
> 基线分支：`main` @ `8b6158f8`  
> 整合来源：ROADMAP / PROJECT_STATUS / FUTURE_DIRECTION / Sprint Plans / 解耦计划

---

## 一、已完成 Sprint

### Sprint 22 — MCP 错误检测 + Agent 熔断 + devbase 路径修复（2026-05-04）

| ID | 事项 | 状态 |
|----|------|------|
| S22-A1 | MCP 工具错误分类（用户错误/内部错误） | ✅ |
| S22-A2 | Agent 熔断（recoverable 3次 → fatal） | ✅ |
| S22-B1 | devbase 路径/Metrics 修复 | ✅ |

### Sprint 23 — MCP 契约硬化 + clarity-core 解耦 Phase 1（2026-05-04）

| ID | 事项 | 状态 |
|----|------|------|
| S23-A1 | devbase 14 工具 `"success"` 统一 | ✅ `27aad1e` |
| S23-A2 | MCP E2E 4 场景契约测试 | ✅ `36c65559` |
| S23-A3 | `clarity-mcp` 独立 crate 提取 + egui 任务面板 | ✅ `84f48ba1` |
| S23-A4 | API Key 前缀校验 + 凭证脱敏 regex | ✅ `99ecde2d` |

### Sprint 24 — Provider 韧性 + Cancellation Token + Loop Detector 增强（2026-05-04）

| ID | 事项 | 状态 |
|----|------|------|
| S24-A1 | Provider 指数退避重试 (`retry_with_backoff`) | ✅ `aa43645c` |
| S24-A2 | CancellationToken 穿透工具执行层 | ✅ `0561b8ca` |
| S24-A3 | LoopDetector 增强（Warning/Break + args 模式检测） | ✅ `5e51cfa6` |

### Sprint 25 — ReliableProvider + Event 模型 + 子代理共享迭代预算（2026-05-05）

| ID | 事项 | 状态 |
|----|------|------|
| S25-A1 | `ReliableProvider` 回退链包装器 | ✅ `59c886cd` |
| S25-A2 | `clarity-wire` Event 驱动输出模型 (`Event`/`EventMsg`) | ✅ `18f3abfa` |
| S25-A3 | 共享迭代预算计数器 (`Arc<AtomicUsize>`) | ✅ `02982d24` + `38424772` |

### Sprint 26 — Event 模型接线 + 迭代预算集成测试（2026-05-05）

| ID | 事项 | 状态 |
|----|------|------|
| S26-A1 | EventBus 单点桥接（`send_wire_message` → Event） | ✅ `8b6158f8` |
| S26-A2 | 迭代预算端到端集成测试（budget=0 / 共享语义） | ✅ `8b6158f8` |

---

## 二、当前 Sprint（Sprint 27 — Prompt Reorder + KV Cache 策略层）

> 来源：`vault/clarity/optimization-backlog/kimi-share-19df31da-insights.md`

| ID | 事项 | 优先级 | 状态 | 说明 |
|----|------|--------|------|------|
| S27-A1 | **Prompt Reorder：静态前缀 + 动态尾部** | P0 | 未启动 | `SystemPromptBuilder` 将静态组件（base/tools/skills/security）与动态组件（git/active_files/metadata/memory）分离为独立消息，避免 prefix cache miss |
| S27-A2 | **`prompt_cache_key` 策略层实现** | P0 | 未启动 | 计算静态 system prompt 的 stable hash 作为 cache key；API provider 启用服务端 prefix caching |
| S27-A3 | **LocalGgufProvider KV cache 跨 turn 持久化** | P1 | 未启动 | 保持模型状态跨 turns，不从 `index_pos=0` 重置 |
| S27-A4 | **System Prompt KV Snapshot（跨会话）** | P1 | 未启动 | `~/.clarity/cache/kv/{model_id}/{hash}.kvcache` 序列化/反序列化 |
| S27-B1 | `refresh_context()` 统一移入 `run_streaming_turn()` | P2 | 未启动 | Sprint 15 遗留（Gateway/egui/TUI 所有路径获取最新上下文） |
| S27-B2 | `SystemPromptBuilder` 消耗 `GitContext` + `ProjectMetadata` | P2 | 未启动 | Sprint 15 遗留 |

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

## 五.x、实验性能力 — Jumpy Workflow Orchestration（MVP 已完成）

> **来源**：`crates/clarity-core/src/agent/jumpy/` + `docs/plans/nightcrawler-drax-atom.md` Sprint 14.5 实验窗口  
> **论文**：Farebrother et al., *Compositional Planning with Jumpy World Models*, arXiv:2602.19634  
> **核心思想**：将预训练 Skill 视为可组合的"短跑专家"，通过离线预测模型（世界模型）实现时间抽象，允许 Agent 直接预判宏观状态而非逐消息模拟。

### 已完成 ✅

| ID | 事项 | 状态 | 说明 |
|----|------|------|------|
| J1 | 核心四组件实现 | ✅ | `JumpyState` + `HistoricalPredictor` + `ConsistentPredictor` + `HierarchicalPlanner` + `SkillComposer` |
| J2 | 单元/集成测试 | ✅ | 507 tests passed（含 3 个 jumpy 专用测试） |
| J3 | 模块导出与编译集成 | ✅ | `pub mod jumpy` in `agent/mod.rs`，`cargo check --workspace --lib` 0 warnings |
| J4 | Kimi CLI Skill 封装 | ✅ | `~/.config/kimi/skills/jumpy-orchestrator/SKILL.md` |

### 待连接（避免能力孤岛）

| ID | 事项 | 优先级 | 阻塞/说明 |
|----|------|--------|----------|
| J5 | **从 `clarity-memory::session_store` 自动提取历史观测** | P1 | 将聊天记录转化为 `SkillObservation`，喂给 `HistoricalPredictor::observe_batch()` |
| J6 | **LLM-Augmented Predictor** | P2 | 无历史记录时，调用 LLM 做零样本状态转移预测。需设计 prompt template 和结果解析 |
| J7 | **Flow 节点扩展：`InvokeSkill` / `PredictCheckpoint`** | P2 | `flow/mod.rs` 新增节点类型，`runner.rs` 接入 `SkillRegistry::run_flow()`。保持现有 Flow 零侵入 |
| J8 | **与 `SubagentManager` 打通** | P2 | `SkillComposer::compose()` 的回调绑定到 `SubagentManager::spawn()`，实现层级 Agent 委托 |
| J9 | **`clarity-headless` CLI 入口** | P2 | `jumpy` 子命令暴露编排能力，JSON 输出供外部工具（如 Kimi CLI）消费 |
| J10 | **A/B 验证：Jumpy 规划 vs 传统 Plan** | P3 | 真实任务上的成功率、token 消耗、重规划次数对比。需要 ≥20 条任务轨迹 |

### 设计约束（硬）

- **零侵入现有系统**：Flow / Skill / Plan / AgentLoop 不修改接口，仅新增可选节点类型
- **回调解耦**：`SkillComposer::compose()` 通过 `execute_skill_fn` 回调连接实际执行层，不直接依赖 `Agent` 或 `Subagent`
- **离线优先**：预测模型训练不依赖环境交互，仅从 `session_store` 历史数据学习
- **Rust 核心不外包**：`agent::jumpy` 的所有演进必须在当前窗口内由主 Agent 完成，不可委托子 Agent

### 验收命令

```bash
cd dev/third_party/clarity
cargo test -p clarity-core --lib agent::jumpy   # 3 jumpy tests
cargo test -p clarity-core --lib                # 507 tests
cargo check --workspace --lib                   # 0 warnings
```

---

## 六、技术债务

| 债务项 | 严重度 | 说明 | 计划 |
|--------|--------|------|------|
| egui 零测试 | 🔴 重大 | 当前 0 tests，违反 `test_governance.md` | Sprint 13+ 遗留，待排期 |
| Agent 空响应防御 | 🟡 中 | stream 空内容/LLM 返回空无自动回退 | 收集日志后决策 |
| `unwrap()` 密度 | 🟡 中 | ~170 总量 / ~39 真实风险 | 冻结新增，渐进清理（目标 ≤150） |
| cargo audit 上游漏洞 | 🟡 低 | Tauri 间接依赖（已归档） | 等上游更新 |
| 文档过时 | 🟢 低 | BACKLOG 已更新至 Sprint 26 | 每次重大重构后同步 |

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
cargo test --workspace --lib -- --test-threads=1   # 759 passed, 0 failed, 6 ignored
cargo clippy --workspace -- -D warnings           # 0 warnings
cargo fmt --all -- --check                         # 0 diff
cargo doc --workspace --no-deps                    # 0 doc warnings
cargo audit                                        # 0 RUSTSEC (unmaintained ignored)
```

---

*本文件随开发进度持续更新。下次审视：Sprint 27 结束时。*
