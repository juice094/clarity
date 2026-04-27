# Clarity 项目现状报告

> 版本：v0.3.0 | 日期：2026-04-27 | `main` @ `899d8f9` | 基于实机测试与代码审计  
> 关联文档：[`ENGINEERING_PLAN.md`](ENGINEERING_PLAN.md) · [`ROADMAP.md`](ROADMAP.md) · [`FUTURE_DIRECTION.md`](FUTURE_DIRECTION.md) · [`PROJECT_STATUS.md`](../PROJECT_STATUS.md)

---

## 1. 核心指标（实测数据）

| 指标 | 实测结果 | 评估 |
|------|---------|------|
| **编译检查** | `cargo check --workspace` | ✅ 零错误 |
| **单元测试** | **524 passed, 0 failed, 4 ignored** | ✅ 全绿 |
| **Clippy 检查** | `cargo clippy --workspace --lib --bins --tests -- -D warnings` | ✅ **零警告** |
| **安全审计** | `cargo audit --deny unsound --deny yanked` | ✅ 已集成 CI |
| **代码规模** | ~130 个 Rust 源文件 | 持续增长 |
| **Workspace Crates** | 8 + 1 集成测试 crate | 结构稳定 |

**测试覆盖详情**：
- `clarity-core`: 381 tests passed, 4 ignored
- `clarity-gateway`: 43 tests passed
- `clarity-memory`: 79 tests passed
- `clarity-wire`: 8 tests passed
- `clarity-tui`: 6 tests passed
- `clarity-claw`: 6 tests passed
- `clarity-headless`: 1 test passed
- `clarity-integration-tests`: 0 tests（空骨架）
- `clarity-egui`: **0 tests — 重大技术债务**

**前端测试**：31 passed / 11 test files（smoke + interaction）— Tauri 侧归档前数据

---

## 2. 已完成功能（v0.3.0）

### 2.1 核心引擎（clarity-core）

```
✅ Agent Loop — ReAct 循环、工具调用、多轮对话、审批系统
✅ Plan Mode — 结构化 JSON 计划 + 批量执行，绕过逐工具审批
✅ 并行子代理 — run_parallel() + BackgroundTaskManager 并发调度
✅ 三层审批 — Interactive / Yolo / Plan + T_APPROVAL V1 规则引擎
✅ 上下文压缩 — CompactionService 自动防止 Token 爆炸（Tier-1 截断 + Tier-2 摘要）
✅ 20+ 内置工具 — 文件读写编辑、Shell、搜索、Web、MCP、任务管理、团队管理、推送通知
✅ Daemon 运行时 — 跨平台 PID lockfile + graceful shutdown
✅ AutoDream — 夜间记忆整合调度器（cron 触发 + timeout 保护）
✅ Server 模块 — JSON-RPC over stdio，暴露 AgentController（零网络，单客户端）
✅ ChannelSendTool — 飞书/钉钉/Slack/Webhook 主动消息发送（含 HMAC-SHA256）
✅ Lazy Master — 重型组件（LLM / MemoryStore / SkillRegistry）首次 run() 时按需初始化
✅ 本地 LLM 推理 — Candle 原生 GGUF（Qwen2/DeepSeek-R1-Distill），无需 Ollama
✅ 多 LLM 支持 — Anthropic、Kimi、OpenAI、DeepSeek、Ollama、Local (GGUF)
✅ MCP 生态 — stdio / HTTP / SSE 三协议完整实现
✅ 离线模式 — 网络探测 + 自动 fallback 到本地模型 + 恢复后切回
✅ Skill 系统 — Markdown+YAML 编排，关键字搜索，工具白名单
✅ Wire 事件总线 — SPMC 跨模块通信
✅ LSP 代理 — rust-analyzer 等语言服务器进程管理 + JSON-RPC 调试
✅ Computer Use — 远程桌面控制面板（截图/点击/输入/滚动）
✅ 动态系统提示 — SystemPromptBuilder 条件组装
✅ 模型热切换 — Settings Panel 中 provider/model 切换无需重启
```

### 2.2 记忆系统（clarity-memory）

