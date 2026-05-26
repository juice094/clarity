---
title: Clarity Roadmap
category: Roadmap
date: 2026-05-18
tags: [roadmap, planning]
---

# Clarity Roadmap

> 策略：本地优先 + 零依赖 + 开源  
> 当前：v0.3.0（已完成）→ 目标：v0.3.1+（发行验证 + 集群语义扩展）

---

## 阶段一：最小可用发布（已完成 ✅）

**目标**：切断历史关联，建立独立品牌，实现"下载即用"。

| 里程碑 | 说明 |
|--------|------|
| 法律清理 | README 重写，零泄露源码关联表述 |
| 核心闭环 | `cargo install --git` 可用，TUI + Gateway + Headless CLI 独立运行 |
| 多提供商 | Kimi / OpenAI / Anthropic / DeepSeek / Ollama / Local (Candle GGUF) |
| 记忆验证 | SQLite + BM25 + 向量混合搜索，跨会话持久化 |

**阶段一的核心交付物**：`cargo install --git https://github.com/juice094/clarity --bin clarity-tui` 即可运行一个功能完整的 AI Agent。

---

## 阶段二：本地优先标杆 + 开发环境替代（当前重点 🎯）

**双重目标**：
1. 在"个人 AI 运行时"品类中，成为离线/本地场景的默认选项。
2. 打造能替代 Kimi CLI 的本地开发环境，实现 Claw 模式的持续化存储与多角色认知协同。

| 里程碑 | 交付物 | 优先级 |
|--------|--------|--------|
| 本地 LLM 深度集成 | ✅ Candle 原生 GGUF 支持（Qwen2/DeepSeek-R1-Distill） | **P0** |
| 零依赖发行 | 单二进制 + 嵌入式模型（用户无需安装 Rust/Ollama/Python） | **P0** |
| 三栏工作台 UI | 左侧角色栏 / 顶部实例标签 / 右侧通用工具栏 | **P0** |
| 集群语义验证 | Hub-Worker 调度器 + Wire 消息扩展 + 多窗口协作 | **P1** |
| Claw 持续化存储 | 跨会话 Agent 状态快照、子 Agent 上下文持久化 | **P1** |
| 协议草案 | 开源 Agent 通信协议（供其他 Runtime 参考实现） | **P2** |
| 企业/团队版 | Multi-user session 支持 | **P3** |

### 当前可执行动作

- ✅ Settings Panel 中本地模型路径配置 + 自动扫描
- ✅ 离线模式检测（无网络时自动 fallback 到 LocalGgufProvider）
- ✅ `clarity-egui` 默认启用 `local-llm` feature
- ✅ 单二进制打包 + CI Release workflow（`.msi` / `.exe` / `.nsis`）已交付
- ✅ egui 归一化 UI 完成（tab bar、sidebar 工具栏、统一弹窗、Frame 系统、Glassmorphism）
- ✅ KimiCLI 兼容层（`agent.yaml` 声明式配置 + 工具映射 + 子代理定义）
- ⏸️ 嵌入式模型自动下载（首次启动引导）

### 风险对冲

若 v0.3.0 发布后 **30 天内无实质性社区反馈**（GitHub Star ≥ 50 / Issue+PR ≥ 3），阶段二冻结，资源回拨至 devbase。

---

## 详细功能路线图

### Phase 0：基础夯实（已完成 ✅）

Agent ReAct 循环、Plan Mode、三层审批、MCP 三协议、Memory 系统、Background Tasks、Lazy Master。

### Phase 1：GUI 奠基（Sprint 1-2 已完成 ✅）

> **归档注**：以下基于 `clarity-tauri`（React+Vite）的实现已废弃归档。v0.4.0 起全部功能由 `clarity-egui`（eframe/egui，纯 Rust）承接。
>
> ~~`clarity-tauri` 已可用：Chat Panel、Session Sidebar、Task Panel、Settings Panel、Theme System (Dark/Light/Auto)。~~

### Phase 2：核心补齐（部分已完成 ✅）

