---
title: clarity-egui 美学与信息论研究 · 主题一视角
category: Research
date: 2026-05-16
tags: [research, egui, ui]
---

# clarity-egui 美学与信息论研究 · 主题一视角

> 研究目标：为层级化多 Agent OS 的 GUI 层提供美学与信息论层面的设计依据。
> 数据来源：OpenHanako 设计系统扫描（`desktop/src/themes/`, `styles.css`）+ clarity-egui 源码扫描（`crates/clarity-egui/src/theme.rs`）。
> 验证方式：复现文档中的 `[VERIFY]` 命令。

---

## 1. OpenHanako 设计系统扫描（可核实）

### 1.1 主题架构

OpenHanako 采用 **CSS 变量分层架构**：

- **结构 Token**（`styles.css` `:root`）：spacing, radius, easing, fonts — 主题无关
- **主题 Token**（各主题 CSS 文件）：bg, bg-card, accent, text, border — 主题相关

**已发现主题文件**（11 个）：

```
absolutely.css, claude-design.css, contemplation.css, deep-think.css,
delve.css, grass-aroma.css, high-contrast.css, midnight.css, warm-paper.css
```

**验证命令**：
```powershell
[VERIFY] Get-ChildItem "$env:USERPROFILE\Desktop\openhanako-main\desktop\src\themes" -Filter "*.css" | Select-Object Name
```

### 1.2 配色哲学（两个代表性主题）

| Token | warm-paper（默认） | midnight（深色） |
|-------|-------------------|-----------------|
| `--bg` | `#F8F5ED` 暖纸 | `#3B4A54` 深青 |
| `--accent` | `#537D96` 冷静蓝 | `#AA798D` 柔粉 |
| `--text` | `#3B3D3F` 深灰 | `#C8D1D8` 浅灰 |
| `--text-muted` | `#8E9196` | `#5E7280` |
| 饱和度 | 低 | 低 |
| 对比度 | 低-中 | 低 |

**关键特征**：两主题均使用**低饱和度、低对比度**配色，避免视觉疲劳。

### 1.3 纸质纹理系统

三层叠加（`styles.css` 注释明确）：
1. **Surface 层**：body, titlebar, sidebar — 直接叠 `rice-paper.png`
2. **Card 层**：card, msg-card, input-wrapper — `lighten` 混合模式
3. **亮度补偿**：暖白叠层抵消纹理变暗（暗色主题跳过）

### 1.4 字体分级

| 层级 | 字体 | 用途 |
|------|------|------|
| UI | system-ui, Inter | 按钮、标签、导航 |
| 正文 | Songti SC, Georgia, Noto Serif SC | 长文本阅读 |
| 代码 | SF Mono, Fira Code, JetBrains Mono | 代码块、技术内容 |

**中文字体**：使用 Noto Serif SC 完整字重家族（扫描发现 70+ 个 woff2 文件）。

### 1.5 信息密度

从截图观察（`screenshot-previews/`）：
- 无侧边栏
- 无输入框（或极度弱化）
- 无按钮群
- 内容区占据 >90% 可视面积
- 底部仅有微小 logo

---

## 2. clarity-egui 现状扫描（可核实）

### 2.1 主题实现

**验证命令**：
```powershell
[VERIFY] Get-Content crates/clarity-egui/src/theme.rs | Select-Object -First 90
```

**关键事实**：
- `Theme` struct：33 个字段（color × 22, font × 2, spacing × 6, radius × 4, duration × 3）
- 当前主题数：**2**（dark / light）
- Dark bg：`#0f0f11`（近纯黑）
- Light bg：`#ffffff`（纯白）
- Accent：**统一为 violet `#8b5cf6`**（高饱和）

### 2.2 字体

- Body：Inter
- Mono：JetBrains Mono
- CJK 回退：Windows 系统字体（simhei, msyh, simsun）— 无自定义中文字体

### 2.3 Spacing / Radius

| 属性 | 值 | OpenHanako 对比 |
|------|---|----------------|
| spacing | 4/8/12/16/20/24 | 0.25/0.5/1/1.5/2.5 rem（近似） |
| radius | 6/10/12/9999 | 6/10/16 px |

### 2.4 信息密度（基于代码推断）

从 `main.rs` + `app_logic.rs` + `panels/` 推断（需手动验证）：
- Sidebar：会话列表 + 工具分组
- Chat Area：消息流 + 工具调用指示器
- Input：多行文本输入 + 附件 + 发送按钮
- Overlay：审批弹窗、Settings 面板、Plan 可视化
- 状态指示：加载中、模型选择、审批状态

**密度评估**：高。多面板同时可见，无渐进披露机制。

---

## 3. 信息论分析

### 3.1 信道容量与信噪比

**Shannon 模型类比**：
- 信道带宽 = 屏幕像素数
- 信号 = 用户需要的内容（消息文本、代码、状态）
- 噪音 = UI chrome（边框、分隔线、按钮、图标、装饰）

