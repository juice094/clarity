# Clarity 桌面端技术路线评估：egui 继续投入 vs 迁移 Tauri

> 评估日期：2026-07-05
> 评估范围：`crates/clarity-egui` 及其依赖的 `clarity-ui`、`clarity-apps`
> 目标：回答三个问题——当前 bug/布局问题是否"绝对不可解决"？egui 能否做到主流 AI 客户端的现代感？迁移 Tauri 的代价是否可接受？

---

## 1. 执行摘要

### 1.1 核心结论

- **布局 bug 和"不好看"不是同一个问题。** 布局错位、热路径解析、硬编码坐标等 bug，本质上是工程纪律问题，egui 本身可以做好；已存在的 `EGUI_LAYOUT.md` 七条铁律和 Pretext 冷热路径分离证明这条路走得通。
- **"现代感"的上限是结构性的。** egui 可以做到"干净、统一、专业的原生 Rust GUI"（参考 Rerun Viewer），但无法做到 Kimi/Cursor/Claude Desktop 级别的玻璃态、多层柔和阴影、复杂 z-index、sticky 滚动吸附、子像素字体等效果。这些不是缺少某个库，而是 immediate-mode GPU 渲染管线的根本限制。
- **Tauri 不是银弹，但它直接消除了 egui  hardest 的天花板。** 代价是重写 UI 层、引入 JS/TS 工具链、Rust↔Web IPC 桥接、以及长期维护两套运行时。

### 1.2 推荐方案

**推荐：B（迁移到 Tauri），但采用"后端保持 Rust，前端渐进重写"的分阶段路线，而非一次性大爆炸。**

置信度：**75%**

理由：
- 产品目标（主流 AI 桌面客户端的现代感）与 egui 的能力天花板存在不可调和的冲突。
- clarity 的后端/领域层已经按 crate 拆分，`clarity-core` / `clarity-apps` 与 egui 解耦程度较好，具备迁移基础。
- 一次性重写风险过高；先用 Tauri 套壳复用现有 Rust 后端，再逐步替换前端面板，是可控路径。

### 1.3 为什么不选 A 或 C

- **A（继续 egui）**：适合"接受 egui 美学上限、优先稳定交付"的保守场景。若产品定位是"开发者的本地 Agent 运行时"而非"消费级 AI 客户端"，A 是合理选择。
- **C（混合 egui + webview）**：理论上可行，但 clarity 当前没有 webview 嵌入经验，两套渲染管线（GPU immediate-mode + webview retained-mode）在同一窗口内会引入输入焦点、字体、动画、主题同步等大量边界问题。短期维护成本高于直接迁移。

---

## 2. 详细对比

### 2.1 渲染模型

| 维度 | egui | Tauri（webview） | 对 AI 客户端的影响 |
|------|------|------------------|---------------------|
| 模式 | Immediate mode：每帧重建 UI 并重新 layout | Retained mode：DOM 只更新变化部分 | AI 客户端大量流式文本：egui 必须做虚拟列表/冷路径解析；webview 可直接插入 DOM 节点 |
| 渲染后端 | GPU triangle mesh（wgpu/glow） | 系统 WebView（Windows WebView2 / macOS WKWebView / Linux WebKitGTK） | WebView 免费获得子像素抗锯齿、CSS 滤镜、硬件合成；egui 需自研或受限 |
| 重绘策略 | 仅在有输入/动画时重绘；但滚动流式文本会触发高频重绘 | 浏览器合成器优化，滚动/文本增量渲染成熟 | 长对话历史：egui 必须严格限制热路径工作量；webview 容错空间更大 |
| 自定义绘制 | 直接操作 painter/shader，上限高 | 受限于 WebView 能力，复杂效果需 canvas/WebGL | egui 更适合游戏化可视化；Tauri 更适合文本/界面密集型应用 |
| 跨平台一致性 | 高度一致（自绘所有像素） | 取决于系统 WebView 版本，Linux WebKitGTK 明显落后 | Tauri 在 Linux 上可能遇到 CSS 新特性不可用的问题 |

