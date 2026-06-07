# Kimi Desktop v3.0.15 vs Clarity egui — 批判性视觉审计

> 审计日期: 2026-06-07
> 基于截图: 屏幕截图 2026-06-07 161809.png (主页), 161830.png (聊天页)
> 当前 egui commit: HEAD (9 个面板文件已 revert 到 origin)

---

## 一、整体印象对比

| 维度 | Kimi Desktop | Clarity egui (当前) | 差距等级 |
|------|-------------|---------------------|----------|
| 视觉密度 | 极度简洁，大量留白 | 信息过载，元素拥挤 | **P0** |
| 色彩质感 | 纯黑底 + 高饱和蓝 | 蓝灰底 + 冰蓝 accent | **P1** |
| 排版层级 | 大标题 + 清晰分区 | 全大写标题 + 混合层级 | **P0** |
| 输入区域 | 圆角卡片式 composer | TUI 命令行风格 | **P0** |
| 侧边栏 | 扁平图标列表 | 多层嵌套卡片 + 工具栏 | **P0** |
| 空状态 | 大 Logo + 居中输入框 | 小字 + 设置按钮 | **P1** |
| 标题栏 | 原生系统标题栏 | 自定义 36px + 3 区布局 | **P1** |

---

## 二、逐项批判

### 2.1 侧边栏 (`panels/sidebar.rs`) — 最严重问题

**Kimi 实际结构 (截图 161809):**
```
[Work] [Chat]          ← 顶部 pill toggle
新建会话 Ctrl+K        ← 新会话按钮
────────────────────
📊 PPT
📄 文档
🔍 深度研究
🌐 网站
📋 表格
⚙️ Agent 集群
💻 Kimi Code
🌉 Kimi WebBridge
🐾 Kimi Claw  Beta  1  ← Badge + 计数
────────────────────
🕐 历史会话
  测试
  这种作文怎么写
  翻译
  查看全部
────────────────────
[头像] 酒宿_juice  Allegretto  ← 底部用户区
```

**Clarity 当前结构:**
```
[◀] [⚙️] [📤] [📥] [🔌 3] [🧩] [EN] [123∑]  ← 8 个图标挤在 36px 高工具栏
────────────────────
ROLES                 ← 全大写英文
[卡片] Emotion    ● 3 sessions
[卡片] Knowledge  ● 0 sessions
[卡片] Engineering ● 1 session ...
────────────────────
LIVE
[可折叠] Web Tabs
[可折叠] Thinking Log
[可折叠] Subagents (2)
────────────────────
WORKSPACE
[可折叠] Tools
Teams (3)          ▶
Cron Jobs          ▶
────────────────────
ANALYTICS
Dashboard          ▶
Plan Timeline      ▶
```

**批判:**
1. **信息架构混乱**: Kimi 侧边栏是"功能入口 + 历史记录"两层；Clarity 是"ROLES/LIVE/WORKSPACE/ANALYTICS"四层嵌套 + 可折叠组，认知负荷 4 倍
2. **全大写标题**: "ROLES"/"LIVE" 等全大写英文在中文界面中极度突兀，Kimi 没有任何全大写文本
3. **卡片过度设计**: `sidebar_card` 56px 高，包含图标+标题+副标题+badge+状态点；Kimi 只是 32px 的 icon+text 行
4. **工具栏过载**: 顶部 8 个功能图标，其中 export/import/locale/token 使用率极低却常驻
5. **缺失核心元素**: 没有"新建会话"按钮（在 titlebar 的 [+] 里），没有用户头像/名称

**修复方向:**
- 扁平化为 icon+text 行列表，取消卡片和折叠组
- 取消 ALL CAPS，改用中文或首字母大写英文
- 顶部只保留"新建会话" + 折叠按钮
- 底部增加用户区域
- 将 Teams/Cron/Dashboard 等低频功能移出侧边栏（可收进 Command Palette 或菜单）

---

### 2.2 输入区域 (`panels/chat/input/tui_style.rs`) — 最严重问题

