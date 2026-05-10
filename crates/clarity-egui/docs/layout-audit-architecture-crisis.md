# egui 布局架构危机诊断报告

> 生成日期：2026-05-10  
> 范围：`clarity-egui` crate 全部 `.rs` 源文件  
> 严重程度：**P0 — 阻塞后续所有 UI 迭代**

---

## 一、核心结论

当前 `clarity-egui` 的 UI 代码存在**系统性架构缺陷**。代码作者以 **retained-mode GUI 的思维**（Qt/WPF/Flutter 式）在 egui 的 **immediate-mode** 框架上重建了一个自定义布局引擎。这导致：

- **任何尺寸/间距/字号的调整都需要修改 3~7 处硬编码坐标**
- **新增 UI 元素时无法利用 egui 的布局自动推导，必须手动计算位置**
- **主题系统（theme.rs）与实际渲染脱节** — theme 改了间距，但硬编码坐标不变
- **组件无法提取复用** — 每个面板都是内联的坐标算术，没有封装边界

这不是"代码风格问题"，这是**架构范式错配**。

---

## 二、反模式分类与实例

### 反模式 A：Ghost Button（幽灵按钮）— 最严重

**定义**：创建一个透明的 `Button::new("")` 仅为了获取一个 `Rect`，然后用 `painter` 在该 Rect 上自行绘制所有内容。

**影响文件**：
- `panels/sidebar.rs:387-493`（角色卡片）
- `main.rs:238-378`（窗口控制按钮 × 4、设置按钮）

**实例（sidebar.rs 角色卡片）**：

```rust
// Step 1: 创建一个透明的幽灵按钮，只用来占位置和获取点击
let btn_resp = ui.add(
    egui::Button::new("")
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .stroke(egui::Stroke::NONE)
        .min_size(egui::vec2(ui.available_width(), 56.0)),
);

// Step 2: 手动绘制悬停背景（按钮本身不做这件事）
if !is_active && btn_resp.hovered() {
    ui.painter().rect_filled(btn_resp.rect, ..., theme.bg_hover.linear_multiply(0.5));
}

// Step 3: 手动计算每个子元素的绝对坐标
let content_left = btn_resp.rect.min.x + 12.0;  // 硬编码左内边距
let line_y = btn_resp.rect.min.y + 10.0;         // 硬编码顶部偏移

// Step 4: 用 painter 在绝对坐标上画图标
painter.text(
    egui::pos2(content_left + 10.0, line_y + 10.0),  // 硬编码相对偏移
    egui::Align2::CENTER_CENTER,
    role_icon,
    theme.font_icon(theme.text_base),
    text_color,
);

// Step 5: 用 painter 在绝对坐标上画名称
painter.text(
    egui::pos2(content_left + 24.0, line_y),  // 硬编码：图标宽 + 间距
    egui::Align2::LEFT_TOP,
    label,
    theme.font(theme.text_base),
    text_color,
);

// Step 6: 手动计算状态点的垂直位置
let dot_y = line_y + theme.text_base + 4.0;  // 依赖字体高度 + 硬编码间距
let dot_center = egui::pos2(content_left + 4.0, dot_y + 5.0);
painter.circle_filled(dot_center, 4.5, theme.status_online);

// Step 7: 手动计算 latest name 的垂直位置
let name_y = line_y + theme.text_base + 4.0 + if count > 0 { theme.text_xs + 4.0 } else { 0.0 };
painter.text(
    egui::pos2(content_left, name_y),
    ...
);
```

**这个 56px 高的卡片里，有 7 个硬编码坐标偏移**。如果我要：
- 把卡片高度从 56 改为 48 → 需要重新计算 `line_y`、验证所有元素的垂直对齐
- 把图标从 16px 改为 20px → 需要调整 `content_left + 10.0` 和 `content_left + 24.0`
- 把左内边距从 12px 改为 16px → 需要改 `content_left` 并级联调整所有 `content_left + N`
- 把字体从 14px 改为 13px → `theme.text_base` 变了，`dot_y` 和 `name_y` 需要重新验证

**正确的 egui 模式**：