```
✅ SQLite 持久化 — PersistentMemoryStore
✅ BM25 + Hybrid 搜索 — FTS5 召回 + 内存 BM25 重排序
✅ RAG Chunking — 可配置大小、重叠、分隔符
✅ 向量存储 — CosineIndex + TfidfVectorizer
```

### 2.3 Gateway（clarity-gateway）

```
✅ HTTP REST API — /v1/chat/completions, /v1/tasks, /v1/parallel, /api/files/*
✅ WebSocket — 实时事件流
✅ Session Store — SQLite 持久化（CRUD、消息追加、请求计数、过期清理）
✅ Admin API — /api/tools, /api/stats, /api/approval-mode, /api/config
✅ Web UI — 聊天界面、任务面板、设置面板、并行执行面板
✅ CORS — 支持 localhost:3000/5173/18800
```

### 2.4 TUI（clarity-tui）

```
✅ 终端聊天界面 — ratatui 组件化设计
✅ 命令系统 — /plan, /parallel, /skill, /task
✅ 实时流式响应 — SSE 解析 + 打字机效果
```

### 2.5 系统托盘（clarity-claw）

```
✅ 后台任务监控 — 实时读取 .clarity/tasks/ 目录
✅ OS 通知 — 任务完成/失败推送
✅ 任务列表弹窗 — 状态、名称、时间
```

### 2.6 桌面 GUI（clarity-egui）— 当前主力栈 🚀

```
✅ Chat Panel — 多会话聊天 + 流式响应 + 虚拟列表
✅ Session Sidebar — 创建/切换/删除/自动命名
✅ Settings Panel — Provider 选择 + API Key + Local Model Path + Approval Mode
✅ 主题系统 — Dark/Light/Auto + CJK 字体自动加载
✅ 文件浏览器 — 工作目录树 + 点击预览
✅ 工具调用可视化 — Running/Done 状态气泡 + 参数/结果摘要
✅ Compaction Banner — 压缩状态提示条
✅ 后台任务面板 — 只读列表，3秒轮询刷新
✅ MCP 配置面板 — 服务器列表、启用/禁用、保存到 mcp.json
✅ 网络状态 Banner + Toast — 离线探测、fallback 提示
✅ 消息队列 — Streaming 时自动排队，完成后自动发送
✅ 附件拖拽 — 支持文件拖入作为附件
```

---

## 3. 前后端功能 Parity 矩阵（关键差距标注）

| 功能 | clarity-core | clarity-egui | clarity-tui | clarity-gateway | clarity-headless |
|------|:------------:|:------------:|:-------------:|:-----------:|:---------------:|:----------------:|
| Agent 运行/流式 | ✅ | ✅ | ✅ | ✅ | ✅ |
| 工具调用可视化 | ✅ (产生) | ✅ | ✅ | ❌ | ❌ |
| Compaction Banner | ✅ (产生) | ✅ | ❌ | ❌ | ❌ |
| **审批交互 UI** | ✅ (后端) | ❌ | ❌ | ❌ | CLI only |
| **Plan 模式可视化** | ✅ (后端) | ❌ | `/plan` | ❌ | CLI only |
| **子代理/并行执行** | ✅ | ❌ | ❌ | ✅ | ✅ | ❌ |
| 后台任务面板 | ✅ (完整) | 只读 | 命令行 | API only | ❌ |
| 后台任务创建/取消 | ✅ | ❌ | ✅ | ✅ | ❌ |
| Cron 调度 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 团队协调 (Team) | ✅ | ❌ | ❌ | ❌ | ❌ |
| **技能系统 UI** | ✅ | ❌ | ✅ | ❌ | ❌ |
| MCP 配置/管理 | ✅ | 配置面板 | ❌ | ❌ | ❌ |
| MCP 工具执行 | ✅ | 间接 | 间接 | 间接 | 间接 |
| 记忆提取/存储 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 会话持久化 | ✅ | ✅ | ❌ | ✅ (SQLite) | ❌ |
| **Token 用量显示** | ✅ | ❌ | ✅ | ❌ | ✅ |
| LSP 集成 | ✅ (core 支持) | ❌ | ❌ | ❌ | ❌ |
| 模型下载 GUI | ❌ (非 core 职责) | ❌ | ❌ | ❌ | ❌ |
| 日志面板 | ❌ | ❌ | ❌ | ❌ | ❌ |

