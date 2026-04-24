# Clarity 开发计划书

> 版本：v0.2.0-dev → v1.0.0 | 日期：2026-04-25 | 状态：Draft

---

## 1. 项目愿景

**Clarity = 开发者的 AI 标准运行时**

> 一个 opinionated、多入口的 AI 运行时引擎：plan → execute → monitor → remember。

不是聊天客户端，不是代码补全插件，而是**个人 AI 的标准基础设施**——像操作系统调度进程一样，Clarity 调度 LLM、工具、子代理和记忆。

### 1.1 目标用户

| 场景 | 用户 | 入口 |
|------|------|------|
| 日常编码 | 开发者 | Desktop GUI / TUI |
| 移动办公 | 开发者 | iOS/Android APP |
| 团队协作 | 技术团队 | Web + Server |
| 后台自动化 | 运维/脚本 | CLI + Daemon |

### 1.2 核心差异化

```
Clarity vs Kimi/DeepSeek APP：
  • 本地运行，数据不上云
  • 可接入任意 LLM（不绑定单一提供商）
  • 可编程（Skills + Tools + Plugin SDK）

Clarity vs OpenClaw：
  • Rust 原生性能（<10MB 二进制）
  • Plan Mode + 并行子代理
  • 更深的记忆系统（SQLite + BM25 + 四级编译）

Clarity vs cc-haha：
  • 完全自研代码，无版权风险
  • 更清晰的架构（6 crates 单向依赖）
  • UI 重新设计，不照搬
```

---

## 2. 设计哲学

### 2.1 工程理论指导

| 理论 | 领域 | 在 Clarity 中的应用 |
|------|------|---------------------|
| **单一职责原则 (SRP)** | 软件设计 | 6 个独立 crates，每个只做一件事 |
| **依赖倒置原则 (DIP)** | 架构 | `gateway → core` 单向依赖，core 不依赖任何前端 |
| **Sidecar Pattern** | 微服务 | Tauri 2 Desktop 中 Rust core 作为 sidecar 进程运行 |
| **CQRS** | 数据流 | Gateway 读操作走缓存，写操作直达 core |
| **Event Sourcing** | 状态管理 | `Wire` 事件总线记录所有 Agent 事件，支持回放 |
| **渐进披露 (Progressive Disclosure)** | UX | 高级功能默认隐藏，按场景逐步展开 |
| **Fitt's Law** | 交互设计 | 高频操作（发送、审批、中断）按钮大且靠近光标 |
| **Hick's Law** | 决策设计 | 审批选项不超过 3 个，减少认知负荷 |
| **即时反馈 (Immediate Feedback)** | 感知设计 | 流式响应、打字机效果、进度指示器 |
| **工作记忆限制 (7±2)** | 信息架构 | 每屏同时显示的消息/任务不超过 7 条 |

### 2.2 美学原则

**"工业风"设计美学**

> 参考：Zed Editor、Linear、Raycast

- **克制**：少即是多，无多余装饰
- **对比度**：高对比度暗色主题，减少眼部疲劳
- **间距**：大量留白，信息呼吸感
- **动效**：微妙、有目的（150ms 内完成），不花哨
- **字体**：等宽字体用于代码/日志，无衬线字体用于 UI

**色彩系统（暗色主题）**

```
Background  : #0A0A0F (深空黑)
Surface     : #14141B (面板底色)
Border      : #27272E (分隔线)
Primary     : #6366F1 (靛蓝，主操作)
Success     : #22C55E (绿，成功/通过)
Warning     : #F59E0B (琥珀，警告/待审)
Danger      : #EF4444 (红，错误/拒绝)
Text Primary: #E2E2E8
Text Secondary: #8A8A96
```

---

## 3. 系统架构

