# UI 实现对比：Clarity vs OpenHanako

> 生成时间：2026-04-27  
> 目的：为"家人陪伴"情感窗口与三栏工作台的分治设计提供事实基线  
> 范围：仅对比前端实现（布局、视觉、交互、架构），不涉及后端能力

---

## 一、技术栈与架构

| 维度 | Clarity (egui) | OpenHanako (Electron) |
|------|----------------|----------------------|
| **框架** | egui 0.31 (immediate mode, Rust) | React 18 + Vite + Electron |
| **渲染层** | wgpu/OpenGL 原生 GPU | Chromium 浏览器引擎 |
| **状态管理** | 无外部库；App struct 35+ 字段 + mpsc channel | Zustand (useStore) + localStorage |
| **架构模式** | 单窗口单线程；`update()` 热路径约束 | 多面板多标签；React 组件树 |
| **样式系统** | Rust struct (Theme) → egui::Style | CSS Modules + CSS Variables |
| **跨平台** | 原生编译 (Win/Mac/Linux) | Electron 打包 |
| **Binary size** | ~15-30MB (静态链接) | ~150MB+ (含 Chromium) |
| **启动速度** | < 1s | 2-5s |

**关键约束差异**：
- Clarity 的 `update()` 每帧运行，禁止字符串解析 / markdown / I/O / JSON → 所有预处理必须在事件循环外完成
- OpenHanako 依托浏览器，DOM 操作和 CSS 动画由引擎优化，开发者无需关心帧预算

---

## 二、布局结构（空间组织）

### Clarity：1.5 栏 + 浮动面板

```
┌─────────────────────────────────────────┐
│ [➡] Chat    [⚙][📝][🔌] ●Online  tok │  ← 顶部内嵌工具栏
├──────────┬──────────────────────────────┤
│          │                              │
│ Sidebar  │    Central Chat Area         │
│ (240px)  │    (ScrollArea + Input)      │
│          │                              │
│ Sessions │                              │
│ Files    │                              │
│          │                              │
│ [Skills] │                              │
├──────────┴──────────────────────────────┤
│ 浮动面板（Settings / Task / MCP / Skill  │
│ / Approval / Onboarding / Toast）        │
└─────────────────────────────────────────┘
```

- **左侧**：`SidePanel::left` — 会话列表 + 文件树 + 文件预览 + Skills 按钮
- **中央**：`CentralPanel` — 聊天区（虚拟列表 + 输入框）
- **无右侧边栏**
- **浮动**：Window/Modal 叠加层（Settings 是全屏半透明遮罩）

### OpenHanako：3 栏 + N 浮动面板 + 标签页

```
┌──────┬──────────────────────────────┬──────────┐
│  TB  │    ChannelTabBar             │   TB     │
├──────┼──────────────────────────────┼──────────┤
│      │                              │          │
│ Side │    MainContent               │  Jian    │
│ bar  │    ├─ chat-area              │  Side    │
│      │    ├─ channel-view           │  bar     │
│      │    ├─ plugin-page            │  (Desk)  │
│      │                              │          │
│      │    input-area                │          │
├──────┴──────────────────────────────┼──────────┤
│ StatusBar / Toast / MediaViewer     │          │
│ LeavesOverlay / FloatPreviewCard    │          │
└─────────────────────────────────────┴──────────┘
```

- **左侧 Sidebar**：会话列表 + ActivityBar（Bridge/Activity/Automation/Browser）+ 频道列表（channels tab）
- **中央 MainContent**：标签页驱动 — `chat` / `channels` / `plugin:*`
- **右侧 Jian Sidebar**：Desk（文件工作台）/ PluginWidget / ChannelInfo — 可 resize，有独立 resize handle
- **PreviewPanel**：独立可 resize 面板（580px），用于 artifact 预览
- **浮动面板**：ActivityPanel / AutomationPanel / BridgePanel / SkillViewerOverlay / ChannelCreateOverlay

**核心差距**：Clarity 缺少右侧边栏概念；OpenHanako 的 Jian 是一个完整的「第二工作空间」。

