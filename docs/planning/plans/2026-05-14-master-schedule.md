---
title: Clarity 后续开发主计划书
category: Plan
date: 2026-05-14
tags: [plan]
---

# Clarity 后续开发主计划书

> **历史归档**：本计划书为 2026-05-15 的 Sprint S7/S8 主计划，已不反映当前开发状态。当前状态见 [`../current-phase.md`](../current-phase.md) 与 [`../../AGENTS.md`](../../AGENTS.md) §11。
>
> **Date**: 2026-05-15 (revised) | **Status**: S7 Phase 3A 已闭环；准备 S8 | **Owner**: juice094 + Clarity Agent
> **Single source of truth**: 本文件整合 `pretext-ui-evolution.md` + `ROADMAP.md` + `FUTURE_DIRECTION.md` + `ENGINEERING_PLAN.md`
> **基线 commit**: `edbb615f` on `main` | 所有前置 ADR: 011/012/013/014

> **2026-05-15 修订说明**: 经代码实证审计，S3/S4/S5/S6 的核心基础设施在 master-schedule 起草时（2026-05-14）已实际完成，本次修订将其状态标记为 ✅，并将焦点转移到 S7/S8。

---

## 1. 当前状态锚点

| 维度 | 状态 |
|------|------|
| 版本 | v0.3.3（已发布，含 Windows + Linux 双平台） |
| Rust 测试 | 全绿（含 19 个 S7 新增 parity gate） |
| Clippy | 零 warning（`-D warnings`） |
| `clarity-egui` 主要测试 | RenderLine pipeline 已接入；ChatArea line-mode flag 切换完成 |
| 主力 UI 栈 | egui 0.31（Tauri 已归档） |
| 状态机 | `ViewState` 已上线，bridge reversal 完成（P1.5.4d）|
| ShortcutRegistry | 焦点感知路由实现 + 12 tests pass |
| 右侧栏 | Tab D 形式（Team / Task / Dashboard 互斥） |
| Skill/MCP | 已重分类为 `ModalType`（真正模态）|
| RenderLine | 13-variant enum + markdown_to_lines + line_renderer + GUI/TUI 双端接入 |
| TUI parity | render_line_to_ratatui + 12 snapshot tests + plain_text parity gate |

---

## 2. 双维度路线图

### 2.1 Session 维度（Pretext UI 演进）

| Session | Phase | 内容 | 工时 | 状态 | 关键交付 |
|---------|-------|------|------|------|----------|
| S1 | 0.5 | 5 P0 阻塞项（token、focus ring、palette 执行） | 4h | ✅ | 审计闭环 |
| S2 | 1 | StripBuilder TitleBar + Lucide 迁移 | 5h | ✅ | RULE 6/7 |
| S3 | 1.5 | 状态机迁移 + bridge reversal + 非法状态测试 | ~8h | ✅ | ViewState 权威化 + 44 tests |
| S4 | 2A | `RenderLine` 13-variant（ADR-012） | 6h | ✅ | markdown→lines + 28 tests |
| S5 | 2B | 行渲染器 + 虚拟滚动 + j/k 导航 | 6h | ✅ | line_renderer.rs + cursor logic |
| S6 | 2C | ChatArea + Sidebar + Workspace 三栏 | 6h | ✅ | `line-mode` flag + Message::lines |
| **S7** | **3A** | **TUI parity + snapshot 测试** | **6h** | **✅** | **render_line_to_ratatui + 19 parity gates** |
| S8 | 3B | 信息架构重构（persona/notes/equipment/快捷键） | 6h | ⏸️ | ADR-011/013 落地 |
| S9 | 3C | 文档 + 性能基准 + 闭环 | 4h | ⏸️ | 4 份架构笔记 |

**已完成**: 47h | **剩余**: ~10h

### 2.2 Release 维度（版本里程碑）

```
2026-05-15 ── v0.3.3 已发布     S3/S4/S5/S6/S7 核心基础设施全部就绪
    │                            ├─ RenderLine + line_renderer
    │                            ├─ ChatArea line-mode 切换完成
    │                            ├─ TUI parity（19 gates）
    │                            └─ Windows + Linux 双平台二进制
    │
2026-05 末 ── v0.3.4 候选       S8 信息架构（如范围允许）
    │
2026-06 ── v0.4.0-beta          S9 闭环 + 性能基准 + Phase A 基础设施
    │                            ├─ WebSocket MCP 传输
    │                            ├─ Gateway↔BTM 集成
    │                            ├─ Worker 池自动扩缩容
    │                            └─ 性能验证：10K 消息 60fps
    │
2026-07 ── v0.5.0-beta          集群语义验证（FUTURE_DIRECTION Phase C）
2026-08 ── v0.6.0-rc            Sandbox + Plugin SDK
2026-09 ── v0.7.0-rc            Bridge + Voice + Canvas
2026-10 ── v1.0.0               稳定版发布
```