### 3.1 总体架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                         入口层 (Presentation)                        │
├──────────┬──────────┬──────────────┬────────────────────────────────┤
│  Mobile  │ Desktop  │    Web       │           CLI                  │
│  (APP)   │  (GUI)   │  (Browser)   │      (TUI)                     │
│          │          │              │                                │
│• Tauri 2 │• Tauri 2 │• Axum + WASM │• ratatui                       │
│• iOS/安卓│• 本地渲染│• 静态部署    │• 快捷键驱动                     │
│• 推送    │• Sidecar │• 无需安装    │• 最小依赖                       │
└─────┬────┴────┬─────┴──────┬───────┴──────────┬─────────────────────┘
      │         │            │                  │
      └─────────┴────────────┴──────────────────┘
                        │
          ┌─────────────┴─────────────┐
          │      API 层 (Gateway)     │
          │  REST / WebSocket / JSON-RPC
          └─────────────┬─────────────┘
                        │
          ┌─────────────┴─────────────┐
          │      核心层 (clarity-core) │
          │  • Agent (ReAct / Plan)    │
          │  • ToolRegistry            │
          │  • MemoryStore             │
          │  • Subagent / Teams        │
          │  • MCP Manager             │
          │  • Hook Registry           │
          │  • CompactionService       │
          └─────────────┬─────────────┘
                        │
          ┌─────────────┴─────────────┐
          │      持久层 (Storage)      │
          │  • SQLite (session/memory) │
          │  • FileSystem (skills/logs)│
          └───────────────────────────┘