---

## 三、主题与视觉系统

### Clarity

| 维度 | 值 |
|------|-----|
| **Tokens** | 22 color + 6 spacing + 4 radius + 3 duration + 2 font |
| **Dark bg** | `#0f0f11` (Zed-inspired deep space) |
| **Light bg** | `#ffffff` |
| **Accent** | Violet `#8b5cf6` (统一，无角色区分) |
| **Texture** | 无 |
| **Shadow** | 轻量 (offset [0,2], blur 6) |
| **动画** | 仅 duration tokens，无 CSS transition 等效 |

### OpenHanako

| 维度 | 值 |
|------|-----|
| **Tokens** | CSS Variables (~40+ 自定义属性) |
| **Themes** | warm-paper / midnight / sakura |
| **Warm bg** | `#F8F5ED` (rice paper) |
| **Midnight bg** | `#3B4A54` |
| **Accent** | 主题绑定 — warm-paper 用 `#537D96` (blue-grey)，midnight 用 `#AA798D` (soft pink) |
| **Texture** | 3-layer rice-paper texture overlay，`background-blend-mode: lighten` |
| **Shadow** | CSS box-shadow + 多层 z-index |
| **动画** | CSS transitions + keyframes (leaf drift, typing pulse) |

**关键差距**：OpenHanako 有「材质」概念（纸质纹理）；Clarity 是 flat digital。材质对「侘寂/陪伴」美学至关重要。

---

## 四、字体与排版

### Clarity

| 用途 | 字体 |
|------|------|
| Body | Inter (Latin), 系统 CJK (simhei/msyh/simsun) |
| Mono | JetBrains Mono |
| 加载方式 | 运行时从 `C:\Windows\Fonts\` 读取 |
| CJK 风险 | 依赖系统字体，Linux/Mac 可能缺失；未内嵌 |
| Size 影响 | 不增加 binary |

### OpenHanako

| 用途 | 字体 |
|------|------|
| Body (warm) | Noto Serif SC + Inter |
| Body (midnight) | Inter |
| Mono | JetBrains Mono |
| 加载方式 | CSS `@font-face`，字体文件内嵌在 app 包内 |
| CJK | Noto Serif SC (~5-15MB) 内嵌，跨平台一致 |
| Size 影响 | 增加 app 包体积 |

**对格雷的启示**：Noto Serif SC 的「书信感」是 warm-paper 主题的情绪核心。Clarity 若要做侘寂风，必须解决 CJK 字体加载问题。

---

## 五、消息渲染

### Clarity

- **架构**：虚拟列表 — `last_scroll_offset` + `estimate_height()` → 只渲染可视区 ±3 条
- **预处理**：`Message::prepare()` 一次解析 markdown → `Vec<RenderBlock>`
- **热路径约束**：`render.rs` 只迭代预解析 blocks，禁止实时 markdown 解析
- **气泡样式**：
  - User：右对齐，Violet 填充，`radius_lg` 左下+左上+右下，右下 4px
  - AI：左对齐，`#27272a` 填充，左上 4px，其余 `radius_lg`
  - 阴影：`bg_elevated.linear_multiply(0.25)`
- **Tool call**：卡片式，图标 + 名称 + 截断结果
- **Typing**：● ● ● 静态文本（无动画帧）

### OpenHanako

- **架构**：React 组件树，DOM 虚拟滚动（或原生 scroll）
- **预处理**：服务端/主进程侧 markdown 解析（推测，基于 hanaFetch）
- **气泡样式**：
  - CSS Module 驱动，主题变量绑定
  - User/AI 区分通过 `--user-bubble` / `--ai-bubble` CSS var
  - 圆角和阴影全由 CSS 控制
- **Tool call**：未在扫描代码中直接发现，推测由后端渲染为 markdown
- **Typing**：动画 CSS keyframes（pulse）

**关键差距**：Clarity 的虚拟列表在 egui 中是手写实现，OpenHanako 依赖浏览器引擎。Clarity 的 typing 无动画（`request_repaint_after(16ms)` 仅在有内容更新时触发）。

