---
title: UI 技术栈选型决策：Tauri 2
category: Design
date: 2026-05-16
tags: [design, tauri, ui]
---

# UI 技术栈选型决策：Tauri 2

> 日期：2026-04-25 | 决策人：开发团队 | 状态：Approved

## 背景

Clarity 定位为"开发者的 AI 标准运行时"，当前拥有三个入口：
- **TUI** (`clarity-tui`)：ratatui，终端交互
- **Web** (`clarity-gateway`)：Axum + 嵌入式前端
- **系统托盘** (`clarity-claw`)：后台监控

与竞品 cc-haha 的横向对比暴露出显著差距：
- **无 Desktop GUI**：cc-haha 拥有 Tauri 2 + React 的完整桌面端
- **无手机 APP**：Kimi、DeepSeek、OpenClaw 均有 iOS/Android 客户端
- **无多标签**：cc-haha Desktop 支持多会话标签页

约束条件：**类 Rust 技术栈**——核心 Runtime 必须是 Rust，UI 层由 Rust 驱动，前端可用 Web 技术渲染。

---

## 候选方案评估

| 方案 | 技术 | Web | Desktop | Mobile | 核心 Rust | 前端技术 | 成熟度 | verdict |
|------|------|-----|---------|--------|-----------|----------|--------|---------|
| A | **Tauri 2** | ✅ | ✅ iOS/Android | ✅ | ✅ Rust | Web (React/Vue) | ⭐⭐⭐⭐⭐ | **推荐** |
| B | **Dioxus** | ✅ WASM | ✅ | ⚠️ 发展中 | ✅ Rust | RSX (Rust) | ⭐⭐⭐ | Mobile 不成熟 |
| C | **Flutter + Rust FFI** | ⚠️ | ✅ | ✅ | ⚠️ FFI | Dart | ⭐⭐⭐⭐⭐ | 复杂度高 |
| D | **PWA (WASM)** | ✅ | ✅ | ⚠️ | ✅ Rust | Web | ⭐⭐⭐ | 非原生 APP |
| E | Tauri 1 | ❌ | ✅ | ❌ | ✅ Rust | Web | ⭐⭐⭐⭐ | **过时** |

### 方案 A：Tauri 2（推荐）

**优势：**
1. **一套前端代码，五个平台**：iOS / Android / Desktop (Win/Mac/Linux) / Web
2. **核心 Rust**：Agent/Memory/Tools 全部在 Rust 后端，前端只是"皮肤"
3. **生产级验证**：2024年10月稳定发布，GitButler、Hoppscotch、Spacedrive 等已上线
4. **Bundle 极小**：3-10MB vs Electron 120-200MB
5. **与现有架构兼容**：
   - Desktop：直接链接 `clarity-core`（同进程，零序列化开销）
   - Web：通过 Gateway Axum API（REST + WebSocket）
   - Mobile：Rust 后端 + WebView 前端
6. **前端生态无限**：React/Vue/Svelte 组件库、图表库、富文本编辑器应有尽有
7. **900+ 插件生态**：推送通知、生物识别、NFC、扫码等

**劣势：**
1. **前端需要 Web 技术**：HTML/CSS/JS，不是纯 Rust UI
2. **WebView 渲染差异**：不同平台 WebView 引擎略有差异（需测试）
3. **Rust 学习曲线**：自定义原生功能需要写 Rust

**适用场景：**
- 全平台 APP（iOS/Android/Desktop/Web）
- 需要原生系统能力（推送、文件系统、生物识别）

### 方案 B：Dioxus（备选）

**优势：**
- 纯 Rust UI（RSX 语法），零 JavaScript
- Desktop + Web 支持成熟

**劣势：**
- **Mobile 不成熟**：iOS/Android 支持还在发展中，无生产级案例
- 生态较小：组件库、第三方库不如 React
- 无法满足"替代 Kimi APP"的需求

**结论**：仅适合 Desktop/Web 场景，不适合全平台产品。

### 方案 C：Flutter + Rust FFI

**优势：**
- Mobile/Desktop 原生渲染最成熟
- Google 背书

**劣势：**
- **复杂度极高**：Rust ↔ Dart FFI 桥接、类型映射、异步处理
- 两套代码库：Flutter UI + Rust Core，维护成本高
- Web 支持弱

**结论**：技术可行但成本过高，不适合快速迭代。

