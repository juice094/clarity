# Sprint 43 执行计划 — 风险点判断与推进路线

> 生成日期：2026-05-10  
> 状态：Plan 模式  
> 主会话：质量与风险把控  
> 子代理：许可并行协作

---

## 一、当前上下文快照

| 维度 | 状态 |
|------|------|
| HEAD | `e6b5491a`（Sprint 42 完成 + `[+]` 恢复） |
| Working tree | 干净 |
| 编译 | `cargo check` 0 errors, 0 warnings |
| 未提交文档 | 0 |
| Sprint 42 遗留 | **全部清理完毕** |
| Sprint 43 规划 | `sprint-43-egui-layout-refactor-plan.md` 已交付 |

---

## 二、已知问题全量清单（风险点判断）

### P0 — 架构危机（阻塞所有 UI 迭代）

| # | 问题 | 文件 | 影响范围 | 风险等级 |
|---|------|------|----------|----------|
| P0-1 | Ghost Button：透明 Button + painter 覆盖 | `sidebar.rs`, `main.rs` | 所有交互组件 | 🔴 极高 |
| P0-2 | 混合布局系统：ui.horizontal / allocate_exact_size / painter 三种混用 | 12+ 文件 | 全局布局稳定性 | 🔴 极高 |
| P0-3 | 硬编码坐标偏移（47+ 处） | `sidebar.rs`, `main.rs`, `header.rs` 等 | 主题系统失效 | 🔴 高 |
| P0-4 | `ui.interact` on raw rect | `sidebar.rs`, `header.rs`, `file_browser.rs` | 焦点/键盘导航缺失 | 🟠 高 |

### P1 — 高优先级（功能或体验受损）

| # | 问题 | 文件 | 影响 | 风险等级 |
|---|------|------|------|----------|
| P1-1 | file_browser 文件行：raw rect + painter | `ui/file_browser.rs` | 文件选择交互脆弱 | 🟠 高 |
| P1-2 | render/turn_renderer：allocate_exact_size + painter | `render/turn_renderer.rs` | 消息操作按钮 | 🟡 中 |
| P1-3 | settings tabs：allocate_exact_size + painter | `components/settings/*.rs` | 设置面板 | 🟡 中 |
| P1-4 | TitleBar "111" 谜团 | `main.rs`（未定位） | 未知 | 🟡 中 |
| P1-5 | workspace drawer compact 模式信息密度 | `panels/workspace.rs` | 文件预览 | 🟡 低（已优化 60→80px） |

### P2 — 中优先级（工程债务）

| # | 问题 | 影响 | 风险等级 |
|---|------|------|----------|
| P2-1 | 缺少 `EGUI_LAYOUT.md` 规范 | 团队无法统一布局决策 | 🟡 中 |
| P2-2 | CI 无布局防护 | 新代码可继续引入反模式 | 🟡 中 |
| P2-3 | 组件零复用 | 每个面板都内联坐标算术 | 🟢 低 |

---

## 三、风险评估矩阵

| 风险项 | 严重性 | 可能性 | 风险值 | 缓解措施 |
|--------|--------|--------|--------|----------|
| 重构引入视觉 regression | 高 | 中 | **高** | 逐文件重构，每文件截图对比 |
| 新功能在反模式代码上堆叠 | 中 | 高 | **高** | **冻结新功能** 至 Phase 2 完成 |
| 子代理理解偏差导致接口不统一 | 中 | 中 | **中** | 主会话审查所有组件 API |
| 组件接口设计不当需二次重构 | 中 | 低 | **低** | 参考 egui demo app  idiomatic 模式 |
| 工期超出 Sprint 43 边界 | 中 | 中 | **中** | Phase 3.5 明确为可降级项 |
| 编译/测试在重构中断裂 | 高 | 低 | **中** | 每文件重构后立即 `cargo check` |

**关键决策**：
- ✅ **冻结新 UI 功能**（已生效）
- ✅ **禁止新 painter.text() / painter.rect_filled() 在 UI 面板中**
- ✅ **禁止新硬编码坐标 > 8.0px**

---

## 四、推进路线（分阶段 + 子代理分工）

### Phase 0：基线固化（0.5 天）— 主会话

| 任务 | 负责人 | 验收标准 |
|------|--------|----------|
| 0.1 全量截图保存 | 主会话 | sidebar / titlebar / chat / workspace / settings 各 1 张 |
| 0.2 确认编译基线 | 主会话 | `cargo check` 0 errors, `cargo test --lib` 全部通过 |
| 0.3 发布冻结声明 | 主会话 | 在 `AGENTS.md` 中标注 Sprint 43 冻结期 |

### Phase 1：规范 + 组件提取（2 天）— 子代理并行

**子代理分工表**：

