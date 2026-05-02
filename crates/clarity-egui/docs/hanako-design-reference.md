# Hanako → Clarity egui 设计参考规范

> 本文档从 `openhanako-UI` 提取关键设计参数，供子代理优化 clarity-egui 时参考。
> **原则**：只参考视觉/交互设计，不参考技术实现（React/CSS → Rust/egui 不可直译）。

---

## 一、主题系统映射

### 1.1 Dark 主题（Midnight）— 优先参考

openhanako 的 `midnight.css` 是当前最成熟的 dark 主题，clarity 的 dark 主题应对齐此配色逻辑。

| 语义 | openhanako (midnight) | clarity 当前 | 建议调整 |
|------|----------------------|--------------|----------|
| 主背景 | `#3B4A54` 深青蓝 | `#050507` 极黑 | ✅ 已改为极黑，玻璃质感基底 |
| 卡片背景 | `#445560` | `rgba(35,35,48,0.80)` | ✅ 已改为半透明深蓝灰 |
| 侧边栏背景 | `#34424B` | `rgba(20,20,28,0.80)` | ✅ 与主背景形成微妙层级 |
| 主文字 | `#C8D1D8` 冷灰白 | `#E8EAEF` | ✅ 保持高可读性 |
| 次要文字 | `#8A9BA8` | `rgba(200,205,220,0.70)` | ✅ 半透明冷灰 |
| 弱化文字 | `#5E7280` | `rgba(200,205,220,0.55)` | ✅ 半透明冷灰 |
| 强调色 | `#AA798D` 柔粉 | `#5B8DEF` 冰蓝 | ✅ 已改为冰蓝，冷暖统一 |
| 强调 hover | `#B78FA0` | `#7AA8F7` | ✅ hover 态已定义 |
| 边框 | `rgba(170,121,141,0.16)` | `rgba(255,255,255,0.06)` | ✅ 纯白微透明，与玻璃协调 |
| 成功色 | `#8CC790` | `#4CAF50` | 降低饱和度，更柔和 |
| 危险色 | `#EAB2A0` 暖珊瑚 | `#FF5252` | 降低饱和度 |
| 用户气泡 | `rgba(170,121,141,0.10)` | `rgba(91,141,239,0.16)` | ✅ 强调色透明版 |
| AI 气泡 | 透明（无背景） | 透明 | ✅ 保持一致 |

### 1.2 配色核心原则

- **冷暖统一**：midnight 主题全冷调（青蓝底 + 柔粉 accent），无暖色干扰。clarity 当前的铜色 accent `#C98A5E` 与冷灰底 `#111318` 产生冲突。
- **层级通过明度而非色相区分**：背景 `#3B4A54` → 卡片 `#445560` → 侧边栏 `#34424B`，全在同一色相，仅明度差异 5-10%。
- **accent 克制使用**：仅用于交互元素（按钮、选中态、链接），不用于大面积背景。

### 1.3 圆角系统

| 级别 | openhanako | clarity 当前 | 建议 |
|------|-----------|--------------|------|
| sm | 2px | 6px | ✅ 小按钮/标签圆角 |
| md | 3px | 12px | ✅ 卡片/气泡圆角 |
| lg | 4px | 20px | ✅ 输入框/大面板圆角 |
| full | — | 999px | ✅ 仅 avatar/图标按钮使用 |

### 1.4 阴影系统

openhanako midnight 阴影：`rgba(0,0,0,0.36)`，用于 modal/floating panel。

clarity 当前：`shadow_card` 有定义但效果较弱。建议：
- 增加 modal 遮罩层的不透明度（当前 overlay 可能太透明）
- 卡片阴影使用 `rgba(0,0,0,0.2)` 而非彩色阴影

---

## 二、组件级参考

### 2.1 空状态（Welcome Screen）

**参考文件**：`openhanako-UI/src/react/components/WelcomeScreen.tsx`

**openhanako 特征**：
- 中央大头像（80-100px），圆形，带 fallback（根据 yuan 生成默认头像）
- 随机问候语（根据 agent 的 yuan 属性从多语言池中选取）
- Agent 选择芯片（水平排列的可切换 agent 列表，带头像）
- 文件夹选择器（下拉式历史记录 + 浏览按钮）
- 记忆开关（toggle 按钮，图标 + 文字）

