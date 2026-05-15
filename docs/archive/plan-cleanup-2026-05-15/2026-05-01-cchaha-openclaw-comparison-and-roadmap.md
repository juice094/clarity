# Clarity × cc-haha × openclaw 对比分析与长程路线图

> 生成时间：2026-05-01  
> 基线分支：`phase2/protocol-pilot` @ `0b0ec656`  
> 对比对象：cc-haha `v0.1.8` / openclaw `v2026.4.29`

---

## 一、项目定位与规模对比

| 维度 | Clarity | cc-haha | openclaw |
|------|---------|---------|----------|
| **定位** | 个人 AI Agent 桌面平台 | 基于 Claude Code 泄露源码的本地可运行版本 | 全渠道个人 AI 助手（Gateway 为控制面） |
| **语言** | Rust（纯原生） | TypeScript（Bun 运行时） | TypeScript（Node 22+/pnpm） |
| **架构** | 多 crate 工作空间 | 单体 Monorepo | Monorepo + packages + extensions |
| **源文件数** | 197 `.rs` | ~2,161 `.ts/.tsx` | ~7,616 `.ts/.tsx` |
| **源码量** | ~62K 行 | ~580K 行 | ~1.5M+ 行 |
| **GUI** | egui 原生（无 WebView2 依赖） | Tauri 2 + React（desktop/） + Ink TUI | Web UI（ui/） + TUI（src/tui/） |
| **发布** | 单二进制 (~6-8 MB) | Bun 脚本 + Tauri 桌面端 | npm 包 + 桌面端 (apps/) |
| **成熟度** | 可用，核心功能完备 | 社区活跃，功能完整 | 企业级，功能极尽完整 |

### Clarity 独特优势

- **纯 Rust 原生**：单二进制，无 Node/Bun/Python 运行时依赖，包体积 <10MB
- **零 WebView2 依赖**：egui 直接 OpenGL 渲染，无浏览器运行时
- **性能**：编译型语言，Agent 循环零 GC 暂停
- **类型安全**：Rust 的所有权/生命周期系统，全局无 `null`

### 与 cc-haha 的差距

cc-haha 有完整的 TUI 交互（Ink React）和桌面端（Tauri + React），Server 端 API 完备度更高：
- 20+ 服务端 Service（adapterService, cronService, notificationService, taskService 等）
- 完整的多 Agent Teams 系统
- 桌面端包含完整页面（会话管理、任务创建、计划任务、MCP 配置、终端配置等）
- Channel 适配器（Telegram / 飞书 / Discord）

### 与 openclaw 的差距

openclaw 体量庞大，覆盖极广：
- 22+ 第三方 IM 通道
- 企业级插件系统 + SDK
- 完整的实时语音/转录/TTS
- 图像/视频/音乐生成
- 安全审计体系
- 移动端（Android / iOS） + macOS 原生应用

---

## 二、全面特征矩阵

### 2.1 Agent 运行时

| 子特性 | Clarity | cc-haha | openclaw | 优先级 |
|--------|---------|---------|----------|--------|
| Agent 循环 | ✅ 完备 | ✅ 完备 | ✅ 完备 | - |
| Stream/SSE | ✅ | ✅ | ✅ | - |
| Plan 模式（分步规划） | ✅ | ✅ | ✅ | - |
| 审批系统 | ✅ (Interactive/Plan/Yolo) | ✅ | ✅ | - |
| 并行子代理 | ✅ | ✅ | ✅ | - |
| 多 Agent Teams | ✅ core 层 | ✅ 完整 UI | ✅ | P2 |
| Agent 身份隔离 | ✅ | ✅ | ✅ | - |
| 后台任务 | ✅ core+gateway | ✅ 桌面端 UI | ✅ | P1 |
| 任务调度 Cron | ✅ core 层 | ✅ 完整 UI | ✅ 完整 | P2 |
| 子代理进度面板 | ❌ egui 缺口 | ✅ | ✅ | P0 |
| 后台任务创建/取消 UI | ❌ egui 缺口 | ✅ | ✅ | P0 |

### 2.2 LLM / Provider

