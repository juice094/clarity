# Sprint 43：egui 布局架构重构 — 项目规划书

> **文档状态**：草案 → 待审批  
> **编制日期**：2026-05-10  
> **编制人**：juice094 / AI Agent  
> **审批人**：待填写  
> **项目代号**：FOUNDATION-01  
> **优先级**：P0（阻塞所有后续 UI 迭代）  
> **预估工期**：3~4 周（14~20 个工作日）  
> **依赖前置**：Sprint 42 设计缺陷修复已交付（`main` 分支，未提交）

---

## 一、执行摘要

`clarity-egui` 的 UI 代码存在**系统性架构缺陷**：以 retained-mode GUI 思维（Qt/WPF 式绝对坐标控制）在 egui 的 immediate-mode 框架上自建了脆弱的手工布局引擎。这导致：

- **47 处硬编码坐标偏移**分布在 12 个文件中
- **任何尺寸/间距/字号调整需修改 3~7 处代码**
- **主题系统（`theme.rs`）与渲染层脱节**，theme 值变更不生效
- **新增 UI 元素需手动计算坐标**，30 分钟 vs 正确模式下的 <1 分钟

本 Sprint 的目标不是重写 UI，而是**建立 egui 布局规范、提取可复用组件、将核心面板的反模式替换为 idiomatic egui 代码**，使后续 UI 迭代回到可持续的轨道。

---

## 二、问题陈述（定量分析）

### 2.1 反模式分布矩阵

| 反模式 | 严重度 | 文件数 | 出现次数 | 阻塞性 |
|--------|--------|--------|----------|--------|
| A. Ghost Button（透明按钮 + painter 覆盖） | 🔴 P0 | 2 | 8 | 高 |
| B. `allocate_exact_size` + `painter` 绘制 | 🟡 P1 | 8 | 12 | 中 |
| C. `ui.interact` on raw rect | 🟡 P1 | 3 | 3 | 中 |
| D. 混合三种布局系统 | 🟠 P1 | 5 | 15+ | 中 |
| **硬编码坐标偏移（>8.0px）** | 🔴 P0 | 4 | 47 | 极高 |

### 2.2 修改复杂度对比

以 **"角色卡片高度 56px → 48px"** 为例：

| 维度 | 当前代码 | 目标代码 | 效率比 |
|------|----------|----------|--------|
| 需修改位置数 | 5 处（高度 + 4 个级联坐标） | 1 处 | 5× |
| 验证时间 | 10 分钟（手动检查对齐） | 0 分钟（自动） | ∞ |
| 引入 regression 风险 | 高（坐标算术易错） | 无 | — |

以 **"新增子元素（徽章）"** 为例：

| 维度 | 当前代码 | 目标代码 | 效率比 |
|------|----------|----------|--------|
| 开发时间 | 30+ 分钟 | <1 分钟 | 30× |
| 回归测试范围 | 整卡片所有坐标 | 无需 | — |

### 2.3 主题系统失效证据

`theme.rs` 定义了 40+ 语义 token，但以下 token 在核心面板中**未被使用**（被硬编码值替代）：

```rust
// theme.rs 中定义
pub const SIDEBAR_WIDTH: f32 = 240.0;      // 被多处直接使用
pub space_12: f32 = 12.0;                   // sidebar.rs 中用 12.0 硬编码
pub radius_md: f32 = 8.0;                   // 部分使用，部分用 4.0 硬编码

// sidebar.rs 中的实际代码
let content_left = btn_resp.rect.min.x + 12.0;  // 硬编码，未引用 theme.space_12
let line_y = btn_resp.rect.min.y + 10.0;        // 10.0 不在 theme 中
```

**结论**：`theme.rs` 是**装饰性配置**——改了值，UI 不会一致响应。

---

## 三、项目目标与成功标准

### 3.1 目标（Objectives）