---

## 3. S8 工作（当前阻塞点）

| ID | 任务 | 预估 | 验收标准 |
|----|------|------|----------|
| P1.5.7 | 非法状态不可达性测试 | ~1h | `TurnState::from_legacy` 组合测试覆盖所有 2⁵ 种输入；非法组合（如 Loading+Compacting）断言为优先级高者 |
| P1.5.8 | `ARCHITECTURE.md` 状态机章节 | ~30min | §3.7 已补充；本节关闭 |
| P1.5.9 | `ShortcutRegistry::resolve()` 骨架 | ~1.5h | ADR-013 的 5 级焦点路由表编译通过；14+9+6 快捷键绑定注册为常量；`?` 帮助 overlay 数据源可用 |

**S3 验收总闸**:
- `cargo test -p clarity-core --lib ui::` 全绿（含新增测试）
- `cargo clippy --workspace --bins --tests -- -D warnings` 零警告
- `view_state.right: Option<SidePanel>` 为右侧栏唯一权威写入口
- `view_state.modal: Option<ModalType>` 包含 Skill/Mcp；legacy bools 为只读镜像

---

## 4. S4-S6 详细设计（RenderLine + 三栏）

### 4.1 S4 Phase 2A — RenderLine 基础（6h）

**输入**: ADR-012 锁定的 13 变体 + `LineRole` 参数化
**输出**: `clarity-core/src/ui/render_line.rs` + `markdown_to_lines()` + 13 变体单元测试

| 子任务 | 内容 | 时间 |
|--------|------|------|
| P2A.1 | 定义 `RenderLine` enum + `LineRole` + `DiffKind` + `StatusKind` | 1h |
| P2A.2 | `markdown_to_lines(md: &str) -> Vec<RenderLine>` via `pulldown-cmark` | 3h |
| P2A.3 | 单元测试：heading/list/code/table/blockquote 覆盖 | 1.5h |
| P2A.4 | 文档：`docs/architecture/render-line.md` | 30min |

### 4.2 S5 Phase 2B — 渲染器 + 交互（6h）

| 子任务 | 内容 | 时间 |
|--------|------|------|
| P2B.1 | `render_lines(ui, &[RenderLine], theme)` in `clarity-egui/src/ui/line_renderer.rs` | 2h |
| P2B.2 | 虚拟滚动：`scroll_offset / line_height` 精确像素计算 | 1.5h |
| P2B.3 | 键盘导航：`j/k/g/G/Enter/Esc`（焦点范围按 ADR-013） | 2h |
| P2B.4 | 流式追加：按 `\n` 逐行 flush | 30min |

### 4.3 S6 Phase 2C — 三栏迁移（6h）

| 子任务 | 内容 | 时间 |
|--------|------|------|
| P2C.1 | `line-mode` feature flag | 15min |
| P2C.2 | `Message::lines` 字段，与 `parsed` 并存 | 2h |
| P2C.3 | ChatArea 从 `lines` 渲染（flag 启用时） | 2h |
| P2C.4 | Sidebar + Workspace 迁移为 line-rows | 1.5h |
| P2C.5 | `BlockSlot` fallback 接入 table/image | 15min |

**性能目标**: 60fps @ 10K lines / 1MB markdown

---

## 5. S7-S9 详细设计（TUI Parity + 信息架构 + 闭环）

### 5.1 S7 Phase 3A — TUI Parity（6h）

- `clarity-tui` 接入 `ViewState` + `RenderLine`
- ANSI 渲染（box-drawing chrome）
- Snapshot 测试：同 fixture → GUI + TUI，文本内容 assert 匹配

### 5.2 S8 Phase 3B — 信息架构（6h）

**已锁定设计**（ADR-011/013/014）：

| 区域 | 设计 | 实现任务 |
|------|------|----------|
| Top Bar | Persona switcher + Session tabs + Orchestrate badge + Cluster indicator | P3B.1-P3B.3 |
| Left Panel | Sessions + Pinned + Notes（5 种 sticky note） | P3B.4 |
| Center | Z-form：默认行流 + `⤢ Expand` → BlockSlot 全屏；Esc 返回 | P3B.5 |
| Right Panel | Tab D：SSH / Workspace（三层文件树）/ Settings | P3B.6 |
| Status Bar | Equipment 区（`🎯 📋 🔌` 点击展开浮动面板） | P3B.7 |
| Shortcuts | `ShortcutRegistry::resolve(key, scope)` + 29 绑定 + `?` overlay | P3B.8 |

### 5.3 S9 Phase 3C — 闭环（4h）

