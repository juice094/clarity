---
title: Clarity 项目现状报告
category: Status
date: 2026-05-17
tags: [status]
---

# Clarity 项目现状报告

> 版本：v0.3.4-rc | 日期：2026-06-15 | 基于实机测试与代码审计
> 关联文档：`ENGINEERING_PLAN.md` · [`ROADMAP.md`](ROADMAP.md) · [`FUTURE_DIRECTION.md`](FUTURE_DIRECTION.md) · [`plans/2026-05-12-pretext-ui-evolution.md`](plans/2026-05-12-pretext-ui-evolution.md)
> Pretext UI 演进：S1 (Phase 0.5) + S2 (Phase 1) 已完成 — 详见 `plans/2026-05-12-S1-session-archive.md` · `plans/2026-05-12-S2-session-archive.md`

---

## 0. 最新状态（2026-06-13）

**Sprint S5 — egui 模块整理与健康维护已阶段性收尾**：为后续 Pretext 单页面/三栏布局整合完成架构准备。本次未删除任何现有功能，全部面板仍通过 `App::render_layout_shell()` 统一编排。

- **ViewState 单源化**：从各 store 删除 7 个遗留 panel_open 布尔，所有面板可见性统一由 `app.view_state` 驱动。
- **panels/ 目录重组**：按职责迁入 `chat/`、`work/`、`settings/`、`modals/`、`sidebar/`、`workspace/`、`system/`、`legacy/`，保留向后兼容 re-export。
- **公共 widget 提取**：增强 `widgets/avatar.rs`、新增 `widgets/user_avatar.rs`、删除未启用的 `card.rs` / `badge.rs` / `toggle.rs` / `settings_row.rs`。
- **布局外壳接入点**：新增 `layout.rs`（`LayoutMetrics` + `update_and_measure`）；`App::render_layout_shell()` 成为 chrome / 主视图 / 浮层 / 模态框唯一编排入口。
- **Store 文件拆分**：`stores/mod.rs` 拆为 12 个按域子模块，`mod.rs` 通过 `pub use X::*;` 保持原有导入路径。
- **Design System 落地**：注册 `mod design_system`；在 `main.rs::update()` 每帧调用 `design_system::install_theme()`；修复 `design_system.rs` 2 处 clippy 警告。
- **全 workspace 基线修复**：修复 `clarity-core::agent::cost_channel` 因全局静态变量导致的并行测试 flake。
- **迁移规划产出**：`docs/plans/clarity-egui-pretext-layout-migration.md` 明确三栏目标结构、ViewState 扩展草案、pretext 接入点及分阶段路线。

**S6-C3 布局几何精化与人机协作标注器（2026-06-13）**：基于用户提供的手绘 UI 概念图与 Kimi 参考截图，完成 Pretext 三栏布局的像素级比例映射与代码落地。

**Pretext PoC 启动（2026-06-13）**：
- 在 `clarity-egui` 引入 `pretext-core` / `pretext-fontdb` 依赖，PoC 阶段先用本地 path，验证通过后已切换为 git 依赖并固定到稳定 rev。
- 实现 `pretext::EguiFontMetrics`：用 egui 自身字体栈作为 pretext 的 `FontMetrics` backend，保证测量与渲染同源。
- 新增 `widgets/pretext_probe.rs` 校准窗口：对比 10 组样本的 pretext 预测宽度与 egui 实际宽度；提供 Wrap Preview 滑块，实时对比预测行数/高度与实际行数/高度。
- 入口：Settings → Interface → "Open Pretext Measurement Probe"；窗口在 `App::update` 中渲染。
- 修复了 `clarity-core/src/thread/manager.rs` 中因 `SessionSource`/`ThreadSource` 未 clone 及 `ToolCall` 缺 `call_type` 导致的编译错误（前置阻塞问题，与当前 PoC 无直接功能关联）。

**Pretext Phase 2 — Rich Inline Chip（2026-06-13）**：
- 新增 `ui/rich_inline.rs`：轻量化 tokenizer 将文本切分为 text / `code` / `@mention` 三类 token，并为 chip 设置 `RichInlineBreak::Never` 与 `extra_width`（padding）。
- `layout_rich_inline` 使用 `pretext_core::rich_inline` 计算每行 fragment 的 x 偏移，输出可交给 egui 逐 fragment 渲染。
- `widgets/pretext_probe.rs` 增加 Rich Inline Chip Preview：用色块直观显示 chip 是否被整颗换行。
- 回归测试覆盖：token 解析、mention/code chip 不被截断、普通文本可正常换行；同时修复 `clarity-gateway` 测试共享磁盘状态导致的并行 flake（`AppState::new_with_home` + `test_state` 使用独立 temp dir）。

