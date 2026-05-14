# Shortcut Focus Routing — ADR-013 Implementation Notes

> **Scope**: S3 P1.5.9 → S7 Phase 2D | **Status**: Core complete, TUI pending | **Owner**: juice094

## 1. 设计目标

解决 50+ 布尔标志时代的快捷键混乱问题：
- 同一按键在不同焦点上下文有不同语义（如 `Esc` 在 Approval modal 是关闭，在 Chat 是无操作）
- 快捷键泄漏（在 Settings 文本框中按 `Ctrl+N` 不该新建会话）
- GUI/TUI 共享同一套命令 ID 和绑定语义

## 2. FocusScope 五级层级

```rust
pub enum FocusScope {
    Widget,              // specificity = 5
    Panel(PanelKind),    // specificity = 4
    Modal(ModalType),    // specificity = 3
    App,                 // specificity = 2
    Os,                  // specificity = 1
}
```

**兼容性规则** (`is_compatible_with`):
| 绑定定义 | 当前焦点 | 是否触发 | 示例 |
|----------|----------|----------|------|
| `Os` | `Os` | ✅ | `Ctrl+Q` 退出 |
| `App` | `App` / `Panel` / `Modal` / `Widget` | ✅ | `Ctrl+Shift+P` 命令面板 |
| `Panel(ChatStream)` | `Panel(ChatStream)` / `Widget` | ✅ | `j/k` 行导航 |
| `Panel(ChatStream)` | `Panel(Workspace)` | ❌ | 隔离 |
| `Modal(Approval)` | `Modal(Approval)` / `Widget` | ✅ | `1/2` 审批选择 |
| `Widget` | `Widget` | ✅ | `Tab` 循环选项 |

**冲突解决**: 同键多绑定时，specificity 高者胜。`Widget(5) > Panel(4) > Modal(3) > App(2) > Os(1)`。

## 3. ShortcutRegistry

```rust
pub struct ShortcutRegistry {
    bindings: Vec<ShortcutBinding>, // key + scope → command_id
}

impl ShortcutRegistry {
    pub fn resolve(&self, key: &KeyEvent, focus: &FocusScope) -> Option<&'static str> {
        // 1. 过滤 key 匹配
        // 2. 过滤 scope 兼容
        // 3. 取 specificity 最高者
    }
}
```

**注册语义**: LIFO — 后注册的同名绑定覆盖前者，支持运行时动态覆盖（如插件注册自定义快捷键）。

## 4. 命令 ID 统一层

`clarity-core/src/ui/commands.rs` 中的 `ids` 模块是 GUI/TUI 的唯一事实来源：

```rust
pub mod ids {
    pub const NEW_SESSION: &str = "new-session";
    pub const STOP_GENERATION: &str = "stop-generation";
    pub const SEND_MESSAGE: &str = "send-message";
    pub const NAVIGATE_DOWN: &str = "navigate-down";
    pub const NAVIGATE_UP: &str = "navigate-up";
    pub const NAVIGATE_TOP: &str = "navigate-top";
    pub const NAVIGATE_BOTTOM: &str = "navigate-bottom";
    pub const COPY_LINE: &str = "copy-line";
    // ... 共 14+ 个
}
```

**双端一致性检查**: `shortcuts::tests::shortcut_action_command_id_matches_ids_module` 确保 `ShortcutAction` 每个变体都映射到 `ids` 中的常量，防止拼写漂移。

## 5. egui 集成

### 5.1 事件收集 (`shortcuts::collect_actions`)

每帧在 `App::update()` 中调用，将 egui 原生 `Key` + `Modifiers` 翻译为 `ShortcutAction`：

```rust
for action in shortcuts::collect_actions(ctx, self) {
    self.dispatch_command(action.command_id());
}
```

**特殊处理** (S7 Phase 2D):
- `CopyLine` 不通过 `dispatch_command`（需要 `egui::Context` 访问剪贴板），直接在 `update()` 中处理：
  ```rust
  if action == ShortcutAction::CopyLine {
      if let Some(text) = self.selected_line_text() {
          ctx.copy_text(text);
          self.push_toast("Copied to clipboard", ToastLevel::Info);
      }
  }
  ```

### 5.2 行模式导航绑定 (S7)

仅在 `#[cfg(feature = "line-mode")]` 且 `focus == Panel(ChatStream)` 时激活：

| 按键 | 命令 ID | 动作 |
|------|---------|------|
| `j` | `navigate-down` | `LineCursor::move_down()` |
| `k` | `navigate-up` | `LineCursor::move_up()` |
| `g` | `navigate-top` | `LineCursor::move_top()` |
| `G` | `navigate-bottom` | `LineCursor::move_bottom()` |
| `y` | `copy-line` | 整行复制到剪贴板 |

**焦点切换机制**: 当前 `ViewState.focus` 由调用方（如点击面板）通过 `view_state.focus_panel(PanelKind)` 设置。Modal 打开时自动覆盖为 `FocusScope::Modal(...)`，关闭后恢复为 `App`。

## 6. 当前局限与后续

| 局限 | 说明 | 计划 |
|------|------|------|
| `?` 帮助 overlay | 未实现 | S8 Phase 3B 注入（`ShortcutRegistry::help_entries()` 已提供数据源） |
| 自定义绑定 | 用户无法修改快捷键 | S9 评估配置文件支持 |
| TUI 接入 | `ShortcutRegistry` 在 core 中，但 tui 未调用 | S8 Phase 3A |
| 重复键 (gg) | 当前 `g` 直接跳到顶部，未实现 Vim 式 `gg` | 如需，可在 `UiStore` 添加 `last_key_timestamp` 实现 300ms 双击检测 |