### 方案 D：PWA（渐进式 Web 应用）

**优势：**
- 无需 App Store 上架
- 用户"添加到主屏幕"即可

**劣势：**
- **非原生 APP**：无推送通知、无后台运行、体验差于原生
- 无法替代真正的 Kimi APP

**结论**：可作为临时方案，但不是最终目标。

### 方案 E：Tauri 1（排除）

**劣势：**
- 仅支持 Desktop，无 Mobile
- 插件系统老旧
- 官方已停止大功能开发，全力推 Tauri 2

**结论**：新项目**绝对不要**用 Tauri 1。

---

## 决策

**采用 Tauri 2.10.x 作为 Clarity 的统一 UI 框架。**

### 版本锁定策略
```toml
# Cargo.toml
[dependencies]
tauri = { version = "~2.10", features = [...] }
```
- `~2.10`：自动接收 2.10.4/5/6 等补丁（bugfix + 安全更新）
- 不会自动跳到 2.11（如有 breaking change 风险）
- 每季度评估一次升级到最新 2.x 小版本

### 架构设计

```
┌─────────────────────────────────────────────────────────────────┐
│                        CLARITY 入口层                            │
├─────────────┬─────────────────┬─────────────────────────────────┤
│   claw      │     cli         │         clarity-tauri           │
│  (托盘)      │   (TUI)         │   (iOS / Android / Desktop)    │
│             │                 │                                 │
│ • OS 通知    │ • /plan         │ • 多标签多会话                  │
│ • 任务徽章   │ • ratatui       │ • React/Vue 前端               │
│ • 系统托盘   │ • 快捷键        │ • 实时流式 SSE                 │
└──────┬──────┴────────┬────────┴────────────┬────────────────────┘
       │               │                     │
       └───────────────┴─────────────────────┘
                       │
         ┌─────────────┴─────────────┐
         │      clarity-core         │
         │  • Agent (ReAct / Plan)   │
         │  • ToolRegistry           │
         │  • Memory (BM25 + vector) │
         │  • Subagent (parallel)    │
         └───────────────────────────┘
```

### `clarity-tauri` crate 职责

```
crates/clarity-tauri/
├── src/
│   ├── lib.rs              # Tauri 命令注册 + 状态管理
│   ├── main.rs             # Desktop/Mobile 入口
│   ├── commands/           # Tauri IPC 命令（Rust 端）
│   │   ├── agent.rs        # agent/run, agent/interrupt
│   │   ├── memory.rs       # memory/search, memory/store
│   │   ├── tools.rs        # tool/execute, tool/approve
│   │   └── settings.rs     # config get/set
│   └── state.rs            # Tauri AppState（Agent 实例池）
├── capabilities/           # Tauri 2 ACL 权限配置
├── gen/                    # Tauri 生成代码（自动）
├── src-tauri/frontend/     # Web 前端（React/Vue）
│   ├── src/
│   │   ├── App.tsx         # 根组件
│   │   ├── components/
│   │   │   ├── Chat.tsx    # 聊天面板
│   │   │   ├── Sidebar.tsx # 会话列表
│   │   │   ├── Tabs.tsx    # 多标签栏
│   │   │   └── TaskPanel.tsx
│   │   └── api.ts          # Tauri invoke 客户端
│   └── package.json
├── Cargo.toml
└── tauri.conf.json
```

**Desktop/Mobile 模式**：直接链接 `clarity-core`，Agent 实例在本地进程中运行，通过 Tauri Command 与前端通信。

**Web 模式**：前端代码复用，通过 HTTP/WebSocket 连接 `clarity-gateway`。

---

## Tauri 2 关键特性

| 特性 | Tauri 2 支持 | 用途 |
|------|-------------|------|
| **iOS 支持** | ✅ | 替代 Kimi iOS APP |
| **Android 支持** | ✅ | 替代 Kimi Android APP |
| **Desktop (Win/Mac/Linux)** | ✅ | 替代 OpenClaw Desktop |
| **WebView 渲染** | ✅ 原生 WebView | 轻量、省电 |
| **推送通知** | ✅ APNs/FCM | 任务完成提醒 |
| **生物识别** | ✅ Face ID/Touch ID | 安全审批 |
| **深度链接** | ✅ | 从浏览器唤起 APP |
| **文件系统访问** | ✅ 权限控制 | 读写工作目录 |
| **原生菜单/托盘** | ✅ | 系统级集成 |
| **自动更新** | ✅ delta 更新 | 1-5MB 增量包 |

