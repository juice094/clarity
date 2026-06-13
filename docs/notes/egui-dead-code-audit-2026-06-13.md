# clarity-egui 死代码/遗留状态审计报告

> 日期：2026-06-13
> 范围：`crates/clarity-egui/src/**/*.rs`
> 基线：`cargo fmt --check` PASS；`cargo clippy -p clarity-egui --bins --tests -- -D warnings` PASS；`cargo test -p clarity-egui --bins` 82 passed / 0 failed

---

## 一、按文件分类

### 1.1 `main.rs` — 运行时保留但当前未激活的字段

| 位置 | 标记 | 说明 | 处理建议 |
|------|------|------|----------|
| L79 | `#[allow(dead_code)]` on `gateway_manager` | Gateway 自动启动后持有，但当前无面板读取其内部状态 | 保留字段，移除 `dead_code` 改为 `// Gateway process manager (auto-start + manual control)` 即可；或暴露状态到 UI |
| L82 | `#[allow(dead_code)]` on `skill_watcher` | Skill 热重载 watcher，生命周期由 drop 管理 | 同上去掉 dead_code，保留为 RAII 句柄 |

### 1.2 `app_logic.rs` — 已沉淀为内部/备用函数

| 位置 | 名称 | 说明 | 处理建议 |
|------|------|------|----------|
| L728 | `save_settings_and_reload` | 被 `commit_settings + apply_approval_mode_to_runtime + trigger_llm_reload` 替代 | 删除或改为 private helper |
| L748 | `save_settings_internal` | 无调用，功能与 `commit_settings` 重复 | 删除 |
| L756 | `delete_session` | 当前 UI 未提供删除入口 | 保留功能，改为 `pub(crate)` 供未来快捷键/右键菜单使用，移除 dead_code |

### 1.3 `stores/mod.rs` — ViewState 迁移遗留字段

| 位置 | 字段/类型 | 说明 | 处理建议 |
|------|-----------|------|----------|
| L26 | `SessionStore::active_session` | `#[allow(dead_code)]`；`active_session_mut` 已在使用 | 移除 dead_code，保留只读访问 |
| L89 | `KimiCodeLoginState::Waiting.verification_uri` | 字段未读取 | 删除字段或暴露到 UI |
| L102 | `SettingsStore.settings_vm` | SettingsViewModel，当前未接入 UI | 保留（S3.5 决议对象），移除 dead_code |
| L187 | `CronStore.last_refresh` | 未读取 | 删除或用于 UI 刷新时间显示 |
| L208 | `UiStore.start` | 应用启动时间，未使用 | 删除 |
| L303 | `BotStatus` enum | 仅 `Online` 被读取，`Offline/Syncing` 标记 dead_code | 保留完整状态机，移除 enum 级 dead_code |

### 1.4 `components/chat/conversation.rs` — 大量保留字段

该文件有 10 处 `#[allow(dead_code)]`，多为实验性消息样式/交互状态：
- L16, L124, L644, L684, L706, L760, L783, L792, L800, L823

处理建议：整体审查 conversation 渲染路径，区分"实验保留"与"真死代码"。本次计划不删除功能，仅标记为 `// experimental` 并建立跟踪 issue。

### 1.5 `components/agent_turn.rs` — 实验字段

- L20, L30, L33, L52：`AgentTurn` 组件的可选样式/元数据字段

处理建议：同 conversation，标记为 experimental。

### 1.6 `widgets/` — 未启用 widget

| 文件 | 状态 | 说明 |
|------|------|------|
| `widgets/card.rs` | `#[allow(dead_code)]` 整个模块 | 设计系统 Card 原语的早期实现，现被 `design_system::Surface::Card` 替代 |
| `widgets/badge.rs` | `#[allow(dead_code)]` 整个模块 | 早期 badge，现可用 `design_system::status_badge` |
| `widgets/settings_row.rs` | `#[allow(dead_code)]` | 被 settings 内联实现取代 |
| `widgets/toggle.rs` | `#[allow(dead_code)]` 整个模块 | 未启用 |
| `widgets/mod.rs` L17-18, 25, 31 | 注释掉的 re-export | 历史残留，可删除注释 |

处理建议：
- `card.rs` / `badge.rs` / `toggle.rs`：本次不删除，迁移到 `design_system` 等价物后，在 Phase 3 中逐步替换调用并删除文件。
- `settings_row.rs`：确认无调用后删除。

### 1.7 `ui/icons.rs` — 早期图标常量

- L12, L19：`#[allow(dead_code)]` 的图标名常量

处理建议：已迁移到 Phosphor/Lucide，可删除；但需确认无历史代码引用。

### 1.8 `services/` — 服务层保留 API