**Pretext Phase 3 — MessageBubble 渲染器（2026-06-13）**：
- 新增 `widgets/rich_paragraph.rs`：接受 `InlineSpan` 序列，用 pretext 计算精确换行，再用 egui 逐 fragment 绘制；chip 带背景框与描边。
- `ui/render.rs` 中的 `message_bubble` / `agent_text_plain_inner` / `user_bubble` / `render_content_block` 增加 `metrics: Option<&EguiFontMetrics>` 参数。
- 当 `app.ui_store.pretext_estimate_enabled` 开启时，消息列表的预计算高度、fallback 高度估算、实际渲染均走 pretext 路径；关闭时保持原有 markdown 渲染。
- 简单段落（空 `parsed` 或单 `Paragraph`）优先走 rich paragraph；复杂 markdown 仍可在后续迭代中扩展。

**Pretext Phase 4 — 默认启用并删除 heuristic fallback（2026-06-13）**：
- `app_logic.rs` 将 `pretext_estimate_enabled` 默认设为 `true`。
- `ui/render.rs`：`estimate_height()` 改为强制接收 `&EguiFontMetrics`，删除 `estimate_height_heuristic`。
- `components/agent_turn.rs`：`AgentTurn::estimate_height()` 同步接受 `metrics` 并透传。
- `message_list.rs` 所有高度估算路径统一使用 pretext，不再走字符数启发式。
- 顺手修复 `tests/integration/src/thread_api.rs` 编译与断言错误（`>` 污染首行、`BackgroundTaskManager::new` 缺 `context_dir`、创建返回 201、history 结构为扁平 `RolloutItem` 数组），integration tests 从 16 增至 20 并全绿。

**Pretext Phase 5 — 性能与回归测试（2026-06-13）**：
- 新增 `src/pretext_alignment.rs`（23 个样本 + 1000 条性能基准）。
- 回归测试 `pretext_estimate_matches_rendered_height_for_agent_text` / `..._user_text` 各跑 23 个样本，验证 `estimate_height()` 与 `message_bubble()` 实际高度差 ≤ 32 px（Agent）/ ≤ 48 px（User）。
- 性能基准 `pretext_message_list_performance_1000` 标记 `#[ignore]`，release 下可测量 1000 条消息的 pretext 估算与 rich paragraph 渲染耗时；实测 **estimate ≈ 74.4 µs/msg、render ≈ 135.7 µs/msg，聚合高度偏差 ≈ 1.45%**。
- 修复 `estimate_height_pretext()` 的 padding 常数：User bubble 按实际 `inner_margin 14×2 + space_16` 计为 44 px；Agent plain 保持 12 px；Agent card（blocks / structured）按 `inner_margin 12×2 + space_16` 计为 40 px，使估算高度与 `message_bubble()` 实际渲染高度对齐。
- 顺手修复 `crates/clarity-thread-store/src/local.rs` 测试断言与 `std::sync::Arc` 未使用导入。

**Phase 1.5 — 状态机迁移（2026-06-13）**：
- 移除 `clarity-egui/src/main.rs` 顶部全局 `#![allow(dead_code)]`。
- 所有遗留 boolean 标志迁移到 `clarity-core::ui::ViewState` 已有类型：
  - Modal：`team/cron/task/subagent/snapshot/settings` 的 `*_modal_open` → `view_state.modal: Option<ModalType>`。
  - Turn：`chat_store.is_loading/compacting/stopping` 与 `snapshot_store.restoring` → `view_state.turn: TurnState`。
  - Expansion：`cron/web_tabs/thinking_log/tools/subagents/workspace_plan` → `view_state.expansions: PanelExpansion`。
- 顺手修复 `clarity-gateway/src/main.rs` 测试中的 `await_holding_lock`（`std::sync::Mutex` → `tokio::sync::Mutex`）。

