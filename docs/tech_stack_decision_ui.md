# UI 技术栈选型决策：Dioxus

> 日期：2026-04-25 | 决策人：开发团队 | 状态：Approved

## 背景

Clarity 定位为"开发者的 AI 标准运行时"，当前拥有三个入口：
- **TUI** (`clarity-tui`)：ratatui，终端交互
- **Web** (`clarity-gateway`)：Axum + 嵌入式前端
- **系统托盘** (`clarity-claw`)：后台监控

与竞品 cc-haha 的横向对比暴露出显著差距：
- **无 Desktop GUI**：cc-haha 拥有 Tauri 2 + React 的完整桌面端
- **无多标签**：cc-haha Desktop 支持多会话标签页
- **UI 技术栈混杂**：Gateway 前端为嵌入式 HTML/JS，与 Rust 核心栈不一致

约束条件：**客户端/UI 保持类 Rust 的技术栈**——即 UI 层代码也应以 Rust 为主，避免引入 React/TypeScript 前端。

---

## 候选方案评估

| 方案 | 技术 | Web | Desktop | Mobile | 纯 Rust | 成熟度 |  verdict |
|------|------|-----|---------|--------|---------|--------|----------|
| A | **Dioxus** | ✅ WASM | ✅ 原生 | ✅ | ✅ | ⭐⭐⭐ 0.7.6 | **推荐** |
| B | **Leptos** | ✅ SSR | ❌ 需 Tauri | ❌ | ✅ | ⭐⭐⭐⭐ 0.7 | Web 强，Desktop 弱 |
| C | **egui** | ✅ WASM | ✅ | ⚠️ | ✅ | ⭐⭐⭐⭐ | 即时模式，不适合聊天 UI |
| D | **iced** | ✅ | ✅ | ⚠️ | ✅ | ⭐⭐⭐ | Elm 架构，生态较小 |
| E | **Slint** | ⚠️ | ✅ | ✅ | ✅ (DSL) | ⭐⭐⭐ | 声明式 DSL，学习曲线独特 |
| F | Tauri + React | ✅ | ✅ | ❌ | ❌ | ⭐⭐⭐⭐⭐ | **排除**（违反 Rust 约束） |

### 方案 A：Dioxus（推荐）

**优势：**
1. **React-like RSX**：组件化、hooks、signals，现代前端开发者友好
2. **一套代码，多平台**：RSX 组件同时编译为 Desktop（winit + skia/vello）和 Web（WASM）
3. **纯 Rust**：零 JavaScript/TypeScript，完全符合技术栈约束
4. **与现有架构兼容**：
   - Desktop：直接链接 `clarity-core`（同进程，零序列化开销）
   - Web：通过 Gateway Axum API（REST + WebSocket）
5. **性能**：Rust 原生渲染，比 Electron/Tauri+React 更轻量
6. **社区活跃**：~23K GitHub stars，0.7.6 已发布

**劣势：**
1. **SSR 不成熟**：Web 端无 SEO 需求（内部工具），可接受
2. **API 稳定性**：1.0 预计 2026 年 Q2-Q3，存在 breaking change 风险
3. **生态较小**：组件库、第三方库不如 React 丰富

**适用场景：**
- Desktop GUI（多标签、多会话、实时流式）
- Web UI（WASM 部署到 Gateway 静态目录）

### 方案 B：Leptos（备选）

**优势：**
- SSR 成熟，Web 性能顶尖（TechEmpower 排名）
- 与 Axum 集成极佳

**劣势：**
- 无 Desktop renderer，必须搭配 Tauri（引入 webview + TS 前端，违反约束）
- 不适合 Clarity 的多平台需求

**结论**：仅适合纯 Web 场景，不符合本项目需求。

### 方案 C：egui

**优势：**
- 极简单，即时模式，性能优异
- 适合开发者工具、调试面板

**劣势：**
- 不适合复杂聊天 UI、富文本渲染、多标签管理
- 与现有 ratatui 定位重叠

**结论**：作为 TUI 补充价值不大。

---

## 决策

**采用 Dioxus 作为 Clarity 的统一 UI 框架。**

### 架构设计