| # | 目标 | 优先级 | 度量方式 |
|---|------|--------|----------|
| O1 | 制定并落地 egui 布局规范（`EGUI_LAYOUT.md`） | P0 | 规范文档 merged + 团队 acknowledged |
| O2 | 核心面板零 Ghost Button | P0 | `sidebar.rs`、`main.rs` titlebar 无 `Button::new("")` + painter 覆盖 |
| O3 | 核心面板零 `ui.interact` on raw rect | P0 | 同上 + `file_browser.rs` |
| O4 | 主题系统生效 | P1 | 改 `theme.space_12` → sidebar 左内边距自动变化（无需改代码） |
| O5 | 提取 3+ 可复用布局组件 | P1 | `sidebar_card`、`status_capsule`、`tab_button` 存在于 `widgets/` |
| O6 | 建立 CI 防护 | P2 | Clippy lint 检测 painter.text() 在 UI 面板中的使用 |

### 3.2 成功标准（Definition of Done）

- [ ] `cargo check -p clarity-egui` 0 errors, 0 warnings（含新增 lint）
- [ ] `cargo test --workspace --lib` 全部通过
- [ ] 截图对比：重构前后像素级一致（视觉回归测试）
- [ ] PR 通过布局规范 checklist 审查
- [ ] 主题 token 变更（如 `space_12` → `space_16`）在 0 代码修改下影响 UI

---

## 四、范围界定

### 4.1 In Scope（范围内）

| 模块 | 文件 | 动作 |
|------|------|------|
| 规范制定 | `crates/clarity-egui/EGUI_LAYOUT.md` | 新建 |
| 可复用组件 | `widgets/sidebar_card.rs` | 新建 |
| 可复用组件 | `widgets/status_capsule.rs` | 新建 |
| 可复用组件 | `widgets/tab_button.rs` | 新建 |
| Sidebar 面板 | `panels/sidebar.rs` | 重构 |
| TitleBar | `main.rs`（`render_titlebar`） | 重构 |
| Chat Header | `panels/chat/header.rs` | 重构 |
| 文件浏览器 | `ui/file_browser.rs` | 重构 |
| 主题系统 | `theme.rs` | 补充缺失 token |
| CI 防护 | `.github/workflows/` 或 `clippy.toml` | 新增 lint |

### 4.2 Out of Scope（范围外）

| 项 | 原因 |
|----|------|
| 新增 UI 功能（如新的 sidebar 节、新的设置页） | 冻结，防止在反模式上堆叠新债务 |
| 非核心面板的深度重构（dashboard、workspace、settings tabs） | 优先级降级至 Sprint 44，除非阻塞 O1~O3 |
| 替换 egui 为其他框架 | 成本过高，egui 本身无缺陷 |
| 动画/过渡效果 | 超出当前债务清偿范围 |
| 单元测试覆盖 UI 渲染 | egui 的即时模式难以单元测试，依赖视觉回归 |

### 4.3 冻结声明（Scope Freeze）

**自本规划书批准之日起，以下活动冻结至 Phase 3 完成：**
- 任何新的 UI 面板或组件
- 任何涉及 `painter.text()` / `painter.rect_filled()` 的新代码
- 任何新的硬编码坐标（>8.0px）

**例外流程**：如需解冻，需提交书面申请，说明为何不能在重构后的架构上实现，由技术负责人审批。

---

## 五、详细行动计划

### Phase 0：基线建立（0.5 天）

**输入**：Sprint 42 完成后的 `main` 分支  
**输出**：可对比的基线

| 任务 | 负责人 | 验收标准 |
|------|--------|----------|
| P0.1 提交 Sprint 42 未提交更改 | 待分配 | `git status` clean，所有更改在 `main` |
| P0.2 截取当前 UI 全量截图 | 待分配 | sidebar、titlebar、chat header、file browser 各 1 张 |
| P0.3 建立基线编译状态 | 待分配 | `cargo check` 0 errors, 已知 warnings 记录 |

### Phase 1：规范制定（1~2 天）

**输入**：诊断报告 + egui 0.31 文档  
**输出**：`EGUI_LAYOUT.md` + 团队共识

| 任务 | 负责人 | 验收标准 |
|------|--------|----------|
| P1.1 起草 `EGUI_LAYOUT.md` | AI Agent | 包含 5 条铁律 + 正反例代码 + 决策树 |
| P1.2 补充 `theme.rs` 缺失 token | 待分配 | 所有硬编码值（12.0, 10.0, 56.0 等）映射到 theme token |
| P1.3 团队评审规范 | 待分配 | 至少 1 人 ack，无 blocking comment |
| P1.4 更新 `AGENTS.md` | 待分配 | 引用 `EGUI_LAYOUT.md` 作为布局决策依据 |