```

### 3.2 Crate 职责

| Crate | 职责 | 依赖方向 |
|-------|------|----------|
| `clarity-core` | Agent 循环、工具执行、记忆管理、子代理编排 | 被所有其他 crate 依赖 |
| `clarity-memory` | BM25 搜索、向量检索、Chunking、编译管道 | 被 `clarity-core` 依赖 |
| `clarity-wire` | SPMC 事件总线，UI ↔ Agent 通信 | 被 `clarity-core` 和前端依赖 |
| `clarity-gateway` | Axum HTTP 服务器、Web UI、Session Store | 依赖 `clarity-core` |
| `clarity-tauri` | Tauri 2 Desktop + Mobile GUI | 依赖 `clarity-core` + `clarity-wire` |
| `clarity-tui` | ratatui 终端界面 | 依赖 `clarity-core` + `clarity-wire` |
| `clarity-claw` | 系统托盘后台监控 | 依赖 `clarity-core`（轻量） |

**关键约束**：`clarity-core` 无任何网络监听，不依赖任何前端 crate。

---

## 4. 功能路线图

### Phase 0：基础夯实（已完成 ✅）

| 功能 | 状态 | 验证标准 |
|------|------|----------|
| Agent ReAct 循环 | ✅ | 多轮对话 + 工具调用 |
| Plan Mode | ✅ | JSON 计划 + 批量执行 |
| 三层审批 | ✅ | Interactive / Yolo / Plan |
| MCP 三协议 | ✅ | stdio/HTTP/SSE |
| Memory 系统 | ✅ | SQLite + BM25 + 四级编译 |
| Background Tasks | ✅ | 持久化 + 实时监控 |
| Lazy Master | ✅ | LLM/Memory/Skill 延迟初始化 |

### Phase 1：GUI 奠基（4 周）

**目标**：`clarity-tauri` crate 可用，Desktop 最小可运行。

| 周 | 工作项 | 交付物 | 理论依据 |
|----|--------|--------|----------|
| W1 | `clarity-tauri` crate 初始化 | Cargo.toml + Tauri 2 依赖 + 构建脚本 | Sidecar Pattern |
| W1 | Tauri Command 桥接 | Rust 端暴露 agent/run, interrupt, settings | IPC 最小化原则 |
| W2 | 聊天面板骨架 | React 消息列表 + 输入框 + 流式渲染 | 渐进披露 |
| W2 | 会话侧边栏 | 历史列表 + 新建会话 | 信息架构 (IA) |
| W3 | 多标签框架 | 标签栏 + 标签页容器 + 拖拽排序 | Fitt's Law（标签靠近顶部） |
| W3 | 任务面板 | 后台任务列表 + 状态指示器 | 即时反馈 |
| W4 | 设置面板 | 模型选择、审批模式、主题切换 | 渐进披露 |
| W4 | 暗色主题系统 | CSS 变量 + 色彩系统 | 工业风美学 |

### Phase 2：核心补齐（4 周）

**目标**：功能追平 cc-haha 桌面端核心体验。

| 周 | 工作项 | 交付物 | 对标 |
|----|--------|--------|------|
| W5 | 审批系统增强 | AI 分类器 + 规则引擎 + 非阻塞 Toast | cc-haha 7 层审批 |
| W5 | 文件浏览器集成 | 工作目录树 + 文件预览 | cc-haha 文件操作 |
| W6 | LSP 支持 | `tower-lsp` + 语言服务器管理 | cc-haha LSPTool |
| W6 | WebBrowserTool | `headless_chrome` / `fantoccini` | cc-haha WebBrowser |
| W7 | 快捷键系统 | 全局快捷键 + Vim 键位引擎 | cc-haha Vim |
| W7 | 搜索增强 | 全局搜索（Command Palette 风格） | Raycast / Linear |
| W8 | 性能优化 | 虚拟滚动、懒加载、Bundle 分析 | 60fps 目标 |
| W8 | 桌面端打包 | macOS/Windows/Linux 自动构建 | CI/CD 集成 |

### Phase 3：Mobile 适配（3 周）

**目标**：iOS/Android APP 可运行。

| 周 | 工作项 | 交付物 |
|----|--------|--------|
| W9 | iOS 构建链 | Xcode 项目 + 签名 + TestFlight |
| W9 | Android 构建链 | Gradle + APK/AAB |
| W10 | 移动端 UI 适配 | 底部导航 + 手势 + 软键盘处理 |
| W10 | 推送通知 | APNs (iOS) + FCM (Android) |
| W11 | 生物识别审批 | Face ID / 指纹替代手动确认 |
| W11 | Mobile 测试 | 真机测试 + 性能基准 |

### Phase 4：生态扩展（6 周）

**目标**：超越 cc-haha 现有能力。

| 周 | 工作项 | 交付物 | 理论依据 |
|----|--------|--------|----------|
| W12 | Bridge 远程控制 | 跨设备 Agent 调度协议 | 分布式系统 |
| W12 | Vector Search | `sqlite-vec` 语义检索 | 信息检索理论 |
| W13 | Sandbox | `landlock` (Linux) + Windows API | 最小权限原则 |
| W13 | Plugin SDK | Rust dylib / WASM 插件系统 | 开闭原则 |
| W14 | Voice 集成 | 语音识别/合成 | 多模态交互 |
| W14 | Canvas 支持 | 可视化工作区 | 空间记忆理论 |
| W15-17 | 稳定性 + 文档 | E2E 测试、用户文档、API 文档 | — |

---

## 5. UI/UX 设计规范

### 5.1 布局系统

**Desktop 默认布局（三栏）**

```
┌─────────────────────────────────────────────────────────┐
│  [≡]  Clarity    [🔍 Search...]          [⚙] [👤]     │  ← 顶部栏 (48px)
├──────────┬──────────────────────────────┬───────────────┤
│          │                              │               │
│  Sidebar │        Chat Area              │  Right Panel  │
│  (240px) │        (flex: 1)              │  (320px)      │
│          │                              │               │
│  📁 Sessions          User                    Tasks      │
│  ────────             ┌──────────┐         ─────        │
│  💬 Current           │ Agent    │         ⏳ Running   │
│  💬 Yesterday         │ response │         ✅ Done      │
│  💬 Project A         │          │         ❌ Failed    │
│                       └──────────┘                       │
│  [+ New Session]      ┌──────────┐         [Details →]  │
│                       │ Input... │                      │
│                       └──────────┘                      │
├──────────┴──────────────────────────────┴───────────────┤
│  Status: Idle | Tokens: 1,240 / 8,192 | Model: kimi-code │ ← 状态栏 (24px)
└─────────────────────────────────────────────────────────┘
```

**Mobile 默认布局（单栏 + 底部导航）**

```
┌─────────────────────────┐
│  [←] Chat Name    [⋮]  │
├─────────────────────────┤
│                         │
│      Chat Area          │
│                         │
│  ┌──────────┐          │
│  │ Agent    │          │
│  │ response │          │
│  └──────────┘          │
│                         │
├─────────────────────────┤
│  [🎤] [Input...] [↑]   │
├──────────┬──────────────┤
│  💬 Chat  │  📋 Tasks   │
│  (active) │             │
└──────────┴──────────────┘
```

### 5.2 交互规范

| 交互 | 行为 | 理论依据 |
|------|------|----------|
| **发送消息** | Enter 发送，Shift+Enter 换行 | 即时反馈 |
| **中断 Agent** | Esc 或 Ctrl+C，0ms 延迟 | 控制感 |
| **审批弹窗** | 非阻塞 Toast（底部滑入），3 秒后自动最小化 | 渐进披露 |
| **流式响应** | 逐字渲染，50ms/字，带光标闪烁 | 即时反馈 |
| **多标签切换** | Ctrl+Tab / 手势滑动，150ms 过渡动画 | Fitt's Law |
| **全局搜索** | Cmd+K / Ctrl+K，Command Palette | Hick's Law（减少选项） |
| **文件拖拽** | 拖拽到输入区自动读取内容 | 直接操作 |

### 5.3 响应式设计断点

| 断点 | 宽度 | 布局 |
|------|------|------|
| Mobile | < 768px | 单栏，底部导航 |
| Tablet | 768-1024px | 双栏（Sidebar + Chat） |
| Desktop | 1024-1440px | 三栏（Sidebar + Chat + Panel） |
| Ultra-wide | > 1440px | 三栏，Chat 区最大宽度限制 960px |

---

## 6. 质量标准

### 6.1 代码质量

```
cargo test --workspace --lib      # 必须 474+ passed
cargo clippy --workspace --lib --bins --tests -- -D warnings  # 零 warning
cargo fmt --all -- --check        # 格式检查通过
cargo audit                       # 无高危漏洞
```

### 6.2 性能基准

| 指标 | 目标 | 测试方法 |
|------|------|----------|
| 冷启动时间 | Desktop < 500ms, Mobile < 1s | Criterion |
| 内存占用 | Desktop < 150MB, Mobile < 100MB | `memory_stats` |
| 消息渲染延迟 | 首字 < 100ms | 人工测试 |
| 流式帧率 | 60fps | Chrome DevTools |
| Bundle 大小 | Desktop < 15MB, Mobile < 20MB | `cargo bundle` |

### 6.3 可用性标准

- **无障碍**：支持键盘导航、屏幕阅读器标签、高对比度模式
- **国际化**：预留 i18n 框架（英语 + 中文优先）
- **离线能力**：核心功能无需网络（本地模型可选）

---

## 7. 风险管理

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| Tauri 2 Mobile 稳定性 | 中 | 高 | 先 Desktop 验证，Mobile 用 TestFlight 灰度 |
| 前端技能缺口 | 低 | 中 | React 学习曲线平缓，必要时外包 UI 设计 |
| 性能不达标 | 低 | 高 | 每 Phase 结束做性能基准，虚拟滚动兜底 |
| 竞品功能追赶 | 高 | 中 | 聚焦差异化（Plan Mode + Memory 深度），不追全部 |
| App Store 审核 | 中 | 中 | 避免敏感 API，预留审核缓冲时间 |

---

## 8. 里程碑

```
2026-04 ── v0.2.0-dev（当前）基础功能完备
    │