**Kimi 实际 (截图 161830):**
- 圆角矩形输入框，深色填充（`#1f1f1f` 左右）
- 占位符文字居中偏左
- 底部工具栏: [+] [Agent]    [发送按钮]
- 模型选择器在右下角: "K2.6 快速 ↓"
- 整体像一个"卡片"

**Clarity 当前:**
```
────────────────────────────────────────  ← 1px 水平线
📸 Snapshot #1
📎 file.rs
~/project (main) ❯ ◐ [输入文字...]
────────────────────────────────────────  ← 1px 水平线
Ctrl+Enter Send · Shift+↑ History
```

**批判:**
1. **TUI 风格与 GUI 产品定位冲突**: 水平分隔线 + shell 提示符(`❯`) + cwd 显示，这是终端模拟器的美学，不是消费级 AI 客户端
2. **附件/快照占用垂直空间**: 每个附件/快照独占一行，Kimi 将它们收在输入框内部或底部工具栏
3. **微提示文字过于密集**: "Ctrl+Enter Send · Shift+↑ History · /coder · !cmd" 一行 4 个指令，视觉噪音
4. **没有圆角**: 直角输入区与 Kimi 的 16px 圆角形成鲜明对比

**修复方向:**
- 改为圆角卡片式输入框（`radius: 16~20px`）
- 将 shell prompt 和微提示移除或收进 hover tooltip
- 附件以 chip 形式内嵌在输入框上方
- 底部增加 + / Agent / 发送按钮工具栏

---

### 2.3 空状态 (`panels/chat/message_list.rs`)

**Kimi 实际 (截图 161809):**
- 屏幕中央: 巨大的 "KIMI" 文字（估计 48-64px，字重 bold）
- 下方: 圆角输入框，占位符"输入'/'可快捷使用技能"
- 再下方: "Agent 精选案例" 标题 + 3 个卡片（带缩略图）

**Clarity 当前:**
```
          Clarity              ← 24px，theme.text_dim
    Local-first AI agent runtime  ← 12px，theme.text_dim
    [ Configure Settings ]     ← 按钮
```

**批判:**
1. **Logo 尺寸过小**: 24px 的 "Clarity" 在 1200x800 窗口中几乎看不见；Kimi 的 "KIMI" 占据视觉中心
2. **副标题无意义**: "Local-first AI agent runtime" 是技术描述，不是用户价值主张
3. **设置按钮突兀**: 空状态不应引导用户去设置，而应引导用户开始对话
4. **缺少快捷入口**: Kimi 的卡片提供了一键启动的 Agent 案例

**修复方向:**
- 放大品牌文字到 36-48px
- 改为居中圆角输入框作为视觉焦点
- 添加快捷启动建议（可用文字按钮代替卡片图）

---

### 2.4 标题栏 (`main.rs`)

**Kimi 实际:** 原生 Windows 标题栏（截图可见标准的最小化/最大化/关闭按钮）

**Clarity 当前:** 自定义 36px 标题栏，包含:
- LEFT: [☰] Clarity
- CENTER: Persona switcher + Session tabs + Model name
- RIGHT: [✕] [□] [─] [⚙️] [Online] [Gateway]

**批判:**
1. **自定义标题栏增加了 36px 的固定开销**，而内容区域 already 被 sidebar + workspace 挤压
2. **Session tabs 放在标题栏**导致标题栏高度无法压缩，且 tab 多了会溢出
3. **Persona switcher + Model indicator + Status capsules** 三项竞争有限空间
4. **Window controls 自定义**增加了跨平台风险（macOS/Linux 行为不同）

**修复方向:**
- 恢复原生标题栏（`.with_decorations(true)`）
- 将 session tabs 移到 sidebar 顶部或内容区顶部
- 将状态指示器移入 sidebar 底部或系统托盘

---

### 2.5 消息气泡 (`components/chat/conversation.rs`)

**Kimi 实际 (截图 161830):**
- 用户消息: 右对齐，深灰气泡（约 `#2a2a2a`），圆角约 12px，宽度约 70%
- AI 消息: 左对齐，无气泡（或极淡背景），32px 圆形头像（"K" 字母）
- **没有持久的 action bar**: 复制/重新生成等操作不常驻显示

