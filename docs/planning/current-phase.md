---
title: 当前阶段与已知问题
category: Planning
date: 2026-06-25
tags: [planning, status, current-phase]
---

# 当前阶段与已知问题

> 详细项目状态、测试基线、功能清单与技术债务见 [`PROJECT_STATUS.md`](./PROJECT_STATUS.md)。历史 Sprint 摘要见 [`sprint-archive.md`](./sprint-archive.md)。

---

## 当前阶段

- **版本**：v0.4.0（`Cargo.toml`）
- **默认分支**：`main`
- **Rust 版本**：2024 edition · MSRV 1.85
- **许可证**：AGPL-3.0-or-later

### 近期交付焦点

- **Gray Migration 通道层**：WeChat iLink 实现位于 `clarity-channels::chkit`；Gateway 已切至 DeepSeek `deepseek-v4-pro`。
- **Provider/Secret 体系**：Stage A/B/C 已完成，支持 `enc2:` 加密 key、`models.toml` per-alias 配置、`ReliableProvider` 链式 failover、`runtime_router` 与 OAuth device flow。
- **egui 模块整理**：ViewState 单源化、panels/ 目录重组、widget 提取、design system 落地、layout shell 接入点。
- **S6 Pretext 三栏布局 Phase A/B/C3**：新增 `LeftRailSection` / `RightRailSection` 与 `ViewState` rail 字段；移除 `UiStore.sidebar_collapsed`；`clarity-egui` 形成左 icon rail + 中主舞台 + 右工具 rail 的单页面外壳；右 rail 卡片（Status / Tools / Subagents / Memory）已接入真实内容，`legacy/task.rs` 与 `legacy/team.rs` 已迁移删除；**S6-C3** 完成布局几何精化（默认窗口 1280×800、sidebar 200px、header 全宽、内容居中）与 header far-right 定位修复。
- **人机协作图片标注器**：新增 `assets/ui_annotator.html` + schema + `render_annotations.py`，建立“用户框选 → JSON → AI 生成/修正 egui 代码”的协作闭环。
- **Pretext PoC Phase 1~5 已完成**：`clarity-egui` 已接入 `pretext-core` / `pretext-fontdb`；`pretext::EguiFontMetrics` 用 egui 字体栈作为 measurement backend；`MessageBubble` 已迁移为 pretext-aware（`widgets/rich_paragraph.rs`）；默认启用 pretext 高度估算；23 样本对齐回归测试与 1000 条消息 release 性能基准通过（聚合高度偏差 ≈ 1.45%，estimate ≈ 74.4 µs/msg，render ≈ 135.7 µs/msg）。下一步可考虑虚拟列表裁剪、富文本渲染扩展或三栏布局更深度的 pretext 集成。
- **Phase 1.5 状态机迁移已完成**：所有遗留 boolean modal / turn / expansion 标志已迁移到 `view_state.modal` / `view_state.turn` / `view_state.expansions`；移除了 `clarity-egui` 全局 `#![allow(dead_code)]`。
- **Phase E 设计系统替换已完成**：右 rail 全部卡片（status / tools / subagent / memory / context / progress）与关键 widgets（provider_row / sidebar_card / user_avatar）已使用 `design_system` 语义原语；未使用原语已清理，`design_system.rs` 无模块级 `#[allow(dead_code)]`。
- **方向 B（OpenClaw 连接管理）**：修复 `openclaw_pair` 示例 token 保存 URL；token 支持 `${env:VAR}` 解析；`OpenClawAuthMode` 在发现与连接逻辑中生效；Settings 新增 **Claw** 标签页，支持增删改查远程连接；egui 内实现完整设备配对流程（请求 → 等待审批 → 保存 token）。
- **方向 C（OKF 前端接入）**：新增 `KnowledgeStore`；右 rail `Knowledge` 面板支持输入 OKF bundle 路径、Load/Reload、搜索概念、浏览概念列表并查看详情（frontmatter + Markdown 正文）。
- **方向 D（Gateway `/ws` 回路硬化）**：修复原生 Gateway WebSocket 聊天在服务端挂起的问题；`/ws` 现在与 HTTP `/v1/chat/completions` 共用 `AgentController` 流式路径，并补全 `WsResponse::Done` 作为 turn 结束标记，移动端可正确收到 `TurnEnd`。
- **方向 E（Knowledge Field 端到端接入）**：`clarity-knowledge` 完成 `KnowledgeGraph` 激活/传播/抑制/衰减/休眠（Phase 1）；`clarity-core` 在对话 turn 中自动提取 wikilink / `.md` 链接注入激活，并将 `MemoryCompiler` 产物索引到知识场（Phase 2）；`clarity-egui` 右 rail Knowledge 面板接入 `KnowledgeField`，支持搜索框查询、Top active 节点浏览与详情展示（Phase 3）；新增 `KnowledgeField::index_directory` / `Agent::index_vault`，支持把外部 Markdown vault 索引到知识场（Phase 4a）；新增 `KnowledgeField::apply_watcher_event` / `KnowledgeStore::start_watching_vault`，实现 vault 变更的增量同步（Phase 4b），形成 **外部笔记 → 知识场 → 会话引用 → 增量同步** 的闭环。全 workspace 测试通过。

---

## 当前测试基线

| 测试类型 | 通过 | 失败 | 忽略 |
|----------|------|------|------|
| `cargo test --workspace --lib` | 2037 | 0 | 13 |
| `cargo test --workspace --bins -- --test-threads=2` | 339 | 0 | 2 |
| `cargo test --workspace --doc -- --test-threads=2` | 41 | 0 | 12 |
| `cargo test -p clarity-integration-tests --lib` | 37 | 0 | 0 |
| `cargo clippy --workspace --lib --bins --tests -- -D warnings` | 0 warning | 0 | - |
| `cargo fmt --all -- --check` | pass | 0 | - |

---

## 已知问题与注意事项

| 问题 | 状态 | 说明 |
|------|------|------|
| Discord/Telegram 默认禁用 | 🔒 等待上游 | `rustls-webpki` CVEs via `serenity` |
| Gateway HTTP Chat Completions 默认无状态 | 📝 设计如此 | WebSocket 支持完整 session；HTTP 可传 `session_id` |
| Tokenizer 离线依赖 | ✅ 已缓解 | 自动检测同目录 `tokenizer.json`；损坏文件 (<1KB) 报错 |
| 文件 sniff 误报 | ✅ 已修复 | 已知文本扩展名 bypass magic sniff |
| 跨目录文件读取 | ✅ 已修复 | `resolve_path()` 允许绝对路径直接通过 |
| Windows bash 工具注册 | ✅ 已修复 | Windows 仅注册 PowerShellTool |

---

- **前端架构审计与 P6 收尾（2026-07-06）**：完成 `docs/planning/architecture-audit-2026-07-06.md`；落地路由去重、虚拟列表高度缓存、右 rail 同步硬化、语言持久化、Plugins 导航语义修正 5 项改造；全 workspace `lib` / `bin` / `doc` / `clippy` / integration 测试通过。

---

*最后更新：2026-07-06*