- `docs/architecture/ui-axis.md`（grid-vs-cursor 分类）
- `egui-layout-canons` SKILL 更新
- CHANGELOG 全阶段条目
- 性能基准：GUI 60fps / TUI 60Hz
- CI 跨平台回归门

---

## 6. 长期架构演进（FUTURE_DIRECTION Phase A→D）

| Phase | 目标 | 版本 | 关键交付 |
|-------|------|------|----------|
| **A** | 基础设施联通 | v0.3.3 | WebSocket MCP / Gateway↔BTM 集成 / Worker 池扩缩容 / 跨会话检索 |
| **B** | 会话层统一 | v0.3.4 | SQLite 单一事实来源 / SessionManager / Session Handoff |
| **C** | 运行时重构 | v0.4.0~v0.5.0 | AgentPool / Identity 路由 / Wire 跨 Agent / IPC 回环 / 多窗口 |
| **D** | 跨设备验证 | v0.5.0+ | Syncthing P2P / Session CRDT / Agent 状态迁移 |

**约束**: Phase A-D 不新增 crate，只重构现有 9 crate。

---

## 7. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| S4 RenderLine 变体覆盖不足 | 高 | `BlockSlot` 为 table/image 提供逃生舱；旧 `RenderBlock` 路径保留 2 个 release |
| S6 ChatArea 迁移引入滚动回归 | 高 | `line-mode` feature flag；默认关闭直到基准通过 |
| S8 信息架构改动面大 | 中 | 每个 affordance 独立 feature flag；persona/orchestrate/cluster 可分别开关 |
| Phase C 重构破坏现有 egui | 高 | `AgentController` 向后兼容 API；`AgentPool` 作为包装层 |
| Session 迁移丢失数据 | 高 | 迁移工具支持 dry-run + `.bak`；删除 JSON 前跑完整验证套件 |
| 项目广度超限 | 高 | Phase A-D 冻结新增 crate；Mobile/Plugin/Voice 已 veto |

---

## 8. 冻结项（约束解除前零投入）

| 项 | 冻结原因 | 解除条件 |
|----|----------|----------|
| Mobile iOS/Android | 项目广度 > 5 核心工具 | v0.4.0 社区反馈 ≥ 50 stars |
| Plugin SDK / WASM | 安全边界未定 | Sandbox 技术选型完成 |
| Voice / Canvas | 非核心路径 | 本地 Whisper/TTS 方案验证 |
| Pretext 重型 TeX 引擎 | 重型移植工程 | 仅作个人探索，不入主 repo |
| T_APPROVAL V2 | 约束解除前不投入 | LLM 分类器方案验证完成 |

---

## 9. Hard Veto 边界（不可逾越）

| 约束 | 说明 |
|------|------|
| 本地 LLM 优先 | 任何新功能必须支持离线模式；云端是可选增强 |
| 禁止数据外泄 | API key 不离开本机；Session 数据本地持久化 |
| 禁止 Docker / Electron | 无容器化/浏览器运行时依赖 |
| 禁止 RAG(Qdrant) | SQLite + BM25 + CosineIndex 已足够 |
| 项目广度 ≤ 5 核心工具 | 新增功能需裁减旧功能，或放入冻结区 |
| Rust 核心不外包 | 子 Agent 可辅助调研，但核心模块代码必须由本机 Agent 审查 |

---

## 10. 验收标准（每次提交到 main 前）

```bash
cargo test --workspace --lib              # 全绿
cargo clippy --workspace --lib --bins --tests -- -D warnings  # 零 warning
cargo fmt --all -- --check                # 格式检查
cargo audit --deny unsound --deny yanked  # 无高危漏洞
cargo doc --no-deps                       # 零 warning
```

---

## 11. 决策日志

| Date | 决策 | 来源 |
|------|------|------|
| 2026-05-12 | StripBuilder 替代 hand-rolled layout | Audit §F |
| 2026-05-12 | Phase 1.5 纳入（不减） | 用户：engineering-dimension gains 计入 |
| 2026-05-13 | 右面板 = Tab D（SSH/Workspace/Settings） | 用户对话 + ADR-011 |
| 2026-05-13 | 中心面板 = Z-form（行流 + BlockSlot） | 用户对话 + ADR-011 |
| 2026-05-13 | RenderLine = 13 变体 | ADR-012 |
| 2026-05-13 | Equipment 区在底部 Status Bar | 用户对话 |
| 2026-05-13 | `Ctrl+S` 焦点范围化（Workspace tab only） | ADR-013 |
| 2026-05-13 | 快捷键深度参考 ClaudeCode | ADR-013 |
| 2026-05-14 | 创建本主计划书作为单一事实来源 | 本文件 |

---

*本文件由 AI Agent 维护，人类开发者可直接编辑。每次 Session 启动时首先对照本节确认当前相位。*