**Clarity 当前:**
- 用户气泡: `theme.surface` 填充，12px 圆角，80% 宽度 — 较接近
- AI 气泡: 56px 头像（太大），"K" 字母 — 头像尺寸超标
- **持久 action bar**: 每条消息下方都有时间戳 + copy/edit/regenerate 按钮行，造成视觉重复

**修复方向:**
- 用户气泡背景改为更暗的色值（接近 `#2a2a2a`）
- AI 头像缩小到 32-36px
- Action bar 改为 hover 时显示（conversation.rs 中已部分实现 hover，但 message_list.rs 中强制显示）

---

### 2.6 主题系统 (`theme.rs`)

| Token | Kimi | Clarity 当前 | 建议 |
|-------|------|-------------|------|
| 背景色 | `#121212` | `#12121a` (偏蓝) | 改为 `#121212` |
| 强调色 | `#1a88ff` | `#5B8DEF` (偏紫) | 改为 `#1a88ff` |
| 用户气泡 | `#2a2a2a` | `rgba(45,80,160,0.32)` (半透明蓝) | 改为 `#2a2a2a` |
| AI 气泡 | 无/极淡 | `rgba(255,255,255,0.06)` | 可保持或调更淡 |
| 文字主色 | `#d6d6d6` | `#E8EAEF` | 接近，可不改 |
| 圆角大 | `16px` | `28px` (过大) | 消息气泡改为 `12px`，卡片 `16px` |

---

## 三、egui 的硬性限制（无法修复）

以下差距属于 egui 架构限制，**任何优化都无法弥补**:

| 能力 | Kimi (Electron+CSS) | egui | 结论 |
|------|---------------------|------|------|
| `backdrop-filter: blur()` | 全局玻璃态 | ❌ 不支持 | 无法实现毛玻璃侧边栏 |
| `box-shadow` 层级 | 多层柔和阴影 | 基础 Shadow 结构 | 阴影质感差距大 |
| `z-index` / 层叠 | 精确控制 | 按添加顺序 | 复杂弹层难以实现 |
| `position: sticky` | 滚动吸附 | ❌ 不支持 | 标题栏无法随滚动吸附 |
| `line-clamp` | 文本截断 | ❌ 需手动计算 | 多行截断不可靠 |
| 图片纹理 | 任意图片 | 需预加载 Texture | 卡片缩略图难以实现 |
| 字体抗锯齿 | 系统级子像素 | 灰度 | 文字锐利度差距 |

---

## 四、优化优先级

### P0 — 高影响 / 低风险（建议立即实施）
1. **主题调色板**: 背景 `#12121a` → `#121212`，accent `#5B8DEF` → `#1a88ff`
2. **侧边栏扁平化**: 移除 ALL CAPS 标题，将卡片改为 32px 行高，取消折叠组
3. **输入框圆角化**: TUI 风格 → 圆角卡片 composer
4. **空状态重做**: 放大品牌文字 + 居中输入框

### P1 — 高影响 / 中风险（需谨慎实施）
5. **移除自定义标题栏**: 恢复原生 decorations，将 tabs/status 移入内容区
6. **AI 头像缩小**: 56px → 32px
7. **Action bar 隐藏**: 默认隐藏，hover 显示

### P2 — 中影响 / 结构性改动（需设计）
8. **design_system.rs 接入**: 将现有面板逐步迁移到语义 API
9. **快捷启动卡片**: 空状态下的 Agent 案例入口
10. **底部用户区**: sidebar 底部添加头像 + 名称 + model 选择器

---

## 五、结论

当前 egui 实现的核心问题是**信息架构过载**和**视觉风格不统一**。侧边栏塞入了 15+ 个功能入口，标题栏承担了 3 个职责，输入区混合了 TUI 和 GUI 美学。

Kimi 的设计哲学是**"做减法"**: 侧边栏只有功能入口 + 历史记录，标题栏完全交给系统，输入区是一个纯粹的圆角卡片，空状态只引导用户开始对话。

如果目标是"一比一复刻 Kimi"，egui 可以做到 70% 的相似度（布局 + 颜色 + 圆角），但**玻璃态、阴影层次、图片卡片**这三项是硬性天花板。Tauri 路线是正确的长期选择。