---

## 六、输入系统

### Clarity

| 特性 | 实现 |
|------|------|
| 组件 | `TextEdit::multiline` |
| 动态高度 | `(line_count * 20.0 + 24.0).clamp(44.0, 120.0)` |
| IME 支持 | 300ms heuristic — 输入修改后 300ms 内 Enter 视为确认而非发送 |
| 换行 | Shift+Enter |
| 发送 | Enter（非 IME 状态） |
| 附件 | 拖拽到窗口 → `raw.dropped_files` → 路径列表 |
| Steer | 加载中可 Queue-send（▶ 按钮），输入区 hint 切换 |
| 提示文本 | "Type a message..." / "Steer message queued..." |

### OpenHanako

| 特性 | 实现 |
|------|------|
| 组件 | `<textarea>` 或 contenteditable（未直接读取源码，推测） |
| 动态高度 | ResizeObserver 测量 input card，CSS var `--input-card-h` 驱动 |
| IME 支持 | 浏览器原生 |
| 换行 | Shift+Enter |
| 发送 | Enter |
| 附件 | 拖拽到 main-content → `handleDrop` → upload API / desk attach |
| Slash commands | InputArea 内支持 `/` 命令 |
| Context menu | 自定义 InputContextMenu（cut/copy/paste） |
| 提示文本 | i18n 化，含 agent name 变量 |

**Clarity 的优势**：Steer 机制（Queue-send + 取消当前响应）是 Sprint 13.5 的成果，OpenHanako 无此能力。
**OpenHanako 的优势**：Slash commands、Context menu、多附件类型（desk / upload / skill install）。

---

## 七、侧边栏/导航

### Clarity Sidebar

- **宽度**：240px，可折叠（`sidebar_collapsed`），可 resize（220-360）
- **内容**：
  1. Logo "Clarity" + 折叠按钮
  2. "+ New Chat" 按钮
  3. "Sessions" 列表 — 简单 `for` 循环，active 高亮（surface bg + accent stroke）
  4. "Files" 文件树 — `render_file_tree`，可点击预览
  5. 文件预览面板（2000 字符截断，TextEdit multiline）
  6. 底部：Skills 按钮 + Token usage + FPS（debug）
- **会话管理**：无分组、无归档、无搜索、无 pin
- **导航**：纯会话切换，无标签页/频道概念

### OpenHanako Sidebar

- **宽度**：CSS var `--sidebar-width`，可折叠，可 resize（有 resize handle）
- **内容**：
  1. 标题 + 操作按钮（New / Settings / Collapse）
  2. **ActivityBar**：Bridge / Activity / Automation / Browser — 小按钮+色彩活跃状态
  3. **SessionList**：时间分组（Today/Yesterday/Earlier），pin 置顶，streaming 状态，browser URL 指示，archived 分离
  4. 底部：ArchivedChatsButton
- **频道模式**：`currentTab === 'channels'` 时显示 ChannelListSidebar
- **导航**：多标签页（chat / channels / plugin:*）

**关键差距**：OpenHanako 的 ActivityBar 是 Kimi 方案中「右侧全局导航」的左侧对位物；Clarity 的 Skills 按钮只是一个普通按钮，无活跃状态/计数/跳转能力。

---

## 八、面板系统

### Clarity 面板清单

| 面板 | 触发 | 形态 | 内容 |
|------|------|------|------|
| Settings | ⚙ 按钮 | Window 叠加（半透明遮罩） | API key, Model, Approval mode, Batch grants clear |
| Task | 📝 按钮 | Window 叠加 | Task 列表，refresh 每 3s |
| MCP | 🔌 按钮 | Window 叠加 | MCP server 配置 |
| Skill | Skills 按钮 | Window 叠加 | Skill 查看器 |
| Approval | 运行时触发 | Modal（独占焦点） | Diff popup, Enter=Approve/Esc=Reject |
| Onboarding | 首次启动 | Window 叠加 | 引导流程 |
| Task Create | 用户触发 | Modal | 新建 task 表单 |
| Toast | 事件触发 | 浮动条 | 通知消息 |