```rust
ui.horizontal(|ui| {
    ui.add_space(12.0);  // 左内边距
    ui.label(RichText::new(role_icon).font(theme.font_icon(theme.text_base)));
    ui.add_space(8.0);   // 图标与文本间距
    ui.vertical(|ui| {
        ui.label(RichText::new(label).strong().size(theme.text_base));
        ui.horizontal(|ui| {
            ui.colored_label(theme.status_online, "●");
            ui.label(RichText::new(format!("{} sessions", count)).size(theme.text_xs));
        });
        if let Some(s) = latest {
            ui.label(RichText::new(s.title).size(theme.text_xs).color(theme.text_dim));
        }
    });
})
.interact(Sense::click())  // 整行可点击
```

**差异**：egui 自动处理所有位置计算。改高度 → 只改一处。改字体 → 不需要改坐标。改间距 → 只改 `add_space`。

---

### 反模式 B：`allocate_exact_size` + `painter` 绘制

**定义**：先用 `allocate_exact_size` 占一块固定大小的空间，然后用 `painter` 在该空间上绘制内容。

**影响文件**（共 12 处）：

| 文件 | 行号 | 用途 |
|------|------|------|
| `main.rs` | 221 | TitleBar 弹性拖拽区域 |
| `main.rs` | 396, 431 | 状态胶囊内的圆点 |
| `panels/chat/header.rs` | 82 | Tab 项 |
| `panels/sidebar.rs` | 281 | Group header 分隔线 |
| `panels/workspace.rs` | 41, 146 | 工作区预览占位 |
| `render/turn_renderer.rs` | 222, 261 | 消息操作按钮占位 |
| `ui/markdown.rs` | 350 | 引用条 |
| `panels/dashboard.rs` | 205 | 仪表盘指示器 |
| `widgets/toggle.rs` | 11 | Toggle 开关 |
| `components/settings/*.rs` | 多行 | 设置面板占位 |

**实例（chat/header.rs Tab）**：

```rust
let (tab_rect, tab_resp) = ui.allocate_exact_size(
    egui::vec2(tab_width, 28.0),
    egui::Sense::click(),
);
// ... 手动计算文本颜色 ...
// ... 手动绘制底部 accent 线 ...
// ... 手动计算文本截断 ...
// ... 用 painter 在 tab_rect 上画文本 ...
// ... 手动构造 close_rect 并 interact ...
// ... 用 painter 画关闭按钮 ...
```

**问题**：
1. `allocate_exact_size` 在 egui 中用于**已知尺寸的内容占位**，不是用于交互组件
2. Tab 的文本截断、悬停反馈、关闭按钮交互全部手动实现
3. 文本对齐使用 `painter.text(CENTER_CENTER)`，但如果字体改变，视觉中心可能偏移

**正确的 egui 模式**：

```rust
let tab_btn = ui.add(
    egui::Button::new(&display_title)
        .min_size(egui::vec2(tab_width, 28.0))
        .fill(if is_active { theme.bg_active } else { Color32::TRANSPARENT })
        .stroke(Stroke::NONE)
        .corner_radius(theme.radius_sm)
);
// 关闭按钮作为独立的小按钮放在 tab 内部右侧
```

或者使用 `SelectableLabel` + 自定义 `Frame`：

```rust
let tab_response = egui::Frame::new()
    .fill(if is_active { theme.bg_active } else { Color32::TRANSPARENT })
    .corner_radius(theme.radius_sm)
    .inner_margin(Margin::symmetric(8, 4))
    .show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&display_title).color(text_color));
            if ui.button("×").clicked() { /* close */ }
        });
    })
    .response;
```

---

### 反模式 C：`ui.interact` 在 raw rect 上

**定义**：手动构造一个 `Rect`，然后用 `ui.interact(rect, id, Sense::click())` 获取交互响应，而不是使用 egui 的内置组件。

**影响文件**：
- `panels/sidebar.rs:300`（`clickable_row` 辅助函数）
- `panels/chat/header.rs:147`（Tab 关闭按钮）
- `ui/file_browser.rs:115`（文件浏览器行）

**实例（sidebar.rs clickable_row）**：

