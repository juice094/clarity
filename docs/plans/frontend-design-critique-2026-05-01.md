# clarity-egui 前端批评式设计审查 · Phase 2 延伸

> 参考基准：Kimi 网页版 / App 深色模式（Swiss International Style + Agent-Native UI）
> 审查范围：`crates/clarity-egui/src/panels/`, `theme.rs`, `ui/render.rs`
> 审查日期：2026-05-01

---

## 一、美学评分总览

| 维度 | 得分 | 权重 | 加权分 | 核心问题 |
|------|------|------|--------|----------|
| 色彩系统 | 6/10 | 1.0 | 6.0 | 基底偏蓝亮，缺 OLED Black 深邃感；强调色暖铜在低对比场景不够醒目 |
| 排版与字体 | 5/10 | 1.2 | 6.0 | 字号全硬编码未 tokenized；无 CLI 命令前缀美学；CJK 回退 fragile |
| 布局与空间 | 5/10 | 1.2 | 6.0 | 无会话列表侧边栏；emotion 特殊处理破坏一致性；magic number 泛滥 |
| 组件一致性 | 4/10 | 1.0 | 4.0 | raw painter API 组件无交互态；硬编码 emoji 无 fallback；图标语义模糊 |
| 交互与反馈 | 4/10 | 1.2 | 4.8 | render 路径 block_on；IME 启发式脆弱；stick_to_bottom 劫持阅读 |
| 信息层级 | 5/10 | 1.0 | 5.0 | 未实现混合气泡策略；技能未外露；空状态覆盖不全 |
| 品牌气质 | 6/10 | 0.8 | 4.8 | frameless 有辨识度，但缺 Agent-Native 气质与 Companion UI 叙事层 |
| **总分** | — | — | **36.6/80** | **≈ 4.6/10** |

---

## 二、逐项批评分析

### 2.1 色彩系统 — 6/10

**Kimi 基准**：
- 背景层：纯黑/近黑 `#000000 ~ #0A0A0A`（OLED Black）
- 卡片层：`#1C1C1E` 带微妙提亮
- 强调色：科技蓝 `#007AFF` 类，极度克制，仅用于选中态

**clarity-egui 现状**（`theme.rs:12-341`）：
- `bg`: `#12141e` — 偏蓝偏亮，不够"深空"，OLED 优化不足
- `bg_accent` / `surface`: `#181a26` — 与 bg 对比度过低，卡片浮起感弱
- `accent`: `#c98a5e`（暖铜）— 品牌识别度有，但在 warning/danger 旁边色域拥挤
- `user_bubble`: 与 accent 同色 — 用户消息和全局强调色混用，层级混淆

**不合理之处**：
1. **基底不够黑**：`#12141e` 在 OLED 屏幕上仍会有微弱光晕，缺乏"终端沉浸感"
2. **卡片层级不足**：bg 与 bg_accent 的明度差太小（约 3%），导致 sidebar、chat input bar、settings cards 等区域在视觉上几乎坍缩为同一平面
3. **暖铜色的功能过载**：同时承担 accent、user_bubble、plan tracker progress 三个角色，当 user bubble 和 plan tracker 同时出现时，视觉噪音大

---

### 2.2 排版与字体 — 5/10

**Kimi 基准**：
- 等宽命令前缀（`/new`, `/compact`, `/status`）建立 CLI 心智
- 字号层级清晰：命令 18-20px / 描述 14-15px / 技能标题 17-18px
- 字间距宽松，移动端不拥挤

**clarity-egui 现状**：
- 字体选择合理：Inter + JetBrains Mono（`theme.rs:431-436`）
- **字号完全硬编码**，分散在 15+ 个文件中，无 token 体系
- 最小字号 9px（API format badge），在 HiDPI 屏幕边缘可读性临界
- **无斜体/粗体字体文件注册**：`RichText::strong()` / `italics()` 依赖默认字体回退，可能无实际效果
- CJK 回退硬编码 Windows 路径（`C:\Windows\Fonts\simhei.ttf`），跨平台必崩