**`EGUI_LAYOUT.md` 核心内容预览**：

```
RULE 1: 禁止 Ghost Button
  任何需要交互的区域，必须使用 egui 内置组件。
  反例：Button::new("") + painter.text(rect.center(), ...) → 正例：Button::new(icon)

RULE 2: painter 仅用于装饰
  反例：painter.text() 用于 UI 标签 → 正例：ui.label(RichText::new(...))

RULE 3: 禁止 raw rect + ui.interact()
  反例：ui.interact(row_rect, id, Sense::click()) → 正例：SelectableLabel / Button

RULE 4: allocate_exact_size 仅用于占位
  允许：spacer、拖拽手柄、装饰图形
  禁止：交互组件、文本、按钮

RULE 5: 所有布局常量通过 theme
  禁止硬编码 > 8.0 的像素值
```

### Phase 2：组件提取（2~3 天）

**输入**：Phase 1 规范 + 现有代码中的重复模式  
**输出**：3+ 可复用组件，编译通过

| 任务 | 文件 | 替换目标 | 验收标准 |
|------|------|----------|----------|
| P2.1 `widgets/sidebar_card.rs` | 新建 | `sidebar.rs:387-493` 角色卡片 | 零硬编码坐标，使用 Frame + vertical/horizontal |
| P2.2 `widgets/status_capsule.rs` | 新建 | `main.rs:388-414`, `423-440` 状态胶囊 | 零 painter.circle_filled，使用 ui.label("●") 或 Image |
| P2.3 `widgets/tab_button.rs` | 新建 | `panels/chat/header.rs:82-192` Tab | 零 allocate_exact_size，使用 Frame + SelectableLabel |
| P2.4 `widgets/window_control.rs` | 新建 | `main.rs:238-378` 窗口控制 × 4 | 零 Ghost Button，使用 Button::new(icon) |
| P2.5 组件文档与使用示例 | 新建 | — | 每个组件有 doc comment + 使用示例 |

**组件设计原则**：
- 每个组件是一个纯函数：`fn component_name(ui: &mut Ui, theme: &Theme, ...) -> Response`
- 内部使用 `Frame`、`horizontal`、`vertical`、`add_space` 组合
- 零 `painter` 调用（除非装饰性图形）
- 零硬编码坐标（所有尺寸来自 theme 或参数）

### Phase 3：核心面板重构（1~2 周）

**输入**：Phase 2 组件  
**输出**：重构后的面板，视觉回归通过

| # | 文件 | 反模式 | 重构策略 | 预估工时 | 风险 |
|---|------|--------|----------|----------|------|
| 3.1 | `panels/sidebar.rs` | Ghost Button ×1, painter ×8, interact ×1 | 用 `sidebar_card` 替换角色卡片；用 `SelectableLabel` 替换 `clickable_row` | 2 天 | 中（视觉细节多） |
| 3.2 | `main.rs` titlebar | Ghost Button ×5, painter ×10 | 用 `window_control` 替换控制按钮；用 `status_capsule` 替换状态指示器 | 1.5 天 | 低（组件已提取） |
| 3.3 | `panels/chat/header.rs` | allocate_exact_size ×1, painter ×3, interact ×1 | 用 `tab_button` 替换 tab 渲染 | 1 天 | 低（组件已提取） |
| 3.4 | `ui/file_browser.rs` | interact ×1, painter ×3 | 用 `SelectableLabel` + `Frame` 替换行渲染 | 0.5 天 | 低 |
| 3.5 | 其他 P1 文件（dashboard、workspace、settings） | allocate_exact_size + painter | 视 Phase 3.1~3.4 进度决定是否在本 Sprint 处理 | 2 天 | 低 |

**重构原则**：
1. **一次一个文件**：每文件独立 PR，便于回滚
2. **视觉冻结**：重构前后截图对比，像素差异 < 2px
3. **功能冻结**：不添加新功能，不修改业务逻辑
4. **编译守护**：每文件重构后立即 `cargo check`，0 errors