```rust
let row_rect = egui::Rect::from_min_size(
    ui.cursor().min,
    egui::vec2(available_width, 28.0),
);
let row_resp = ui.interact(row_rect, id, egui::Sense::click());
if row_resp.hovered() {
    ui.painter().rect_filled(row_rect, theme.radius_sm, theme.bg_hover);
}
// ... 然后手动在 row_rect 内画文本 ...
```

**问题**：
- 没有焦点环（keyboard navigation 完全缺失）
- 没有内置的悬停/按下状态管理
- 没有 `on_hover_text` 的便捷接口（需要手动实现）
- 没有 `Sense::click()` 以外的交互（如右键菜单）

**正确的 egui 模式**：

```rust
let response = ui.add(
    egui::SelectableLabel::new(*is_open, label)
        .text_style(theme.font(theme.text_sm))
);
if response.clicked() { *is_open = !*is_open; }
```

或者使用 `ui.button()` / `ui.selectable_value()`。

---

### 反模式 D：混合布局系统的碎片化

**定义**：在同一个函数甚至同一行代码中，混合使用三种互斥的布局策略。

**三种被混合的系统**：

| 系统 | 示例 | 适用场景 |
|------|------|----------|
| **egui 响应式布局** | `ui.horizontal()`, `ui.label()`, `ui.button()` | 标准 UI |
| **精确分配** | `ui.allocate_exact_size()`, `ui.allocate_space()` | 已知尺寸的占位 |
| **直接绘制** | `painter.text()`, `painter.rect_filled()` | 装饰性图形 |

**问题**：这三个系统的坐标空间不兼容。

- `ui.horizontal()` 会**推进 cursor**，后续元素自动获得正确位置
- `ui.allocate_exact_size()` 会**推进 cursor 到分配区域的右侧**
- `painter.text()` **不推进 cursor**，它只是在当前 layer 上画东西

当这三个混在一起时，代码必须手动管理 cursor 位置、layer 顺序、clip rect。这导致了一个**隐式的状态机**（"我现在在 cursor 的哪里？""这个 painter 调用会不会被后面的 layout 覆盖？"），而 egui 的设计哲学正是为了消除这种心智负担。

**实例（main.rs TitleBar 右侧区域）**：

```rust
ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
    // 系统 A: egui 响应式布局（Button + Frame）
    let close_resp = ui.add_sized(btn_size, egui::Button::new("").fill(TRANSPARENT)...);
    // 系统 C: painter 直接绘制（覆盖在 Button 的 rect 上）
    ui.painter().rect_filled(close_resp.rect, ..., close_fill);
    ui.painter().text(close_resp.rect.center(), ..., ICON_X, ...);
    
    // 系统 A: Frame 响应式布局
    let conn_resp = egui::Frame::new().show(ui, |ui| {
        ui.horizontal(|ui| {
            // 系统 B: 精确分配
            let dot_rect = ui.allocate_exact_size(dot_size, Sense::hover()).0;
            // 系统 C: painter 直接绘制
            ui.painter().circle_filled(dot_rect.center(), 4.0, conn_color);
            // 系统 A: 响应式 label
            ui.label(RichText::new(conn_label)...);
        });
    });
});
```

**一个 20 行的函数里，三种布局系统切换了 5 次。**

---

## 三、根因分析

### 表层原因：缺乏 egui 规范

团队没有一份 `EGUI_LAYOUT.md` 规定：
- 什么时候用 `painter`
- 什么时候用 `ui.label()`
- 什么时候用 `Button` vs `SelectableLabel` vs `ui.interact()`
- 布局常量（间距、边距、高度）应该存在哪里

### 深层原因：retained-mode 思维惯性

代码作者在写这些代码时，心智模型是：
> "我要在 (x, y) 位置画一个图标，然后在 (x+24, y) 位置画文本，然后在 (x+4, y+20) 位置画一个点..."

这是 Qt/WPF/Flutter 的思维。在这些框架中，你创建 widget 对象并设置它们的 `x`、`y`、`width`、`height`。

egui 的心智模型应该是：
> "我要一个水平行，里面先有一个图标，然后有一段文本，下面还有一行小字..."

然后让 `ui.horizontal()` 和 `ui.vertical()` 自动安排位置。

### 根本原因：早期代码没有重构

