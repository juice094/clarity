# Clarity 竞品分析报告

> 分析范围：`C:\Users\22414\dev\third_party` 36 个开源项目
> 分析日期：2026-04-15
> Clarity 版本：主干同期

---

## 前言：双视角分析框架

本报告采用**双视角**分析框架，因为 Clarity 同时处于两个时间维度：

- **视角一 · 产品视角**：Clarity 是**"分布式 AI 认知容器"**——基于当前已实现的功能（三层入口、会话隔离、MCP 工具、模型中立），分析其在现有产品生态中的位置。
- **视角二 · 愿景视角**：Clarity 是**"未来的 AI 基础设施"**——基于长期目标（人机交互接触面、智能引擎、自动化整理、自我进化），分析其在 AI 基础设施层级中的位置。

两个视角并非矛盾，而是**同一事物的现在与未来**。产品视角回答"我们现在在哪里"，愿景视角回答"我们要去哪里"。

---

## 一、执行摘要（双视角对比）

| 维度 | 产品视角：分布式 AI 认知容器 | 愿景视角：AI 基础设施 |
|:-----|:--------------------------|:-------------------|
| **核心定义** | 同一个智能体在三层中以不同形态存在 | 人机交互的底层支撑系统 |
| **竞争本质** | "容器设计"的竞争——如何让 AI 更好地分布式存在 | "层级占位"的竞争——谁成为 AI 时代的操作系统 |
| **直接竞品** | claude-code-rust（多入口）、5ire（桌面） | ollama（模型基础设施）、5ire（桌面基础设施） |
| **护城河** | 三层差异化 + 模型中立 | 模型中立 + 自我进化 + 气生根开放 |
| **当前短板** | 单点功能弱于专用工具 | 无本地模型能力、claw 层未成型 |
| **长期机会** | 跨层记忆连续性、devbase 联动 | 成为个人 AI 的标准运行时 |

---

## 二、竞品全景分类

