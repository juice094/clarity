---
title: Clarity 文档中心
category: Index
date: 2026-05-16
tags: [index, moc]
---

# Clarity 文档中心 #moc

> 项目文档索引和导航 | 当前版本：**v0.3.0** | 最后更新：2026-05-11
> 本文档为 **Map of Content**，所有文档均带有 frontmatter 标签，支持 Obsidian Graph View 和标签筛选。

---

## 快速入口

| 按分类 | 标签 |
|--------|------|
| 架构文档 | `#architecture` |
| 决策记录 | `#adr` |
| 执行计划 | `#plan` |
| 安全审计 | `#security` `#audit` |
| 研究参考 | `#research` `#reference` |
| UI 设计 | `#ui` `#ui-design` |
| 路线图 | `#roadmap` |

---

## 新用户必读

| 文档 | 说明 | 阅读时间 |
|------|------|----------|
| [`../README.md`](../README.md) | 项目简介、核心差异化、快速开始 | 3 分钟 |
| [`../CHANGELOG.md`](../CHANGELOG.md) | 版本变更日志 | 3 分钟 |
| [`../AGENTS.md`](../AGENTS.md) | Agent 开发指南、环境变量、架构注记 | 5 分钟 |

---

## 技术文档

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`ARCHITECTURE.md`](./ARCHITECTURE.md) | 代码级架构参考（crate 拓扑、模块边界、数据流） | 开发者、架构师 |
| [`architecture-positioning.md`](./architecture-positioning.md) | **项目定位文档**（五层架构、竞品关系、Hard Veto） | 开发者、架构师 |
| [`mcp_integration_guide.md`](./mcp_integration_guide.md) | MCP 协议集成指南（stdio/HTTP/SSE 三协议） | 开发者 |
| [`channel_architecture.md`](./channel_architecture.md) | 三层运行时架构（claw/window/cli） | 开发者、架构师 |
| [`llm_provider_refactor.md`](./llm_provider_refactor.md) | LLM Provider 重构说明 | 开发者 |
| [`tools_roadmap.md`](./tools_roadmap.md) | 工具系统路线图 | 开发者 |
| [`skill-mcp-protocol-relationship.md`](./skill-mcp-protocol-relationship.md) | Skill 与 MCP 协议关系 | 开发者 |
| [`tool-capability-layers.md`](./tool-capability-layers.md) | 工具能力分层设计 | 开发者 |
| [`agent-state-machine.md`](./agent-state-machine.md) | Agent 状态机设计 | 开发者 |
| [`rag-research.md`](./rag-research.md) | RAG 检索增强生成研究 | 开发者 |

---

## GUI 桌面端文档

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`tech_stack_decision_ui.md`](./tech_stack_decision_ui.md) | Tauri 2 + React 技术栈选型 | 前端开发者 |
| [`ui_design_theory.md`](./ui_design_theory.md) | UI 设计理论 | 前端开发者 |

---

## 安全与风险

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`risk-assessment.md`](./risk-assessment.md) | 技术风险评估与缓解 | 架构师、安全负责人 |
| [`competitive-analysis.md`](./competitive-analysis.md) | 竞品分析 | 决策者、PM |
| [`comparisons/OPENCLAW_GAP_ANALYSIS.md`](./comparisons/OPENCLAW_GAP_ANALYSIS.md) | OpenClaw 功能差距分析 | 决策者 |

---

## 规划与路线

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`ROADMAP.md`](./ROADMAP.md) | **统一路线图**（策略 + 详细功能规划） | 决策者、开发者 |
| [`THIRD_PARTY_INTEGRATION_ROADMAP.md`](./THIRD_PARTY_INTEGRATION_ROADMAP.md) | 第三方集成路线图 | 决策者、架构师 |
| [`SUBAGENT_PARALLEL_ANALYSIS.md`](./SUBAGENT_PARALLEL_ANALYSIS.md) | 子代理并行执行分析 | 开发者 |
| [`FUTURE_DIRECTION.md`](./FUTURE_DIRECTION.md) | 长期技术路线图 Phase A→D | 开发者、架构师 |
| [`PROJECT_STATUS.md`](./PROJECT_STATUS.md) | 项目现状速览（指标 + 功能清单） | 全员 |
| [`methodology-shape-up.md`](./methodology-shape-up.md) | 工程方法论速查（Cynefin/TOC/Shape Up） | 规划者 |
| [`server_channel_design_analysis.md`](./server_channel_design_analysis.md) | 服务端 Channel 设计分析 | 开发者 |