---

## 追赶路线图（基于 Tauri 2）

### Phase 1：Desktop GUI 基础（2 周）

| 工作项 | 说明 | 依赖 |
|--------|------|------|
| `clarity-tauri` crate 初始化 | Cargo workspace 新增，Tauri 2.10 依赖 | 无 |
| Tauri Command 桥接 | Rust 端暴露 agent/run, agent/interrupt 等命令 | `clarity-core` |
| 聊天面板组件 | React 消息列表、用户输入、流式渲染 | Tauri invoke |
| 会话侧边栏 | 历史会话列表、新建会话 | Gateway Session API |
| 多标签框架 | 标签栏 + 标签页容器 | 无 |

### Phase 2：核心功能补齐（2-3 周）

| 工作项 | 说明 | cc-haha 对标 |
|--------|------|-------------|
| **多标签多会话** | 每个标签独立 Agent 实例 | cc-haha Desktop 多标签 |
| **任务面板** | 后台任务实时监控 | cc-haha 任务徽章 |
| **审批弹窗** | 工具调用审批对话框 | cc-haha 审批系统 |
| **设置面板** | 模型配置、审批模式、主题 | cc-haha Config |

### Phase 3：Mobile 适配（2-3 周）

| 工作项 | 说明 | 平台 |
|--------|------|------|
| **iOS 构建** | Xcode 项目配置、签名、TestFlight | iOS |
| **Android 构建** | Gradle 配置、APK/AAB 打包 | Android |
| **移动端 UI 适配** | 底部导航、手势、软键盘处理 | iOS/Android |
| **推送通知** | APNs (iOS) + FCM (Android) | iOS/Android |
| **生物识别审批** | Face ID / 指纹 替代手动确认 | iOS/Android |

### Phase 4：高级功能（3-4 周）

| 工作项 | 说明 | cc-haha 对标 |
|--------|------|-------------|
| **LSP 支持** | `tower-lsp` 集成，语言服务器管理 | cc-haha LSPTool |
| **审批系统增强** | AI 分类器 + 规则引擎 + 远程中继 | cc-haha 7 模式审批 |
| **WebBrowserTool** | `headless_chrome` 或 `fantoccini` | cc-haha WebBrowserTool |
| **Vim 模式** | 编辑器 Vim 键位引擎 | cc-haha Vim 集成 |

### Phase 5：生态扩展（远期）

| 工作项 | 说明 |
|--------|------|
| **Sandbox** | `landlock` (Linux) + Windows 沙箱 API |
| **Plugin SDK** | Rust dylib / WASM 插件系统 |
| **Voice** | 语音识别/合成集成 |

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Tauri 2 版本迭代快 | 低 | 锁定 `~2.10`，每季度 review |
| WebView 渲染差异 | 中 | 在 Win/Mac/iOS/Android 真机测试 CSS 兼容性 |
| 前端需要 JS 技能 | 低 | 团队已熟悉 Web 技术，前端逻辑薄（仅展示层） |
| Mobile 打包复杂 | 中 | 先 Desktop 验证，再逐步加 Mobile 目标 |
| App Store 审核 | 中 | 避免敏感 API，遵循各平台审核指南 |

---

## 附录：关键依赖

```toml
# crates/clarity-tauri/Cargo.toml
[package]
name = "clarity-tauri"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = { version = "~2.10", features = [] }
tauri-plugin-log = "2"
clarity-core = { path = "../clarity-core" }
clarity-wire = { path = "../clarity-wire" }
tokio = { version = "1", features = ["full"] }

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

```json
// crates/clarity-tauri/src-tauri/frontend/package.json
{
  "name": "clarity-tauri-frontend",
  "dependencies": {
    "react": "^18.3",
    "@tauri-apps/api": "^2.10"
  }
}
```

---

## 决策变更记录

| 日期 | 变更 | 原因 |
|------|------|------|
| 2026-04-25 | 初稿：Dioxus | 追求纯 Rust UI |
| 2026-04-25 | **修正：Tauri 2.10.x** | Dioxus Mobile 不成熟，无法替代 Kimi APP |

---

*本决策随技术演进可复审。Tauri 3.x 发布后（预计 2027+）评估升级路径。*