```
AI 生态栈
│
┌─────────────────────────────────────────────────────────────────┐
│  🏗️ 基础设施层（Foundation）                                      │
│  ├── ollama         — 模型运行基础设施（本地 LLM 部署）            │
│  ├── dify           — 应用开发基础设施（LLM 应用平台，to B）        │
│  ├── rust-sdk(rmcp) — MCP 协议基础设施                           │
│  ├── 5ire           — 桌面 AI 基础设施（知识库+MCP，to C）         │
│  └── **Clarity**    — AI 交互基础设施（多接触面+智能引擎+自进化）   │
├─────────────────────────────────────────────────────────────────┤
│  🛠️ 应用层（Application）                                        │
│  ├── claude-code-rust — 多入口 AI 编码助手（TUI/GUI/CLI/插件）     │
│  ├── codex            — CLI 编码助手                            │
│  ├── kimi-cli         — CLI 编码助手                            │
│  ├── OpenHands        — AI 软件工程师（替代人类）                 │
│  ├── 5ire (应用面)    — 桌面 AI 助手                            │
│  ├── openclaw         — 个人 AI 助手（多通道）                    │
│  ├── openhanako       — AI 助手                                 │
│  └── zeroclaw         — 个人 AI 助手                            │
├─────────────────────────────────────────────────────────────────┤
│  🔧 工具/框架层（Tool/Framework）                                │
│  ├── AutoAgent        — 多智能体框架（Python 研究项目）            │
│  ├── EvoAgentX        — 进化多智能体框架                         │
│  ├── deer-flow        — AI 工作流编排                           │
│  ├── nanobot          — AI Agent 实验项目                        │
│  ├── ratatui          — Rust TUI 框架                           │
│  └── rust-sdk (库)    — MCP Rust SDK（被基础设施层消费）          │
├─────────────────────────────────────────────────────────────────┤
│  🧠 ML/LLM 框架层（ML Framework）                                │
│  ├── candle           — Rust ML 框架（HuggingFace）               │
│  ├── burn             — Rust 深度学习框架                        │
│  └── vllm             — LLM 推理引擎（Python）                    │
├─────────────────────────────────────────────────────────────────┤
│  🌐 网络/同步层（Network）                                       │
│  ├── syncthing        — P2P 文件同步                            │
│  ├── syncthing-rust   — P2P 文件同步（Rust）                      │
│  ├── iroh             — P2P 网络协议栈                          │
│  └── tailscale        — VPN Mesh 网络                          │
├─────────────────────────────────────────────────────────────────┤
│  🔧 Git/Dev 工具层（Dev Tool）                                   │
│  ├── lazygit          — Git TUI 客户端                         │
│  ├── gitui            — Git TUI 客户端（Rust）                    │
│  ├── gitoxide         — 纯 Rust git 实现                        │
│  ├── desktop          — GitHub Desktop                         │
│  ├── gws              — Git Workspace 管理                     │
│  └── workspace-tools  — Monorepo 变更管理                      │
├─────────────────────────────────────────────────────────────────┤
│  📦 其他/参考                                                     │
│  ├── AutoCLI          — AI-native CLI（网页抓取）                 │
│  ├── motrix-next      — 下载管理器                              │
│  ├── cheat-engine     — 游戏修改工具                            │
│  └── cpj_ref, agmmu_ref, agricm3_ref — 参考项目                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 三、核心竞品详细功能对比

### 3.1 视角一：产品视角 —— "分布式 AI 认知容器"

> **分析前提**：Clarity 当前是一个让同一个智能体在 claw/window/cli 三层中以不同形态存在的容器系统。三层不是功能对齐，而是认知边界的分化。

#### 表 A：多入口 AI 容器维度

| 功能模块 | **Clarity** | **claude-code-rust** | **codex-rs** | **5ire** | **openclaw** | **zeroclaw** |
|:---------|:-----------:|:--------------------:|:------------:|:--------:|:------------:|:------------:|
| **技术栈** | Rust | Rust | Rust | TS/Electron | Node.js/TS | Rust |
| **容器形态** | ✅ **claw(托盘)+window(Web)+cli(TUI)** | TUI+GUI+CLI | CLI | Electron 桌面 | CLI/TUI | CLI |
| **形态差异化** | ✅ **三层独立会话+差异化提示词** | ❌ 通用提示词 | ❌ 通用 | ❌ 通用 | ❌ 通用 | ❌ 通用 |
| **常驻/后台** | ✅ **claw 系统托盘常驻** | ❌ | ❌ | ⚠️ 桌面常驻 | ❌ | ❌ |
| **LLM 对话** | ✅ SSE 流式 | ✅ REPL + TUI | ✅ REPL | ✅ | ✅ | ✅ |
| **模型中立** | ✅ **多模型切换** | ⚠️ 单模型 | ⚠️ OpenAI 为主 | ❌ 单模型 | ⚠️ 单模型 | ⚠️ 单模型 |
| **MCP 工具** | ✅ **Client（stdio/HTTP/SSE）** | ❌ | ✅ Server + Client | ✅ Client | ❌ | ❌ |
| **Plan Mode** | ✅ **先规划后执行** | ❌ | ❌ | ❌ | ❌ | ❌ |
| **并行子代理** | ✅ **run_parallel()** | ❌ | ❌ | ❌ | ❌ | ❌ |
| **文件操作** | ✅ 读/写/搜索/执行 | ✅ | ✅ | ⚠️ 有限 | ⚠️ 有限 | ⚠️ 有限 |
| **本地记忆** | ✅ 会话持久化+BM25 | ✅ Skill 系统 | ❌ | ✅ 知识库 RAG | ❌ | ❌ |
| **涌现人格** | ✅ **devbase 日记+选择构成** | ❌ 无此概念 | ❌ | ❌ | ❌ | ❌ |
| **Web UI** | ✅ 独立 | ✅ egui | ❌ | ✅ | ❌ | ❌ |
| **i18n** | ✅ 中/英 | ✅ 中/英 | ❌ | ❌ | ❌ | ❌ |
| **Docker 沙箱** | ❌ | ❌ | ✅ | ❌ | ✅ | ❌ |

**产品视角洞察：**

- **claude-code-rust** 是应用层的绝对标杆，入口最丰富、功能最完整。但其入口之间没有差异化——同一个 Agent 在 TUI、GUI、CLI 中使用同一套提示词和行为模式。Clarity 的差异化在于**三层不是功能对齐，而是认知边界的分化**。
- **5ire** 是桌面 AI 助手中最接近 Clarity 的竞品，具备知识库 + MCP + 桌面常驻。但 5ire 是单一桌面形态，而 Clarity 的 window 只是三层之一。
- **codex-rs** 是工业级 Rust Agent，含 MCP Server/Client、Docker 沙箱、技能系统，但无 Plan Mode 和并行子代理。
- **openclaw / zeroclaw** 在命名上与 Clarity 的 claw 层有概念呼应，但均为单入口或弱多入口，无容器设计理念。

---

#### 表 A 营养吸收计划（竞品即开源营养品）

> **数据来源**：GitHub 仓库、官方文档、开发者社区文章（2026-04-15 检索）
> **核心洞察**：所有竞品均为开源项目（MIT/Apache/GPL），其代码、架构、设计模式均可被 Clarity 研究、吸收、改造。竞品不是威胁清单，而是**能力补完的路线图**。

---

##### ① 从 OpenHanako 吸收 — Plugin 系统 + Sandbox + 多智能体架构

| 维度 | 事实 |
|:-----|:-----|
| **实际定位** | TypeScript/Electron 个人 AI 助手，Apache-2.0 协议 |
| **GitHub Stars** | 未披露，个人项目规模 |
| **营养成分清单** | ① **Plugin 系统**（拖放安装，扩展工具/技能/Provider/HTTP 路由/事件钩子）<br>② **两层 Sandbox**（PathGuard 四级访问 + OS 级隔离）<br>③ **Desk 空间**（文件/笔记的异步协作空间）<br>④ **Cron + Heartbeat**（定时任务 + 文件变化监控）<br>⑤ **Multi-Platform Bridge**（Telegram/Feishu/QQ/微信机器人同时连接）<br>⑥ **多智能体协作**（多个独立 Agent 委托任务、群聊协作） |
| **Clarity 怎么吃** | ① **Plugin 系统改造**：Clarity 当前使用 MCP 作为工具扩展机制，但 MCP 是协议层，不是应用层 Plugin。可借鉴 OpenHanako 的"拖放安装 + 两级权限"设计，在 MCP 之上构建 Plugin 包装层<br>② **Sandbox 改造**：Clarity 当前无沙箱机制。可借鉴 PathGuard 四级访问模型（只读/读写/受限/隔离），为文件操作和代码执行增加安全层<br>③ **Desk 空间**：将 devbase 日志目录升级为"Desk"——不仅是日志，而是用户与 AI 的异步协作空间（文件、笔记、待办）<br>④ **Cron/Heartbeat 吸收**：claw 层后台任务的自然实现方式 |
| **消化风险** | ① OpenHanako 是 TypeScript/Electron，架构模式不能直接套用到 Rust。Plugin 系统的动态加载在 Rust 中需要 `dylib` 或 WASM，复杂度更高<br>② 多智能体协作与 Clarity 的"单 Agent 三层分化"理念冲突——Clarity 主张一个 Agent 在不同场景分化，不是多个 Agent 协作<br>③ Bridge 系统（微信/QQ 机器人）涉及第三方平台 API，合规风险和逆向工程成本高 |
| **吸收优先级** | 🟢 **高价值，中难度** — Plugin 包装层 + Sandbox 是 Clarity 当前缺失的核心能力 |

---

##### ② 从 5ire 吸收 — RAG 知识库 + 双进程架构 + UI 设计

| 维度 | 事实 |
|:-----|:-----|
| **实际定位** | TypeScript/Electron 跨平台桌面 AI 助手 + MCP Client，~5,151 Stars |
| **营养成分清单** | ① **本地 RAG 知识库**（bge-m3 向量嵌入，支持 docx/xlsx/pptx/pdf/txt/csv 解析）<br>② **双进程架构**（主进程运行 MCP Server + 嵌入模型，渲染进程运行 React UI）<br>③ **提示词库 + 书签 + 历史搜索 + Token 追踪**<br>④ **Electron UI 设计**（暗色主题、动画、交互细节） |
| **Clarity 怎么吃** | ① **RAG 知识库**：Clarity 当前只有 devbase 原始日志（无检索能力）。可引入 `bge-m3` 或 `fastembed`（Rust 绑定）做本地嵌入，将 devbase 日志转化为可语义检索的向量库。window 层查询时不再只是关键词搜索，而是语义理解<br>② **文档解析**：5ire 支持多种格式解析。Clarity 可引入 `pdfplumber`/`pymupdf`（Python 桥接）或纯 Rust 库（`pdf-extract`、`calamine`）做文档解析，让 window 层具备"读取并理解本地文档"的能力<br>③ **双进程架构参考**：Clarity 当前是 Gateway 单进程 + 前端。可参考 5ire 的"主进程（MCP + 嵌入）+ 渲染进程（UI）"分离，提升稳定性<br>④ **UI 设计吸收**：直接参考 5ire 的暗色主题配色、动画时长、布局比例 |
| **消化风险** | ① **bge-m3 在 Rust 中的绑定不成熟**。`ort`（ONNX Runtime Rust）可以运行 bge-m3，但性能和质量需要验证。替代方案：用 ollama 本地运行嵌入模型（已有成熟支持）<br>② **文档解析的 Rust 生态弱于 Python**。PDF/Excel/PPT 解析在 Rust 中无成熟库，可能需要调用外部进程或 WASM<br>③ 5ire 的 Modified Apache 2.0 (non-commercial) 协议限制商业使用，但架构设计不受版权保护（可清洁室吸收） |
| **吸收优先级** | 🔴 **最高价值，最高难度** — RAG 知识库是 Clarity 当前最大的能力缺口 |

---

##### ③ 从 Claurst/Claux 吸收 — Compaction + Sub-agents + TUI 体验

| 维度 | 事实 |
|:-----|:-----|
| **实际定位** | Claude Code 的清洁室 Rust 重写，~8,272 Stars，MIT 协议 |
| **营养成分清单** | ① **Compaction**（`/compact` 自动总结对话释放上下文）<br>② **Sub-agents**（Agent 工具生成独立子对话，处理子任务）<br>③ **Cost tracking**（Token 用量和 USD 估算）<br>④ **模型切换**（`/model <name>` 对话中实时切换）<br>⑤ **TUI Markdown 渲染**（ratatui 中的代码块/粗体/标题渲染）<br>⑥ **JSONL 会话持久化**（`/resume` 恢复） |
| **Clarity 怎么吃** | ① **Compaction 直接吸收**：Clarity core 已有对话压缩的概念，但无自动触发机制。可直接引入 Claurst 的"对话长度阈值 → 自动总结 → 替换历史"逻辑<br>② **Sub-agents 改造**：Clarity 可将"工具调用"升级为"子 Agent 调用"——当任务复杂时，主 Agent 生成一个子 Agent（带独立上下文和工具集）处理子任务，然后合并结果。这是从"工具"到"Agent"的跃迁<br>③ **Cost tracking**：为每个模型配置价格参数，在每次 LLM 调用后累计 Token 和费用，在 cli 层显示实时成本<br>④ **TUI Markdown 渲染**：直接借鉴 ratatui 的 Markdown 渲染实现，提升 cli 层阅读体验 |
| **消化风险** | ① Claurst 是 MIT 协议，代码可直接参考。但清洁室原则下，应吸收设计思路而非复制代码<br>② Sub-agents 的实现需要会话管理的深层改造——子 Agent 的上下文隔离、结果回传、错误处理都是新增复杂度<br>③ Cost tracking 需要维护各模型 Provider 的价格表，且价格变动频繁 |
| **吸收优先级** | 🟢 **高价值，低难度** — Compaction + Cost tracking + Markdown 渲染可在短期内实现 |

---

##### ④ 从 ZeroClaw 吸收 — 极致性能 + Trait 架构 + SOPs + AIEOS

| 维度 | 事实 |
|:-----|:-----|
| **实际定位** | OpenClaw 的 100% Rust 重写，20,000+ Stars，Agent Runtime Kernel 定位 |
| **营养成分清单** | ① **<5MB RAM + <10ms 启动**（单二进制极致性能）<br>② **Trait-driven 架构**（Provider/Channel/Memory/Tool 全部可插拔替换）<br>③ **SOPs**（事件驱动工作流自动化，MQTT/webhook/cron/外设触发）<br>④ **AIEOS identity profiles**（人格/心理/语言/动机四维配置）<br>⑤ **Multi-agent orchestration**（Hands，自主 Agent 群）<br>⑥ **70+ 工具** 的组织和注册机制<br>⑦ **硬件外设集成**（ESP32/STM32/Arduino/Raspberry Pi GPIO） |
| **Clarity 怎么吃** | ① **性能优化目标**：ZeroClaw 的 <5MB RAM 证明了 Rust AI Agent 可以做到极致轻量。Clarity 当前 Gateway+TUI+Web UI 组合较重。可借鉴 ZeroClaw 的模块化启动策略——按需加载组件（claw 层启动时不加载 window/cli 的全量资源）<br>② **Trait 架构改造**：Clarity 当前的模型/工具/记忆是硬编码组合。可借鉴 ZeroClaw 的 `Provider` trait/`Channel` trait/`Memory` trait 设计，让每层组件可插拔<br>③ **SOPs 吸收**：将 devbase 的"活动记录"升级为"事件驱动工作流"——当检测到特定文件变化/Git 提交/时间触发时，自动执行预设的 Agent 任务<br>④ **AIEOS 改造吸收**：Clarity 主张"无人格"，但 AIEOS 的四维模型（Identity/Psychology/Linguistics/Motivations）可以改造为"场景配置"——不是给 Agent 人格，而是给不同入口配置行为偏好的结构化方式 |
| **消化风险** | ① ZeroClaw 的极致性能来自其极简设计（只做核心，不做 UI）。Clarity 的三层架构天然更重，<5MB RAM 对 Clarity 不现实，但**模块化按需加载**是可实现的<br>② Trait-driven 架构需要大规模重构当前的硬编码组合，是中期工程<br>③ AIEOS 的"人格"概念与 Clarity 的"无人格"立场冲突，需要改造为"场景适配配置"而非"人格模板" |
| **吸收优先级** | 🟡 **最高价值，最高难度** — Trait 架构改造是中长期工程，但性能优化和 SOPs 可在短期内吸收 |

---

##### ⑤ 从 OpenClaw 吸收 — Skill 生态模式 + 社区驱动 + 通道设计

| 维度 | 事实 |
|:-----|:-----|
| **实际定位** | TypeScript/Node.js AI 助手网关，250,000+ Stars，Skill 扩展生态 |
| **营养成分清单** | ① **Skill 生态模式**（社区贡献 + 中央目录 + 一键安装 `openclaw install xxx`）<br>② **通道设计哲学**（每个通道是一个独立适配器，统一消息格式）<br>③ **社区驱动开发**（250K Stars 带来的社区反馈循环）<br>④ **IDENTITY/SOUL Markdown 配置**（用 Markdown 文件定义 Agent 行为） |
| **Clarity 怎么吃** | ① **Skill 生态模式**：Clarity 当前的 MCP 工具是技术层面的扩展，但缺少"应用商店"层面的生态。可借鉴 OpenClaw 的"中央目录 + 一键安装"模式，建立 Clarity 的 Skill 目录（本质上是预配置的 MCP 工具包 + 提示词模板）<br>② **Markdown 配置吸收**：OpenClaw 用 IDENTITY.md/SOUL.md 定义 Agent。Clarity 可将系统提示词配置从代码中抽离，改为 Markdown 文件（与 devbase 的 AGENTS.md 风格一致）<br>③ **通道设计参考**：Clarity 当前的三层是"本地入口"，未来若扩展外部通道（微信/Slack），可借鉴 OpenClaw 的"统一消息格式 + 独立适配器"设计 |
| **消化风险** | ① OpenClaw 的 Skill 生态依赖其 250K Stars 的社区体量。Clarity 若复制"中央目录"模式但社区不足，将是空城<br>② OpenClaw 的 TypeScript 生态（npm 包）与 Clarity 的 Rust 生态完全不兼容，Skill 需要重写<br>③ 外部通道（微信/QQ）涉及平台合规，不能简单复制 |
| **吸收优先级** | 🟡 **高价值，高难度** — Skill 生态模式需要先有社区，再建目录。Markdown 配置可在短期内吸收 |

---

##### 营养吸收路线图

```
阶段 1（现在-2个月）        阶段 2（2-4个月）           阶段 3（4-6个月）
    │                         │                          │
    ▼                         ▼                          ▼