### OpenHanako 面板清单

| 面板 | 触发 | 形态 | 内容 |
|------|------|------|------|
| ActivityPanel | ActivityBar | 浮动面板 | 运行时活动日志 |
| AutomationPanel | AutomationBar | 浮动面板 | 自动化任务/定时器 |
| BridgePanel | BridgeBar | 浮动面板 | 外部连接状态 |
| SkillViewerOverlay | 懒加载 | Overlay | Skill 详情（全屏覆盖） |
| PreviewPanel | 用户触发 | 可 resize 侧边面板 (580px) | Artifact 预览 |
| ChannelCreateOverlay | 用户触发 | Overlay | 新建频道 |
| MediaViewer | 用户触发 | Overlay | 图片/视频全屏查看 |
| ToastContainer | 事件触发 | 浮动条 | 通知消息 |
| InputContextMenu | 右键 | 浮动菜单 | Cut/Copy/Paste |
| FloatPreviewCard | hover | 浮动卡片 | 预览提示 |

**关键差距**：OpenHanako 的面板类型更丰富，且存在「持久性侧边面板」（Jian/Preview）vs「临时浮动面板」的区分。Clarity 所有面板都是临时叠加层。

---

## 九、响应式与自适应

### Clarity

- 无响应式逻辑
- 窗口最小 900×600
- Sidebar 折叠纯手动

### OpenHanako

- `CHAT_MIN_WIDTH = 400`
- Resize handler：内容区 < 400 时自动折叠 Jian → 仍不足则折叠 Sidebar
- 自动恢复：窗口放大时按 localStorage 记忆恢复侧边栏
- Plugin tab 时 Sidebar 自动隐藏

**关键差距**：OpenHanako 有成熟的响应式折叠链；Clarity 在窗口缩小时可能截断内容。

---

## 十、能力覆盖矩阵

| 能力 | Clarity | OpenHanako | 备注 |
|------|---------|-----------|------|
| 多会话管理 | ✅ 基础 | ✅ 高级（分组/归档/pin） | |
| 频道/群聊 | ❌ | ✅ ChannelsPanel | Clarity 无此概念 |
| 文件浏览 | ✅ 侧边栏树 | ✅ Desk（工作台） | OpenHanako 文件系统更深 |
| 文件预览 | ✅ 2000字符截断 | ✅ PreviewPanel | |
| 拖拽附件 | ✅ | ✅ | |
| 插件/Widget | ❌ | ✅ PluginPageView | |
| 浏览器集成 | ❌ | ✅ BrowserCard | |
| 自动化/定时器 | ✅ Task (后端) | ✅ AutomationPanel | Clarity 前端面板简陋 |
| Bridge/外部连接 | ❌ | ✅ BridgePanel | |
| MCP 配置 | ✅ 面板 | ❌ (未扫描到) | Clarity 独有 |
| Approval 工作流 | ✅ Smart + 4 modes | ❌ (未扫描到) | Clarity 独有 |
| Steer/Queue-send | ✅ | ❌ | Clarity 独有 |
| Plan 执行追踪 | ✅ Plan tracker | ❌ | Clarity 独有 |
| Skill 管理 | ✅ 面板 | ✅ SkillViewerOverlay | |
| 国际化 (i18n) | ❌ | ✅ `useI18n` + `t()` | |
| 主题切换 | ✅ Dark/Light | ✅ 3 themes + texture | |
| 多窗口 | ❌ | ❌ | **两者皆无** |
| 纸质纹理 | ❌ | ✅ 3-layer | |
| CJK 内嵌字体 | ❌ | ✅ Noto Serif SC | |
| 欢迎页/品牌 | ✅ 基础（Logo + subtitle） | ✅ 动画 + agent selector + greeting | |

---

## 十一、对「分治策略」的启示

### 1. 工作台窗口（知识/工程）

