# Pretext PoC 计划书

> 日期：2026-06-13  
> 范围：`crates/clarity-egui`  
> 依据：概念图 / Kimi 参考截图 / `C:/Users/22414/dev/pretext-rust` 源码  
> 目标：在正式推进 Phase D 之前，用最小成本验证 pretext 能否与 egui 的现有渲染管线协同工作，并给出“继续 / 回退 / 折中”的量化决策依据。

## 1. 目标与假设

在 `clarity-egui` 中接入 `pretext-core` + `pretext-fontdb`，验证以下假设：

1. **测量精度**：pretext 预测的文本宽度与 egui 实际渲染宽度误差 < 2px（覆盖 CJK/英文/代码/emoji 混排）。
2. **动态高度**：可用 pretext 提前计算消息气泡的精确换行高度，消除 `ScrollArea` 滚动时的气泡高度抖动。
3. **Rich Inline**：`pretext_core::rich_inline` 可用于布局 mention / code chip，且跨行时不会把 chip 截断。

## 2. 非目标

- **不替换 egui 的文本渲染管线**（epaint / galley）。pretext 只负责测量与换行决策，实际绘制仍交给 egui。
- **不引入 `pretext-slint`**。Slint 与 egui 是两个前端栈，本次 PoC 只关注 egui。
- **不实现文本选择 / 光标 / IME**。这些是 Phase D/E 之后的事。

## 3. 决策点与退出条件

| 测量误差 | 结论 | 下一步 |
|---|---|---|
| < 2px 且 ≥ 90% 样本通过 | **继续** | 将 `MessageBubble` 全面替换为 pretext-aware 实现，进入 Phase D 主线 |
| 2~5px | **折中** | 用 pretext 做高度估算并加入微调因子，保留 egui 原生渲染 |
| > 5px 或字体解析失败率高 | **回退** | 停止使用 pretext，记录原因，改走 egui 原生 `Galley` + 二次测量路线 |

## 4. 范围与交付物

- 在 `clarity-egui` 新增可选依赖 `pretext-core` + `pretext-fontdb`（PoC 阶段可用 `path` 指向本地 `pretext-rust`）。
- 新增 `widgets/pretext_probe.rs`：测量校准小工具，直观对比 pretext 预测 vs egui 实际渲染。
- 新增 `widgets/pretext_bubble.rs`：基于 pretext 高度计算的气泡 PoC widget。
- 新增 `tests/pretext_alignment.rs`：20+ 样本的自动化宽度/高度对齐测试。
- 输出 `docs/notes/pretext-poc-results-2026-06-XX.md` 决策报告。

## 5. 实施步骤（预计 4~5 天）

### Step 1 — 依赖接入与字体对齐（0.5 天）

1. 在 `crates/clarity-egui/Cargo.toml` 添加：
   ```toml
   [dependencies]
   pretext-core = { path = "C:/Users/22414/dev/pretext-rust/crates/pretext-core" }
   pretext-fontdb = { path = "C:/Users/22414/dev/pretext-rust/crates/pretext-fontdb" }
   ```
   > 注：若 PoC 通过，后续需改为 git 依赖、子模块或发布到 crates.io，避免硬编码绝对路径。
2. 实现 `PretextMetrics`：
   - 持有 `FontdbBackend`（系统字体 + 可选加载 egui 嵌入字体目录）。
   - 提供 `measure(text, font_descriptor) -> f32`。
   - 缓存 `FontdbBackend` 在 `App` 中，避免每气泡重建。
3. 字体映射：将 egui 当前字号（`theme.text_base`、`theme.text_md` 等）转换为 `pretext_core::Font { family, size_px, weight, style }`，确保 family 与 `fontdb` 查询一致（如 `"Inter"`、`"Noto Sans SC"`）。

### Step 2 — 测量校准小工具（1 天）

新建 `widgets/pretext_probe.rs`：

- 在独立窗口或设置页中渲染一组样本文本。
- 每行同时显示：
  - `pretext` 预测的每行宽度。
  - egui `Label::wrap` 实际渲染后通过 `ui.min_rect().width()` 回读的宽度。
  - 差值（红色高亮 > 2px）。
- 样本覆盖：短英文、长 CJK、中英混排、inline `code`、`@mention`、URL、emoji、换行符。
- 导出 `pretext_alignment.csv` 供离线分析。

成功标准：误差 < 2px 的样本 ≥ 90%。

