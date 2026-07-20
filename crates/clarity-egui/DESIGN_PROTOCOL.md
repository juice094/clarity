# Clarity egui Design Protocol v2.0 (P3)

> 目标：把 clarity-egui 从“每帧随手画”变成“按协议组装”。
> 本协议优先于个人审美；任何视觉改动必须先改协议/token，再改调用处。

## 0. 哲学

- **egui 不是浏览器**。没有 CSS、没有自动排版、没有布局引擎兜底。
- 每一帧都是一张白纸，靠纪律才能不偏移、不抖动、不“土”。
- 因此：
  1. 所有视觉决策必须来自 `clarity-ui` 的 token 与组件。
  2. `clarity-egui` 只负责布局编排、事件桥接、业务状态。
  3. 禁止在视图代码里临时决定颜色、间距、圆角、阴影。

## 1.  crate 职责边界

| Crate | 能做什么 | 禁止做什么 |
|---|---|---|
| `clarity-ui` | 定义 Theme、设计 token、可复用 widget、Frame 预设、动画辅助 | 引用 `clarity-egui` 或业务状态 |
| `clarity-egui` | 组装界面、管理窗口生命周期、调度子应用、处理用户输入 | 直接调用 `Button::new`、`TextEdit::singleline`、`Frame::new`、`Window::new` 等 raw egui |
| `clarity-core` / `clarity-shell` | 业务模型、路由、跨平台 app 抽象 | 任何绘制代码 |

## 2.  布局宪法（Layout Constitution）

### 2.1 Chrome 是唯一拥有 Panel 的地方

- `TopPanel`、`BottomPanel`、`LeftPanel`、`RightPanel`、`CentralPanel` 只允许出现在 `clarity-egui/src/chrome.rs`。
- 子应用、modal、panel、overlay 都通过 `Ui` 渲染，不得自行创建 `Panel`。

### 2.2 主区域一次分完

中央三区（左 rail / 主舞台 / 右 rail）必须用 `egui_extras::StripBuilder` 一次分配宽度。禁止让多个 `Panel::left/right` 互相挤占。

### 2.3 Modal 不是 Window

- Modal 使用显式 `Area::new(Id)` + `fixed_pos()` + `Order::Foreground`。
- 禁止 `Window::new` 做对话框（`Window` 会记忆位置、会被拖动、层级不可控）。
- Modal 必须在 scrim 之后绘制，且 scrim 使用 `Order::Foreground` 之下的独立 `Order`。
- Modal 内容宽度受 `theme.space_32` 双边距约束，禁止写死 `margin = 32.0`。

### 2.4 层级顺序（z-order）

每一帧绘制顺序即层级。必须按以下顺序：

1. `Background` — 全屏背景、主舞台边框阴影。
2. `Rails` — 左右侧边栏。
3. `MainStage` — 当前子应用。
4. `Overlays` — skill / mcp / command palette / file preview。
5. `Scrim` — modal 遮罩。
6. `Modal` — 对话框。
7. `Toast` — 通知（最高，不阻塞）。

## 3.  视觉层级（Elevation）

所有表面必须属于一个 `Elevation` 层级。每个层级定义：

- 背景色（`bg`）
- 边框色（`stroke`）
- 圆角（`radius`）
- 阴影（`shadow`）
- 内边距（`padding`）

| Elevation | 用途 | 例子 |
|---|---|---|
| `Base` | 最底层背景 | 窗口背景 |
| `Surface` | 可滚动/承载内容的平面 | 设置面板、左侧导航 |
| `Elevated` | 浮起的内容卡片 | 消息气泡、列表项 |
| `Overlay` | 悬浮面板 | skill/mcp 面板、command palette |
| `Modal` | 对话框 | 新建任务、登录 |
| `Toast` | 通知条 | 右下角提示 |

禁止直接写 `Frame::new().fill(...).corner_radius(...)`。改用：

```rust
use clarity_ui::design_system::{surface, Elevation, card, overlay, modal_elevated};
```

## 4.  设计 Token 使用法则

### 4.1 颜色

- 只能使用 `Theme` 字段：
  - 背景：`bg`, `surface`, `bg_elevated`
  - 文字：`text`, `text_strong`, `text_muted`, `text_dim`
  - 强调：`accent`, `accent_hover`, `accent_subtle`
  - 状态：`ok`, `warn`, `danger`, `info`
  - 边框：`border`, `border_strong`, `border_hover`
- 禁止字面量 `Color32::from_rgb(...)`，除非是协议已定义的派生函数内部。

### 4.2 间距