**不合理之处**：
1. **9px 超小字**：Settings 中的 provider 卡片 URL 截断标签（`settings.rs:134`），违反 WCAG 2.1 可读性底线
2. **无 CLI 语法提示**：clarity 有大量斜杠命令（`/compact`, `/status`），但 UI 中完全以按钮/菜单呈现，未利用用户的 CLI 心智模型
3. **字重单一**：全局几乎只用 Regular 400，缺少 Medium 500 建立中等层级，导致标题只能靠字号放大区分

---

### 2.3 布局与空间 — 5/10

**Kimi 基准**：
- 三区结构：顶部导航 / 中部内容 / 底部输入
- 大圆角（16-20px）卡片，充裕留白，通过间距而非分割线区分区块
- 内容区最大宽度受限（约 720-800px），保护阅读行长

**clarity-egui 现状**：
- 面板架构完整（sidebar / chat / task panel / toolbar），但 **Sidebar 没有会话列表**
- 用户只能通过 chat header 的 category tabs（emotion/knowledge/engineering）切换会话
- **emotion category 特殊处理**（`chat.rs:96-104`）：不显示 tabs，显示静态 `情感` 标题，破坏导航一致性
- Task Panel 和 Toolbar **强制互斥**（`toolbar.rs:7-10`），打开一个关闭另一个，无视觉解释
- File tree 高度：`available_height - 260.0`（`sidebar.rs:87`）— magic number，footer 内容变化即断裂
- **无最大内容宽度限制**：CentralPanel 的聊天消息撑满全宽，在宽屏显示器上行长可达 120+ 字符，阅读疲劳严重

**不合理之处**：
1. **Sidebar 的"空心化"**：240px 的 sidebar 承担了导航、文件树、预览、footer，却没有最核心的"历史会话"功能。用户无法浏览、搜索、快速跳转过去对话
2. **emotion 的特权**：同为 category，emotion 的 UI 模式完全不同，用户需要记忆例外规则
3. **宽屏阅读灾难**：Swiss Style 的核心是限制行长（66-72 字符最优），当前 layout 完全放任

---

### 2.4 组件一致性 — 4/10

**Kimi 基准**：
- 幽灵按钮为主，无边框或细边框
- 输入框圆角，内部放置功能图标暗示多模态
- 技能卡片：无分隔线，纯靠 padding 区隔，极简到"裸列表"

**clarity-egui 现状**：
- 有 `bubble_frame()` / `card_frame()` 辅助函数（`theme.rs:366-386`），气泡不对称圆角有细节
- **Settings provider cards 使用 raw painter API**（`settings.rs:98-150`）：无 hover 光标、无键盘 focus、无 accessibility，点击目标是隐形矩形
- **硬编码 emoji 泛滥**：🛠 📎 ⏳ ✅ ❌ ▶ ■ — 无 fallback，在缺少 emoji 字体的系统上显示为方框
- **Stop 按钮用 "■"，Send 用 "▶"**（`chat.rs:691`, `chat.rs:707`）：媒体播放图标用于聊天动作，语义错位；无文本标签，新用户无法识别
- 代码块和工具调用未统一为卡片容器：代码块是简单 `Frame::none().fill()`，工具调用是另一种渲染路径（`ui/render.rs:90-125`）

**不合理之处**：
1. **raw painter 的反模式**：在 immediate mode GUI 中绕过 widget 体系直接 painter.draw_rect，丧失了 egui 的交互反馈、焦点管理、tooltip 等全部能力
2. **emoji 作为唯一语义载体**：可访问性灾难。屏幕阅读器无法朗读 emoji，色盲用户难以区分 ✅/❌
3. **图标语义污染**：▶ 在用户的认知中是"播放"，不是"发送"；■ 是"停止播放"，不是"中断生成"。应使用 "Send"/"Stop" 文本或更明确的图标（如纸飞机/方块）

---

### 2.5 交互与反馈 — 4/10

**Kimi 基准**：
- Bottom Sheet 弹窗从底部滑入，背景 Scrim 暗化
- 模式切换显性化（快速/思考/Agent/Agent集群）
- 叙事层作为环境氛围，不干扰功能但建立情感连接