### Step 3 — 气泡高度 PoC（1 天）

新建 `widgets/pretext_bubble.rs`：

```rust
pub struct PretextBubble {
    pub text: String,
    pub max_width: f32,
    pub font: pretext_core::Font,
}

impl PretextBubble {
    /// 用 pretext 计算换行后返回期望高度
    pub fn desired_height(&self, metrics: &dyn FontMetrics) -> f32 { ... }
}
```

- 在 `panels/chat/message_list.rs` 中**临时**替换 1~3 个消息气泡为 `PretextBubble`。
- `PretextBubble` 内部仍使用 egui `Label` 绘制文本；但在绘制前先调用 `pretext` 计算高度，并调用 `ui.allocate_exact_size` 或 `ui.set_min_size` 固定区域。
- 观察 `ScrollArea` 是否还有高度抖动、滚动条跳动。

成功标准：连续打开同一会话 10 次，气泡高度抖动次数为 0。

### Step 4 — Rich Inline chips（1~1.5 天）

1. 构造 `RichInlineItem` 序列：
   - 普通文本：`RichInlineItem::new("...", font)`
   - `@mention` chip：`RichInlineItem { text: "@Kimi", font, break_mode: Never, extra_width: 16.0 }`
   - inline code：同理 `break_mode: Never`，`extra_width` 包含左右 padding。
2. 调用 `prepare_rich_inline` + `walk_rich_inline_line_ranges` 得到每行 fragments。
3. 渲染：
   - 对每个 fragment，若 `item_index` 对应普通文本，用 egui `Label` 绘制。
   - 若对应 chip，用 `egui::Frame` + `Label` 绘制圆角背景 pill。
4. 验证 chip 在 max_width 边界处**整颗换行**，不被截断。

成功标准：10 个包含 chip 的样本全部整颗换行，无截断。

### Step 5 — 性能与内存（0.5 天）

- 构造 1000 条消息的虚拟会话。
- 在 release 模式下测量：
  - `prepare` + `layout` 总耗时。
  - `FontdbBackend` 首次加载系统字体耗时。
  - 内存占用（`FontdbBackend` 缓存）。
- 评估是否需要在 `App` 级别共享 `FontdbBackend`，还是每 widget 独立即可。

成功标准：1000 条消息准备/布局总耗时 < 50ms（release）。

### Step 6 — 决策与文档（0.5 天）

- 整理 Step 2~5 的数据到 `docs/notes/pretext-poc-results-2026-06-XX.md`。
- 根据“决策点与退出条件”给出结论。
- 更新 `docs/planning/plans/clarity-egui-pretext-layout-migration.md` Phase D 状态：
  - 若继续：细化 Phase D 任务列表。
  - 若折中：标注 fudge factor 方案。
  - 若回退：标注替代路线。

## 6. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| `FontdbBackend` 在 Windows 上选不到与 egui 完全一致的字体 | 误差 > 2px | 明确加载 egui 当前嵌入字体目录；使用 `pretext_core::Font::new("14px Inter")` 与 egui family 严格对齐 |
| egui 的 letter-spacing / line-height 与 pretext 不一致 | 高度估算偏差 | 在 `PretextBubble` 中加入 line_height_factor 微调，并在校准工具中显式测量 |
| rustybuzz shaping 开销大 | 滚动/输入卡顿 | 在 `App` 中共享 `FontdbBackend`；对不可见消息延迟准备 |
| pretext-rust 尚未发布 | CI/其他机器无法构建 | PoC 用 path 依赖；落地前改为 git 子模块或发布 |

## 7. 需要用户确认的事项

1. **是否允许在 `clarity-egui` 临时引入 `pretext-core` / `pretext-fontdb` 的 path 依赖？**（PoC 通过后再讨论长期依赖形式。）
2. **PoC 样本集是否需要包含你常用的特定对话内容？** 如果有目标截图/对话，可直接作为校准样本。
3. **是否接受“测量-only，不替换 egui 渲染”的折中方案？** 这是本 PoC 的默认路线。

## 8. 与迁移规划的关系

本 PoC 对应 `docs/planning/plans/clarity-egui-pretext-layout-migration.md` 中 **Phase D — 聊天区域 pretext 升级** 的前置验证。Phase D 后续工作（聊天气泡全面迁移、思维节点图、虚拟滚动）都建立在本次 PoC 结论之上。
