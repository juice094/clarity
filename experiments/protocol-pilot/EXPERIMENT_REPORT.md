# Protocol Pilot — 协议驱动 UI 性能基准实验

> **版本**：v1.0  
> **日期**：2026-04-29  
> **环境**：Windows 11, egui 0.31.1, Rust 1.86, release profile (opt-level=3, LTO)  
> **实验性质**：Phase 2 协议驱动试点可行性验证

---

## 一、实验定位

回答**一个具体问题**：

> **协议驱动 UI（ViewModel → ViewCommand → Protocol Renderer → egui）相比直接 egui 渲染，性能开销是多少？是否可接受？**

**不回答的问题**（需独立验证）：
- 真实复杂面板（chat 虚拟列表、Plan Review）的协议化可行性
- 跨网络传输 ViewCommand 的序列化开销（本实验为内存传递）
- TUI/Web 前端的协议翻译实现

---

## 二、实验设计

### 对照组（Direct）
直接调用 egui API，与当前 `panels/settings.rs` 实现一致。

### 实验组（Protocol-driven）
1. `SettingsViewModel::render()` → 生成 `Vec<ViewCommand>`
2. `render_commands()` → 枚举匹配 + 递归遍历，翻译为 egui draw calls
3. 收集 `UserAction` 回传 ViewModel

### 测试负载
模拟 `settings_panel` 的完整 UI：
- Provider ComboBox（3 选项）
- Model ComboBox（动态 2-3 选项）
- API Key TextInput（password 模式）
- Local Model Path TextInput
- Approval Mode ComboBox（3 选项）
- Save / Cancel 按钮

### 测量方法
- 运行 1,000 帧，每帧在独立 `egui::Context` 中完整渲染
- 测量指标：总时间、平均帧时间、Min/Max/P99
- Release profile，LTO 启用

---

## 三、关键结果

| 指标 | Direct（基准） | Protocol-driven | 差异 |
|------|---------------|-----------------|------|
| **总耗时** | 39.66 ms | 57.65 ms | **+45.4%** |
| **平均/帧** | 39.66 µs | 57.65 µs | +18.0 µs |
| **Min** | 32.0 µs | 34.3 µs | +2.3 µs |
| **Max** | 2.16 ms | 3.07 ms | +0.91 ms |
| **P99** | 69.5 µs | 93.6 µs | +24.1 µs |
| **ViewCommand 纯生成**（无 egui） | — | 4.59 µs/帧 | — |

### 开销拆解

| 来源 | 耗时/帧 | 占比 |
|------|---------|------|
| 纯 egui 渲染（Direct） | 39.66 µs | 基准 100% |
| ViewCommand 生成 | 4.59 µs | ~8% |
| Protocol Renderer（枚举匹配+递归） | ~13.4 µs | ~23% |
| **Protocol-driven 总计** | **57.65 µs** | **145%** |

---

## 四、分析与结论

### 4.1 绝对开销可接受

在 60fps 预算（16,667 µs/帧）下：
- Protocol-driven 占用 **0.35%** 的帧预算
- 额外开销仅 **18 µs/帧**

即使扩展到 10 倍复杂度的面板（如 chat），预计占用仍 **< 5%** 帧预算。

### 4.2 相对开销显著

**+45%** 的相对开销不可忽略，来源主要是：
1. **递归遍历**：`VStack`/`HStack` 嵌套导致每层都有函数调用开销
2. **枚举匹配**：每个 `ViewCommand` 变体都需要一次 `match`
3. **临时分配**：每帧重新生成 `Vec<ViewCommand>` 和 `Vec<UserAction>`

### 4.3 优化空间

| 优化方向 | 预期收益 | 复杂度 |
|---------|---------|--------|
| **Arena 分配器**：用 bump allocator 替代每帧 `Vec` 分配 | -30~50% 分配开销 | 低 |
| **扁平化 ViewCommand**：减少 `VStack`/`HStack` 嵌套层级 | -10~20% 递归开销 | 低 |
| **缓存 ViewCommand**：状态未变时复用上帧命令树 | 接近 Direct 性能 | 中 |
| **内联 renderer**：`#[inline]` 关键 match arm | -5~10% 调用开销 | 低 |

### 4.4 工程决策建议

| 场景 | 建议 |
|------|------|
| **Settings/Toast/Task 简单面板** | ✅ 立即协议化，开销可忽略 |
| **Sidebar/Approval 中等面板** | ✅ 协议化，预期 < 1ms/帧 |
| **Chat 核心面板** | ⚠️ 最后迁移，需配合缓存优化 |
| **高频交互（输入、滚动）** | ❌ 保持本地处理，不上报协议 |

---

## 五、风险验证

| 文档列出的风险 | 实验结论 |
|---------------|---------|
| 协议膨胀 | 未验证（单面板，15 种原子控件足够） |
| 延迟感知 | **不成立**：内存传递无延迟，18µs 人类不可感知 |
| 调试复杂度 | 未验证（需实际开发体验） |
| egui 生态不兼容 | 未涉及第三方库 |
| IMGUI 本质冲突 | **部分成立**：+45% 开销证明范式摩擦真实存在，但绝对值可控 |

---

## 六、一句话结论

**协议驱动 UI 在 settings_panel 级别引入 +45% 渲染开销（18µs/帧），绝对值远低于 60fps 预算。优化后（缓存+Arena）预期可压缩到 +15% 以内。建议继续推进 Phase 2 试点，按复杂度从低到高迁移面板。**