2026-05 ── v0.3.0-alpha  Desktop GUI 最小可运行
    │
2026-06 ── v0.4.0-beta   Desktop GUI 功能完整 + LSP
    │
2026-07 ── v0.5.0-beta   Mobile iOS/Android 适配
    │
2026-08 ── v0.6.0-rc     Sandbox + Plugin SDK
    │
2026-09 ── v0.7.0-rc     Bridge + Voice + Canvas
    │
2026-10 ── v1.0.0        稳定版发布
```

---

## 9. 附录

### 9.1 参考项目（功能设计层面）

| 项目 | 可借鉴点 | 边界 |
|------|----------|------|
| **Zed Editor** | 极简 UI、高性能渲染、多标签 | 仅设计思想 |
| **Linear** | 信息密度、暗色主题、动效 | 仅设计思想 |
| **Raycast** | Command Palette、插件系统 | 仅设计思想 |
| **cc-haha** | 功能清单、Tauri 2 架构思路 | **绝不看代码** |
| **codex-rs** | Rust Agent 实现、Sandbox | 开源，可参考代码 |

### 9.2 设计理论参考

- **Don Norman《设计心理学》** —  affordance、反馈、映射
- **Jakob Nielsen 可用性启发式** — 10 条可用性原则
- **Dieter Rams 设计十诫** — 少即是多
- **Fitts's Law / Hick's Law / Miller's Law** — 交互与认知

---

*本计划书随开发进度持续更新。每次重大决策或方向调整时同步修订。*