**clarity 当前**：无空状态设计，首次打开直接显示输入框。

**egui 实现建议**：
- 使用 `ui.vertical_centered()` 居中布局
- 头像用 `Frame::group` + `Circle` painter 或加载图片
- 问候语用 `Label`，支持换行
- Agent 芯片用 `Button` + `Frame::group` 组合
- 文件夹选择器用 `ComboBox` 或自定义下拉
- **注意**：egui 无 CSS 动画，所有状态切换瞬时完成

### 2.2 用户消息气泡（User Message）

**参考文件**：`openhanako-UI/src/react/components/chat/UserMessage.tsx`

**openhanako 特征**：
- 右侧对齐，最大宽度 82%
- 圆角不对称：`border-radius: 12px 12px 4px 12px`（右下锐角）
- 用户头像（小圆形，24px）
- 附件芯片（图片预览 + 文件名）
- 消息操作栏（复制、删除、选择）— hover 时显示
- 选中态：边框高亮

**clarity 当前**：`ui/render.rs` 的 `user_bubble()`
- ✅ 右侧对齐、72% 宽度（响应式）、圆角 lg
- ✅ 文字颜色 text_strong
- ❌ 无头像、无附件展示、无操作栏

**egui 实现建议**：
- 头像：在气泡左侧或上方添加小圆形头像（28px）
- 附件：在气泡内添加附件芯片行（参考 `components/input/attachment_chips.rs`）
- 操作栏：egui 支持 `ui.interact()` 获取 hover 状态，hover 时显示复制/删除按钮

### 2.3 助手消息（Assistant Message）

**参考文件**：`openhanako-UI/src/react/components/chat/AssistantMessage.tsx`

**openhanako 特征**：
- 左侧对齐，头像 + 名称行
- Thinking 块（可折叠的推理过程）
- Tool 调用卡片（工具名 + 参数 + 结果）
- Subagent 卡片（子代理执行状态）
- Plugin 卡片（插件渲染）
- Mood 块（情绪/计划展示）
- 代码块带语法高亮 + 复制按钮

**clarity 当前**：`ui/render.rs` 的 `agent_message()`
- ✅ 头像（avatar.rs）+ 名称 header
- ✅ AB 混合：纯文本 = 无气泡 + 底边框；结构化 = 玻璃卡片
- ✅ 代码块带 Copy 按钮
- ❌ 无 thinking 块、无 tool 卡片折叠、无 subagent 卡片

### 2.4 输入区域（Input Area）

**参考文件**：`openhanako-UI/src/react/components/InputArea.tsx`

**openhanako 特征**：
- 底部固定，卡片式容器
- 附件条（水平排列的附件芯片，可删除）
- 引用卡片（quoted selection，可删除）
- 上下文行（技能徽章、模型选择器、思考级别）
- 控制栏（发送按钮、计划模式、斜杠命令）
- TipTap 富文本编辑器（支持 markdown、mention）
- 状态栏（token 预估、连接状态、模型信息）

**clarity 当前**：`panels/chat/input.rs` + `components/input/`
- ✅ 附件芯片（空状态时自动隐藏）
- ✅ 发送/停止/队列按钮
- ✅ 安全宽度计算（防溢出）
- ❌ 无引用卡片、无技能徽章、无模型选择器、无 token 预估
- ❌ 纯文本输入（无富文本）

---

## 三、不可映射的差异（技术约束）

### 3.1 CSS 动画 → egui 无动画

openhanako 有大量 CSS transition/animation：
- 按钮 hover 颜色渐变（0.15s ease）
- 消息气泡出现动画
- 滚动条显隐动画
- 树叶飘落 overlay（`LeavesOverlay.tsx`，视频纹理）

**egui 限制**：
- egui 是 immediate mode，无 CSS 动画系统
- 可通过 `ui.ctx().animate_value_with_time()` 实现简单数值动画
- 但每帧重新渲染，复杂动画消耗性能
- **建议**：放弃动画，用瞬时状态切换 + 颜色反馈替代

### 3.2 CSS 伪元素 → 无等价物