| 位置 | 说明 | 处理建议 |
|------|------|----------|
| `gateway_task_client.rs:63` | 某个内部方法 | 保留，移除 dead_code |
| `gateway_manager.rs:63` | 内部状态或方法 | 保留，移除 dead_code |

### 1.9 `panels/` — 少量保留字段

| 位置 | 说明 | 处理建议 |
|------|------|----------|
| `cron.rs:5,13` | cron 面板 | 保留，移除 dead_code |
| `gantt.rs:240` | Gantt 字段 | 保留，移除 dead_code |
| `task.rs:6` | Task 面板 | 保留，移除 dead_code |

### 1.10 其他

| 文件 | 位置 | 说明 | 处理建议 |
|------|------|------|----------|
| `i18n.rs` | L32, L50 | Locale 相关 | 保留国际化骨架，移除 dead_code |
| `error.rs` | L8 | 错误变体 | 保留，移除 dead_code |
| `provider.rs` | L482, L523 | provider 工具函数 | 保留，移除 dead_code |
| `theme.rs` | L45, L224 | 主题字段/常量 | 保留，移除 dead_code |
| `window_manager.rs` | L32, L57 | window 状态 | 保留，移除 dead_code |
| `ui/types.rs` | L53, L251, L272, L478 | 类型字段/函数 | 保留，移除 dead_code |
| `settings.rs` | L174, L190 | 设置字段 | 保留，移除 dead_code |
| `shortcuts/mod.rs` | L39-48 | line-mode feature 相关 | 保留 feature-gated，移除 dead_code |
| `components/file_preview_overlay.rs` | L10 | 覆盖层 | 保留，移除 dead_code |
| `handlers/cron.rs` | L6 | handler | 保留，移除 dead_code |
| `handlers/team.rs` | L4 | handler | 保留，移除 dead_code |

### 1.11 TODO/FIXME

| 位置 | 内容 | 处理建议 |
|------|------|----------|
| `shortcuts/mod.rs:190` | `// TODO: implement once App test-harness is available.` | 保留，转为 issue 跟踪 |
| `panels/chat/input/tui_style.rs:90` | `// TODO: remove specific attachment` | 保留 |
| `panels/chat/input/tui_style.rs:166` | `// TODO: toggle agent mode` | 保留 |

---

## 二、处理策略

本次计划**不删除业务功能**，仅做以下两类动作：

1. **真死代码**：确认无任何调用、且非实验保留的函数/字段，直接删除。
   - `app_logic.rs::save_settings_internal`
   - `stores/mod.rs::UiStore.start`
   - `widgets/mod.rs` 中被注释掉的 re-export 行
   - `ui/icons.rs` 中已迁移的图标常量（确认后）

2. **误标 dead_code**：对仍有合理用途但当前未调用的 API，移除 `#[allow(dead_code)]`，改为明确注释说明其用途。
   - `main.rs` 的 `gateway_manager` / `skill_watcher`
   - `stores/mod.rs` 的 `SessionStore::active_session` / `SettingsStore.settings_vm` / `CronStore.last_refresh`
   - `widgets/card.rs` / `badge.rs` / `toggle.rs`（改为标记 `// legacy: to be replaced by design_system`）

3. **ViewState 双轨字段**：在 Phase 1.3 中统一处理，删除 store 中的 panel_open 类 legacy boolean。

---

## 三、产出清单

- [x] 扫描所有 `#[allow(dead_code)]` / `#[allow(unused)]` / TODO / 注释掉的 re-export
- [x] Phase 1.3：删除 ViewState 双轨遗留字段（`settings_open`、`dashboard_panel_open`、`gantt_panel_open`、`team_panel_open`、`task_panel_open`、`skill_panel_open`、`mcp_panel_open`）
- [x] Phase 2.1：按职责重组 `panels/` 目录并保留向后兼容 re-export
- [x] Phase 2.2：提取/增强公共 widget（`widgets/avatar.rs`、`widgets/user_avatar.rs`）
- [x] Phase 4：新增 `layout.rs` 与 `App::render_layout_shell()` 作为未来单页面/三栏布局的接入点
- [x] Phase 2.3：拆分 `stores/mod.rs` 为按域子模块
- [x] Phase 3：删除未启用的 `widgets/card.rs` / `badge.rs` / `toggle.rs` / `widgets/settings_row.rs`；注册 `mod design_system` 并在 `main.rs::update()` 中接入 `install_theme()`

## 四、本轮已执行变更摘要

- `cargo fmt --all -- --check` PASS
- `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` PASS
- `cargo test --workspace --lib --exclude clarity-slint` 全绿
- `cargo test -p clarity-egui --bins` 89 passed / 0 failed
- `clarity-core::agent::cost_channel` 并行测试污染问题已修复
- 无现有功能被删除；所有面板仍通过 `render_layout_shell()` 统一编排渲染