**clarity-egui 现状**：
- Toast 有淡入动画（`toast.rs:12`，cubic ease-out，0.18s）— 良好
- Approval modal 有键盘快捷键（Enter / Shift+Enter / Esc）— 良好
- **Approval modal 在 render 路径调用 `block_on`**（`approval.rs:49`, `162`, `178`, `194`）：同步阻塞 UI 线程，帧率骤降，体验灾难
- **IME 启发式 300ms**（`chat.rs:632-634`）：Enter 发送 vs IME 合成判断依赖时间阈值，对 Rime、搜狗等慢速输入法必然误触发
- **`stick_to_bottom(true)` 无逃逸机制**（`chat.rs:269-310`）：用户阅读历史时，新消息流式到达会强制拉回底部，打断阅读流
- **Subagent batches 30s 自动消失**（`subagent_progress.rs:149`）：用户无法回顾已完成/失败的历史批处理
- **文件预览 2000 字符硬截断**（`sidebar.rs:140-148`）：无"展开"按钮，用户永远看不到完整内容

**不合理之处**：
1. **render 路径的同步 I/O**：`block_on` 在 `update()` 循环中是绝对禁忌。approval 的 resolve 应通过 `ui_tx` 异步通道下沉到后台
2. **IME 的种族歧视**：300ms 阈值本质上是基于拉丁语系输入速度的假设，对 CJK 输入法不公平且不可靠。应使用 egui 的 `output.events` 检测 `IMECommit` 事件
3. **阅读劫持**：stick_to_bottom 应在用户手动向上滚动后自动释放，直到用户重新滚到底部

---

### 2.6 信息层级 — 5/10

**Kimi 基准**：
- **混合气泡策略**：用户消息有气泡无头像，AI 消息无气泡有头像，代码/工具有气泡
- 技能外露化为可管理卡片（bilibili-search, deep-research 等）
- 叙事层（格雷的文本）作为环境氛围

**clarity-egui 现状**：
- 用户消息有气泡（暖铜色），AI 消息有气泡（深灰）— 但 **AI 消息也有气泡，不是无气泡直排**
- AI 侧无头像/人格锚点 — 聊天界面缺少"是谁在说话"的识别符
- 技能隐藏在 Skills 面板（`panels/skill.rs`），不是外露的快捷能力列表
- 无叙事层 / 环境氛围 / Companion UI 元素
- 空状态仅在零消息时显示"Configure Settings"（`chat.rs:234-267`）：如果用户有历史但当前模型未配置，没有警告

**不合理之处**：
1. **AI 气泡的冗余**：AI 回复本质上是"文档/知识"，用气泡包裹增加了视觉噪音。Kimi 的选择是 AI 直排 + 头像锚定，将气泡留给"需要容器隔离的对象"（代码、工具调用）
2. **技能的黑箱化**：clarity 的技能系统丰富，但用户必须主动打开 Skills 面板才能看到。Kimi 将技能平铺为应用列表，降低发现成本
3. **人格缺位**：clarity 的 AI 没有面孔/头像/名称，用户面对的是匿名深灰气泡，难以建立情感连接

---

### 2.7 品牌气质 — 6/10

**Kimi 基准**：
- "以深色终端为底色、以斜杠命令为交互语法、以技能卡片为能力单元、以拟人叙事为情感润滑剂的 Agent 操作系统界面"

**clarity-egui 现状**：
- frameless 窗口 + 自定义 titlebar 有辨识度
- 暖铜色强调色有温度感，区别于千篇一律的科技蓝
- 但整体上更像"一个功能完整的聊天客户端"，而非"Agent 原生操作系统"
- 缺少：命令面板（Command Palette）、技能启动器、Agent 状态仪表盘、叙事层

**不合理之处**：
1. **定位模糊**：clarity 的 backend 是强大的 Agent 运行时（plan、subagent、MCP、background tasks），但 frontend 仍停留在"消息气泡 + 输入框"的聊天范式。前端未能放大后端的能力密度
2. **缺少仪式感**：Agent 开始执行 plan、subagent 并行批次启动、background task 完成 — 这些事件值得有庆祝/通知/状态转移的视觉仪式，而不仅仅是状态点变色

---

## 三、改进路线图