- 4px 基线网格。
- 只能使用 `Space::S0..=S6`：`4, 8, 12, 16, 20, 24, 40`。
- 使用 `gap(ui, Space::S1)` 或 `with_item_spacing(ui, Space::S1, |ui| ...)`。
- 视口边缘边距（modal / overlay 安全区）使用 `theme.space_32`，禁止在组件里写 `32.0`。

### 4.3 字体

- 只能使用 `TextStyle` 常量：
  - `Body`, `Accent`, `CaptionStrong`, `Small`, `Heading`, `Subheading`, `Title`, `Mono`
- 禁止直接 `RichText::new(...).size(13.0)`。
- chip / badge 里的小号图标使用 `t.text_xs`（10 px），不要写死 `10.0`。

### 4.4 圆角

- 只能使用 `radius_sm/md/lg/xl/full`，对应 `8/16/28/36/999`。

### 4.5 阴影

- 只能使用 `shadow_card/panel/modal/toast`。
- 禁止自定义 `Shadow::small()`。

### 4.6 按钮尺寸

- 只能使用 Theme 的 `button_height_sm/md/lg`，对应 `24/32/40`。
- 按钮宽度由内容 + 水平内边距自动计算；需要固定宽度时通过组件 API 传入。

## 5.  组件垄断（Component Monopoly）

| 用途 | 必须使用 |
|---|---|
| 按钮 | `clarity_ui::widgets::button::Button` |
| 图标按钮 | `clarity_ui::widgets::icon_button::icon_button` |
| 文字输入 | `clarity_ui::widgets::text_input::TextInput` |
| 下拉选择 | `clarity_ui::widgets::select::Select`（存在后）|
| 卡片/面板 | `clarity_ui::design_system::surface` / `card` |
| Modal | `clarity_ui::widgets::modal::Modal` |
| Overlay | `clarity_ui::widgets::overlay::Overlay` |
| Toast | `clarity_ui::widgets::toast::Toast`（存在后）|
| 列表项 | `clarity_ui::widgets::list_item::ListItem`（存在后）|
| 状态点 | `clarity_ui::design_system::status_dot` |
| 徽标 | `clarity_ui::design_system::badge` |
| 开关 | `clarity_ui::design_system::toggle` |
| 加载骨架 | `clarity_ui::design_system::skeleton` |
| chip / tag | `clarity_ui::design_system::chip` |

### 何时用 `clarity_ui` 组件 vs 手写 egui widget

| 场景 | 选择 |
|---|---|
| 标准交互元素（按钮、输入框、开关） | **必须用** `clarity_ui` 组件 |
| 业务专用布局（聊天记录、diff 行、pretext 测量） | 可在 `clarity-egui` 用 `painter` 实现，但颜色/半径/间距必须来自 Theme |
| 一次性装饰性图形（进度条、分隔线、状态点） | 可在 `clarity-ui` 用 `design_system` 辅助函数 |
| 需要新变体的组件 | 先在 `clarity-ui` 扩展组件，禁止在业务代码里临时覆写样式 |

## 6.  布局模式

### 6.1 Central panel

- 由 `chrome.rs` 统一创建。
- 子应用只接收 `&mut Ui`，不得调用 `CentralPanel::default().show()`。

### 6.2 Side panels

- 左 rail：固定 `theme.size_sidebar` 或折叠为 `theme.size_sidebar_collapsed`。
- 右 rail：`theme.size_panel_right`，可折叠。
- 内部使用 `ScrollArea::vertical()` 处理溢出。

### 6.3 Overlay

- 使用 `Overlay::new(id).width(...).top_center(theme.modal_offset_y)`。
- 需要遮罩时先调用 `overlay_scrim(ctx)`，再调用 `Overlay::show`。
- Overlay 宽度受 `theme.space_32` 双边距约束。

### 6.4 Modal

- 使用 `Modal::new(id).width(...).show(ctx, \|ui\| ...)`。
- 先调用 `modal_scrim(ctx)`，再调用 `Modal::show`。
- 内容区使用 `gap(ui, Space::S2)` 作为字段间距，按钮组使用 `ui.with_layout(Layout::right_to_left(...))`。

## 7.  响应式/自适应规则

- 只使用 Theme 里定义的 breakpoints：
  - `breakpoint_compact: 680.0`
  - `breakpoint_medium: 1100.0`
  - `breakpoint_wide: 1400.0`
- 布局切换必须通过明确的 `LayoutProfile`（Compact / Medium / Wide），每个 profile 对应一套固定的尺寸，禁止随处写 `if width < 600.0`。
- 最小窗口尺寸：`theme.window_min_w` / `theme.window_min_h`。
- 内容最小宽度：`theme.content_min_width`。
- 左 rail 折叠阈值由 `layout` 模块统一计算，禁止面板自己决定。

