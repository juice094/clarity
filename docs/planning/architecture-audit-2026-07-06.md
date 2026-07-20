# Clarity 前端/核心 UI 架构审计报告

> 生成时间：2026-07-06
> 范围：`clarity-egui` / `clarity-apps` / `clarity-core::ui` / `clarity-ui`
> 审计方法：基于源码阅读与运行基线复核（`cargo test -p clarity-egui`、`cargo clippy --workspace --lib --bins --tests` 已绿）

## 1. 审计目标

对 Clarity 前端应用架构与核心 UI 状态进行结构化审计，识别：

1. 模块边界与职责错位
2. 状态管理单源真相（single source of truth）缺口
3. UI 一致性与 DESIGN_PROTOCOL 合规性
4. 交互模式与现代化体验差距
5. 性能/资源热点

并输出可执行的改造计划，落地优先级最高的 3–5 项改造。

---

## 2. 关键结论总览

| # | 优先级 | 类别 | 关键文件 | 问题 | 建议方案 | 规模/风险 |
|---|--------|------|----------|------|----------|-----------|
| 1 | P0 | 资源 | `panels/chat/message_list.rs` | 虚拟列表最后一 agent turn 每帧重建 `AgentTurn`；`compute_unit_estimates` 与 `estimate_total_height` 在 Idle 时仍全量重算 | 当 `TurnState::Idle` 且最后 turn 未变更时缓存 `AgentTurn`；为总高度估算加 `(units_len, max_w, editing_idx, turn_state)` 缓存键 | 中/中 |
| 2 | P1 | 状态 | `clarity-core/src/ui/router.rs` | `Router::navigate` 无条件 `push`，重复路由会无限堆叠 | `navigate` 对相同路由做去重/替换；提供显式 `push_distinct` 语义 | 小/低 |
| 3 | P1 | 状态 | `panels/right_ide_panel/mod.rs` + `main.rs` | 右 rail 存在 `right_rail_router` 与 `egui_dock::DockState` 双写；dock 切 tab 时 `reset` 会清空历史 | dock 仅作为路由器的视图反映；切 tab 时用 `replace` 或带去重的 `navigate` 回写 | 中/中 |
| 4 | P1 | 持久化 | `app_logic.rs`, `settings_panels/interface_tab.rs` | `GuiSettings` 已有 `language` 字段，但启动时未读取；语言切换只改内存 | 启动时从 `settings_edit.language` 恢复 `locale`；切换语言时写回 settings 并 `auto_save_settings` | 小/低 |
| 5 | P1 | 交互 | `panels/navigation_tree/nav_items.rs` | 左侧导航「Plugins」实际打开的是 MCP modal，名实不符 | 改为打开统一 plugin picker（与 composer `/` 一致）并聚焦输入框 | 小/低 |
| 6 | P1 | 资源 | `widgets/rich_paragraph.rs` | 每帧对所有 span 做哈希构造 `LayoutKey` | 将 `Message` 的 span fingerprint 提前计算，theme 切换时失效 | 小/低 |
| 7 | P2 | UI | `clarity-apps/src/settings.rs` | 设置页为顶部 tab + Overlay 模态，用户希望 VSCode/Obsidian 式功能区/两栏布局 | 设计左侧 icon rail + 右侧内容区的 settings chrome（较大改动，待专项设计） | 大/中 |
| 8 | P2 | 架构 | `ui/types.rs`, `components/agent_turn.rs` | `Message` 同时保留 `content`、`blocks`、`parsed`、`lines`；`AgentTurn` clone Message | 归一化为一份 prepared 表示；布局缓存内聚到 Message | 大/高 |
| 9 | P2 | 可维护 | `main.rs`, `app_logic.rs`, `ui/render.rs`, `message_list.rs` | 文件远超项目自定的 300 行面板规范 | 按域拆分，但需等待 P1d 迁移稳定 | 大/中 |

---

## 3. 详细发现

### 3.1 虚拟列表高度计算（P0）

**位置**：`crates/clarity-egui/src/panels/chat/message_list.rs:159-181, 285-361`

**现象**：
- 每帧遍历全部 `RenderUnit` 构造 `estimate_buffer`。
- `cache[units.len() - 1] = None` 无条件清空最后一 turn 缓存，导致 `AgentTurn::from_messages` 每帧重建。
- 即使 `TurnState::Idle`（无流式输出），最后一 turn 仍被重建。

**影响**：
- 长会话（1000+ 消息）帧率下降。
- 流式期间 CPU 浪费在重复解析/布局同一 turn。

**根因**：
- 缺少“最后一 turn 是否仍在变化”的判断；把“可能变化”等同于“必然变化”。
- `estimate_total_height` 没有缓存键，调用方每次重新计算。

**建议**：
1. 仅当 `view_state.turn != Idle` 或最后 turn 的 message slice 发生变更时，才清空最后一 turn 缓存。
2. 为 `estimate_total_height` 引入 `(units_len, max_w, editing_idx, turn_state, last_message_timestamp)` 缓存键；命中时直接返回缓存值。
3. `compute_unit_estimates` 在全部高度已缓存且缓存键未变时，复用上一帧的 `estimate_buffer`。

### 3.2 路由栈无限增长（P1）

**位置**：`crates/clarity-core/src/ui/router.rs:29-31`

**现象**：
- `Router::navigate` 直接 `self.stack.push(route)`，不检查是否与当前路由相同。
- 反复点击同一导航项会导致栈无限增长，`go_back` 需要多次才能退出。