### Phase 1 — 色彩与排版基础设施（低风险，1-2 天）
- [ ] 引入 `bg_deep: #0A0A0E` 作为 OLED Black 选项，当前 `#12141e` 降级为 `bg_standard`
- [ ] Tokenize 字号系统：`text-xs`(10) / `text-sm`(12) / `text-base`(14) / `text-lg`(16) / `text-xl`(20) / `text-2xl`(26)
- [ ] 删除 9px 超小字，最小字号提升至 10px
- [ ] 在 `FontDefinitions` 中注册 Bold / Italic / BoldItalic 字体文件，替换 `RichText::strong()` 的空壳调用
- [ ] CJK 字体路径改为运行时探测（font-kit / system-fonts），移除硬编码 Windows 路径

### Phase 2 — 布局重构（中风险，2-3 天）
- [ ] Sidebar 新增 Session List 区域：按 category 分组的历史会话，支持搜索/滚动
- [ ] 统一 emotion category 的导航模式：与其他 category 一样显示 tabs，移除特殊处理
- [ ] CentralPanel 引入 `max_width: 720.0` 的内容约束（Swiss Style 阅读行长保护）
- [ ] Task Panel 与 Toolbar 改为可共存布局（stacked 或 tabbed），移除强制互斥
- [ ] 用 `ui.available_height_before_wrap()` 替代 `available_height - 260.0` magic number

### Phase 3 — 组件体系（中风险，2-3 天）
- [ ] Settings provider cards 改为标准 `Button` / `Selectable` widget，恢复 hover/focus/tooltip
- [ ] 实现混合气泡策略：
  - 用户消息：暖铜气泡，无头像，右对齐
  - AI 文本：无气泡直排，左侧圆形头像（Clarity 品牌图标），左对齐
  - AI 代码/工具调用：深色卡片气泡（`#1C1C1E`），与文本形成材质对比
- [ ] 统一代码块容器：`Frame::none().fill(code_bg).rounding(12.0).inner_margin(16.0)`
- [ ] 引入幽灵按钮体系：无边框/细边框，hover 时轻微提亮
- [ ] 用 `egui::FontId` + `TextStyle` 替换所有硬编码 emoji，或使用 `egui-phosphor` / `egui-material-icons` 等图标字体

### Phase 4 — 交互优化（高风险，3-5 天）
- [ ] Approval modal：移除 `block_on`，改为通过 `ui_tx.send(UiEvent::ResolveApproval { ... })` 异步通道
- [ ] IME 安全：检测 `ui.output().events` 中的 `IMECommit`，替换 300ms 时间阈值
- [ ] `stick_to_bottom` 智能释放：用户向上滚动 > 100px 后自动释放，新消息到达时显示"跳至底部"浮动按钮
- [ ] Subagent batches：移除 30s 自动消失，改为手动 dismiss 或显示最近 5 条历史
- [ ] 文件预览：2000 字符截断处添加 "Show More" 按钮，展开完整内容
- [ ] Status dot 可交互：点击后展开当前 Agent 状态详情（正在调用的工具、plan 进度）

### Phase 5 — 品牌气质提升（高风险，长期）
- [ ] 顶部叙事层：在 chat 区域顶部添加可折叠的 Agent 状态叙事（如"正在分析项目结构..."）
- [ ] 技能外露化：Sidebar 底部或 Chat 输入框上方添加快捷技能栏（类似 Kimi 的 `/new`, `/compact`, `/status` 命令前缀）
- [ ] Agent 仪表盘：独立面板或 overlay，实时展示 plan 执行图、subagent 并行状态、background task 队列
- [ ] 欢迎/空状态重新设计：从"Configure Settings"按钮升级为"Agent 启动器"界面，展示可用技能和快捷操作

---

## 四、结论

clarity-egui 当前处于"功能完备但设计粗糙"的状态。后端能力（plan、subagent、MCP、background tasks）非常丰富，但前端仍停留在传统聊天客户端范式，未能将 Agent-Native 的理念视觉化。

与 Kimi 的对比中，最大的差距不在于某个具体颜色或圆角大小，而在于**设计意图的清晰度**：Kimi 的每个设计选择（混合气泡、技能外露、叙事层）都服务于"Agent 操作系统"这一定位；而 clarity-egui 的许多选择（raw painter API、emoji 语义、stick_to_bottom 劫持）更像是工程便利性的副产品。

**建议优先实施 Phase 1 + Phase 4 的关键修复**（tokenize 字号、修复 block_on、IME、stick_to_bottom），这些改动风险低、用户体验提升显著，且为后续视觉重构奠定基础设施。