| 子特性 | Clarity | cc-haha | openclaw | 优先级 |
|--------|---------|---------|----------|--------|
| OpenAI 兼容 API | ✅ | ✅ (Anthropic + 3rd) | ✅ | - |
| 本地 GGUF 模型 | ✅ | ❌ | ❌ | P3 |
| 模型下载引导 | ✅ onboarding | ❌ | ❌ | - |
| 多 Provider 管理 | ✅ (5 个硬编码) | ✅ (preset 丰富) | ✅ (extensions) | **P0** |
| Provider Schema 化 | ❌ 枚举硬编码 | ✅ | ✅ | P0 |
| Provider 无代码注册 | ❌ 需改代码 | ✅ (config) | ✅ (extensions) | **P0** |
| 模型角色分工 | ❌ 单模型 | ✅ | ✅ | P2 |
| 环境变量注入 | ✅ | ✅ | ✅ | - |
| 用量/Tokens 追踪 | ✅ | ✅ | ✅ | - |
| 模型价格缓存 | ❌ | ❌ | ✅ | P4 |

### 2.3 Gateway / API Server

| 子特性 | Clarity | cc-haha | openclaw | 优先级 |
|--------|---------|---------|----------|--------|
| REST API | ✅ Axum | ✅ Express-like | ✅ 完整 | - |
| WebSocket | ✅ | ✅ | ✅ 完整 | - |
| SSE 流式 | ✅ | ✅ | ✅ | - |
| 双端口架构 | ✅ (API+Admin) | ❌ 单端口 | ❌ 单端口 | - |
| Admin 认证 | ✅ (token) | ❌ | ✅ | - |
| 会话管理 API | ✅ | ✅ | ✅ | - |
| 配置文件 API | ✅ | ✅ | ✅ | - |
| 工具列表 API | ✅ | ✅ | ✅ | - |
| 模型列表 API | ✅ | ✅ | ✅ | - |
| 文件操作 API | ✅ | ✅ | ✅ | - |
| MCP 配置 API | ❌ | ✅ | ✅ | **P1** |
| 插件管理 API | ❌ | ✅ | ✅ | P2 |
| Cron 管理 API | ❌ | ✅ gcronScheduler | ✅ | **P1** |
| 团队管理 API | ❌ | ✅ | ✅ | P2 |
| 搜索 API | ❌ | ✅ searchService | ✅ | P2 |
| OAuth 集成 | ❌ | ✅ hahaOAuthService | ✅ | P3 |

### 2.4 MCP

| 子特性 | Clarity | cc-haha | openclaw | 优先级 |
|--------|---------|---------|----------|--------|
| MCP stdio 协议 | ✅ | ✅ | ✅ | - |
| MCP SSE 协议 | ✅ | ✅ | ✅ | - |
| MCP WebSocket 协议 | ✅ | ✅ | ✅ | - |
| MCP 配置 UI | ✅ egui panel | ✅ desktop page | ✅ | - |
| MCP 服务器管理 | ✅ | ✅ | ✅ | - |
| MCP HTTP 传输 | ❌ | ❌ | ✅ | P4 |
| MCP 工具目录 | ❌ | ❌ | ✅ | P3 |
|   MCP 协议扩展** | ✅ 基础 | ✅ | ✅ 丰富 | - |

### 2.5 Memory

| 子特性 | Clarity | cc-haha | openclaw | 优先级 |
|--------|---------|---------|----------|--------|
| SQLite 持久化 | ✅ | ✅ | ✅ | - |
| 事实提取 | ✅ | ✅ | ✅ | - |
| 全文搜索 | ✅ | ✅ | ✅ | - |
| 向量嵌入 | ❌ | ❌ | ✅ (memory-host-sdk) | P3 |
| 跨会话检索 | ❌ | ✅ | ✅ | P2 |
| 记忆管理 UI | ❌ | ❌ | ❌ | P3 |
| 记忆压缩 | ✅ compaction | ✅ | ✅ | - |

### 2.6 工具系统

| 子特性 | Clarity | cc-haha | openclaw | 优先级 |
|--------|---------|---------|----------|--------|
| Shell 工具 | ✅ | ✅ | ✅ | - |
| 文件工具 | ✅ | ✅ | ✅ | - |
| Web 搜索 | ✅ | ✅ | ✅ | - |
| 网页抓取 | ✅ | ✅ | ✅ | - |
| Thinking | ✅ | ✅ | ✅ | - |
| Computer Use | ✅ | ✅ | ✅ | - |
| 计划工具 | ✅ | ✅ | ✅ | - |
| 队列/待办 | ✅ | ✅ | ✅ | - |
| Cron 工具 | ✅ | ✅ | ✅ | - |
| 团队工具 | ✅ | ✅ | ✅ | - |
| 搜索工具 | ✅ | ✅ | ✅ | - |
| 浏览器工具 | ✅ | ✅ ✅ 增强 | ✅ | - |
| MCP 工具 | ✅ | ✅ | ✅ | - |