这些反模式可能源于早期原型阶段——当时需要快速实现自定义视觉效果（glassmorphism、自定义 titlebar），而 egui 的内置组件默认样式不够灵活。于是作者选择了"绕过 egui，自己画"的捷径。

但问题是：**这些原型代码没有被重构**。它们成为了后续所有 UI 代码的"参考实现"，导致反模式被复制粘贴到 12+ 个文件中。

---

## 四、影响评估

### 修改复杂度量化

以"角色卡片高度从 56px 改为 48px"为例：

| 步骤 | 当前代码 | 正确代码 |
|------|----------|----------|
| 改高度 | 1 处 | 1 处 |
| 调整垂直居中 | 4 处（line_y, dot_y, name_y 及它们的偏移） | 0 处（自动） |
| 验证文本不溢出 | 手动检查 | 自动（egui clip） |
| 验证悬停背景对齐 | 手动检查（painter.rect_filled 使用 btn_resp.rect） | 自动（Button 内置） |
| **总计** | **~5 处修改 + 手动验证** | **1 处修改** |

以"新增一个子元素（比如在角色卡片上加一个徽章）"为例：

| 步骤 | 当前代码 | 正确代码 |
|------|----------|----------|
| 计算新元素的坐标 | 需要理解现有 7 个坐标的相对关系，找到合适的插入点 | 在 `ui.horizontal()` 或 `ui.vertical()` 中添加一行 |
| 处理与其他元素的重叠 | 手动调整现有元素的坐标为新元素腾出空间 | 自动（egui 流式布局） |
| 处理响应式（sidebar 宽度变化） | 需要验证所有绝对坐标在宽度变化时仍正确 | 自动 |
| **总计** | **30+ 分钟 + 高 bug 风险** | **< 1 分钟** |

### 主题系统失效

`theme.rs` 定义了语义化 token（`space_8`, `space_12`, `radius_md`, `text_base` 等），但这些 token 只在部分代码中被使用。硬编码坐标直接绕过了 theme 系统：

```rust
let content_left = btn_resp.rect.min.x + 12.0;  // 12.0 应该是 theme.space_12？
let line_y = btn_resp.rect.min.y + 10.0;         // 10.0 不在 theme 中
```

这导致**theme 是装饰性的**——改了 theme 值，UI 不会一致变化。

---

## 五、重构路线图

### 阶段 1：止血 — 制定规范（1~2 天）

创建 `EGUI_LAYOUT.md`，强制规定：

```
RULE 1: 禁止 Ghost Button
  - 任何需要交互的区域，必须使用 egui 内置组件（Button, SelectableLabel, Checkbox 等）
  - 如果内置组件的样式不满足需求，用 Frame + inner layout 包装，而不是 painter 覆盖

RULE 2: painter 仅用于装饰
  - painter.text() 仅用于：画布标注、图表标签、游戏 HUD
  - UI 文本必须使用 ui.label() / ui.heading() / RichText
  - painter.rect_filled() 仅用于：分割线、装饰性背景、图表
  - UI 背景必须使用 Frame::fill() 或 Button::fill()

RULE 3: 禁止 raw rect + ui.interact()
  - 交互元素必须使用：Button, SelectableLabel, Checkbox, RadioButton, DragValue, Slider
  - 特殊需求用：Frame::show() + .response + Sense::click()

RULE 4: allocate_exact_size 仅用于占位
  - 允许：已知尺寸的 spacer、拖拽手柄、装饰性图形
  - 禁止：交互组件、文本区域、按钮

RULE 5: 所有布局常量必须通过 theme 系统
  - 禁止硬编码 > 8.0 的像素值
  - 所有间距使用 theme.space_* 或 ui.spacing()
  - 所有尺寸使用 theme.text_* 或 theme.radius_*
```

### 阶段 2：提取可复用组件（2~3 天）

将当前内联的坐标算术提取为响应式组件：