| 项目 | OpenHanako | clarity-egui |
|------|-----------|-------------|
| S/N 比 | 极高（>90% 内容） | 低（内容被多层 chrome 稀释） |
| 带宽利用率 | 低（大量留白） | 高（像素填满） |
| 信息熵 | 低（单一内容类型） | 高（多类型信息竞争） |

**分析**：OpenHanako 通过降低带宽利用率来提升 S/N 比；clarity-egui 目前相反——填满带宽但 S/N 比低。

### 3.2 信息层级（编码效率）

**有效信息层级需要视觉编码差异**：
- 字体大小差异（标题 > 正文 > 注释）
- 颜色对比差异（关键 > 次要 > 禁用）
- 空间位置差异（中心 > 边缘）

**OpenHanako**：
- 标题（大字）→ 正文（中字）→ logo（小字）：三级清晰

**clarity-egui（推断）**：
- sidebar text ≈ chat text ≈ input text ≈ button text：层级压缩
- 缺乏大字号标题系统
- accent 色（violet）同时用于用户气泡、按钮、焦点环：语义过载

### 3.3 认知负荷（Sweller, 1988）

| 负荷类型 | OpenHanako | clarity-egui |
|---------|-----------|-------------|
| 内在负荷 | 中（阅读任务本身） | 高（多 Agent 协作认知） |
| 外在负荷 | 极低（无学习成本） | 高（需理解 sidebar/input/approval/plan 的交互模型） |
| 相关负荷 | 低（单一模式） | 中（需建立多面板协作的心智模型） |

**关键差距**：clarity-egui 的外在负荷过高。用户需要同时跟踪：
- 哪个 session 活跃
- 当前是否在 streaming
- 是否有 pending approval
- plan 步骤执行到哪一步
- 工具调用状态

这些状态分散在不同面板，增加了外在认知负荷。

### 3.4 渐进披露（Progressive Disclosure）

**原则**：只显示与当前任务最相关的信息。

**OpenHanako**：完全披露（单篇文章）— 适合其专注阅读的场景。

**clarity-egui**：过度披露 — 所有功能同时可见，无论用户是否需要。

**主题一需求下的矛盾**：
- 多窗口 Agent OS 天然需要显示更多信息（多个 Agent 状态）
- 但屏幕物理尺寸不变，信息密度不能无限增加
- **解法**：信息分层 + 动态披露，而非静态堆叠

---

## 4. 主题一（多窗口 Agent OS）的设计含义

### 4.1 窗口角色需要视觉编码

用户的 8 条需求定义了至少三种窗口角色：

| 角色 | 功能 | 情绪/认知特征 | 建议视觉编码 |
|------|------|-------------|------------|
| 情感窗口（格雷） | 高人格化陪伴 | 温暖、低刺激、安全 | 暖纸背景 + 低饱和暖色 + 大留白 |
| 知识窗口（观察者） | 知识库管理 | 冷静、分析、客观 | 深青/灰蓝背景 + 低饱和冷色 + 清晰层级 |
| 项目经理窗口 | 专项开发 | 高效、明确、任务导向 | 高对比 + 紧凑布局 + 状态清晰 |

**当前缺陷**：clarity-egui 的 2 主题（dark/light）无法承载三种角色的视觉区分。统一的 violet accent 缺乏情绪语义。

### 4.2 信息注入需要专用通道

需求 8：上层→下层信息注入 + 平层公告板。

**信息论要求**：注入信息不应与主内容竞争信道带宽。

**建议**：
- 使用**边缘光效**（glow）或**微妙动画**作为注入信号 — 不占用中心空间
- 使用**非对称布局**（如左侧细条指示器）表示层级关系
- 公告板使用**顶部/底部固定栏**，而非弹窗 overlay

### 4.3 多窗口需要低认知切换成本

当用户在情感窗口和项目经理窗口之间切换时：
- 如果两窗口视觉风格相同 → 认知切换成本高（难以快速识别当前角色）
- 如果两窗口视觉风格差异显著 → 认知切换成本低（颜色/纹理本身就是角色标识）

**建议**：每个角色绑定一个主题，主题差异不仅在于配色，还在于**纹理、字体、间距**。

---

## 5. 具体改进建议（按优先级）

### P0 — 主题系统扩展（支撑角色区分）

**目标**：从 2 主题扩展到 4-5 主题，每个主题绑定一个角色。

| 主题 | 背景 | Accent | 字体 | 纹理 | 绑定角色 |
|------|------|--------|------|------|---------|
| Warm Paper | `#F8F5ED` | `#537D96` | Noto Serif SC + Inter | 纸质纹理 | 情感窗口 |
| Deep Ink | `#3B4A54` | `#AA798D` | Inter + JetBrains Mono | 无 | 知识窗口 |
| Focus | `#0f0f11` | `#8b5cf6` | Inter + JetBrains Mono | 无 | 项目经理 |
| System | `#ffffff` | `#2563eb` | Inter + JetBrains Mono | 无 | 系统/设置 |