### 2.7 Channel / IM 集成

| 子特性 | Clarity | cc-haha | openclaw | 优先级 |
|--------|---------|---------|----------|--------|
| Telegram | ⚠️ 禁用(CVE) | ✅ | ✅ | P3 |
| Discord | ⚠️ 禁用(CVE) | ❌ | ✅ | P3 |
| Slack | ✅ | ❌ | ✅ | P3 |
| Webhook | ✅ | ❌ | ✅ | P3 |
| 飞书 | ❌ | ✅ | ✅ | P4 |
| WhatsApp | ❌ | ❌ | ✅ | P4 |
| Signal | ❌ | ❌ | ✅ | P4 |
| iMessage | ❌ | ❌ | ✅ | P4 |
| 其他 15+ | ❌ | ❌ | ✅ | P5 |

### 2.8 画面 / 前端

| 子特性 | Clarity (egui) | cc-haha (Tauri+React) | openclaw (Web) | 优先级 |
|--------|----------------|----------------------|----------------|--------|
| 聊天界面 | ✅ | ✅ | ✅ | - |
| 流式消息 | ✅ | ✅ | ✅ | - |
| Markdown 渲染 | ✅ | ✅ | ✅ | - |
| Mermaid 图表 | ✅ | ✅ | ✅ | - |
| 多会话管理 | ✅ | ✅ | ✅ | - |
| 模型选择 | ⚠️ TextEdit 文本输入 | ✅ 下拉菜单 | ✅ | **P0** |
| 审批交互 | ✅ | ✅ | ✅ | - |
| Plan 可视化 | ✅ | ✅ | ✅ | - |
| Skill 面板 | ✅ | ✅ | ✅ | - |
| Token 显示 | ✅ | ✅ | ✅ | - |
| MCP 配置 | ✅ | ✅ | ✅ | - |
| 文件浏览 | ✅ | ✅ | ✅ | - |
| **子代理进度** | ❌ | ✅ | ✅ | **P0** |
| **后台任务 UI** | ❌ 只读 | ✅ 创建/取消/Cron | ✅ | **P0** |
| 设置页面 | ✅ | ✅ 完整 | ✅ | - |
| 自定义主题 | ✅ 深蓝灰系 | ✅ | ✅ | - |
| i18n | ✅ 中/英 | ✅ 中/英 | ✅ 多语言 | - |
| 日志面板 | ❌ | ✅ | ✅ | P1 |
| 终端面板 | ❌ | ✅ | ❌ | P2 |
| 插件管理 UI | ❌ | ✅ | ✅ | P2 |
| Onboarding | ✅ 首次启动引导 | ❌ | ✅ wizard | - |
| 快捷搜索 | ❌ | ❌ | ✅ | P3 |
| 暗色沉浸 | ✅ 已深度定制 | ✅ 通用 | ✅ 通用 | - |

### 2.9 基础设施

| 子特性 | Clarity | cc-haha | openclaw | 优先级 |
|--------|---------|---------|----------|--------|
| CI/CD | ✅ GitHub Actions | ✅ GitHub Actions | ✅ 完备 | - |
| 代码质量 | clippy -D warnings | ESLint | oxlint | - |
| 测试 | 577 lib tests | ✅ 丰富 | ✅ 极丰富 | P2 |
| 安全审计 | cargo audit | ✅ | ✅ 完整 | P1 |
| Docker | ❌ | ❌ | ✅ | P4 |
| 文档 | ✅ 英文 + 中文 | ✅ 丰富 | ✅ 完备 | - |
| 发布流程 | ✅ 手动 + CI | ✅ | ✅ CI 自动 | P3 |
| 性能分析 | ❌ | ❌ | ✅ | P4 |

### 2.10 Claw（系统托盘 / 守护进程）

| 子特性 | clarity-claw | cc-haha daemon | openclaw daemon | 优先级 |
|--------|-------------|----------------|-----------------|--------|
| 系统托盘 | ✅ tao+tray-icon | ❌ | ✅ | - |
| 任务监控 | ✅ 轮询 Gateway | ❌ | ✅ | P1 |
| OS 通知 | ✅ notify-rust | ❌ | ✅ | - |
| 任务完成通知 | ✅ | ❌ | ✅ | - |
| 快捷输入 | ⚠️ 仅打开网页 | ❌ | ✅ | **P1** |
| Wire 监听 | ✅ 预留 | ❌ | ✅ | P2 |
| 状态持久化 | ❌ | ❌ | ✅ | P2 |
| 自动启动 | ❌ | ❌ | ✅ | P3 |
| 多 tray 菜单 | ⚠️ 基础 | ❌ | ✅ 丰富 | P2 |
| 自检/健康 | ❌ | ❌ | ✅ heartbeat | P2 |

