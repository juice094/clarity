---
title: Phase E — clarity-egui Design System Rollout
category: Plan
date: 2026-06-13
tags: [plan, ui, design_system]
---

# Phase E — clarity-egui Design System Rollout

> **状态**: 已完成（2026-06-15）。
> **前置依赖**: Phase 1.5（状态机迁移）。本阶段只替换视觉原语，不修改状态逻辑。
> **目标**: 用 `design_system.rs` 语义原语替换手写 `Frame::new()` / `RichText::new()` / `add_space()`，并清理未使用原语，使 `design_system.rs` 无模块级 `#[allow(dead_code)]`。

---

## 一、当前可用原语清单

| 原语 | 作用 | 替换目标 |
|------|------|----------|
| `surface(Surface::Card/Well/Warning, \|ui\| ...)` | 语义化背景/边框/圆角/阴影 | 手写 `Frame::new().fill().stroke().corner_radius()` |
| `text(ui, content, Text::...)` | 语义化文字 | 手写 `ui.label(RichText::new(...).size().color().strong())` |
| `gap(ui, Space::S0/S1/S2/S3/S6)` | 间距 token | `ui.add_space(theme.space_4/8/12/16/40)` |
| `row / center / push_right` | 布局 helper | 裸 `ui.horizontal` / `ui.vertical_centered` / 右推 |
| `btn(ui, label, ButtonStyle::Primary/Secondary/Danger/Ghost)` | 语义按钮 | 手写 `Button::new(RichText...).fill().corner_radius()` |
| `scroll(ui, Scroll::VerticalMax(h), \|ui\| ...)` | 滚动区 | 手写 `ScrollArea::vertical().max_height(h)` |
| `status_dot(ui, Status::Online/Offline)` | 状态指示 | 手写 painter 圆点 / bullet label |

> 注：pilot 阶段曾 staged 的 `heading`、`row_align`、`col`、`btn_icon`、`modal`、`tab_bar`、`input`、`status_badge`、`responsive` 等原语在本次清理中已移除；如后续有真实消费者，可从 git 历史恢复。

---

## 二、已完成的迁移

### Wave 1 — 纯 Widget
- ✅ `widgets/provider_row.rs` — row + text + status_dot。
- ✅ `widgets/sidebar_card.rs` — text + gap + status_dot。
- ✅ `widgets/user_avatar.rs` — text。
- ✅ 删除 `widgets/status_dot.rs`（功能由 `design_system::status_dot` 覆盖）。

### Wave 2 — 右侧面板 Cards
- ✅ `panels/right_rail/status_card.rs`
- ✅ `panels/right_rail/tools_card.rs`
- ✅ `panels/right_rail/subagent_card.rs`
- ✅ `panels/right_rail/memory_card.rs`
- ✅ `panels/right_rail/context_card.rs`
- ✅ `panels/right_rail/progress_card.rs`

### Wave 3~5 — 未来可选
以下文件仍有手写 `Frame` / `RichText` / `add_space`，但已不在 Phase E 强制范围内；后续如需继续统一视觉语言，可逐文件按相同模式迁移：

- `panels/chat/header.rs`、`panels/chat/message_list.rs`、`panels/chat/input/tui_style.rs`
- `components/chat/conversation.rs`、`components/web_tabs.rs`、`components/thinking_log.rs`、`components/tools_section.rs`
- `panels/modals/*.rs`、`panels/settings/*.rs`
- `main.rs`（标题栏、状态胶囊、布局外壳）、`panels/legacy/*.rs`、`panels/system/dashboard.rs`

---

## 三、验收标准

1. ✅ `cargo clippy -p clarity-egui --bins --tests -- -D warnings` 通过。
2. ✅ `cargo test -p clarity-egui --bins` 通过（116 passed / 0 failed / 2 ignored）。
3. ✅ `design_system.rs` 中所有公共原语均有真实消费者；未使用原语已移除，无模块级 `#[allow(dead_code)]`。
4. ✅ 替换后全 workspace CI 通过：fmt / check / clippy / lib+bin+doc+integration tests。

---

## 四、风险与回退

- **风险**: `design_system` 获取 theme 走 `egui::Context::data()`，无 `ctx` 的纯函数场景可能无法使用。
  - **缓解**: 测试中使用 `run_in_frame` 提供 ctx；面板代码自然带有 ctx。
- **风险**: 替换 `add_space(theme.space_X)` 时若原代码使用非 token 值，会改变间距。
  - **缓解**: 只替换等于 `space_4/8/12/16/40` 的常量；特殊值保留。
- **风险**: `surface()` 默认 padding 可能与原手写 margin 不同。
  - **缓解**: 逐文件比对原 margin；新增 `Surface::Well` 等变体覆盖小 padding 场景。

---

*Phase E 已完成。后续 Wave 3~5 为可选的视觉一致性增强，不设 deadline。*