```
┌─────────────────────────────────────────────────────────────────┐
│                        CLARITY 入口层                            │
├─────────────┬─────────────────┬─────────────────────────────────┤
│   claw      │     cli         │           clarity-dioxus        │
│  (托盘)      │   (TUI)         │      (Desktop + Web GUI)       │
│             │                 │                                 │
│ • OS 通知    │ • /plan         │ • 多标签多会话                  │
│ • 任务徽章   │ • /parallel     │ • RSX 组件化 UI                │
│ • 系统托盘   │ • ratatui       │ • 实时流式 SSE                 │
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

### `clarity-dioxus`  crate 职责

```
crates/clarity-dioxus/
├── src/
│   ├── main.rs          # Desktop 入口 (dioxus-desktop)
│   ├── app.rs           # 根组件 + 路由/状态管理
│   ├── components/
│   │   ├── chat.rs      # 聊天面板
│   │   ├── sidebar.rs   # 会话列表
│   │   ├── tabs.rs      # 多标签栏
│   │   ├── task_panel.rs # 后台任务面板
│   │   └── settings.rs  # 设置面板
│   ├── state.rs         # 全局状态 (Dioxus signals)
│   ├── api.rs           # Gateway API 客户端 (REST + WS)
│   └── theme.rs         # 主题/样式
├── Cargo.toml
└── Dioxus.toml          # Dioxus 构建配置
```

**Desktop 模式**：直接链接 `clarity-core`，Agent 实例在本地进程中运行，通过 `Wire` 事件总线与 UI 通信。

**Web 模式**：编译为 WASM，通过 HTTP/WebSocket 连接 `clarity-gateway`。

---

## 追赶路线图（基于 Dioxus 约束）

### Phase 1：Desktop GUI 基础（2 周）

| 工作项 | 说明 | 依赖 |
|--------|------|------|
| `clarity-dioxus` crate 初始化 | Cargo workspace 新增，Dioxus 0.7 依赖 | 无 |
| 聊天面板组件 | 消息列表、用户输入、流式渲染 | `Wire` 事件总线 |
| 会话侧边栏 | 历史会话列表、新建会话 | Gateway Session API |
| 多标签框架 | 标签栏 + 标签页容器 | 无 |

### Phase 2：核心功能补齐（2-3 周）

| 工作项 | 说明 | cc-haha 对标 |
|--------|------|-------------|
| **多标签多会话** | 每个标签独立 Agent 实例 | cc-haha Desktop 多标签 |
| **任务面板** | 后台任务实时监控 | cc-haha 任务徽章 |
| **审批弹窗** | 工具调用审批对话框 | cc-haha 审批系统 |
| **设置面板** | 模型配置、审批模式、主题 | cc-haha Config |

### Phase 3：高级功能（3-4 周）

| 工作项 | 说明 | cc-haha 对标 |
|--------|------|-------------|
| **LSP 支持** | `tower-lsp` 集成，语言服务器管理 | cc-haha LSPTool |
| **审批系统增强** | AI 分类器 + 规则引擎 + 远程中继 | cc-haha 7 模式审批 |
| **WebBrowserTool** | `headless_chrome` 或 `fantoccini` | cc-haha WebBrowserTool |
| **Vim 模式** | 编辑器 Vim 键位引擎 | cc-haha Vim 集成 |

### Phase 4：生态扩展（远期）

| 工作项 | 说明 |
|--------|------|
| **Sandbox** | `landlock` (Linux) + Windows 沙箱 API |
| **Plugin SDK** | Rust dylib 插件系统 |
| **Voice** | 语音识别/合成集成 |

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Dioxus API breaking changes | 中 | 锁定版本 `0.7.x`，升级前审查 changelog |
| Dioxus Desktop 渲染 bug | 中 | 保持 Web（WASM）作为 fallback |
| 组件库生态不足 | 低 | 自研基础组件（聊天气泡、标签页等），无复杂需求 |
| 构建时间增加 | 低 | Dioxus 增量编译快，CI 缓存 WASM 工具链 |

---

## 附录：关键依赖

```toml
# crates/clarity-dioxus/Cargo.toml
[dependencies]
dioxus = { version = "0.7", features = ["desktop", "web"] }
dioxus-signals = "0.7"
# Desktop only
clarity-core = { path = "../clarity-core" }
clarity-wire = { path = "../clarity-wire" }
# Web only (WASM)
reqwest = { version = "0.12", default-features = false }
```

---

*本决策随技术演进可复审。若 Dioxus 1.0 发布后 API 稳定，本决策自动确认；若出现重大障碍，可回退到 Leptos (Web) + egui (Desktop) 的分离方案。*