| 工作项 | 状态 | 说明 |
|--------|------|------|
| 审批系统增强 | ✅ 已完成 | T_APPROVAL V1（规则引擎）已交付；egui 审批弹窗 UI 已补齐（Sprint 12）|
| 跨前端 Settings 协议化 | ✅ 已完成 | `SettingsViewModel` 下沉 core + `ViewCommand` 协议通道 + 三前端统一接入 |
| 文件浏览器集成 | ✅ 已完成 | 工作目录树 + `@path` 引用 |
| LSP 支持 | ✅ 已完成 | LSP proxy layer + GUI panel |
| WebBrowserTool | ✅ 已完成 | reqwest+scraper 轻量实现 |
| 快捷键系统 | ✅ MVP 完成 | 全局快捷键 (`Ctrl+N/Enter/K/Shift+P/Period/Shift+T`) + 焦点守卫；Vim 键位引擎待 Phase 3 |
| 搜索增强 | ⏸️ 未启动 | Command Palette 风格 |
| 性能优化 | 🔄 部分完成 | 基准脚本已交付（dev 数据已采集），release 跑分待执行 |
| 桌面端打包 | ✅ 已完成 | `.msi` / `.exe` / `.nsis` + GitHub Actions Release workflow |
| egui 归一化 UI | ✅ 已完成 | Sprint 14.5+ 密集迭代：全宽 tab bar、顶部全局工具栏、统一弹窗风格（Settings/MCP/Skill）、`Frame::new()` 归一化、Glassmorphism 视觉系统 |
| KimiCLI 兼容层 | ✅ 已完成 | `agent.yaml` 解析器 + `tool_map.rs` 工具映射 + `AppState` 自动加载 + 8 个单元测试 |
| `clarity-egui` | ✅ **主力栈成熟** | `crates/clarity-egui` 71 个 .rs 文件，承担全部桌面 GUI 功能。`clarity-tauri` 废弃归档 |

### Sprint 14.5 — 架构解耦与空响应修复（2026-05-02，1 天）

| 工作项 | 状态 | 说明 |
|--------|------|------|
| 统一 Agent Streaming Loop | ✅ | 提取 `run_streaming_turn()`，消除重复编排 |
| 复活 ChatDriver + 解耦 Op | ✅ | Gateway 通过 `ConversationChatDriver` 注入消息历史；`Op` 恢复纯净 |
| AppState 死字段清理 | ✅ | 移除 `initialized`、`active_connections`、外层 `RwLock`、重复 `approval_runtime` |
| **Agent 空响应修复** | ✅ | 修复 stream error fallback、tool filter 缺失、`finish_turn()` 不执行（详见 `docs/plans/2026-05-02-agent-empty-response-followup.md`） |

### Phase 3：集群语义验证 + Claw 持续化（4-6 周）

目标：将单 Agent 单进程假设重构为多 Agent Hub-Worker 调度器；实现跨会话状态快照与多角色认知协同。

详见 [`FUTURE_DIRECTION.md`](FUTURE_DIRECTION.md) Phase A→C 与 [`docs/visions/AGENT_OS_VISION.md`](docs/visions/AGENT_OS_VISION.md)。

#### 中间协议层状态

| 协议层 | 状态 | 说明 |
|--------|------|------|
| `clarity-wire`（UI↔Agent EventBus） | ✅ 成熟 | SPMC 广播 + ViewCommand 协议通道，三前端统一消费 |
| Turn ID 注入 WireMessage | ✅ Phase A 完成 | ADR-007 已落地；Phase A（Wire+Core backend）已完成；Phase B（前端存储/聚合）待并行 session 收敛后 |
| MCP 传输 | ✅ 成熟 | stdio / SSE / HTTP 三种，注册表管理，工具发现 |
| Gateway HTTP API | ✅ 成熟 | Axum + session store + REST handlers |
| Local LLM KV Cache | ✅ 已交付 | Sprint 28：`LocalGgufProvider` LCP-based 跨 turn KV 复用 + static prompt hash 失效机制 |
| 跨会话状态快照 | 🔄 后端就绪 | `subagents::builder` + `AgentPool` 概念存在；缺 egui 独立面板与持久化格式 |
| Claw 联邦运行时 | 🔄 空壳 | `clarity-claw` system-tray 占位；runtime 逻辑待填充 |
| IPC 传输层 | ❌ 未启动 | TCP 回环 / UDS / Named Pipe，规划中 |
| 多窗口 Agent 隔离 | ❌ 未启动 | `AppState.agent` → `AgentPool` 重构未开始 |

#### Phase 3 工作项