---

## 三、核心差距与根因分析

### 3.1 最大缺口：Provider 架构

**现状**：Clarity 的 Provider 是 Rust 枚举硬编码 5 个（Kimi, OpenAI, Anthropic, Google, Local）。

**cc-haha 做法**：`providerPresets.json` + `providerPresets.ts` 外部配置，可无代码添加。桌面端有完整的多提供商管理 UI。

**openclaw 做法**：`extensions/` 目录下每个 provider 独立扩展包，通过 `plugin-sdk` 标准化接口加载。可动态注册、版本管理、依赖注入。

**影响**：每新增一个 Provider 都需要改 core crate 源码，对社区贡献极不友好。

### 3.2 第二缺口：前端功能 Parity

**现状**：egui 在 12 天 Sprint 内补齐了大量功能（审批、Plan、Skill、Token），但仍缺子代理进度面板和后台任务创建/取消 UI。

**cc-haha 做法**：desktop 有完整的 `NewTaskModal.tsx`、`ScheduledTasks.tsx`、`TaskStore`。

**影响**：用户无法在 GUI 中创建、取消、调度后台任务，体验断崖。

### 3.3 第三缺口：Gateway API 完备度

**现状**：Clarity Gateway 路由 16 条，核心功能覆盖但缺失 MCP 配置、Cron、插件、团队等管理 API。

**cc-haha 做法**：20+ Service + 20+ API endpoints，覆盖面完整。

**openclaw 做法**：191K 行 gateway，极端完备，涵盖 auth、plugins、sessions、cron、model-catalog 等全部领域。

### 3.4 第四缺口：Plugin / Skills 生态

**现状**：Clarity 的 Skills 是 core 内置的静态系统，不支持外部扩展加载。无 Plugin SDK。

**cc-haha**：`pluginService.ts` 管理外部 plugins，支持的机制较完整。

**openclaw**：46K 行 plugin-sdk + 144K 行 plugins 基础设施，是最核心的优势。

### 3.5 Clarity 的独特优势保留

- **单二进制交付** — vs cc-haha 需 Bun + npm install，vs openclaw 需 Node + pnpm install + Docker
- **零外部运行时** — 无需解释器、无需 WebView
- **类型全链条安全** — Rust 从 core 到 gateway 静态度量

---

## 四、长程路线图（按优先级排序）

### Phase 0：立即修复（当前窗口）

| # | 事项 | 预估 | 目标 |
|---|------|------|------|
| 0.1 | **Gateway API 补齐** | 1 天 | 增加 `/api/mcp`、`/api/cron`、`/api/search` 端点 |
| 0.2 | **clarity-claw 增强** | 1 天 | 快捷输入弹窗（Tao 原生窗口）+ 任务创建/取消菜单项 |
| 0.3 | **egui 子代理进度 + 后台任务 UI** | 2 天 | 补齐 P0 egui 功能缺口 |
| 0.4 | **Provider Schema 化设计** | 1 天 | 定义外部配置格式，为无代码注册铺路 |

### Phase 1：架构解耦与 Provider 重构（1-2 周）

| # | 事项 | 目标 |
|---|------|------|
| 1.1 | **Provider Registry trait 化** | `Provider` 从枚举变为 trait，允许外部 crate 注册新 Provider |
| 1.2 | **Provider 配置外部化** | TOML/JSON Schema 定义 Provider 元数据（baseURL, authType, models） |
| 1.3 | **模型角色分工** | chat / utility / utilityLarge 三模型配置 |
| 1.4 | **MCP 配置持久化 API** | Gateway 端 MCP 服务器 CRUD + egui UI 联动 |
| 1.5 | **Cron 管理 API** | Gateway 端 Cron 任务 CRUD + 删除过期任务 |

### Phase 2：Gateway 能力对齐（2-3 周）