**最大差距**：egui 缺少 core 已实现的**交互型功能**（审批、Plan、子代理、技能、任务创建）。这些功能在 TUI 中已通过命令行暴露，但在 GUI 中完全缺失，导致 `Interactive`/`Plan` approval mode 在 egui 中实际上无法使用。`clarity-tauri` 已完全归档移出仓库。

---

## 4. 已知问题（已审计，待修复）

### I1. Settings 模型配置体验缺陷（P1）

**诊断**：`model` 字段的持久化机制本身完好（`GuiSettings` 含 `model: String`，`save()/load()` 通过 serde 处理）。问题集中在 UI 交互层和边缘容错：

| # | 子问题 | 根因 | 影响 |
|---|--------|------|------|
| I1.1 | 无模型下拉列表 | `TextEdit` → `ComboBox` + provider 联动 | ✅ 已修复 |
| I1.2 | Provider/Model 不联动 | 切换 provider 时自动更新 model | ✅ 已修复 |
| I1.3 | `load()` 静默吞错 | 解析失败时日志 + `.bak` 备份 | ✅ 已修复 |
| I1.4 | 环境变量 model 互斥缺失 | provider 匹配 env var 选择 | ✅ 已修复 |
| I1.5 | `ensure_llm` 无网络 fallback | 断网时自动 fallback 到 local | ✅ 已修复 |

**修复 commit**：`ff3227d`（Phase 1 维护批次）

### I2. egui 零测试（P0 技术债务）

`clarity-egui` 当前 **0 tests / 0 test modules**。Pretext 运维 plan 已规划 Phase 2 注入测试基线（≥ 20 个纯逻辑测试）。

### I3. egui 交互型功能缺口（P2）

审批弹窗、Plan 步骤可视化、子代理进度、任务创建/取消、Token 用量显示、模型下载 GUI、日志面板为当前最大功能缺口。

---

## 5. 安全状态

| 漏洞 | 状态 | 修复版本 |
|------|------|----------|
| S1: `resolve_path` 目录遍历 | ✅ 已修复 | v0.1.1 |
| S2: Gateway `sanitize_path` 目录遍历 | ✅ 已修复 | v0.1.1 |

**安全措施**：
- MCP 命令注入防护 — `validate_mcp_command()` 拦截 shell 元字符、相对路径
- 敏感文件检测 — `.env`、SSH key、kubeconfig 等自动识别
- TLS 未禁用 — reqwest 默认系统 TLS
- 无 `unsafe` 代码块

---

## 6. CI/CD 状态

| Job | 状态 | 说明 |
|-----|------|------|
| `check` | ✅ | `cargo check --workspace` |
| `test` | ✅ | `cargo test --workspace --lib` |
| `clippy` | ✅ | `cargo clippy --workspace --lib --bins --tests -- -D warnings` |
| `fmt` | ✅ | `cargo fmt --all -- --check` |
| `audit` | ✅ | `cargo audit --deny unsound --deny yanked` |
| `coverage` | ✅ | `cargo tarpaulin --workspace --lib --out Xml` |
| `release` | ✅ | Tag-triggered GitHub Actions workflow，产出 `.msi` / `.exe` / `.nsis` |

**平台矩阵**：ubuntu-latest, windows-latest, macos-latest

---

## 7. 技术债务