**事实**：egui 官方 README 明确将 "Native looking interface" 列为 non-goal。[egui GitHub](https://github.com/emilk/egui)

### 2.2 布局能力

| 维度 | egui | Tauri/CSS | 备注 |
|------|------|-----------|------|
| 布局语言 | `ui.vertical` / `ui.horizontal` / `StripBuilder` / `Grid` | Flexbox / Grid / position / float | CSS 是数亿人验证过的布局系统 |
| 双向尺寸协商 | 困难；immediate mode 需先定位置再定尺寸 | 自然；浏览器多 pass layout | 这是 egui 官方文档承认的 "fundamental shortcoming" |
| 响应式断点 | 需手动根据 `available_width` 分支 | CSS media queries / container queries | Tauri 对响应式支持远胜 |
| 复杂换行/截断 | 手动计算，`line-clamp` 无原生支持 | `-webkit-line-clamp` / `text-overflow` | AI 消息摘要、文件名截断是高频需求 |
| sticky/吸附 | 不支持 | `position: sticky` 原生 | 聊天标题、工具栏吸附是 Kimi/Claude 标配 |

**为什么 AI 写 egui 布局容易出错？**
1. **缺少心智模型**：egui 每帧重建，需要手动管理 ID、滚动状态、hover 状态；AI 模型训练语料中 egui 代码极少，容易产生 retained-mode 的直觉错误。
2. **太容易绕过 layout**：`painter`、 `allocate_exact_size`、 `ui.interact(rect, ...)` 给了"硬编码坐标"的逃生口；AI 生成的代码经常为了"看起来对"而使用这些 API，引入后续 bug。
3. **错误不会立即崩溃**：布局错位、焦点丢失、主题不传播都是静默错误，需要人工 review 才能发现。

**为什么 AI 写 CSS 更容易出"快速样本"？**
1. **训练语料巨大**：shadcn/ui、Tailwind、Radix 等组件库被大量复制，AI 能生成可直接运行的代码。
2. **即时可视化**：浏览器 DevTools 让"调 CSS"变成分钟级反馈循环；egui 需要编译-运行-调窗口。
3. **组件化更成熟**：CSS 组件天然可复用，egui 组件需要手写 builder API 和状态管理。

### 2.3 现代感 / 美观效果

| 效果 | egui 可行性 | Tauri 可行性 | 备注 |
|------|-------------|--------------|------|
| 深色/浅色主题 | ✅ 完全可控 | ✅ CSS variables | 两者均可 |
| 圆角卡片 | ✅ `CornerRadius` + `Frame` | ✅ `border-radius` | 两者均可 |
| 玻璃态/毛玻璃 | ❌ 不支持 `backdrop-filter: blur` | ✅ 原生 | egui 硬性天花板 |
| 多层柔和阴影 | ⚠️ 基础 `Shadow`，质感弱 | ✅ `box-shadow` | egui 阴影扁平 |
| 复杂动画/过渡 | ⚠️ 可手动插值，成本高 | ✅ CSS transitions / Framer Motion | Tauri 生态更成熟 |
| 子像素字体渲染 | ❌ 灰度抗锯齿 | ✅ 系统级 ClearType/Subpixel | 长文本阅读体验差距明显 |
| 图片/视频卡片 | ⚠️ 需预加载 Texture，SVG 需 resvg | ✅ `<img>` / `<video>` | Tauri 处理富媒体零成本 |
| 代码高亮 | ✅ `egui_commonmark` + `syntect` | ✅ `react-syntax-highlighter` / Shiki | 两者均可 |
| Markdown 表格/数学公式 | ⚠️ `egui_commonmark` 功能有限 | ✅ `remark-math` / KaTeX | Tauri 对复杂 Markdown 更友好 |

**参照目标对比**（基于 `docs/kimi-egui-audit-2026-06-07.md` 的审计）：
- egui 能做到 Kimi 视觉的约 **70%**（布局、颜色、圆角）。
- **玻璃态、多层阴影、图片卡片、sticky、子像素字体** 是 egui 无法逾越的硬限制。

### 2.4 生态与社区方案

#### egui 可用方案

| 需求 | crate / 方案 | 状态 |
|------|--------------|------|
| Dock 面板 | `egui_dock` | 已使用，活跃 |
| 虚拟列表 | `egui_virtual_list` | 已使用 |
| Markdown 渲染 | `egui_commonmark` | 已使用，基础功能足够 |
| 代码编辑器 | `egui_code_editor` / `egui` 自带 `TextEdit` | 简单场景可用 |
| 图标字体 | `lucide-icons` | 已使用 |
| 语法高亮 | `syntect` | 已使用 |
| 图表/可视化 | `egui_plot` / `egui_plotters` | 活跃 |
| 富文本编辑 | ❌ 无成熟方案 | egui 文本编辑能力弱于浏览器 |

#### Tauri 可用方案

| 需求 | 方案 | 状态 |
|------|------|------|
| 组件库 | shadcn/ui + Radix UI | 社区最活跃 |
| 样式 | Tailwind CSS | 与 shadcn 深度集成 |
| 状态管理 | Zustand / Jotai / TanStack Query | 成熟 |
| 路由 | TanStack Router / React Router | 成熟 |
| 流式聊天 | Vercel AI SDK | 行业标准 |
| Markdown | `react-markdown` + `remark-gfm` + Shiki | 成熟 |
| 虚拟列表 | `@tanstack/react-virtual` | 成熟 |
| 桌面壳能力 | Tauri v2 plugins（fs、notification、global shortcut、updater） | v2 已稳定 |

**关键观察**：Tauri 的前端生态可以让 clarity 用社区方案替换大量自研模块；egui 生态则要求很多 UI 能力自研或做大量封装。

### 2.5 性能、包大小、启动时间

| 维度 | egui | Tauri | 说明 |
|------|------|-------|------|
| 二进制大小 | 中等（~20-50 MB，取决于依赖和 assets） | 很小（~3-15 MB，无 bundled Chromium） | Tauri 显著领先 |
| 启动时间 | 快（纯原生，无 JS 引擎初始化） | 较快，但受 WebView 初始化影响 | egui 略胜或持平 |
| 内存占用 | 较低（无多进程 WebView） | 较低（~30-100 MB），但 WebView 进程额外占用 | 两者均优于 Electron |
| 滚动/流式文本 | 必须严格虚拟化，否则 CPU 吃紧 | 浏览器合成器优化，容错高 | Tauri 对长对话更友好 |
| 离线一致性 | 完全一致（自绘） | 依赖系统 WebView 版本 | Tauri 在 Linux 上风险最高 |

参考实际迁移案例：Electron → Tauri 后，包从 138 MB 降至 14 MB，冷启动从 1.4 s 降至 380 ms，内存从 210 MB 降至 65 MB。[Tauri vs Electron: What I Learned Shipping a Desktop App in Both](https://alanregaya.dev/blog/tauri-vs-electron-lessons)

### 2.6 AI 开发效率

| 维度 | egui | Tauri/CSS |
|------|------|-----------|
| 组件复用 | 需手写 widget，AI 容易绕过 layout 规则 | shadcn 组件即插即用 |
| 设计系统 | 需 Rust token + 手动同步 | Tailwind theme + CSS variables |
| 提示工程 | 需反复强调"不要使用 painter"、"走 StripBuilder" | 提示更自然 |
| 视觉迭代 | 编译-运行-调试循环慢 | 热重载 + DevTools 极快 |
| 长期可维护 | 依赖人工 review 防止 layout 反模式 | 社区组件库降低审查负担 |

---

## 3. 迁移成本粗估

### 3.1 当前 egui 专用代码规模

| crate | .rs 文件数 | 代码行数 | egui/eframe 依赖程度 |
|-------|------------|----------|----------------------|
| `clarity-egui/src` | 154 | ~31,800 | 极高（93 个文件引用 egui/eframe） |
| `clarity-ui/src` | - | ~6,100 | 高（728 处 egui 引用） |
| `clarity-apps/src` | - | ~5,300 | 中（171 处引用，主要是 `ChatStore` 等状态） |
| `clarity-core/src` | - | ~43,800 | 低（纯领域逻辑，几乎无 UI 依赖） |

### 3.2 迁移工作量估算

假设团队有 1-2 名熟悉 Rust 的工程师，前端需要补充 1 名熟悉 React/TS 的工程师：

| 阶段 | 内容 | 估算 |
|------|------|------|
| 1. 基础设施 | Tauri v2 项目骨架、CI、打包、签名、自动更新 | 1-2 周 |
| 2. IPC 协议 | 将 `UiEvent` / AppState 序列化为 Tauri command + event | 2-3 周 |
| 3. 后端复用 | 将现有 Rust 服务暴露为 Tauri commands（chat、session、settings、gateway 等） | 3-4 周 |
| 4. 前端重写 | 侧边栏、聊天、输入框、消息气泡、设置、modals、右键 IDE 面板 | 8-12 周 |
| 5. 虚拟列表/流式 | 长对话性能调优、Markdown/代码高亮/图片预览 | 2-3 周 |
| 6. 主题/设计系统 | Tailwind + shadcn 设计系统对齐现有品牌 | 2-3 周 |
| 7. 测试/回归 | 集成测试、跨平台测试、CI 适配 | 3-4 周 |
| **总计** | | **约 4-7 个月（2-3 人全职）** |

**推断依据**：
- `clarity-egui` 31,800 行 UI 代码基本需要重写为前端代码；按每人月 3,000-5,000 行有效 UI 代码估算，仅前端约需 2-3 人月。
- IPC 和后端封装工作量取决于现有 `clarity-apps` 抽象的干净程度；从 `main.rs` 看，状态访问已通过 `chat_store()`、`settings_store()` 等封装，有利于复用。
- 没有计入产品经理、设计师、测试人员的时间。

### 3.3 不可迁移资产

以下 egui 专用资产需要重新实现：
- `ui/render.rs`：消息气泡布局 → React 组件。
- `ui/markdown.rs`：自定义 Markdown 解析 → `react-markdown`。
- `pretext.rs` / `pretext_alignment.rs`：Pretext 文本测量 → 浏览器原生测量或 virtual list 库。
- `widgets/`：自定义 widget → shadcn 组件或自研 React 组件。
- `theme.rs`：Rust Theme token → Tailwind/CSS variables。

---

## 4. 如果选择继续 egui：立即要做的 5 件事

按优先级排序：

1. **执行 Kimi 视觉审计的 P0 项**（`docs/kimi-egui-audit-2026-06-07.md`）
   - 背景色改为 `#121212`，accent 改为 `#1a88ff`。
   - 侧边栏扁平化为 32px 行高，取消 ALL CAPS 和折叠组。
   - 输入区从 TUI 风格改为圆角卡片 composer。
   - 这是验证"egui 能走多远"的最小实验。

2. **把 `clarity-ui` 中的 egui 依赖彻底隔离**
   - 当前 `clarity-ui` 仍有 728 处 egui 引用，说明设计系统没有真正与渲染层解耦。
   - 目标：`clarity-ui` 只提供语义 token 和纯数据结构，不引用 egui。

3. **引入/升级社区 crate，替换自研模块**
   - 用 `egui_markdown` 或保持 `egui_commonmark` 最新版替换部分自定义 Markdown 解析。
   - 评估 `egui_tiles` 替换当前自定义面板管理。
   - 这是用户明确接受的"用社区方案替换自研复杂模块"。

4. **建立 UI 快照测试 + egui_kittest 回归**
   - 现有 `EGUI_LAYOUT.md` 七条铁律需要自动化检查，而不是依赖人工 review。
   - 添加 kittest 快照测试，防止布局回归。

5. **设定 egui 美学"红线"，停止追逐不可能的效果**
   - 明确产品接受 egui 的硬限制（无玻璃态、无多层阴影、无 sticky）。
   - 若产品决策要求突破红线，则触发迁移评估。

---

## 5. 如果选择 Tauri：立即要做的 5 件事

按优先级排序：

1. **搭建 Tauri v2 最小可运行骨架**
   - 新建 `crates/clarity-tauri` 或 `apps/clarity-desktop`。
   - 技术栈：Tauri v2 + Vite + React 19 + TypeScript + Tailwind + shadcn/ui。
   - 目标：一个窗口能调用 Rust 后端的 `greet` 命令并显示。

2. **定义 Rust↔TS IPC 协议**
   - 将 `UiEvent` 枚举映射为 Tauri event / command。
   - 设计错误类型（`AppError` 序列化），避免字符串化错误丢失语义。
   - 这一步决定后续重写能否复用现有后端。

3. **抽取可复用的后端服务层**
   - 将 `services/agent_runner.rs`、`services/wire_dispatcher.rs`、`handlers/chat.rs` 中的业务逻辑封装为 Tauri command handler。
   - 避免在 command handler 里直接操作 UI 状态。

4. **实现聊天面板 PoC**
   - 用 `react-markdown` + Shiki + `@tanstack/react-virtual` 实现消息列表。
   - 验证流式输出、虚拟列表、代码复制、重新生成等核心交互。
   - 这是用户最痛的场景，PoC 成功后再扩展其他面板。

5. **制定 egui 版本的日落计划**
   - 明确 egui 版本维护到何时、新功能是否停止、bug 修复策略。
   - 避免长期同时维护两套完整 UI。

---

## 6. 混合方案的可能性与边界

### 6.1 什么场景适合混合

| 场景 | 方案 | 理由 |
|------|------|------|
| 复杂 Markdown 渲染（数学公式、Mermaid 图） | 在 egui 窗口内嵌入一个 webview 面板 | egui 没有成熟公式/图表渲染 |
| 登录/设置等表单密集页面 | 用 Tauri 做完整页面 | 表单用 web 技术成本低 |
| 主聊天界面 | 不建议混合 | 输入焦点、滚动、主题同步复杂 |
| 视频/图片预览 | webview 展示 | egui 媒体处理弱 |

### 6.2 为什么不把混合作为主线

- **输入焦点地狱**：egui  immediate-mode 与 webview retained-mode 的 focus、快捷键、上下文菜单会冲突。
- **主题同步成本**：两套渲染层需要同步颜色、字体、圆角、动画曲线。
- **调试复杂度**：问题可能出现在 Rust、egui、webview、JS 任意一层。
- **AI 生成代码更难**：AI 难以同时理解两套渲染模型，生成代码的错误率会更高。

**结论**：混合方案适合作为**特定富媒体面板的补充**，不适合作为 clarity 主 UI 的长期架构。

---

## 7. 风险与反向指标

### 7.1 迁移 Tauri 的主要风险

| 风险 | 严重性 | 缓解 |
|------|--------|------|
| Linux WebView 版本碎片化，CSS 新特性不可用 | 高 | 明确 Linux 支持范围；对旧版 WebKitGTK 做降级处理 |
| 团队 Rust 能力强但前端经验不足 | 中高 | 补充前端工程师；先 PoC 验证 |
| IPC 性能瓶颈（大量流式 token） | 中 | 批量推送、使用 Tauri event stream、WebSocket 桥接 |
| 长期维护两套 UI | 高 | 设定 egui 版本明确的 EOL |
| 安全模型学习成本（Tauri capabilities） | 中 | 跟随官方模板，逐步放开权限 |
| 自动更新/签名/代码签名证书 | 中 | Tauri v2 updater 已支持 |

### 7.2 继续 egui 的主要风险

| 风险 | 严重性 | 缓解 |
|------|--------|------|
| 产品视觉上始终落后竞品 | 高 | 明确产品定位；若定位消费级，则必须迁移 |
| AI 生成代码持续引入 layout 反模式 | 中 | 强化 kittest 自动化检查 |
| 每版 egui 升级可能破坏 UI | 中 | egui 官方："If you want something that doesn't break when you upgrade it, egui isn't for you (yet)." |
| 高级 UI 效果无法实现导致设计妥协 | 高 | 设计系统与 egui 能力对齐 |

### 7.3 反向指标：什么时候应该重新考虑

**应该重新考虑"继续 egui"的情况：**
- 产品明确要求与 Kimi/Cursor/Claude Desktop 视觉对齐。
- 连续两个迭代，egui 的 layout bug 仍然占新 bug 的 30% 以上。
- 设计团队提出玻璃态、复杂阴影、sticky 吸附等需求。

**应该重新考虑"迁移 Tauri"的情况：**
- Linux 是核心目标平台且用户 WebView 版本无法控制。
- 团队无法招募/培养 React/TS 前端工程师。
- 迁移 PoC 中 IPC 延迟导致流式输出卡顿无法解决。
- 产品定位调整为开发者工具/可视化工具，egui 上限可接受。

---

## 8. 信息来源与事实/推断声明

### 8.1 事实（基于实际代码或可验证来源）

- `clarity-egui/src` 有 154 个 .rs 文件，约 31,800 行代码（`find` + `wc -l`）。
- 93 个文件引用 `eframe`/`egui`，75 个文件引用 `clarity_ui`/`clarity_apps`（`grep -rln`）。
- `clarity-egui` 当前使用 egui 0.35.0、eframe 0.35.0、`egui_dock` 0.20.1、`egui_commonmark` 0.24.0（`Cargo.toml`）。
- egui 官方 README 将 "Native looking interface" 和 "something that doesn't break when you upgrade it" 列为 non-goal。
- egui 官方文档承认 immediate mode 的 "fundamental shortcoming" 是 layout 的双向尺寸协商。
- Tauri v2 使用系统 WebView，对比 Electron 典型包大小从 138 MB 降至 14 MB，内存从 210 MB 降至 65 MB（第三方案例）。

### 8.2 推断（基于经验和间接证据）

- egui 能达到 Kimi 视觉的约 70%，玻璃态/多层阴影等为硬限制（基于 `docs/kimi-egui-audit-2026-06-07.md` 的审计结论）。
- 迁移 Tauri 的前端重写工作量约 4-7 个月（基于代码行数、组件复杂度和类似项目经验估算）。
- AI 生成 CSS 的效率高于 egui（基于训练语料量、组件库成熟度和即时可视化反馈推断）。

---

## 9. 推荐的下一步行动

1. **本周内**：召开 30 分钟决策会，确认产品定位——"开发者工具"还是"消费级 AI 客户端"。这会直接决定 A 或 B。
2. **若定位消费级 AI 客户端**：启动 Tauri PoC，用 2 周时间验证 IPC + 聊天面板 + 虚拟列表。
3. **若选择继续 egui**：立即执行 Kimi 审计的 P0 视觉改动（2 周），并隔离 `clarity-ui` 中的 egui 依赖。
4. **无论选哪条路线**：不要长期维持两套完整 UI；设定明确的 EOL 或红线。

---

*本报告基于 2026-07-05 的代码状态与公开资料。建议每 3 个月或在产品定位变化时复核一次。*