**建议**：
- `navigate` 遇到与当前路由相等的路由时，替换当前路由（不增加历史），或忽略重复 push。
- 保持 `replace` 用于显式替换当前路由。

### 3.3 右 rail 双写问题（P1）

**位置**：
- `crates/clarity-egui/src/panels/right_ide_panel/mod.rs:276-282, 357-369`
- `crates/clarity-egui/src/main.rs:552-594`

**现象**：
- 预渲染同步：`panel_at_start != prev` 时把路由器状态同步到 dock。
- 后渲染同步：若 dock active tab 变化，则 `app.right_rail_router.reset(panel)`。
- 这是双向桥接：dock 既是视图又是输入源，路由器历史会被 `reset` 清空。

**影响**：
- 路由历史不可靠。
- 动画/关闭手势可能与路由器状态竞争。

**建议**：
- 维持 `right_rail_router` 为唯一真相源。
- dock 切 tab 时，用 `replace`（或去重后的 `navigate`）回写，而不是 `reset`。
- 关闭 tab 时调用 `collapse_right_rail()` 并清空 dock。

### 3.4 语言切换未持久化（P1）

**位置**：
- `crates/clarity-egui/src/app_logic.rs:326`
- `crates/clarity-apps/src/settings_panels/interface_tab.rs:280-286`
- `crates/clarity-apps/src/settings_data.rs:171, 413`

**现象**：
- `GuiSettings` 已持久化 `language` 字段，默认 `"zh"`。
- 但 `App::new` 中 `locale` 被硬编码为 `crate::i18n::Locale::default()`，未读取 settings。
- 设置页切换语言仅调用 `state.set_locale(locale)`，未写回 `settings_edit.language`，也未保存。

**影响**：
- 重启后语言恢复为默认，用户设置丢失。

**建议**：
- 启动时根据 `settings_edit.language` 初始化 `locale`。
- 切换语言时同步更新 `settings_edit.language` 并 `auto_save_settings()`。

### 3.5 左侧导航「Plugins」名实不符（P1）

**位置**：`crates/clarity-egui/src/panels/navigation_tree/nav_items.rs:37-50`

**现象**：
- 导航项名为 `Plugins`，图标为 layers，点击后打开 `ModalType::Mcp`。
- 注释称“Unified entry for skills, MCP tools, web tabs, and built-in actions”，但实现只打开 MCP modal。

**建议**：
- 短期：改名为「MCP」或「Tools」，避免误导。
- 中期：改为打开统一 plugin picker（与 composer `/` 唤起的 picker 一致）。

### 3.6 设置页布局（P2）

**位置**：`crates/clarity-apps/src/settings.rs:107-160`

**现象**：
- 当前为 Overlay + 顶部 tab 栏，宽度固定 640px。
- 用户反馈希望 VSCode/Obsidian 式功能区/两栏布局。

**建议**：
- 作为 P2 专项：左侧固定 icon rail（Provider / Interface / Ops / Claw / About），右侧滚动内容区；可保留 overlay 进入动画。
- 需先出 mock/ADR，再改动，避免与现有快捷键、焦点管理冲突。

---

## 4. 改造计划（第三批落地）

本轮落地 **4 项**改造，均可在当前测试基线上验证：

| # | 改造 | 文件 | 验证方式 |
|---|------|------|----------|
| 1 | 虚拟列表高度缓存 | `panels/chat/message_list.rs` | 新增单元测试：Idle 时缓存键命中返回缓存高度；active turn 不缓存 |
| 2 | 路由去重 | `clarity-core/src/ui/router.rs` | 新增单元测试：重复 `navigate` 不增长栈；`go_back` 行为正确 |
| 3 | 右 rail 同步硬化 | `panels/right_ide_panel/mod.rs`, `main.rs` | 代码审查 + 现有测试 |
| 4 | 语言持久化 | `app_logic.rs`, `settings_panels/interface_tab.rs`, `clarity-ui/i18n.rs` | 新增单元测试：locale code 双向转换；切换语言写回 settings 并触发保存 |
| 5 | Plugins 导航语义修正 | `panels/navigation_tree/nav_items.rs` | 新增单元测试：渲染不 panic；行为改为打开 plugin picker + 聚焦输入 |

未在本次落地的 P2 项（设置页功能区、Message 表示归一化、大文件拆分）写入 `docs/planning/optimization-plan-2026-07-06.md` 作为后续路线。

---

## 5. 验证结果

本轮改造后执行：

```bash
cargo fmt --all -- --check                 # pass
cargo test --workspace --lib --bins -- --test-threads=2   # pass
cargo test --workspace --doc -- --test-threads=2          # pass
cargo test -p clarity-integration-tests --lib             # pass
cargo clippy --workspace --lib --bins --tests -- -D warnings   # pass
```

新增/更新测试：

- `clarity-core::ui::router` — `navigate_deduplicates_current_route`
- `clarity-ui::i18n` — `locale_codes_roundtrip`, `locale_from_code_is_case_and_dash_tolerant`
- `clarity-apps::settings_panels::interface_tab` — `locale_change_is_persisted_to_settings`
- `clarity-egui::panels::chat::message_list` — `estimate_total_height_caches_when_idle`, `estimate_total_height_invalidates_cache_when_turn_active`
- `clarity-egui::panels::navigation_tree::nav_items` — `render_nav_items_does_not_panic`