| 工作项 | 状态 | 说明 |
|--------|------|------|
| WebSocket MCP 传输 | ⏸️ 未启动 | `McpTransport` 新增变体 |
| Gateway ↔ BackgroundTaskManager 集成 | ✅ 已完成 | `GatewayTaskClient` + `GatewayManager` — egui 自动启动 Gateway，BTM ops 全走 `/v1/tasks` REST API，本地 store fallback |
| Worker 池自动扩缩容 | ⏸️ 未启动 | `ScalableWorkerPool` 去下划线前缀 |
| 会话层统一（SQLite） | ⏸️ 未启动 | 替代 JSON+JSONL 双系统 |
| Hub-Worker 调度器 | ⏸️ 未启动 | `AgentPool` + `AgentInstance` |
| 子 Agent UI 接入（IS-1） | 🔄 后端就绪，前端待接入 | `subagents::builder` 已具备 spawn 能力，缺 egui 独立面板 |
| Token 权限校验前端（IS-3） | 🔄 后端就绪，前端待接入 | `verify_sandbox_escape` 已存在，缺 UI 层的权限可视化 |
| 跨会话状态快照 | 🔄 后端就绪，前端待接入 | Agent 思考过程、计划、未完成子任务的完整持久化 |
| 角色方向性文件系统 | ⏸️ 未启动 | 情感/知识/工程的课题声明与权限矩阵落地 |

### Jumpy World Model (J 系列)

| 工作项 | 状态 | 说明 |
|--------|------|------|
| J5 SessionStore 适配器 | ✅ 完成 | `session_store_adapter.rs` — SessionRecord → SkillObservation |
| J6 LLM-Augmented Predictor | ✅ 完成 | `predictor.rs` — HistoricalPredictor + LlmAugmentedPredictor + HybridPredictor |
| J7 Flow 节点扩展 | ✅ 完成 | `flow/mod.rs` + `flow/runner.rs` — InvokeSkill + PredictCheckpoint |
| J8 SubagentManager 集成 | ✅ 完成 | `subagents/mod.rs` — predictor 注入 + 路由策略 |
| J9 clarity-headless jumpy 子命令 | ⏸️ 未启动 | headless CLI 集成 Jumpy 预测 |
| J10 A/B 验证数据集 | 🔄 Phase 1 | `training-data/baseline/` — 19 sessions + 15 memory facts 已导出 |

### Phase 4：生态扩展（6 周）

Bridge 远程控制、Vector Search (`sqlite-vec`)、Sandbox (`landlock`)、Plugin SDK (Rust dylib / WASM)、Voice 集成、Canvas 支持。

---

## 技术债务

| 债务项 | 状态 | 处理策略 |
|--------|------|---------|
| cargo audit warnings | ⚠️ 待确认 | 需重新运行 `cargo audit` 确认当前警告来源；`.cargo/audit.toml` 已配置忽略规则 |
| S3.3 Settings 单源化 | 🔄 进行中 | llm 层已改造（`build_provider` 公开，`ACTIVE_CONFIG` 标 DEPRECATED）；egui 层 `provider_tab.rs` 仍写全局缓存，阻塞于并行 session 暂停协议 |
| Discord/Telegram CVE | ❌ 已禁用 | 等上游修复 |
| Mobile app | ❌ 已否决 | Hard Veto 禁止（项目广度 > 5 核心工具） |
| `clarity-tauri` 冻结 | ⏸️ 冻结 | 停止新功能开发，仅维护现有代码至 egui 主控成熟 |
| Pretext 排版引擎（重型 TeX 移植） | ❌ 已否决 | 重型移植工程，不入主 repo 路线图；可作为个人探索项目 |
| Pretext UI 设计哲学（轻型采纳） | ✅ 已采纳 | 见 [`plans/2026-05-12-pretext-ui-evolution.md`](plans/2026-05-12-pretext-ui-evolution.md)：S1+S2 已完成 Phase 0.5+1（图标即字符、Chrome StripBuilder），不重做布局引擎 |

---

## 质量标准

```bash
cargo test --workspace --lib      # 全绿
cargo clippy --workspace --lib --bins --tests -- -D warnings  # 零 warning
cargo fmt --all -- --check        # 格式检查通过
cargo audit                       # 无高危漏洞
```

---

## 里程碑时间线

```
2026-04 ── v0.3.0      本地优先标杆（已发布）— Local LLM + 离线 fallback + Desktop GUI + 健康维护基线
    │
2026-05 ── v0.3.1+      质量硬化 + egui 方向验证 + 零依赖发行 — 单二进制打包 + 嵌入式模型 + unwrap 清理
    │
2026-06 ── v0.4.0-beta   性能优化 + 快捷键 + 搜索增强 + 审批系统增强
    │
2026-07 ── v0.5.0-beta   集群语义验证（Hub-Worker + 多窗口 + IPC）
    │
2026-08 ── v0.6.0-rc     Sandbox + Plugin SDK
    │
2026-09 ── v0.7.0-rc     Bridge + Voice + Canvas
    │
2026-10 ── v1.0.0        稳定版发布
```

---

*本文件随开发进度持续更新。每次重大决策或方向调整时同步修订。*