┌───────────┐            ┌───────────┐             ┌───────────┐
│ Claurst   │  ───────▶  │  5ire     │  ───────▶  │ ZeroClaw  │
│ 营养包    │            │ 营养包    │             │ 营养包    │
├───────────┤            ├───────────┤             ├───────────┤
│•Compaction│            │•RAG 向量库 │             │•Trait 架构 │
│•Cost 追踪 │            │•文档解析  │             │•SOPs 工作流│
│•Markdown │            │•UI 设计   │             │•模块化启动 │
│ 渲染     │            │•双进程参考│             │•AIEOS 改造│
└───────────┘            └───────────┘             └───────────┘
      │                        │                         │
      ▼                        ▼                         ▼
┌───────────┐            ┌───────────┐             ┌───────────┐
│OpenHanako │            │OpenClaw   │             │           │
│ 营养包    │            │ 营养包    │             │           │
├───────────┤            ├───────────┤             │           │
│•Plugin 层 │            │•Skill 目录 │             │           │
│•Sandbox  │            │•Markdown  │             │           │
│•Desk 空间 │            │ 配置      │             │           │
│•Cron/心跳 │            │•通道设计  │             │           │
└───────────┘            └───────────┘             └───────────┘
```

---

##### 营养吸收总结：Clarity 的能力补完清单

| 能力缺口 | 营养来源 | 吸收方式 | 难度 | 优先级 |
|---------|---------|---------|------|--------|
| **RAG 向量知识库** | 5ire (bge-m3) | 引入本地嵌入模型 + 向量索引 | 高 | 🔴 P0 |
| **文档解析** | 5ire | Python 桥接或 WASM 方案 | 高 | 🔴 P0 |
| **Plugin 系统** | OpenHanako | MCP 之上构建 Plugin 包装层 | 中 | 🟡 P1 |
| **Sandbox 机制** | OpenHanako | PathGuard 四级访问模型 | 中 | 🟡 P1 |
| **Compaction** | Claurst | 对话长度阈值 → 自动总结 | 低 | 🟢 P2 |
| **Sub-agents** | Claurst | 工具调用升级为子 Agent 调用 | 中 | 🟡 P1 |
| **Cost tracking** | Claurst | 模型价格表 + Token 累计 | 低 | 🟢 P2 |
| **Markdown 渲染** | Claurst | ratatui Markdown 组件 | 低 | 🟢 P2 |
| **模块化启动** | ZeroClaw | 按需加载组件，减少内存占用 | 中 | 🟡 P1 |
| **Trait 架构** | ZeroClaw | Provider/Memory/Tool 可插拔 | 高 | 🔴 P0 |
| **SOPs 工作流** | ZeroClaw | 事件触发 → 自动 Agent 任务 | 中 | 🟡 P1 |
| **Skill 目录** | OpenClaw | 预配置 MCP 包 + 一键安装 | 中 | 🟡 P1 |
| **Markdown 配置** | OpenClaw | 系统提示词抽离为 Markdown | 低 | 🟢 P2 |

**核心洞察：**

竞品不是敌人，是**免费的研发团队**。250K Stars 的 OpenClaw、20K Stars 的 ZeroClaw、8K Stars 的 Claurst、5K Stars 的 5ire——它们已经帮 Clarity 验证了哪些功能有价值、哪些架构可行、哪些设计模式优雅。Clarity 不需要重新发明轮子，只需要**站在这些开源巨人的肩膀上**，把验证过的设计用 Rust 重新实现，并加上自己的三层差异化。

> "榕树的气生根不是与大树竞争阳光，而是向下蔓延，吸收大地的一切养分。" —— 这正是 Clarity 的竞品策略。

---

#### 表 B：CLI 编码助手维度（与 Clarity `cli` 层竞争）

| 功能模块 | **Clarity cli** | **codex** | **kimi-cli** | **OpenHands** |
|:---------|:---------------:|:---------:|:------------:|:-------------:|
| **技术栈** | Rust | TypeScript | TypeScript | Python |
| **交互方式** | TUI (ratatui) | CLI REPL | CLI REPL | Web UI + CLI |
| **文件操作** | ✅ 读/写/搜索/执行 | ✅ | ✅ | ✅ |
| **代码执行** | ✅ 本地执行 | ✅ sandbox | ✅ sandbox | ✅ sandbox |
| **工具系统** | ✅ MCP 动态工具 | ❌ 内置工具 | ❌ 内置工具 | ✅ 多工具 |
| **会话持久化** | ✅ 独立命名空间 | ❌ | ❌ | ✅ |
| **与其他层共享记忆** | ✅ **同 devbase** | ❌ | ❌ | ❌ |
| **云端依赖** | ⚠️ 需 API Key | ⚠️ OpenAI API | ⚠️ Moonshot API | ⚠️ 多模型 API |

**产品视角洞察：**

- **codex** 和 **kimi-cli** 是 CLI 编码助手的标杆，但它们是"用完即走"的工具，没有跨会话记忆，更不存在与其他入口的连续性。
- **OpenHands** 定位是"AI 软件工程师"，与 Clarity 的 cli 层赛道不同——OpenHands 尝试替代工程师，Clarity cli 层是增强工程师。
- **Clarity cli 的差异化**：不是最强的编码助手，而是"工程师在终端与 AI 协作的容器"——长会话、记忆连续性、与 claw/window 共享 devbase 上下文。

---

### 3.2 视角二：愿景视角 —— "AI 基础设施"

> **分析前提**：Clarity 的长期目标是成为 AI 时代的底层基础设施——如同操作系统之于计算机。它不解决某个具体问题，而是为所有 AI 交互场景提供运行时支撑。

#### 表 C：AI 基础设施维度

| 功能模块 | **Clarity** | **ollama** | **dify** | **5ire** | **rust-sdk** |
|:---------|:-----------:|:----------:|:--------:|:--------:|:------------:|
| **基础设施类型** | 交互基础设施 | 模型基础设施 | 应用开发基础设施 | 桌面 AI 基础设施 | 协议基础设施 |
| **目标用户** | 个人开发者 | 开发者/运维 | 企业开发者 | 个人用户 | 开发者 |
| **接触面形态** | ✅ **claw+window+cli 三层** | ❌ API only | ❌ Web IDE | ⚠️ Electron 桌面 | ❌ 库/SDK |
| **模型中立** | ✅ **多模型切换** | ✅ 多模型本地运行 | ✅ 多模型 | ❌ 单模型 | N/A |
| **MCP 生态** | ✅ **Client（消费工具）** | ❌ | ❌ | ✅ Client | ✅ 协议实现 |
| **工具扩展性** | ✅ 动态 MCP 工具加载 | ❌ | ⚠️ 内置工具 | ✅ MCP 工具 | N/A |
| **本地运行** | ✅ 可选本地模型 | ✅ **核心能力** | ⚠️ 需部署 | ⚠️ 可选本地 | ✅ |
| **记忆/上下文** | ✅ **跨会话+跨层+devbase** | ❌ | ⚠️ 会话级 | ✅ 知识库 RAG | ❌ |
| **自我进化** | ✅ **长期使用涌现适应性** | ❌ | ❌ | ❌ | ❌ |
| **自动化整理** | ✅ **devbase 日志+活动记录** | ❌ | ❌ | ❌ | ❌ |
| **编程接口** | ✅ HTTP API + 库 | ✅ API | ✅ API + SDK | ❌ | ✅ SDK |

**愿景视角洞察：**

- **ollama** 是 AI 基础设施的标杆，但它只做"模型运行"这一件事。Clarity 不运行模型，而是消费模型——**ollama 是发电厂，Clarity 是电网**。两者天然互补，但若 ollama 未来向上扩展（会话管理、工具调用），将直接侵蚀 Clarity 的空间。
- **dify** 是 to B 的 LLM 应用开发平台，与 Clarity 的 to 个人定位完全不同。但 dify 的工作流编排、知识库管理的设计理念，值得 Clarity 参考。
- **5ire** 最接近 Clarity 的"个人 AI 基础设施"定位，但 5ire 仍是"应用思维"：用户打开 5ire 使用 AI。Clarity 的哲学是**"基础设施思维"：AI 应该无处不在，用户不需要"打开"Clarity，Clarity 已经在那里**。

---

#### 表 D：智能引擎/自动化维度

| 功能模块 | **Clarity** | **OpenHands** | **AutoAgent** | **EvoAgentX** |
|:---------|:-----------:|:-------------:|:-------------:|:-------------:|
| **定位** | 个人 AI 基础设施 | AI 软件工程师 | 多智能体框架 | 进化多智能体 |
| **自动化程度** | 辅助（用户主导） | 自主（替代人类） | 半自主 | 自主进化 |
| **人机关系** | 增强工程师 | 替代工程师 | 研究性质 | 研究性质 |
| **长期使用适应** | ✅ **涌现适应性** | ❌ 任务级 | ❌ | ⚠️ 进化算法 |
| **可解释性** | ✅ 用户可见每一步 | ⚠️ 黑盒 | ⚠️ 黑盒 | ⚠️ 黑盒 |
| **产品化程度** | 产品化基础设施 | 产品化应用 | 研究项目 | 研究项目 |

**愿景视角洞察：**

- **OpenHands** 是"AI 替代工程师"路线的标杆。Clarity 明确不走这条路——Clarity 是"增强工程师"，不是"替代工程师"。
- **Clarity 的"自我进化"不是 Agent 的自主进化**，而是**对用户的适应性进化**——长期使用中，Clarity 逐渐理解用户的偏好、习惯、工作模式。这是"关系进化"，不是"能力进化"。

---

## 四、竞争威胁评估矩阵（双视角）

### 4.1 产品视角：容器设计的竞争

```
                 高威胁 ←————————————————→ 低威胁
                    🔴                    🟡          🟢