| 子代理 | 任务 | 输入 | 输出 | 验收标准 | 预估工时 |
|--------|------|------|------|----------|----------|
| **A** | 起草 `EGUI_LAYOUT.md` | 诊断报告 + egui 0.31 官方文档 | `crates/clarity-egui/EGUI_LAYOUT.md` | 5 条铁律 + 决策树 + 正反例代码 | 0.5 天 |
| **B** | 提取 `widgets/sidebar_card.rs` | `sidebar.rs:387-493`（角色卡片） | 纯函数组件，零硬编码坐标 | 使用 Frame + horizontal/vertical，零 painter 调用 | 1 天 |
| **C** | 提取 `widgets/status_capsule.rs` | `main.rs:388-414`（Connection 胶囊） | 纯函数组件 | 零 painter.circle_filled，使用 Label("●") | 0.5 天 |
| **D** | 提取 `widgets/tab_button.rs` | `panels/chat/header.rs:82-192` | 纯函数组件 | 零 allocate_exact_size，使用 SelectableLabel 或 Button | 1 天 |
| **E** | 提取 `widgets/window_control.rs` | `main.rs:238-378`（窗口控制 ×4） | 纯函数组件 | 零 Ghost Button，Button::new(icon) | 0.5 天 |

**主会话把控点**：
- 每个组件 PR 必须经过主会话代码审查
- 审查清单：零硬编码坐标 / 零 painter 调用（装饰除外）/ doc comment / 编译通过
- 接口签名统一为：`fn widget_name(ui: &mut Ui, theme: &Theme, ...) -> Response`

### Phase 2：核心面板重构（1 周）— 主会话主导，子代理辅助

| # | 文件 | 反模式 | 重构策略 | 子代理 | 主会话把控 |
|---|------|--------|----------|--------|------------|
| 2.1 | `panels/sidebar.rs` | Ghost Button ×1, painter ×8, interact ×1 | 用 `sidebar_card` 替换角色卡片；用 `SelectableLabel` 替换 `clickable_row` | B + 辅助 | **审查 + 截图对比** |
| 2.2 | `main.rs` titlebar | Ghost Button ×5, painter ×10 | 用 `window_control` + `status_capsule` 替换 | C + E | **审查 + 截图对比** |
| 2.3 | `panels/chat/header.rs` | allocate_exact_size ×1, painter ×3, interact ×1 | 用 `tab_button` 替换 tab 渲染 | D | **审查 + 截图对比** |
| 2.4 | `ui/file_browser.rs` | interact ×1, painter ×3 | 用 `SelectableLabel` + `Frame` 替换行渲染 | 辅助 | **审查 + 截图对比** |
| 2.5 | 其他 P1 文件 | allocate_exact_size + painter | 视 2.1~2.4 进度决定是否在本 Sprint 处理 | 辅助 | **审查 + 决策** |

**重构铁律**：
1. 一次一个文件，独立 commit
2. 视觉冻结：截图对比，像素差异 < 2px
3. 功能冻结：不添加新功能，不修改业务逻辑
4. 编译守护：每文件后立即 `cargo check`

### Phase 3：验证与防护（1~2 天）— 子代理执行，主会话验收

| 任务 | 负责人 | 验收标准 |
|------|--------|----------|
| 3.1 全量截图对比 | 子代理 | 所有面板与 Phase 0 基线无可见差异 |
| 3.2 主题响应性测试 | 子代理 | 改 `theme.space_12` → `space_16`，UI 自动响应 |
| 3.3 Clippy lint | 子代理 | 自定义 lint 检测 `painter.text()` 在 `panels/` 和 `main.rs` 中的使用 |
| 3.4 PR 模板更新 | 子代理 | 新增 "Layout规范检查" checklist |
| 3.5 团队同步会 | 主会话 | 30 分钟 walkthrough，确保所有人理解规范 |

---

## 五、子代理协作协议

### 启动条件

子代理在以下条件下启动：
1. 主会话明确分配了 bounded 任务（有明确输入、输出、验收标准）
2. 任务之间无代码依赖（可并行编译）
3. 主会话保留审查权和否决权

### 通信协议

- 子代理完成后，向主会话提交：代码 diff + 编译结果 + 自评检查清单
- 主会话审查后：批准 / 要求修改 / 驳回并说明原因
- 所有子代理输出必须可回滚（独立 commit）

### 质量门槛

| 门槛 | 标准 |
|------|------|
| 编译 | `cargo check -p clarity-egui` 0 errors, 0 warnings |
| 测试 | `cargo test --workspace --lib` 全部通过 |
| 规范 | 零 Ghost Button，零 raw rect interact，零 painter text（装饰除外） |
| 文档 | 每个新组件必须有 doc comment + 使用示例 |
| 视觉 | 与基线截图对比，无可见差异 |

---

## 六、立即执行项

**今天（主会话）**：
1. [ ] 批准本执行计划
2. [ ] 启动子代理 A（EGUI_LAYOUT.md）+ 子代理 B（sidebar_card）并行
3. [ ] 保存全量基线截图

**本周（子代理并行）**：
4. [ ] 子代理 A 交付规范文档
5. [ ] 子代理 B/C/D/E 交付组件
6. [ ] 主会话审查所有组件接口

**下周（主会话 + 子代理）**：
7. [ ] 启动 Phase 2 核心面板重构
8. [ ] 逐文件审查 + 截图验证

---

## 七、附录：冻结声明（硬约束）

**在 Sprint 43 Phase 2 完成前，以下活动禁止：**

- ❌ 任何新的 UI 面板或组件
- ❌ 任何涉及 `painter.text()` / `painter.rect_filled()` 的新代码
- ❌ 任何新的硬编码坐标（>8.0px）
- ❌ 任何新的 `ui.interact` on raw rect
- ❌ 修改 `theme.rs` 中已定义的 token 语义（可补充新 token）

**例外流程**：提交书面申请 → 主会话评估是否可在重构后架构上实现 → 批准/驳回