- `::before`/`::after` 装饰线 → 需用 `painter` 手动绘制
- `::placeholder` 样式 → egui `TextEdit::hint_text()` 有限定制

### 3.3 富文本编辑器 → 不支持

- openhanako 使用 TipTap（ProseMirror 内核）实现富文本输入
- egui 的 `TextEdit` 是纯文本，无 rich text 编辑能力
- **建议**： clarity 保持纯文本输入，通过 markdown 语法实现格式化

### 3.4 SVG 图标 → Phosphor Icon Font

- openhanako 内联大量 SVG icons（Feather icons 风格，1.5-2px stroke）
- clarity 已嵌入 Phosphor Regular TTF
- **映射关系**：子代理应将 openhanako 的 SVG 语义映射到 Phosphor 图标常量
  - 设置齿轮 → `ICON_SETTINGS`
  - 发送箭头 → `ICON_SEND`
  - 播放 → `ICON_PLAY`
  - 停止 → `ICON_STOP`
  - 文件夹 → 需新增 `ICON_FOLDER`
  - 复制 → 需新增 `ICON_COPY`
  - 剪贴板 → 需新增 `ICON_CLIPBOARD`

---

## 四、组件映射表

| openhanako 组件 | 文件路径 | clarity 对应 | 文件路径 | 参考级别 |
|----------------|---------|-------------|---------|----------|
| WelcomeScreen | `components/WelcomeScreen.tsx` | 无（待实现） | — | L2 视觉 |
| UserMessage | `components/chat/UserMessage.tsx` | `user_bubble` | `ui/render.rs` | L2 视觉 |
| AssistantMessage | `components/chat/AssistantMessage.tsx` | `agent_message` | `ui/render.rs` | L2 视觉 |
| ChatArea | `components/chat/ChatArea.tsx` | `render_chat_area` | `panels/chat/mod.rs` | L1 结构 |
| InputArea | `components/InputArea.tsx` | `render_input` | `panels/chat/input.rs` | L1+L2 |
| SessionList | `components/SessionList.tsx` | `render_header` (tabs) | `panels/chat/header.rs` | L1 结构 |
| Sidebar | `components/Sidebar.tsx` | `render_sidebar` | `panels/sidebar.rs` | L1 结构 |
| ToolsSection | `components/Toolbar.tsx` | `tools_section::render` | `components/tools_section.rs` | L2 视觉 |
| FilesPanel | `components/FileTree.tsx` | `render_task_panel` | `panels/task.rs` | L1+L2 |
| SettingsApp | `settings/SettingsApp.tsx` | `render_settings_panel` | `components/settings/mod.rs` | L1 结构 |
| ProviderTab | `settings/tabs/ProviderTab.tsx` | `provider_tab::render_provider` | `components/settings/provider_tab.rs` | L2 视觉 |
| StatusBar | `components/StatusBar.tsx` | 无（待实现） | — | L2 视觉 |

**参考级别定义**：
- **L1 结构**：参考组件拆分方式和职责边界（什么逻辑放哪里）
- **L2 视觉**：参考颜色、间距、圆角、布局比例（需转译为 egui 代码）
- **L3 交互**：参考动画、过渡、手势（egui 大多不可实现，谨慎参考）

---

## 五、任务模板（供子代理使用）

### 任务：优化 [组件名]

**背景**：
- clarity 使用 egui 0.31（Rust immediate mode GUI）
- 设计参考：openhanako（Electron + React），设计规范见本文档
- 技术约束：无 CSS 动画、无富文本编辑、无 SVG 内联（使用 Phosphor icon font）

**步骤**：
1. 读取本文档的「组件映射表」，确认 openhanako 参考文件
2. 读取 clarity 当前实现文件
3. 按 L1/L2 级别提取可应用的设计改进
4. 实施修改（仅修改 Rust 代码，不引入新依赖）
5. 运行 `cargo check -p clarity-egui` 验证编译
6. 运行 `cargo test --workspace --lib` 验证测试基线

**禁止**：
- 引入新的 crate 依赖（除非用户明确许可）
- 尝试用 egui 实现 CSS 动画
- 直接复制 React/JSX 代码结构到 Rust
