# Phase 2 协议驱动试点执行计划

> 承接 `2026-04-27-egui-architecture-evolution.md` §2.1–§2.5。
> 基于 `experiments/protocol-pilot/` 性能基准结论（+45% 开销，绝对值 < 0.35% 帧预算）。

---

## 一、目标与范围

**目标**：将 `settings_panel` 从直接 egui 调用改写为"协议生成 + 协议渲染"，验证端到端可行性。

**范围边界**：
- ✅ 仅 `panels/settings.rs` 重写
- ✅ 仅新增 `ui/protocol_renderer.rs`（egui 翻译层）
- ✅ 仅扩展 `clarity-wire/src/lib.rs`（协议定义）
- ❌ 不新增 crate（遵守 Hard Veto：项目广度 ≤ 5 核心工具）
- ❌ 不改动其他 7 个面板
- ❌ 不将 settings 表单逻辑下沉到 `clarity-core`（避免过早抽象）

---

## 二、关键设计决策

### 2.1 协议定义位置：`clarity-wire` 扩展

`clarity-wire` 已有 `WireMessage`（backend→frontend 流式协议）。本次扩展 UI 渲染协议：

- `ViewCommand`：描述"渲染什么"（声明式）
- `UserAction`：描述"用户做了什么"（事件式）
- `TextRole` / `ButtonStyle`：语义化样式标记，前端映射到本地主题

全部带 `Serialize + Deserialize`，为未来 Web/TUI 前端跨进程传输预留。

### 2.2 `ComboBox` 双值设计

egui `selectable_value` 的底层是 `(value, label)` 分离。协议层直接暴露：

```rust
ComboBox {
    id: String,
    selected_value: String,           // 业务 key（如 "openai"）
    options: Vec<(String, String)>,   // (value, label) 对
    width: f32,
}
```

避免 label→key 回映的 awkward 逻辑。

### 2.3 `Button` 语义样式

```rust
enum ButtonStyle { Primary, Secondary, Danger }

Button {
    id: String,
    label: String,
    style: ButtonStyle,
    min_width: f32,
    min_height: f32,
}
```

渲染器映射：`Primary` → `theme.accent`，`Secondary` → `theme.border`。

### 2.4 TextInput 变化检测策略

每帧 `value.clone()` → `TextEdit::singleline(&mut local)` → 比较前后值。Settings panel 仅 2 个输入框， allocations 可忽略（benchmark 已验证 < 5µs）。

### 2.5 高频交互本地缓存

| 交互 | 策略 |
|---|---|
| 文本输入 | 每帧检测，即时生成 `TextInputChange`（无防抖，settings 低频） |
| ComboBox | 选项变化即时生成 `ComboChange` |
| Button | 点击即时生成 `ButtonClick` |
| Hover / 鼠标移动 | 协议完全不参与 |

---

## 三、文件变更清单

| 文件 | 操作 | 行数预估 | 说明 |
|------|------|---------|------|
| `crates/clarity-wire/src/lib.rs` | 追加 | +80 | `ViewCommand` / `UserAction` / `TextRole` / `ButtonStyle` + serde + 测试 |
| `crates/clarity-egui/src/ui/protocol_renderer.rs` | 新建 | ~120 | `render_view_commands()` egui 翻译层 |
| `crates/clarity-egui/src/ui/mod.rs` | 修改 | +1 | `pub mod protocol_renderer;` |
| `crates/clarity-egui/src/panels/settings.rs` | 重写 | ~140 → ~100 | 拆分为 `settings_commands()` + `handle_settings_action()` + orchestrator |

---

## 四、实施步骤（顺序执行）

### Step 1: 协议定义（`clarity-wire`）
- 在 `lib.rs` 末尾追加 `TextRole`、`ButtonStyle`、`ViewCommand`、`UserAction` enum
- `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`
- `#[serde(tag = "type", rename_all = "snake_case")]` 与 `WireMessage` 风格一致
- 追加 roundtrip 单元测试（序列化 → 反序列化 → 比较）
- `cargo test -p clarity-wire` 通过

