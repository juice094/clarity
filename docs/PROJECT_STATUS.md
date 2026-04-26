# Clarity 项目现状报告

> 版本：v0.2.1 | 日期：2026-04-25 | 基于实机测试与代码审计

---

## 1. 核心指标（实测数据）

| 指标 | 实测结果 | 评估 |
|------|---------|------|
| **编译检查** | `cargo check --workspace` | ✅ 零错误 |
| **单元测试** | **524 passed, 0 failed, 4 ignored** | ✅ 全绿 |
| **Clippy 检查** | `cargo clippy --workspace --lib --bins --tests -- -D warnings` | ✅ **零警告** |
| **安全审计** | `cargo audit` | ✅ 已集成 CI |
| **代码规模** | ~125 个 Rust 源文件 | 持续增长 |
| **Workspace Crates** | 6 + 1 集成测试 crate | 结构稳定 |

**测试覆盖详情**：
- `clarity-core`: 381 tests passed, 4 ignored
- `clarity-gateway`: 43 tests passed
- `clarity-memory`: 79 tests passed
- `clarity-wire`: 8 tests passed
- `clarity-tui`: 6 tests passed
- `clarity-claw`: 6 tests passed
- `clarity-integration-tests`: 0 tests（空骨架）

---

## 2. 已完成功能（v0.1.1）

### 2.1 核心引擎（clarity-core）

