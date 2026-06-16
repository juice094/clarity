---
title: 当前阶段与已知问题
category: Planning
date: 2026-06-15
tags: [planning, status, current-phase]
---

# 当前阶段与已知问题

> 详细项目状态、测试基线、功能清单与技术债务见 [`PROJECT_STATUS.md`](./PROJECT_STATUS.md)。历史 Sprint 摘要见 [`sprint-archive.md`](./sprint-archive.md)。

---

## 当前阶段

- **版本**：v0.3.0（`Cargo.toml`），v0.3.4-rc（开发中）
- **默认分支**：`main`
- **Rust 版本**：2024 edition · MSRV 1.85
- **许可证**：AGPL-3.0-or-later

### 近期交付焦点

- **Gray Migration 通道层**：ZeroClaw WeChat iLink 已移植至 `clarity-channels::zeroclaw`；Gateway 已切至 DeepSeek `deepseek-v4-pro`。
- **Provider/Secret 体系**：Stage A/B/C 已完成，支持 `enc2:` 加密 key、`models.toml` per-alias 配置、`ReliableProvider` 链式 failover、`runtime_router` 与 OAuth device flow。
- **egui 模块整理**：ViewState 单源化、panels/ 目录重组、widget 提取、design system 落地、layout shell 接入点。
- **S6 Pretext 三栏布局 Phase A/B/C3**：新增 `LeftRailSection` / `RightRailSection` 与 `ViewState` rail 字段；移除 `UiStore.sidebar_collapsed`；`clarity-egui` 形成左 icon rail + 中主舞台 + 右工具 rail 的单页面外壳；右 rail 卡片（Status / Tools / Subagents / Memory）已接入真实内容，`legacy/task.rs` 与 `legacy/team.rs` 已迁移删除；**S6-C3** 完成布局几何精化（默认窗口 1280×800、sidebar 200px、header 全宽、内容居中）与 header far-right 定位修复。
- **人机协作图片标注器**：新增 `assets/ui_annotator.html` + schema + `render_annotations.py`，建立“用户框选 → JSON → AI 生成/修正 egui 代码”的协作闭环。
- **Pretext PoC Phase 1~5 已完成**：`clarity-egui` 已接入 `pretext-core` / `pretext-fontdb`；`pretext::EguiFontMetrics` 用 egui 字体栈作为 measurement backend；`MessageBubble` 已迁移为 pretext-aware（`widgets/rich_paragraph.rs`）；默认启用 pretext 高度估算；23 样本对齐回归测试与 1000 条消息 release 性能基准通过（聚合高度偏差 ≈ 1.45%，estimate ≈ 74.4 µs/msg，render ≈ 135.7 µs/msg）。下一步可考虑虚拟列表裁剪、富文本渲染扩展或三栏布局更深度的 pretext 集成。
- **Phase 1.5 状态机迁移已完成**：所有遗留 boolean modal / turn / expansion 标志已迁移到 `view_state.modal` / `view_state.turn` / `view_state.expansions`；移除了 `clarity-egui` 全局 `#![allow(dead_code)]`。
- **Phase E 设计系统替换已完成**：右 rail 全部卡片（status / tools / subagent / memory / context / progress）与关键 widgets（provider_row / sidebar_card / user_avatar）已使用 `design_system` 语义原语；未使用原语已清理，`design_system.rs` 无模块级 `#[allow(dead_code)]`。

---

## 当前测试基线

| 测试类型 | 通过 | 失败 | 忽略 |
|----------|------|------|------|
| `cargo test --workspace --lib --exclude clarity-slint` | 1147 | 0 | 8 |
| `cargo test --workspace --bins --exclude clarity-slint` | 171 | 0 | 2 |
| `cargo test --workspace --doc --exclude clarity-slint` | 34 | 0 | 3 |
| `cargo test -p clarity-integration-tests --lib` | 26 | 0 | 0 |
| `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` | 0 warning | 0 | - |
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

*最后更新：2026-06-13*