高能力  ┌─────────┬─────────┬─────────┬─────────┬─────────┐
        │ claude- │ 5ire    │ openclaw│ codex   │ ollama  │
        │ code-rs │         │         │         │         │
        ├─────────┼─────────┼─────────┼─────────┼─────────┤
        │         │         │ openha- │ kimi-   │ dify    │
        │         │         │ nako    │ cli     │         │
        ├─────────┼─────────┼─────────┼─────────┼─────────┤
低能力  │         │         │ zeroclaw│ OpenHan │ rust-sdk│
        │         │         │         │ ds      │ ratatui │
        └─────────┴─────────┴─────────┴─────────┴─────────┘
```

### 4.2 愿景视角：基础设施层级的竞争

```
              模型基础设施    交互基础设施    应用开发基础设施   协议基础设施
                 ollama        Clarity         dify          rust-sdk
              ┌─────────┬─────────┬─────────┬─────────┐
   高能力     │         │ 🔴      │         │         │
              │         │ claude- │         │         │
              │         │ code-rs │         │         │
              ├─────────┼─────────┼─────────┼─────────┤
   中能力     │         │ 🟡      │         │         │
              │         │ 5ire    │         │         │
              │         │ codex   │         │         │
              ├─────────┼─────────┼─────────┼─────────┤
   低能力     │         │ 🟢      │         │         │
              │         │ openclaw│         │         │
              │         │ OpenHan │         │         │
              └─────────┴─────────┴─────────┴─────────┘