**推荐复用 OpenHanako 布局骨架**：
- 左侧 Sidebar（会话+ActivityBar）→ 直接映射 Kimi 方案
- 右侧 Jian Sidebar（Desk/工作台）→ 映射为「容器」占位区
- 标签页切换（chat / channels / plugin）→ 可扩展为「角色容器」切换

**Clarity 已有基础**：
- Sidebar 可 resize、可折叠 — 骨架存在
- 浮动面板系统 — 可承载 Activity/Automation/Bridge
- 缺少：右侧边栏、标签页、ActivityBar 形态

### 2. 情感窗口（格雷）

**必须从现有架构中剥离**：
- 不能复用三栏布局 — 信息密度过高
- 不能复用 Dark 主题 — 冷色调与「陪伴」冲突
- 需要：
  - Warm-paper 主题移植（或新建 Wabi-sabi theme）
  - Noto Serif SC 字体内嵌（5-15MB 成本）
  - 大留白、低密度排版（与现有虚拟列表逻辑冲突）
  - 时间戳模糊化（"下午"而非秒级）
  - 常驻后台线程 + 异步留言（后端需支持）

**技术债评估**：
- egui 支持纹理渲染（`Image` + `TextureHandle`）— 纸质纹理可行
- egui 字体系统支持内嵌 — 但 binary size 增加
- 多窗口在 egui 中可行（`eframe` 多 `NativeOptions`）— 但需重构 App 为单例模式

### 3. 颜色去角色化的可行性

Clarity 当前：User = Violet, AI = dark grey — 颜色兼职身份标识。
OpenHanako：未在扫描中看到 User/AI 气泡颜色差异（可能也按主题统一）。

**修正路径**：
- Clarity 的气泡颜色应改为「语义通道」— 用户操作 = accent，系统消息 = muted，错误 = danger
- 角色区分通过：信息密度（格雷低密度 vs Kimi 高密度）、排版节奏（书信式 vs 技术文档）、材质（纸质 vs 数字 flat）
- **不需要**为每个角色分配专属颜色

### 4. 右侧全局导航的形态参考

OpenHanako 的 **ActivityBar** 是最佳参考：
- 小按钮 + SVG icon
- 色彩活跃状态（dot/badge）
- 点击展开浮动面板
- 计数 badge（AutomationBadge, BridgeDot）

这与 Kimi 方案的「第一阶段（小按钮/色彩活跃状态/可跳转）」完全对齐。

---

## 十二、关键风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| egui 多窗口不成熟 | 格雷独立窗口可能不稳定 | 先在同一窗口内用 Tab/View 切换，后端预留多窗口接口 |
| CJK 字体 5-15MB | Binary size 膨胀 | 可选加载（feature flag），首次使用时下载 |
| 纸质纹理性能 | egui 纹理每帧上传 GPU | 预加载为 `TextureHandle`，不每帧重建 |
| 虚拟列表 vs 大留白 | 格雷不需要虚拟列表，但代码路径共享 | 为格雷独立渲染路径（非 `render_chat_area`） |
| 主题系统扩展 | 当前 2 主题，需扩展到 5+（各角色+系统） | Theme struct 已支持动态构造，只需新增 builder |

---

## 十三、下一步行动建议

1. **工作台窗口**：在现有 sidebar + chat 基础上，新增 `RightPanel`（参考 Jian sidebar），移植 ActivityBar 形态
2. **情感窗口**：新建独立 `companion_window.rs`，不共享 `render_chat_area`，使用独立 Theme + 字体 + 排版
3. **主题系统**：扩展 Theme 为 enum（`Dark`, `Light`, `WarmPaper`, `WabiSabi`, `CyberHUD`, `Bauhaus`），`apply()` 统一入口
4. **右侧导航**：在 sidebar 底部或新增 ActivityBar 组件，先实现 3 个按钮（Activity/Automation/Bridge）+ badge
5. **能力孤岛激活**：IS-1（subagents spawn UI）→ 需要 `RightPanel` 预留容器接口；IS-4（Cron面板）→ AutomationPanel 前端

---

*文档状态：研究完成，待用户审阅后转为架构决策。*