| 债务项 | 严重程度 | 说明 | 计划 |
|--------|----------|------|------|
| `std::sync::RwLock` → `tokio::sync::RwLock` | ✅ 已解决 | `background/` 模块已完成迁移（`1141ba9`） | — |
| MCP HTTP E2E 验证 | ✅ 已解决 | Axum 最小 server E2E 测试通过（`8db4db3`） | — |
| MCP SSE Transport | ✅ 已解决 | 完整 SSE 协议实现（endpoint discovery + reconnection + handshake），注释已同步 | — |
| Gateway handler 单元测试 | ✅ 已解决 | mock 测试已覆盖（v0.1.1） | — |
| 文档过时 | 🔄 持续 | 本次审计已清理 3 份过时计划文件 | 见 [`ENGINEERING_PLAN.md`](ENGINEERING_PLAN.md) |
| unwrap() 密度 | 🔄 持续 | 171 总量 / ~39 真实风险；11 处已清理，8 处已注释 | 冻结新增，渐进清理 |
| cargo audit 上游漏洞 | ⚠️ 监控 | 3 处 Tauri 间接依赖（2 moderate, 1 low） | 已配置 `.cargo/audit.toml` 忽略；等待上游更新 |
| **clarity-egui 零测试** | 🔴 **重大** | 0 tests，违反 test_governance.md 基线 | Pretext 运维 plan Phase 2（2-4 周） |
| **clarity-egui 审批 UI 缺失** | 🔴 **重大** | Interactive/Plan approval mode 在 GUI 中无法使用 | 待排期，预计 1-2 周 |
| Settings 模型选择体验 | 🟡 中等 | 无下拉列表、provider/model 不联动 | 短期修复（½ 天） |

---

## 8. 目标用户画像

> 2026-04-26 更新：基于 Web 侧讨论收敛，从"开发者/技术用户"细化为"长期 AI 协作者"。

Clarity 面向**长期 AI 协作者** — 将 AI Agent 作为持续工作伙伴的技术从业者，而非"用一次即走"的终端消费者。

**核心诉求排序**：
1. **存在稳定性**：Agent 可长期驻留，内存可控（< 100MB），离线可用，秒级启动
2. **主权可控**：源码可审计，数据不离开本机，模型/配置可热切换，无外部运行时依赖
3. **扩展性**：工具、记忆、协议可按需定制，MCP 生态即插即用

**非目标用户**：追求"开箱即用美观 UI"的终端消费者、无技术背景的普通办公人员、需要 IM 深度集成的聊天机器人用户。

---

## 9. 与竞品对比（简要）

| 维度 | Clarity (v0.3.0) | cc-haha | OpenClaw | zeroclaw | codex-rs |
|------|------------------|---------|----------|----------|----------|
| **技术栈** | Rust (egui 0.31 UI) | Bun/TS (Tauri UI) | Node.js | Rust | Rust |
| **本地 LLM (零依赖)** | ✅ Candle GGUF | ✅ (Ollama 可选) | ❌ | ✅ | ✅ |
| **离线模式** | ✅ 自动 fallback | ⚠️ | ❌ | ❌ | ❌ |
| **Task/Team 工具暴露** | ✅ TaskCreate + TeamCreate/Delete/List + PushNotify | ✅ | ❌ | ❌ | ❌ |
| **Plan Mode** | ✅ | ✅ (5 阶段) | ❌ | ❌ | ❌ |
| **并行子代理** | ✅ | ✅ (Coordinator) | ⚠️ | ❌ | ❌ |
| **MCP** | ✅ stdio/HTTP/SSE | ✅ + OAuth + Channel 协议 | ⚠️ | ❌ | ✅ |
| **审批系统** | ✅ 三层 + V1 规则引擎 | ✅ | ❌ | ❌ | ❌ |
| **预构建安装包** | ✅ `.msi` + `.exe` | ❌ | ❌ | ❌ | ❌ |
| **Voice** | ❌ | ✅ | ✅ | ❌ | ❌ |

**注**：竞品对比中的 "Clarity" UI 栈已更新为 egui（不再是 Tauri），但功能维度一致。

---

## 10. 代办与冻结项

| ID | 事项 | 状态 | 说明 |
|----|------|------|------|
| T_KALOSM_REAL | agri-paper 7B 模型数据 | 🔴 阻塞 | 本地模型首次体验路径不完整 |
| T_KIMICLI_REF | 借鉴 Kimi CLI settings/模型选择设计 | ⏸️ 冻结 | 仅作设计参考，不推进实现。归档于 `docs/plans/2026-04-27-egui-pretext-health-plan.md` |
| T_APPROVAL_V2 | AI 分类器混合审批 | ⏸️ 冻结 | 约束解除前不投入 |
| T_SHORTCUTS | 快捷键系统 | ⏸️ 冻结 | 约束解除前不投入 |
| T_MOBILE | Mobile 适配 | ⏸️ 冻结 | 约束解除前不投入 |

---

*本文件随版本发布同步更新。上次全面审计：2026-04-27。*