### Phase 4：验证与防护（1~2 天）

**输入**：Phase 3 完成的所有重构  
**输出**：CI 防护 + 团队培训材料

| 任务 | 负责人 | 验收标准 |
|------|--------|----------|
| P4.1 全量截图对比 | 待分配 | 所有面板截图与 Phase 0 基线对比，无可见差异 |
| P4.2 主题响应性测试 | 待分配 | 改 `theme.space_12` → `space_16`，sidebar 内边距自动变化 |
| P4.3 Clippy lint | 待分配 | 新增 lint 检测 `painter.text()` 在 `panels/` 和 `main.rs` 中的使用 |
| P4.4 PR 模板更新 | 待分配 | 新增 "Layout规范检查" checklist |
| P4.5 团队同步会 | 待分配 | 30 分钟 walkthrough，确保所有人理解规范 |

---

## 六、时间表与里程碑

```
Week 1 (5.11 - 5.17)
├── Day 1-2:  Phase 0 + Phase 1（基线 + 规范）
│   └── Milestone 1: EGUI_LAYOUT.md merged
├── Day 3-5:  Phase 2（组件提取）
│   └── Milestone 2: 4 个组件编译通过，单元测试通过
│
Week 2 (5.18 - 5.24)
├── Day 1-3:  Phase 3.1 + 3.2（sidebar + titlebar）
├── Day 4-5:  Phase 3.3 + 3.4（chat header + file browser）
│   └── Milestone 3: 核心面板重构完成，视觉回归通过
│
Week 3 (5.25 - 5.31)
├── Day 1-2:  Phase 3.5（其他面板，如有余量）
├── Day 3-4:  Phase 4（验证 + CI 防护）
│   └── Milestone 4: CI lint 生效，PR 模板更新
├── Day 5:    Buffer / 回归修复
│   └── Milestone 5: Sprint 43 关闭
```

**缓冲机制**：
- 每周五为 buffer day，处理本周 regression
- 若 Phase 3.5 无法在 Week 3 完成，降级至 Sprint 44
- 每日站会 15 分钟（异步：更新 Sprint 43 issue 状态）

---

## 七、资源需求

### 7.1 人力资源

| 角色 | 人数 | 职责 | 投入比例 |
|------|------|------|----------|
| Rust 开发者 | 1 | Phase 2~4 编码 | 80% |
| 技术负责人 | 1 | Phase 1 评审、Phase 4 验收、风险决策 | 20% |
| AI Agent | 1 | 辅助重构、代码生成、文档编写 | 按需 |

### 7.2 技术资源

| 资源 | 用途 | 状态 |
|------|------|------|
| egui 0.31 源码 | 参考 idiomatic 模式 | 已有（Cargo.lock） |
| 截图对比工具 | 视觉回归测试 | 需调研（egui 无内置，可用 `cargo test` + image crate） |
| Clippy 自定义 lint | CI 防护 | 需开发（`clippy.toml` 或 rustc plugin） |

---

## 八、风险矩阵与对策

| 风险 | 可能性 | 影响 | 对策 |
|------|--------|------|------|
| **R1: 重构引入视觉 regression** | 高 | 高 | 每文件重构后截图对比；一次只重构一个文件；保留回滚分支 |
| **R2: 团队不遵守新规范** | 中 | 高 | CI lint 强制；PR checklist；Code Review 阻塞机制 |
| **R3: 工期超出 3 周** | 中 | 中 | Phase 3.5 明确为可降级项；每周五 buffer day |
| **R4: egui 内置组件无法满足自定义视觉** | 低 | 高 | 使用 `Frame` + 自定义 `Margin`/`CornerRadius` 包装，而非 painter；如确实需要 painter，需在 `EGUI_LAYOUT.md` 中登记例外 |
| **R5: 重构过程中需紧急修复线上 bug** | 低 | 高 | 在重构分支上 cherry-pick 修复；或先回滚重构再修复 |
| **R6: 性能退化（egui 布局计算 vs 直接 painter）** | 低 | 中 | egui 的 layout 计算在即时模式下每帧都发生，但成本极低；如有性能问题，使用 `ui.horizontal_wrapped()` 或 `ui.with_layout()` 优化，而非回退到 painter |