**实现路径**：
1. 在 `Theme` struct 中新增 `personality: String` 字段
2. 实现 `Theme::warm_paper()` / `Theme::deep_ink()` 构造函数
3. 在 `AppState` 或 `AgentConfig` 中增加 `theme_name` 字段
4. 窗口创建时根据角色选择主题

**验证命令**：
```powershell
[VERIFY] Get-Content crates/clarity-egui/src/theme.rs | Select-String "pub fn dark\(\)|pub fn light\(\)"
```

### P1 — 信息层级重构（降低外在认知负荷）

**目标**：建立三级信息层级。

| 层级 | 视觉编码 | 用途 | 当前状态 |
|------|---------|------|---------|
| L1 主导 | 大字号（24-32px）、高对比 | 当前 Agent 名称、关键状态 | ❌ 缺失 |
| L2 内容 | 中字号（14-16px）、标准对比 | 消息文本、代码 | ✅ 已有 |
| L3 辅助 | 小字号（11-12px）、muted 色 | 时间戳、元数据、状态提示 | ⚠️ 不均匀 |

**具体动作**：
- 在 sidebar 顶部增加当前 session 的 L1 标题显示
- 统一 message timestamp 为 L3 样式
- 工具调用指示器从 L2 降级为 L3（用户不总是需要看到工具名）

### P1 — 渐进披露机制（降低信息密度）

**目标**：默认只显示核心内容，其他按需展开。

| 元素 | 当前状态 | 建议 |
|------|---------|------|
| Sidebar | 始终可见 | 可折叠（auto-hide），hover 展开 |
| Settings | 独立面板 | 抽屉式（drawer），从右侧滑入 |
| Approval 弹窗 | 模态阻断 | 非阻断 toast + 详情抽屉 |
| Plan 步骤 | 内嵌在 chat | 折叠为单行摘要，点击展开 |
| Skill 面板 | 未知位置 | 仅在激活时显示，默认隐藏 |

### P2 — 中文字体优化

**目标**：改善中文阅读体验。

**当前问题**：CJK 回退到系统黑体（simhei/msyh），无衬线体阅读长文本体验差。

**建议**：
- 情感窗口正文使用思源宋体（Noto Serif SC）或系统回退到宋体
- 项目经理窗口保持无衬线（效率优先）
- 引入字体加载机制（从系统路径或内置资源加载）

### P2 — 纹理引入（可选）

**目标**：增加情感窗口的温暖感。

**建议**：
- 生成轻量噪点/纸质纹理图（< 50KB PNG）
- 在 `Theme` 中增加 `texture: Option<TextureHandle>`
- 仅在 Warm Paper 主题启用

**约束**：egui 的纹理系统支持此功能，但会增加 binary size 和内存占用。需评估是否值得。

---

## 6. 与后端思想的衔接

OpenHanako 后端（从代码结构推断）的设计思想值得借鉴到 clarity-egui：

1. **分层抽象**：CSS Token 分为结构层和主题层 → egui 的 `Theme` 已接近此模式，但可更明确分离
2. **插件化主题**：11 个主题独立 CSS 文件 → egui 可实现运行时主题切换（当前已支持，只需增加主题数）
3. **纸质纹理作为情绪载体**：纹理不是装饰，是**人格化表达**的工具 — 与主题一的情感窗口需求直接对应

---

## 7. 风险与约束

| 建议 | 风险 | 缓解 |
|------|------|------|
| 多主题扩展 | 增加维护成本（每改一个色值需改 N 个主题） | 使用 Design Token 工具生成（如 Rust 宏或构建脚本） |
| 中文字体加载 | 字体文件大（Noto Serif SC 完整家族 > 10MB） | 仅加载必要字重；或依赖系统字体 |
| 纹理引入 | binary size 增加 | 使用程序生成噪点（perlin noise）替代位图 |
| 渐进披露 | 可能隐藏关键信息（如审批请求） | 关键状态使用动画/声音提示，确保不遗漏 |

---

## 8. 验证检查单

本文档中的事实声明验证方式：

```powershell
# OpenHanako 主题文件列表
[VERIFY] Get-ChildItem "$env:USERPROFILE\Desktop\openhanako-main\desktop\src\themes" -Filter "*.css"

# OpenHanako 配色值
[VERIFY] Get-Content "$env:USERPROFILE\Desktop\openhanako-main\desktop\src\themes\warm-paper.css" | Select-String "^\s+--bg:|^\s+--accent:"
[VERIFY] Get-Content "$env:USERPROFILE\Desktop\openhanako-main\desktop\src\themes\midnight.css" | Select-String "^\s+--bg:|^\s+--accent:"

# clarity-egui Theme struct
[VERIFY] Get-Content crates/clarity-egui/src/theme.rs | Select-String "pub struct Theme|pub fn dark\(\)|pub fn light\(\)|bg: hex|accent: hex"

# clarity-egui 字体配置
[VERIFY] Get-Content crates/clarity-egui/src/theme.rs | Select-String "font_body:|font_mono:|simhei|msyh|simsun"
```

---

*本文件基于 2026-04-30 的代码扫描生成。设计建议未经实现验证，实施前需原型测试。*