### Step 2: egui 协议渲染器
- 新建 `crates/clarity-egui/src/ui/protocol_renderer.rs`
- `pub fn render_view_commands(ui, commands, theme, actions)` 递归渲染
- 实现映射表：
  - `VStack` → `ui.vertical()`
  - `HStack` → `ui.horizontal()`
  - `Text` → `ui.label(RichText)`（按 `TextRole` + `Theme` 着色）
  - `TextInput` → `TextEdit::singleline()` + 变化检测 → `UserAction::TextInputChange`
  - `ComboBox` → `ComboBox::from_id_salt()` + `selectable_value()` + 变化检测 → `UserAction::ComboChange`
  - `Button` → `Button::new()` + `clicked()` → `UserAction::ButtonClick`
  - `Space` → `ui.add_space()`
- `cargo check -p clarity-egui` 通过

### Step 3: settings_panel 协议化
- 重写 `panels/settings.rs`：
  - `fn settings_commands(settings: &GuiSettings, theme: &Theme) -> Vec<ViewCommand>` — 纯函数，零副作用
  - `fn handle_settings_action(action: UserAction, app: &mut App)` — 状态变更处理器
  - `pub fn render_settings_panel(app: &mut App, ctx: &egui::Context)` — orchestrator：
    1. `if !app.settings_open { return; }`
    2. 画 dimmer overlay + `Window::new("Settings")`（框架保留）
    3. `let commands = settings_commands(&app.settings_edit, &app.theme);`
    4. `let mut actions = Vec::new();`
    5. `ui.vertical(|ui| render_view_commands(ui, &commands, &app.theme, &mut actions));`
    6. `for action in actions { handle_settings_action(action, app); }`
- 保留原有行为：Provider 变化自动选第一个 Model、Save 时 persist + reload_llm、Cancel 关闭面板

### Step 4: 回归验证
- `cargo test --workspace --lib` → 523+ 测试通过
- `cargo clippy --workspace -D warnings` → 0 警告
- `cargo check` → 0 错误
- 手动验证：编译运行，打开 Settings 面板，切换 Provider / Model / 修改 API Key / Save / Cancel，行为与改造前一致

---

## 五、验收标准

| # | 标准 | 验证方式 |
|---|------|---------|
| 1 | `settings_panel` 零直接 egui widget 调用（除 Window 框架外） | `grep -n "ui\.label\|ui\.button\|TextEdit::\|ComboBox::" panels/settings.rs` 应无匹配 |
| 2 | 其他 7 个面板完全未改动 | `git diff --stat` 仅触及 4 个文件 |
| 3 | 行为零回归 | 手动功能测试 + `cargo test` 全绿 |
| 4 | clippy 0 警告 | `cargo clippy --workspace -D warnings` |
| 5 | 协议可序列化 | `clarity-wire` 新增 roundtrip 测试通过 |
| 6 | settings.rs 代码量减少 ≥ 20% | 改造前 135 行 → 目标 ≤ 108 行 |

---

## 六、风险与回退

| 风险 | 概率 | 缓解 |
|------|------|------|
| egui `ComboBox` 闭包内变化检测与协议 action 生成冲突 | 中 | 闭包外比较 `selected_value` 前后值，避免闭包内返回值逃逸问题 |
| `TextInput` 每帧 clone 导致输入延迟感知 | 低 | 仅 2 个输入框，且实验已验证 < 5µs |
| 协议 enum 设计不当，后续面板无法复用 | 中 | 本次设计预留 `VStack`/`HStack`/`Text`/`TextInput`/`ComboBox`/`Button`/`Space` 7 种原子，覆盖 task_panel / approval_modal 需求；chat_panel 需扩展时再增 |
| 编译时间增加 | 低 | `clarity-wire` 已有 serde 依赖，无新增外部 crate |

**回退策略**：若 Step 3 遇到不可解的 egui 范式冲突，保留 `experiments/protocol-pilot/` 作为独立参考，回滚 `panels/settings.rs` 到 Phase 1 版本，仅合并 Step 1（协议定义）作为基础设施。