---

## 九、验收标准（Checklist）

Sprint 43 关闭前必须全部勾选：

### 规范
- [ ] `EGUI_LAYOUT.md` 存在于 `crates/clarity-egui/` 根目录
- [ ] 规范包含 5 条铁律 + 决策树 + 正反例
- [ ] `theme.rs` 补充所有缺失 token（对照附录硬编码值清单）

### 代码
- [ ] `cargo check -p clarity-egui` 0 errors, 0 warnings
- [ ] `cargo test --workspace --lib` 全部通过
- [ ] `sidebar.rs`、`main.rs`、`chat/header.rs`、`file_browser.rs` 无 Ghost Button
- [ ] `sidebar.rs`、`main.rs`、`chat/header.rs`、`file_browser.rs` 无 `ui.interact` on raw rect
- [ ] `panels/` 和 `main.rs` 中 `painter.text()` 数量 = 0（装饰性除外）

### 组件
- [ ] `widgets/sidebar_card.rs` 编译通过，有 doc comment
- [ ] `widgets/status_capsule.rs` 编译通过，有 doc comment
- [ ] `widgets/tab_button.rs` 编译通过，有 doc comment
- [ ] `widgets/window_control.rs` 编译通过，有 doc comment

### 视觉
- [ ] 基线截图 vs 重构后截图：所有面板无可见差异
- [ ] 主题响应性测试：改 `theme.space_12` → `space_16`，UI 自动响应

### CI
- [ ] PR 模板包含 "Layout规范检查" checklist
- [ ] Clippy lint 或自定义脚本检测 `painter.text()` 在 UI 面板中的使用

---

## 十、附录

### 附录 A：硬编码值全量清单（待主题 token 化）

| 值 | 位置 | 当前用途 | 建议 theme token |
|----|------|----------|------------------|
| 56.0 | `sidebar.rs:392` | 角色卡片高度 | `sidebar_card_height` |
| 12.0 | `sidebar.rs:409` | 角色卡片左内边距 | `space_12`（已存在） |
| 10.0 | `sidebar.rs:410` | 角色卡片顶部偏移 | `sidebar_card_padding_top` |
| 28.0 | `header.rs:27` | Tab 预留 [+] 按钮宽度 | `tab_new_button_width` |
| 28.0 | `header.rs:83` | Tab 高度 | `tab_height` |
| 240.0 | `main.rs:45` | Sidebar 默认宽度 | `SIDEBAR_WIDTH`（已存在） |
| 36.0 | `main.rs:46` | TitleBar 高度 | `TITLEBAR_HEIGHT`（已存在） |
| 8.0 | `main.rs:180` | TitleBar 水平内边距 | `space_8`（已存在） |

### 附录 B：参考资源

| 资源 | URL | 用途 |
|------|-----|------|
| egui 0.31 docs | https://docs.rs/egui/0.31.0/egui/ | API 参考 |
| egui demo app source | https://github.com/emilk/egui/tree/master/crates/egui_demo_lib |  idiomatic 模式参考 |
| egui frame/layout guide | https://github.com/emilk/egui/blob/master/crates/egui/src/containers/frame.rs | Frame 使用 |

### 附录 C：术语表

| 术语 | 定义 |
|------|------|
| Ghost Button | 透明 `Button::new("")` 仅用于获取 rect，内容用 painter 自行绘制 |
| Retained-mode GUI | Qt/WPF/Flutter 式：创建 widget 对象，设置属性，框架负责渲染 |
| Immediate-mode GUI | egui 式：每帧重新描述整个 UI，不保留 widget 状态 |
| Theme token | `theme.rs` 中定义的语义化常量（如 `space_12`、`text_base`） |
| 视觉回归 | 重构后 UI 外观与基线不一致 |

---

## 审批记录

| 版本 | 日期 | 修改人 | 修改内容 | 审批人 |
|------|------|--------|----------|--------|
| 0.1 | 2026-05-10 | AI Agent | 初稿编制 | 待审批 |

**下一步动作**：技术负责人审批后，立即执行 Phase 0（提交 Sprint 42 更改 + 基线截图）。