```
✅ Agent Loop — ReAct 循环、工具调用、多轮对话、审批系统
✅ Plan Mode — 结构化 JSON 计划 + 批量执行，绕过逐工具审批
✅ 并行子代理 — run_parallel() + BackgroundTaskManager 并发调度
✅ 三层审批 — Interactive / Yolo / Plan
✅ 上下文压缩 — CompactionService 自动防止 Token 爆炸
✅ 17+ 内置工具 — 文件读写编辑、Shell、搜索、Web、MCP、任务管理、团队管理、推送通知
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

### 2.6 桌面 GUI（clarity-tauri）

```
✅ Chat Panel — 多会话聊天 + 流式响应
✅ Session Sidebar — 创建/切换/删除/重命名
✅ Task Panel — 任务列表与状态追踪
✅ Settings Panel — Provider 选择 + 本地模型扫描 + 主题 + 网络探测配置
✅ File Browser — 工作目录树 + @path 引用
✅ Diff Viewer — 代码变更预览
✅ LSP Panel — 语言服务器调试
✅ Computer Use Panel — 远程桌面控制
✅ 离线状态 banner — 自动 fallback / 恢复提示
✅ i18n — 中文/英文切换，Settings Panel 语言选择 + 持久化
```

---

## 3. 安全状态

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

## 4. CI/CD 状态

| Job | 状态 | 说明 |
|-----|------|------|
| `check` | ✅ | `cargo check --workspace` |
| `test` | ✅ | `cargo test --workspace --lib` |
| `clippy` | ✅ | `cargo clippy --workspace --lib --bins --tests -- -D warnings` |
| `fmt` | ✅ | `cargo fmt --all -- --check` |
| `audit` | ✅ | `cargo audit --deny warnings` |
| `coverage` | ✅ | `cargo tarpaulin --workspace --lib --out Xml` |

**平台矩阵**：ubuntu-latest, windows-latest, macos-latest

---

## 5. 技术债务

| 债务项 | 严重程度 | 说明 | 计划 |
|--------|----------|------|------|
| `std::sync::RwLock` → `tokio::sync::RwLock` | ✅ 已解决 | `background/` 模块已完成迁移（`1141ba9`） | — |
| MCP HTTP E2E 验证 | ✅ 已解决 | Axum 最小 server E2E 测试通过（`8db4db3`） | — |
| MCP SSE Transport | ✅ 已解决 | 完整 SSE 协议实现（endpoint discovery + reconnection + handshake），注释已同步 | — |
| Gateway handler 单元测试 | ✅ 已解决 | mock 测试已覆盖（v0.1.1） | — |
| 文档过时 | ✅ 已解决 | docs 目录已全面整理（v0.1.1） | — |

---

## 6. 与竞品对比（简要）

| 维度 | Clarity (v0.2.0-dev) | cc-haha | OpenClaw | zeroclaw | codex-rs |
|------|----------------------|---------|----------|----------|----------|
| **技术栈** | Rust (Tauri 2 UI) | Bun/TS (Tauri UI) | Node.js | Rust | Rust |
| **Task/Team 工具暴露** | ✅ TaskCreate + TeamCreate/Delete/List + PushNotify | ✅ | ❌ | ❌ | ❌ |
| **Plan Mode** | ✅ | ✅ (5 阶段) | ❌ | ❌ | ❌ |
| **并行子代理** | ✅ | ✅ (Coordinator) | ⚠️ | ❌ | ❌ |
| **MCP** | ✅ stdio/HTTP/SSE | ✅ + OAuth + Channel 协议 | ⚠️ | ❌ | ✅ |
| **Voice** | ❌ | ✅ | ✅ | ❌ | ❌ |
| **Desktop GUI** | ✅ Tauri 2 + React | ✅ Tauri 2 + React | ❌ | ❌ | ❌ |
| **多标签** | 🔄 进行中 | ✅ | ❌ | ❌ | ❌ |
| **LSP** | ✅ | ✅ | ❌ | ❌ | ❌ |
| **Vim** | 🔄 计划中 | ✅ | ❌ | ❌ | ❌ |
| **Sandbox** | 🔄 计划中 | ✅ OS-level | ❌ | ❌ | ✅ Docker |
| **Plugin SDK** | 🔄 计划中 | ✅ | ✅ | ❌ | ❌ |
| **审批系统** | ✅ 3 层 | ✅ 7 层 + AI 分类器 | ❌ | ❌ | ❌ |
| **Memory 深度** | ✅ SQLite + BM25 + 四级编译 | ⚠️ 文件目录 | ❌ | ❌ | ❌ |
| **性能** | ✅ 原生二进制 ~40MB | ⚠️ Bun runtime | ⚠️ Node.js | ✅ <5MB | ✅ |
| **代码合法性** | ✅ 自研 | ⚠️ 泄露源码补丁 | ✅ | ✅ | ✅ |

**定位差异**：
- **Clarity** = 开发者的 AI 标准运行时（Rust 全栈 + Plan Mode + Memory 深度）
- **cc-haha** = 个人 AI 编码助手（功能全面但架构沉重，法律风险）
- **OpenClaw** = 个人 AI 助手（Channels + Voice + Canvas + 移动端）
- **zeroclaw** = 极简 Rust AI 助手（极低资源）
- **codex-rs** = 编码助手（沙箱 + MCP）

---

## 7. 下一步（Phase 3）

### 7.1 已完成（v0.1.2 交付）

| 工作项 | 状态 | Commit |
|--------|------|--------|
| Channels Webhook E2E 验证 | ✅ 已完成 | `dedb6bd` — 18 个集成测试覆盖飞书/钉钉/企业微信/通用端点 |
| 性能基准测试（Criterion） | ✅ 已完成 | `dedb6bd` — ToolRegistry 31.5µs / AgentPrompt 89.2µs / SkillContext 158ns |
| MCP SSE Transport | ✅ 已完成 | 完整实现（endpoint discovery + reconnection + handshake） |
| Channels 原型（Telegram/Discord/Webhook） | ✅ 已实现 | Gateway 已集成 Telegram、Discord、Webhook 三渠道 |
| 本地模型支持（Candle GGUF） | ✅ 已实现 | `LocalGgufProvider` 原生推理，无需 Ollama；E2E 测试通过 |
| **MemoryTicker 版本统一** | ✅ 已完成 | `5514209` — 删除 `clarity-core` 简化版，全项目统一为 `clarity-memory::SharedMemoryTicker`（session 隔离 + compile callback + 防重入） |
| **Settings Panel 本地模型配置** | ✅ 已完成 | `1b2ded2` — `get_local_models()` 扫描 `~/models/` + `CLARITY_LOCAL_MODEL_PATH`；前端增加 `Local (GGUF)` provider 选择器 + 路径显示 + 无模型提示 |
| **Gateway Memory 激活** | ✅ 已完成 | `5514209` — `create_agent()` 接入 `PersistentMemoryStore` + `SharedMemoryTicker`，默认 5 turns 触发 |
| **MemoryCompiler 四级编译管道** | ✅ 已完成 | `5514209` — today→week→longterm→facts，LLM 自动摘要 + 事实提取 + 去重 |
| **Slack 渠道** | ✅ 已完成 | `4a3d2e0` — Web API 实现（长消息分块、HMAC-SHA256 签名验证、Events API challenge） |
| **统一配置系统（TOML）** | ✅ 已完成 | `cfcc24c` — 三层配置加载 + `export_to_env()` + Gateway/TUI 双端接入，向后兼容 |

### 7.2 进行中 / 待启动

| 优先级 | 工作项 | 工作量 | 说明 | Track |
|--------|--------|--------|------|-------|
| P2 | clarity-tauri Desktop GUI | ✅ 已完成 | Chat/Session/Task/Settings/FileBrowser/Diff/ComputerUse/LSP；离线 fallback + 预加载 + Settings 缓存 | — |
| P2 | 审批系统增强 | 🔄 部分完成 | 规则引擎 V1 已完成（RiskLevel + RuleEngine + 执行集成）；AI 分类器 V2 待启动 | — |
| P2 | T_FTUE 首次体验 | ✅ 已完成 | `get_launch_status()` + OnboardingModal + i18n (en/zh) | — |
| P2 | T_DYNAMIC_PROMPT | ✅ 已完成 | `SystemPromptBuilder` + `PromptComponent` 条件注入 | — |
| P2 | LSP 支持 | ✅ 已完成 | LSP proxy layer + GUI panel（rust-analyzer 等进程管理 + JSON-RPC 调试） | — |
| P3 | Bridge 远程控制 | 1-2 周 | 跨设备 Agent 远程调度 | — |
| P3 | Vector Search（sqlite-vec） | 1-2 周 | 语义向量检索替换 TF-IDF | — |
| P3 | WebBrowserTool | ✅ 已完成 | 轻量级 reqwest+scraper 实现（navigate/get_text/get_html） | — |
| P3 | Vim 集成 | 1-2 周 | Vim 键位引擎 | — |
| P3 | Sandbox | 1-2 周 | landlock (Linux) + Windows 沙箱 API | — |
| P3 | Plugin SDK | 2-3 周 | Rust dylib 插件系统 | — |

---

*本报告随版本更新。最新状态见 [`../CHANGELOG.md`](../CHANGELOG.md)。*