```rust
// widgets/sidebar_card.rs
pub fn sidebar_card(ui: &mut Ui, icon: &str, title: &str, subtitle: Option<&str>,
                    badge: Option<&str>, is_active: bool, theme: &Theme) -> Response {
    // 使用 Frame + horizontal/vertical 布局，零硬编码坐标
}

// widgets/status_capsule.rs
pub fn status_capsule(ui: &mut Ui, dot_color: Color32, label: &str,
                      theme: &Theme) -> Response {
    // 使用 Frame + horizontal 布局
}

// widgets/tab_button.rs
pub fn tab_button(ui: &mut Ui, title: &str, is_active: bool, is_hovered: bool,
                  theme: &Theme) -> TabResponse {
    // 使用 SelectableLabel + 自定义 Frame
}
```

### 阶段 3：逐文件重构（1~2 周）

按优先级重构文件：

1. `panels/sidebar.rs`（最严重，Ghost Button × 3 + painter × 8）
2. `main.rs` titlebar（Ghost Button × 5 + painter × 10）
3. `panels/chat/header.rs`（allocate_exact_size + painter）
4. `ui/file_browser.rs`（ui.interact + painter）
5. 其他文件

**重构原则**：
- 每次只重构一个文件，确保编译通过
- 保持视觉输出像素级一致（截图对比）
- 不添加新功能，纯重构

### 阶段 4：建立 CI 防护（1 天）

- Clippy lint：检测 `painter.text()` 在 UI 面板中的使用
- 代码审查 checklist：强制检查 RULE 1~5

---

## 六、立即行动建议

**今天可以做**：
1. ✅ 接受本报告，创建 `EGUI_LAYOUT.md`
2. ✅ 冻结所有新的 UI 功能开发，直到阶段 1 完成
3. ✅ 在 PR 模板中添加布局规范 checklist

**本周内**：
4. 提取 `sidebar_card` 和 `status_capsule` 组件，替换 sidebar.rs 和 main.rs 中的内联实现
5. 验证重构后的视觉一致性

**不推荐的方案**：
- ❌ 继续在当前架构上添加新 UI 元素（复杂度指数增长）
- ❌ 一次性重构所有文件（风险太高，难以回滚）
- ❌ 用另一个 UI 框架替换 egui（成本过高，egui 本身没问题）

---

## 附录：反模式全量清单

### painter.text() 在 UI 中的使用（应替换为 ui.label）

| 文件 | 行 | 上下文 |
|------|-----|--------|
| `panels/sidebar.rs` | 181, 189, 220 | 顶部工具栏按钮图标 |
| `panels/sidebar.rs` | 418, 427 | 角色卡片图标、名称 |
| `panels/sidebar.rs` | 483 | 角色卡片 latest name |
| `ui/file_browser.rs` | 189 | 文件列表文本 |
| `ui/icons.rs` | 71, 84 | 图标绘制（合理：纯装饰） |
| `main.rs` | 262, 303, 336, 372 | 窗口控制按钮图标 |

### painter.rect_filled() / circle_filled() 在 UI 中的使用

| 文件 | 行 | 上下文 |
|------|-----|--------|
| `panels/sidebar.rs` | 396 | 角色卡片悬停背景 |
| `panels/sidebar.rs` | 439, 440 | 角色卡片状态点 |
| `ui/file_browser.rs` | 129, 134, 137 | 文件行背景、accent 条 |
| `main.rs` | 257, 286, 326, 362 | 窗口按钮悬停背景 |
| `main.rs` | 398, 433 | 状态胶囊圆点 |
| `ui/icons.rs` | 14, 70, 84 | 图标绘制（合理） |

### allocate_exact_size 在交互组件中的使用

| 文件 | 行 | 上下文 |
|------|-----|--------|
| `panels/chat/header.rs` | 82 | Tab 项（交互） |
| `main.rs` | 221 | 拖拽区域（合理：占位） |
| `main.rs` | 396, 431 | 状态圆点（合理：占位） |
| `render/turn_renderer.rs` | 222, 261 | 消息操作按钮（交互） |
| `widgets/toggle.rs` | 11 | Toggle 开关（自定义组件，合理） |
| `components/settings/*.rs` | 多行 | 设置面板行（交互） |

### ui.interact() 在 raw rect 上的使用

| 文件 | 行 | 上下文 |
|------|-----|--------|
| `panels/sidebar.rs` | 300 | clickable_row |
| `panels/chat/header.rs` | 147 | Tab 关闭按钮 |
| `ui/file_browser.rs` | 115 | 文件浏览器行 |