**Phase E — 设计系统替换（已完成，2026-06-15）**：
- 扩展 `design_system` 原语：新增 `Surface::Warning`、`Surface::Well`、`Text::CaptionStrong`；精简未使用原语，`design_system.rs` 不再保留模块级 `#[allow(dead_code)]`。
- 右 rail 全部卡片迁移到语义原语：`status_card.rs`、`tools_card.rs`、`subagent_card.rs`、`memory_card.rs`、`context_card.rs`、`progress_card.rs`。
- 关键 widgets 迁移：`provider_row.rs`、`sidebar_card.rs`、`user_avatar.rs` 使用 `design_system::text/gap/row/center/surface/status_dot/btn/scroll`。
- 删除被 `design_system::status_dot` 取代的 `widgets/status_dot.rs`。
- 验收：`cargo clippy -p clarity-egui --bins --tests -- -D warnings` ✅、`cargo test -p clarity-egui --bins` ✅（116 passed / 0 failed / 2 ignored）。

- **布局比例 token 调整**：`size_sidebar` 220→200、`size_input` 72→88、`window_default_w/h` 900×700→1280×800；默认窗口比例更接近概念图与 Kimi 参考。
- **聊天区结构重构**：`CentralPanel` 水平内边距清零；`chat_header` 撑满中间列全宽；消息列表仍按 `content_max_width` 居中；底部输入栏保持居中。
- **Header 右栏切换按钮修复**：通过 `right_to_left` 布局将右栏抽屉与上下文切换图标推至最右侧，解决了此前在居中 Ui 内部右对齐被 clip 的问题。
- **人机协作图片标注器**：新增 `assets/ui_annotator.html`（单文件零依赖）、`assets/ui-annotator-schema.md`、`assets/render_annotations.py`；支持拖框、标签、移动/缩放、JSON 导入导出、`localStorage` 自动保存；统一红/绿/蓝/黄颜色语义，便于将用户框选直接转译为 egui 布局代码。

**验证结果**：`cargo fmt --all -- --check` ✅、`cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` ✅、`cargo test --workspace --lib --exclude clarity-slint` ✅（1147 passed / 0 failed / 8 ignored）、`cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2` ✅（171 passed / 0 failed / 2 ignored）、`cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2` ✅（34 passed / 0 failed / 3 ignored）、`cargo test -p clarity-integration-tests --lib` ✅（26 passed / 0 failed）。

---

## 1. 核心指标（实测数据）

| 指标 | 实测结果 | 评估 |
|------|---------|------|
| **编译检查** | `cargo check --workspace --lib --bins --exclude clarity-slint` | ✅ 零错误 |
| **单元测试** | **1147 passed, 0 failed, 8 ignored**（`--workspace --lib --exclude clarity-slint`） | ✅ 全绿 |
| **Binary 测试** | **171 passed, 0 failed, 2 ignored**（`--workspace --bins --exclude clarity-slint`） | ✅ 全绿 |
| **集成测试** | **26 passed, 0 failed**（`cargo test -p clarity-integration-tests --lib`） | ✅ 全绿 |
| **Doc Tests** | **34 passed, 0 failed, 3 ignored**（`cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2`） | ✅ 全绿 |
| **Rustdoc** | `cargo doc --workspace --no-deps --exclude clarity-slint` | ✅ 无警告 |
| **Clippy 检查** | `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` | ✅ **零警告** |
| **安全审计** | `cargo audit --deny unsound --deny yanked` + [`THREAT_MODEL.md`](../security/THREAT_MODEL.md) | ✅ Dependabot #22/#23 已修复；STRIDE 威胁模型 16 条已建档，6 项未缓解风险已排期 |
| **代码规模** | ~200 个 Rust 源文件 | 持续增长 |
| **Workspace Crates** | 17 + 1 集成测试 crate（`clarity-tauri` 已归档排除） | 结构稳定 |

**测试覆盖详情**（lib 目标）：
- `clarity-channels`: 49 passed
- `clarity-claw`: 18 passed
- `clarity-contract`: 47 passed
- `clarity-core`: 557 passed, 1 ignored
- `clarity-llm`: 63 passed, 1 ignored
- `clarity-mcp`: 37 passed
- `clarity-wire`: 13 passed, 1 ignored
- `clarity-memory`: 97 passed
- `clarity-secrets`: 5 passed
- `clarity-tools`: 99 passed, 5 ignored
- `clarity-subagents`: 37 passed
- `clarity-telemetry`: 8 passed
- `clarity-gateway`: 62 passed
- `clarity-thread-store`: 13 passed
- `clarity-rollout`: 6 passed
- `clarity-tui`: 10 passed（lib tests）+ 36 passed（bin tests）
- `clarity-headless`: bin-only，16 passed（bin tests）
- `clarity-egui`: bin-only；binary 测试 116 passed / 0 failed / 2 ignored
- `clarity-integration-tests`: 26 个集成测试（adaptive_loop ×8 / session_v2_migration ×4 / telemetry_end_to_end ×4 / thread_api ×10）
- 各 crate `tests/` 目录：gateway/http/ws/webhook、core_wire、memory_persistence、mcp_end_to_end 等约 70 个测试