---

## 项目状态速览

```
当前阶段: v0.3.0 — 本地优先标杆已发布，进入集群语义验证准备
代码状态: ✅ 可编译，测试全绿，0 个警告
cargo doc: ✅ 零 warning
代码规模: ~130 个 Rust 源文件，13 个 crates（不含已归档的 clarity-tauri）
CI 状态:   check + test + clippy + fmt + audit + coverage (7 jobs)
安全状态: cargo audit 11 个 unmaintained（上游间接依赖），已标记允许；0 high/critical

核心能力:
  ✅ Plan Mode — 结构化计划 + 批量执行
  ✅ 并行子代理 — run_parallel() + Gateway API + TUI /parallel
  ✅ 后台任务 — BackgroundTaskManager + claw 托盘通知
  ✅ MCP 生态 — stdio / HTTP / SSE 三协议完整实现
  ✅ Memory — SQLite + BM25 + 向量混合搜索
  ✅ Skills — Markdown+YAML 技能系统
  ✅ 本地 LLM — Candle 原生 GGUF 推理 (Qwen2/DeepSeek-R1-Distill)
  ✅ 三层运行时 — claw / window / cli
  ✅ Gateway — REST API + WebSocket + Session Store
  ✅ GUI 桌面端 — Chat / Session / Task / Settings / FileBrowser / Diff / ComputerUse / LSP / Onboarding / LogPanel
  ✅ 主题系统 — Dark / Light / Auto
  ✅ 审批系统 — Interactive / Yolo / Plan + T_APPROVAL V1 规则引擎
  ✅ FTUE — 首次启动引导 + 模型下载
  ✅ 自动更新检查 — GitHub Release API
  ✅ 本地构建 — `.msi` / `.exe` / `.nsis`
```

---

## 生态定位

Clarity 是 **"personal AI standard runtime"** — 个人 AI 的标准运行时。

与相关项目的区分：

- **`devbase`**（开发者知识库管理器）：位于**抽象层**。成熟期后，Clarity 将通过 MCP 接口调用 devbase 获取环境上下文。
- **`syncthing-rust-rearch`**（P2P 同步引擎）：位于**实体层**。未来可能负责跨设备文件同步（当前无代码集成）。
- **cc-haha / OpenClaw**（Node.js 个人 AI 助手）：应用层竞品。Clarity 在性能（Rust）、内置工具丰富度、单进程桌面架构和本地 LLM 推理上有优势；cc-haha 在 IM 深度集成和语音交互上领先。

---

## 快速链接

### 代码
- 核心 Agent: `crates/clarity-core/src/agent/`
- 工具注册表: `crates/clarity-core/src/registry.rs`
- MCP Client: `crates/clarity-mcp/src/`
- 记忆系统: `crates/clarity-memory/src/`
- Gateway: `crates/clarity-gateway/src/`
- 桌面 GUI: `crates/clarity-egui/src/`

### 运行
```powershell
# TUI
cargo run -p clarity-tui

# Gateway
cargo run -p clarity-gateway

# 系统托盘（claw）
cargo run -p clarity-claw

# 桌面 GUI（egui，零 Node.js 依赖）
cargo run -p clarity-egui

# 测试
cargo test --workspace --lib
```

---

## 文档维护契约

<!-- DOC-CONTRACT: 本文档只维护索引和链接。不维护功能详情、架构细节或历史变更——这些参见对应的目标文档。 -->

| 信息类型 | 权威来源 |
|----------|----------|
| 代码级架构 | [`ARCHITECTURE.md`](./ARCHITECTURE.md) |
| 项目定位与 Hard Veto | [`architecture-positioning.md`](./architecture-positioning.md) |
| 版本变更历史 | [`../CHANGELOG.md`](../CHANGELOG.md) |
| 未来规划 | [`ROADMAP.md`](./ROADMAP.md) / [`FUTURE_DIRECTION.md`](./FUTURE_DIRECTION.md) |
| 开发环境 | [`../AGENTS.md`](../AGENTS.md) |
| 测试数字 | [`../README.md`](../README.md) |

---

**有疑问？** 请查阅 [`../AGENTS.md`](../AGENTS.md) 获取最新开发指南，或查看 [`../CHANGELOG.md`](../CHANGELOG.md) 获取版本详情。