## 8.  动画契约

- 所有动画必须通过 `theme.animate_bool_*` / `theme.animate_value_*`。
- 禁止在 UI 代码里自己拿 `Instant::now()` 算插值。
- 动画状态必须可以从当前帧单独推导，不依赖上一帧的私有字段。

## 9.  Icon 与图片

- 只允许使用 Lucide 图标。
- 通过 `lucide_icons::Icon::*` 类型安全引用，或 `clarity_ui::theme::ICON_*` 兼容常量。
- 禁止在业务代码里直接使用 Unicode codepoint。

## 10.  测试护栏

- 每个新增/修改的 widget 必须附带 `egui_kittest` 快照测试。
- 每个 modal 至少一张参考图（打开、关闭、不同分辨率）。
- CI 必须跑 visual regression；快照不匹配时阻断合并。
- 在 `clarity-ui` 里为每个组件提供“单组件截图测试”。

## 11.  禁止清单（No-Exceptions List）

以下行为在任何情况下都不允许出现在 `clarity-egui`：

- [ ] `ctx.style_mut()` 或 `ui.visuals_mut()` 在 theme 初始化外使用。
- [ ] `Window::new` 用于 modal。
- [ ] `Panel` 嵌套在 `Panel` 内部。
- [ ] 直接使用 `Button::new`、`TextEdit::singleline`、`TextEdit::multiline`。
- [ ] 直接使用 `Frame::new()` 构造容器。
- [ ] 字面量颜色、间距、字体大小、圆角、阴影。
- [ ] 在 `update()` / 渲染代码里做字符串解析、JSON、IO。
- [ ] 手动 `Instant` 驱动的动画。

## 12.  代码审查清单

新增/修改 UI 代码时，审查者必须检查：

1. 是否有新的 `Color32::from_rgb` / `Color32::from_gray` 字面量？
2. 是否有新的 `add_space(N.N)`、`Margin::same(N)`、`CornerRadius::same(N)` 字面量？
3. 是否有新的 `RichText::new(...).size(N.N)`？
4. 是否直接构造了 `egui::Button`、`egui::TextEdit`、`egui::Window`？
5. Modal / Overlay 是否先 scrim 后内容？顺序是否正确？
6. 是否使用了 `theme.space_32` 作为视口安全边距？
7. 按钮高度是否来自 `theme.button_height_*`？
8. 是否有对应测试或至少一个 `run_in_frame` 烟雾测试？

## 13.  迁移状态与 TODO

### 已完成（P3 基线）

- [x] Theme 增加 `space_32` token，统一 Modal / Overlay 视口边距。
- [x] Theme 增加 `button_height_sm/md/lg`，统一 `Button` 高度。
- [x] `chip` 移除图标字号改用 `text_xs` token。
- [x] 设计协议与代码 token 对齐（半径、间距、按钮高度）。

### 待办（按优先级）

**P1 — 高优先级（下次迭代）**
- [ ] 将 `badge` / `toggle` / `search_box` 中剩余硬编码半径、间距改为对应 token 或语义常量。
- [ ] 为 `Elevation` 与 `Theme::frame_*` 重复提供的 frame builder 去重并标注废弃路径。
- [ ] 统一 `bubble_frame`、`code_frame` 等 one-off frame 的内边距到 spacing token。

**P2 — 中优先级**
- [ ] 将 `clarity-egui` 中剩余的 `add_space(N.N)` 替换为 `gap(ui, Space::S*)`。
- [ ] 将 `clarity-egui` 中剩余的 `CornerRadius::same(N)` 替换为 theme radius token。
- [ ] 引入 `LayoutProfile` 枚举并替换所有 `width < 600.0` 之类的硬编码断点。

**P3 — 低优先级 / 研究**
- [ ] 引入 `egui_kittest` 快照测试并建立视觉基线。
- [ ] 评估 `egui_dock` 社区方案替换自研 panel 管理。
- [ ] 制定深色/浅色/OLED 三套主题的对比度自动检测流程。

## 14.  如何修改本协议

- 任何视觉改动如果与协议冲突，先改协议/token，再改代码。
- 新增组件必须在 `clarity-ui` 中实现，并通过至少一个 snapshot test。
- 协议变更需要同时更新 `DESIGN_PROTOCOL.md` 和 `crates/clarity-ui/src/lib.rs` 的组件清单注释。

---

> ponytail: 本协议是“刚好能跑”的底线，不是完美主义。先垄断 raw egui，再逐步求精。