**前端测试**：`clarity-egui` binary 测试 116 passed / 0 failed / 2 ignored；新增 23 样本 pretext 对齐回归 + 1000 条消息性能基准（`#[ignore]`）

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
✅ 多 LLM 支持 — Anthropic、Kimi、OpenAI、DeepSeek、Ollama、Local (GGUF)；TOML 驱动 `ModelRegistry` 支持自定义 provider
✅ MCP 生态 — stdio / HTTP / SSE 三协议完整实现
✅ 环境变量注入 — API Key 支持 `${env:VAR_NAME}` 语法，避免明文落盘
✅ Settings 增量保存 — `merge_json` 只写入变更字段，保留未知配置
✅ 离线模式 — 网络探测 + 自动 fallback 到本地模型 + 恢复后切回
✅ Skill 系统 — Markdown+YAML 编排，关键字搜索，工具白名单
✅ Wire 事件总线 — SPMC 跨模块通信
✅ LSP 代理 — rust-analyzer 等语言服务器进程管理 + JSON-RPC 调试
✅ Computer Use — 远程桌面控制面板（截图/点击/输入/滚动）
✅ 动态系统提示 — SystemPromptBuilder 条件组装
✅ 模型热切换 — Settings Panel 中 provider/model 切换无需重启
✅ **EndpointDescriptor 抽象** — 统一端点契约（Persona / Site / Frontend），ADR-015
```

### 2.2 记忆系统（clarity-memory）

```
✅ SQLite 持久化 — PersistentMemoryStore
✅ BM25 + Hybrid 搜索 — FTS5 召回 + 内存 BM25 重排序
✅ 语义搜索 — `SemanticIndex` + `MemoryStore::search_semantic`（本地 TF-IDF 余弦相似度）
✅ RAG Chunking — 可配置大小、重叠、分隔符
✅ 向量存储 — CosineIndex + TfidfVectorizer
```

### 2.3 Gateway（clarity-gateway）

```
✅ HTTP REST API — /v1/chat/completions, /v1/tasks, /v1/parallel, /api/files/*
✅ WebSocket — 实时事件流
✅ Session Store — SQLite 持久化（CRUD、消息追加、请求计数、过期清理）
✅ Admin API — /api/tools, /api/stats, /api/approval-mode, /api/config
✅ 多平台通道 — Discord/Slack/Telegram Webhook/Bot + `RetryPolicy` 指数退避
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
✅ 后台任务面板 — Cron/创建/列表/取消/启用，Gateway + local 双路径
✅ MCP 配置面板 — 服务器列表、启用/禁用、保存到 mcp.json
✅ 网络状态 Banner + Toast — 离线探测、fallback 提示
✅ 消息队列 — Streaming 时自动排队，完成后自动发送
✅ 附件拖拽 — 支持文件拖入作为附件
✅ **审批弹窗 UI** — Diff 预览 + Enter/Esc/Shift+Enter 快捷键 + 模态拦截
✅ **Plan 步骤可视化** — 实时状态列表 ⏳/▶️/✅/❌ + 步骤间取消
✅ **Skill 面板** — 浮动窗口 + ON/OFF 切换 + 元数据 + 🔄 刷新
✅ **Token 用量显示** — Session 累计格式化 + Sidebar 底部摘要
✅ **ViewState 状态机** — 七字段聚合替换 50+ 布尔标志（ADR-014）
✅ **Store 文件拆分** — `stores/mod.rs` 按域拆为 12 个子模块，保持原有导入路径
✅ **Design System 落地** — `mod design_system` 注册并每帧 `install_theme()`，语义化 UI 原语可用
✅ **布局外壳接入点** — `layout.rs` + `App::render_layout_shell()` 统一编排 chrome / 主视图 / 浮层 / 模态框
✅ **RenderLine Pipeline** — 13-variant 前端无关行原子 + markdown_to_lines + TUI 映射
✅ **行级导航** — j/k/g/G 焦点感知快捷键 + y 复制选中行
✅ **Persona Switcher** — titlebar pill + `egui::Area` popup + 持久化到 settings（S8 P3B.1）
✅ **布局几何精化（S6-C3）** — `CentralPanel` 去水平边距、`chat_header` 全宽、消息列表/输入栏居中、右栏切换按钮 far-right 定位修复
✅ **人机协作图片标注器** — `assets/ui_annotator.html` + schema + 批量渲染脚本，支持用户框选 UI 元素后直接转译为 egui 布局代码
✅ **红绿蓝黄布局诊断覆盖层** — `debug_overlay.rs` 可视化 `max_rect`/`clip_rect`/锚点/警告，快捷键 `Ctrl+Shift+L`
```

---

## 3. 前后端功能 Parity 矩阵（关键差距标注）

| 功能 | clarity-core | clarity-egui | clarity-tui | clarity-gateway | clarity-headless |
|------|:------------:|:------------:|:-------------:|:-----------:|:---------------:|:----------------:|
| Agent 运行/流式 | ✅ | ✅ | ✅ | ✅ | ✅ |
| 工具调用可视化 | ✅ (产生) | ✅ | ✅ | ❌ | ❌ |
| Compaction Banner | ✅ (产生) | ✅ | ❌ | ❌ | ❌ |
| **审批交互 UI** | ✅ (后端) | ✅ | ❌ | ❌ | CLI only |
| **Plan 模式可视化** | ✅ (后端) | ✅ | `/plan` | ❌ | CLI only |
| **子代理/并行执行** | ✅ | ❌ | ❌ | ✅ | ❌ |
| 后台任务面板 | ✅ (完整) | 只读 | 命令行 | API only | ❌ |
| 后台任务创建/取消 | ✅ | ❌ | ✅ | ✅ | ❌ |
| Cron 调度 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 团队协调 (Team) | ✅ | ❌ | ❌ | ❌ | ❌ |
| **技能系统 UI** | ✅ | ✅ | ✅ | ❌ | ❌ |
| MCP 配置/管理 | ✅ | 配置面板 | ❌ | ❌ | ❌ |
| MCP 工具执行 | ✅ | 间接 | 间接 | 间接 | 间接 |
| 记忆提取/存储 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 会话持久化 | ✅ | ✅ | ❌ | ✅ (SQLite) | ❌ |
| **Token 用量显示** | ✅ | ✅ | ✅ | ❌ | ✅ |
| LSP 集成 | ✅ (core 支持) | ❌ | ❌ | ❌ | ❌ |
| 模型下载 GUI | ❌ (非 core 职责) | ❌ | ❌ | ❌ | ❌ |
| 日志面板 | ❌ | ❌ | ❌ | ❌ | ❌ |

**最大差距**：egui 仍缺少 core 已实现的**子代理进度面板**、**团队协调 UI**（Team 持久化启动同步待修复）。后台任务、Cron 调度、审批、Plan、Skill、Token 已在 Sprint 12 补齐。

### 2.7 Sprint 9 — 服务商支持硬化（2026-04-29）

| 阶段 | 内容 | 状态 |
|------|------|------|
| Phase 1 | API Key `${env:VAR}` 注入 + Settings 增量保存 | ✅ 完成 |
| Phase 2 | `ModelRegistry` 接入 egui UI，消除 Provider 硬编码 | ✅ 完成 |
| Phase 3 | 多模型角色分工（chat/utility/utility_large）| ⏸️ 冻结 |

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
| I1.6 | API Key 明文落盘 | 支持 `${env:VAR}` 语法注入 | ✅ 已修复 |
| I1.7 | Settings Save 覆盖全配置 | 增量 `merge_json` 只写变更字段 | ✅ 已修复 |

**修复 commit**：`ff3227d`（Phase 1 维护批次）

### I2. egui 测试覆盖（P1 技术债务）

`clarity-egui` 当前 **89 passed / 0 failed**（binary 目标），覆盖 `app_state` / `settings` / `provider` / `theme` / `widgets` / `shortcuts` / `window_manager` 等纯逻辑模块。剩余问题：
- 面板级渲染逻辑与 `update()` 热路径仍依赖人工验证；
- UI 集成 / snapshot 测试待 Pretext 三栏布局稳定后补齐。

### I3. egui 交互型功能缺口（P2）

**状态**: Sprint 12（2026-04-28）已补齐审批弹窗、Plan 可视化、Skill UI、Token 用量显示。
**剩余缺口**: 子代理进度面板、Team 持久化启动同步、TaskBoard 视图渲染、模型下载 GUI、日志面板。后台任务创建/取消、Cron 调度 UI 已闭环。

---

## 5. 安全状态

| 漏洞 | 状态 | 修复版本 |
|------|------|----------|
| S1: `resolve_path` 目录遍历 | ✅ 已修复 | v0.1.1 |
| S2: Gateway `sanitize_path` 目录遍历 | ✅ 已修复 | v0.1.1 |

**安全措施**：
- MCP 命令注入防护 — `validate_mcp_command()` 拦截 shell 元字符、相对路径
- 敏感文件检测 — `.env`、SSH key、kubeconfig 等自动识别
- TLS：`rustls-tls`（pure Rust），`openssl` 已从依赖树彻底移除
- 无 `unsafe` 代码块（仅 `clarity-memory` 1 处，已人工审批）

---

## 6. CI/CD 状态

| Job | 状态 | 说明 |
|-----|------|------|
| `check` | ✅ | `cargo check --workspace --exclude clarity-slint` |
| `test` | ✅ | `cargo test --workspace --lib --exclude clarity-slint` |
| `clippy` | ✅ | `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` |
| `fmt` | ✅ | `cargo fmt --all -- --check` |
| `audit` | ✅ | `cargo audit --deny unsound --deny yanked` |
| `doc-guard` | ✅ | README.md + AGENTS.md 存在性检查 + `cargo doc` + `cargo-modules` 结构验证 |
| `integration-test` | ✅ | `cargo test -p clarity-integration-tests --lib` |
| `binary-test` | ✅ | `cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2` |
| `doc-test` | ✅ | `cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2` |
| `coverage` | ✅ | `cargo llvm-cov --workspace --lib --exclude clarity-slint` + LCOV/HTML artifact |
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
| 文档过时 | ✅ 已解决 | 10 个 crate README + AGENTS.md 全覆盖，4 条 ADR，OPERATIONS.md + API_CONTRACT.md + THREAT_MODEL.md 新建 | 持续维护 |
| unwrap() 密度 | 🔄 持续 | Sprint 40 后生产代码 ~80（锁相关 154 处已清零） | 冻结新增，渐进清理 |
| cargo audit 上游漏洞 | ✅ 已解决 | openssl 彻底移除后 zero audit warnings | — |
| **clarity-egui 纯 UI 集成测试缺失** | 🟡 中等 | 89 个 binary 逻辑测试已注入，但面板级渲染/UI 集成仍靠人工验证 | Pretext 三栏布局稳定后引入 `egui_kittest` snapshot |
| **clarity-egui 审批 UI 缺失** | ✅ 已解决 | Interactive/Plan approval mode 在 GUI 中可用 | Sprint 12 |
| Settings 模型选择体验 | ✅ 已解决 | 无下拉列表、provider/model 不联动 | `ff3227d` |
| 服务商配置硬编码 | ✅ 已解决 | `get_available_models()` 硬编码 → `ModelRegistry` 动态读取 | Sprint 9 Phase 2 |
| LlmFactory 双轨制 | ✅ 已解决 | 4 个 provider-specific 方法标记 deprecated，新增 Provider 新增检查单 | Sprint 10 D2 |
| approval_mode 暴露不可用 | ✅ 已解决 | `CapabilityRegistry` 按 surface 暴露可用模式，egui 仅 yolo | Sprint 10 D3 |
| AgentProfile 配置 | ✅ 已解决 | `profiles.toml` + `GuiSettings` 扩展，支持多模型角色切换 | Sprint 10 D1 |

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
| T_KIMICLI_REF | 借鉴 Kimi CLI settings/模型选择设计 | ⏸️ 冻结 | 仅作设计参考，不推进实现。归档于 `docs/planning/plans/2026-04-27-egui-pretext-health-plan.md` |
| T_APPROVAL_V2 | AI 分类器混合审批 | ⏸️ 冻结 | 约束解除前不投入 |
| T_SHORTCUTS | 快捷键系统 | ⏸️ 冻结 | 约束解除前不投入 |
| T_MOBILE | Mobile 适配 | ⏸️ 冻结 | 约束解除前不投入 |

---

## 11. 合规与隐私记录

### 2026-05-15 隐私整改

**问题**：旧文档使用真实人名作为默认 persona 标识符，被写入了代码与活跃文档。  
**整改**：已全部替换为 `"Kin"`（家人）+ 建立 `docs/security/PRIVACY_REVIEW.md` 工程规范。  
**Commit**：`cb4e9406`  
**验证**：`cargo check --workspace` + `cargo test --workspace --lib`（927 passed）+ `cargo test -p clarity-egui --bin clarity-egui`（72 passed）+ `cargo clippy`（零警告）全部通过。

*本文件随版本发布同步更新。上次全面审计：2026-05-08（Sprint 40 + 文档治理完成后）。*