```

---

## 五、SWOT 交叉分析（双视角）

### 5.1 产品视角 SWOT

| | **机会 (Opportunities)** | **威胁 (Threats)** |
|:---|:---|:---|
| **优势 (Strengths)** | ① 三层差异化是独特定位，暂无竞品在"分布式存在"层面与 Clarity 竞争<br>② 模型中立是大厂因商业封锁无法做到的护城河<br>③ devbase 联动提供代码库全景感知，应用层竞品均无此能力 | claude-code-rust 若引入 MCP + 入口差异化，将直接侵蚀 Clarity 核心差异化 |
| **劣势 (Weaknesses)** | ① 单点功能弱于专用工具：cli 编码 < codex，桌面体验 < 5ire<br>② 无本地模型运行能力<br>③ claw 层仅有骨架，未形成实际交互能力 | 5ire 若扩展 TUI/CLI 入口，将直接竞争"多入口容器"定位 |

### 5.2 愿景视角 SWOT

| | **机会 (Opportunities)** | **威胁 (Threats)** |
|:---|:---|:---|
| **优势 (Strengths)** | ① "AI 交互基础设施"定位在 36 个项目中无直接对标，属于空白赛道<br>② 自我进化是全新维度，目前无竞品在做"关系进化"<br>③ 气生根开放策略可将 Clarity 从"产品"升级为"平台" | ollama 若向上扩展（会话管理+工具调用），将直接威胁 Clarity 的基础设施定位 |
| **劣势 (Weaknesses)** | ① 从"容器"到"基础设施"的跃迁需要巨大的工程投入和生态建设<br>② 无本地模型能力，在"基础设施"层级存在短板<br>③ 自我进化尚未验证，停留在设计阶段 | dify 若推出个人版，或 5ire 若开放 API，将与 Clarity 竞争"个人 AI 基础设施"定位 |

---

## 六、战略建议（双视角统一）

### 6.1 短期（1-2 个月）：产品视角主导

**目标**：把"分布式 AI 认知容器"做扎实，让三层差异化成为可感知的用户体验。

1. **固化三层差异化**
   - 确保 claw/window/cli 的差异化提示词和会话隔离稳定运行
   - 这是产品视角的核心护城河

2. **MCP 工具生态**
   - 确保与主流 MCP Server（filesystem、browser、fetch 等）兼容
   - 目标：MCP 工具数 > 20

3. **claw 层最小可用化**
   - 从"托盘图标"升级为"可交互常驻存在"
   - 最小功能：快捷指令输入、系统通知

### 6.2 中期（3-6 个月）：双视角交汇

**目标**：在产品稳定的基础上，开始向"基础设施"能力渗透。

1. **devbase 深度联动**（产品视角的深化 = 愿景视角的起步）
   - cli 层：自动注入项目上下文
   - window 层：自然语言查询 devbase 知识
   - claw 层：后台监控+主动推送

2. **跨层记忆连续性**（产品视角的独特价值）
   - 实现"用户偏好"在三层的自然流动
   - 不是配置同步，而是上下文的自然传递

3. **高可用保障**（愿景视角的基础设施属性）
   - 模型故障自动降级
   - 工具调用超时处理、会话崩溃恢复

### 6.3 长期（6-12 个月）：愿景视角主导

**目标**：从"好用的 AI 容器"升级为"个人 AI 的标准运行时"。

1. **涌现人格系统**
   - 基于 devbase 日志和活动记录，构建对用户的长期理解
   - 不是预设人格，不是 RAG 检索，而是统计性涌现

2. **本地推理能力**
   - 集成 ollama，为敏感场景提供本地模型选项
   - 小型任务使用本地模型，大型任务使用云端模型

3. **自动化整理**
   - 自动归档过期会话、提取关键决策、生成周/月回顾
   - devbase 日志自动分类、标签化、摘要

4. **气生根开放**
   - 将 Clarity 核心能力开放为 Rust 库
   - 第三方应用可接入 Clarity 的 Agent 层
   - 目标：Clarity 不仅是一个产品，更是一个 AI 交互的标准运行时

---

## 七、附录：竞品索引

| # | 项目 | 层级 | 技术栈 | 产品视角关系 | 愿景视角关系 |
|---|------|------|--------|-------------|-------------|
| 1 | **Clarity** | 交互基础设施 | Rust | 基准 | 基准 |
| 2 | claude-code-rust | 应用层 | Rust | 🔴 直接竞品（多入口标杆） | 🔴 应用层标杆 |
| 3 | 5ire | 桌面 AI 基础设施 | TS/Electron | 🔴 直接竞品（桌面+知识库） | 🔴 直接竞品（个人基础设施） |
| 4 | codex | 应用层 | TypeScript | 🟡 间接竞品（CLI 编码） | 🟡 接触面竞争者 |
| 5 | kimi-cli | 应用层 | TypeScript | 🟡 间接竞品（CLI 编码） | 🟡 接触面竞争者 |
| 6 | openclaw | 应用层 | Rust | 🟡 概念相关（claw 命名） | 🟢 低威胁 |
| 7 | openhanako | 应用层 | Rust | 🟡 概念相关 | 🟢 低威胁 |
| 8 | zeroclaw | 应用层 | Rust | 🟡 概念相关（claw 命名） | 🟢 低威胁 |
| 9 | OpenHands | 应用层 | Python | 🟢 赛道不同（替代工程师） | 🟢 赛道不同 |
| 10 | ollama | 模型基础设施 | Go | 🟢 互补 | 🟡 互补/潜在威胁 |
| 11 | dify | 应用开发基础设施 | TS/Python | 🟢 无关（to B） | 🟢 互补（to B vs to C） |
| 12 | rust-sdk (rmcp) | 协议基础设施 | Rust | 🟢 基础设施依赖 | 🟢 依赖/互补 |
| 13 | ratatui | 框架层 | Rust | 🟢 基础设施依赖 | 🟢 基础设施依赖 |
| 14 | AutoAgent | 框架层 | Python | 🟢 研究性质 | 🟢 研究性质 |
| 15 | EvoAgentX | 框架层 | Python | 🟢 研究性质 | 🟢 研究性质 |
| 16 | deer-flow | 框架层 | Rust | 🟢 工作流编排参考 | 🟢 工作流编排参考 |
| 17 | candle | ML 框架 | Rust | 🟢 未来参考 | 🟢 未来参考 |
| 18 | burn | ML 框架 | Rust | 🟢 未来参考 | 🟢 未来参考 |

---

*报告结束*