| # | 事项 | 参考来源 | 目标 |
|---|------|---------|------|
| 2.1 | **搜索 API**（跨会话全文检索） | cc-haha searchService | 历史消息检索 |
| 2.2 | **会话快照与 Rewind** | cc-haha sessionRewindService | 会话回退 |
| 2.3 | **会话导出 API** | openclaw trajectory | 导出 JSON / Markdown |
| 2.4 | **Workspace 管理 API** | cc-haha filesystem + adapters | 多工作区切换 |
| 2.5 | **健康监控 + 自检** | openclaw heartbeat | claw 端健康探针 |

### Phase 3：Claw 系统托盘硬化（1 周）

| # | 事项 | 参考来源 | 目标 |
|---|------|---------|------|
| 3.1 | **快捷输入窗口**（非网页） | openclaw daemon | Tao 原生浮动窗口，直接输入 prompt |
| 3.2 | **自动启动注册** | openclaw daemon | 注册为 Windows 开机自启 |
| 3.3 | **状态持久化** | openclaw daemon | SQLite 记录最近任务/通知 |
| 3.4 | **Wire 深度集成** | openclaw gateway-broadcast | 直接接收 Agent 事件推送，无需轮询 |

### Phase 4：egui 功能补齐（持续）

| # | 事项 | 目标 |
|---|------|------|
| 4.1 | 日志/Console 面板 | 实时显示 Gateway 日志 |
| 4.2 | 终端面板 | 内置 Shell 终端 |
| 4.3 | 快捷搜索 (Command Palette) | Ctrl+P 全局搜索 |
| 4.4 | 子代理进度面板 | 并行任务实时进度 |
| 4.5 | 后台任务创建/取消 UI | 完整的任务管理 |
| 4.6 | MCP 配置 UI 增强 | 添加/删除/测试 MCP 服务器 |

### Phase 5：平台化扩展（3-4 周）

| # | 事项 | 目标 |
|---|------|------|
| 5.1 | **Plugin SDK 原型** | 参考 openclaw plugin-sdk，定义 Clarity Plugin trait |
| 5.2 | **Plugin 加载机制** | 动态加载 .wasm 或动态库插件 |
| 5.3 | **Channel Adapter SDK** | 第三方 IM 通道标准化接入 |
| 5.4 | **Docker 化部署** | 无头模式 Docker 镜像 |
| 5.5 | **性能 Profiling** | 引入 tracing/flamegraph 性能分析 |

---

## 五、Phase 0 具体执行计划（本窗口）

当前窗口聚焦 Phase 0 的 **0.2** 和 **0.3**：

### 0.3 — egui 子代理进度 + 后台任务 UI

从 cc-haha 的 `desktop/src/pages/NewTaskModal.tsx` 和 `desktop/src/pages/ScheduledTasks.tsx` 获取交互模型参考，在 egui 中实现：
1. **`TaskCreatePanel`** —— 新建后台任务（名称 + prompt + 可选最大迭代数）
2. **`TaskListPanel`** —— 任务列表（状态、进度、取消按钮）

### 0.2 — clarity-claw 增强

1. **快捷输入弹窗**：Tao 原生窗口，带输入框 + 发送按钮，直接调 Gateway `/v1/chat/completions`
2. **任务创建菜单**：托盘菜单增加 "Create Task..." 项，弹出输入窗口
3. **取消任务菜单**：运行中任务可右键取消

### 0.4 — Provider Schema 设计

文档先行：`docs/plans/provider-schema-design.md`

---

## 六、验收标准

每个 Phase 完成后：

```bash
cargo test --workspace --lib          # 全部通过
cargo clippy --workspace -- -D warnings  # 零警告
cargo fmt --all -- --check            # 零 diff
```

新增功能需至少包含 3 个测试用例覆盖正向/边界/异常路径。

---

## 七、参考资源映射

| 参考功能 | 来源项目 | 源文件路径 |
|----------|---------|-----------|
| Provider Schema | cc-haha | `src/server/config/providerPresets.json` |
| 后台任务 UI | cc-haha | `desktop/src/pages/NewTaskModal.tsx` |
| 计划任务 UI | cc-haha | `desktop/src/pages/ScheduledTasks.tsx` |
| 子代理进度 | cc-haha | `desktop/src/components/teams/` |
| 搜索服务 | cc-haha | `src/server/services/searchService.ts` |
| 会话 Rewind | cc-haha | `src/server/services/sessionRewindService.ts` |
| Daemon 框架 | openclaw | `src/daemon/` |
| Plugin SDK 设计 | openclaw | `src/plugin-sdk/` 和 `packages/plugin-sdk/` |
| 心跳/健康 | openclaw | `src/daemon/heartbeat-*.ts` |
