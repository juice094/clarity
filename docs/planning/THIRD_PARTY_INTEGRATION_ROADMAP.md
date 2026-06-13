---
title: Third-Party Integration Roadmap
category: Roadmap
date: 2026-05-16
tags: [roadmap, planning]
---

# Third-Party Integration Roadmap

> 本文档描述 Clarity 与外部项目的关系、集成计划及决策依据。
> 
> 最后更新：2026-04-09

---

## 1. Overview

Clarity 是一个本地优先的 Rust AI Agent 执行框架。它的发展离不开与外部项目和协议的协作：

| 外部项目/协议 | 关系定位 | 当前状态 |
|---------------|----------|----------|
| **Kimi CLI** | 主要架构参考 | 已深入分析，持续借鉴其子代理、后台任务、MCP 等设计 |
| **MCP (Model Context Protocol)** | 协议生态扩展机制 | 骨架实现完成，正在推进 `mcp.json` 配置支持与真实 server 联调 |
| **devbase** | 未来环境上下文伙伴 | 独立开发中，成熟期通过 MCP 对接 |
| **syncthing-rust-rearch** | 未来同步配置伙伴 | 独立开发中，成熟期通过配置接口对接 |

Clarity 的核心理念是：**先让自身执行框架完整、稳定，再以开放协议（MCP 为主）与外部系统对接**，避免过早耦合。

---

## 2. Short-term (1-4 weeks)

### 2.1 MCP Configuration Support (`mcp.json`)
- 支持从工作区 `.clarity/mcp.json` 或用户级配置加载 MCP server 列表
- 支持 `stdio` 和 `sse` transport 配置
- 实现 MCP tool 的动态注册与卸载
- **参考**: `docs/mcp_integration_guide.md`

### 2.2 BackgroundTaskManager Implementation
- 当前骨架已实现（`crates/clarity-core/src/background/mod.rs`）
- 待完成：与 Gateway / TUI 的集成、Wire File 持久化通信、任务恢复
- 增加端到端测试验证后台任务生命周期
- **参考**: Kimi CLI `background/manager.py`

### 2.3 Real MCP Server Testing
- 使用官方 `filesystem` MCP server 进行联调
- 使用 `git` MCP server 验证工具调用链路
- 验证审批流在 MCP tool 调用上的正确性

---

## 3. Medium-term (1-3 months)

### 3.1 Git Context Propagation in Subagents
- 在子代理执行时自动收集并传递 Git 上下文（分支、最近提交、未提交变更）
- 支持通过 `SubagentSpec` 控制是否携带 Git 上下文
- **参考**: Kimi CLI `subagents/git_context.py`

### 3.2 Tool Security Enhancements
- 敏感文件检测（`is_sensitive_file`）：阻止对 `.env`、SSH 密钥、钱包等文件的危险操作
- 媒体文件嗅探（`MEDIA_SNIFF_BYTES`）：在读取文件前自动识别二进制/媒体格式
- 工具沙箱路径限制（可选）：限制文件操作的作用域

### 3.3 Wire File Persistence for Background Tasks
- 实现后台任务与前台 UI 的跨进程/跨会话通信
- 基于文件或本地 socket 的 Wire transport
- 支持任务恢复后的历史消息重放

---

## 4. Long-term (3-6 months)

### 4.1 Integration with `devbase` via MCP
- `devbase` 负责将用户桌面上的 Git 仓库、编译器版本、环境健康状态结构化为知识库
- Clarity 通过 MCP 调用 `devbase` 获取环境上下文，增强 Agent 的推理能力

### 4.2 Integration with `syncthing-rust-rearch`
- `syncthing-rust-rearch` 负责跨设备的块级文件同步
- Clarity 通过配置接口告知 syncthing 哪些工作目录需要同步
- 实现 Agent 配置和记忆数据的跨设备一致性

### 4.3 Potential Web UI / IDE Extension Exploration
- 评估基于 Gateway HTTP API 构建独立 Web UI 的可行性
- 评估 VS Code / IDE 扩展的原型方案（非核心优先级）

---

## 5. Decision Log

### 5.1 Why Kimi CLI was chosen as the primary reference

- **Proven at scale**: Kimi CLI 由 Moonshot AI 团队维护，经过大规模用户验证
- **Open source**: 代码结构清晰，可直接参考实现细节
- **Feature-complete**: 子代理、后台任务、MCP、Wire 协议等关键功能均已验证
- **Architecturally aligned**: 其 soul → tools → protocol 的分层与 Clarity 的设计理念高度契合

### 5.2 Why OpenHanako was chosen as the memory model reference

- **4-level compilation**: OpenHanako 提供了从原始对话 → 事实提取 → 主题聚合 → 身份编译的清晰分层
- 该模型与 `clarity-memory` 的 `Fact → Compilation → Identity` 结构天然匹配
- 为长期记忆的语义压缩提供了可落地的理论框架

### 5.3 Why MCP is the preferred extension mechanism over custom plugins

- **协议标准化**: MCP 正在成为 AI 工具与外部系统对接的事实标准
- **生态兼容性**: 支持 MCP 即可接入不断增长的 server 生态（文件系统、数据库、浏览器等）
- **降低维护成本**: 无需为每个外部系统设计自定义插件接口
- **安全可控**: MCP server 作为独立进程运行，天然具备隔离性

---

## 6. Related Documents

- [`KIMI_CLI_COMPARISON.md`](../comparisons/KIMI_CLI_COMPARISON.md) — 与 Kimi CLI 的详细横向对比
- [`mcp_integration_guide.md`](../development/mcp_integration_guide.md) — MCP 协议集成指南
- `ROADMAP_ANALYSIS.md` — 推进路线分析和选项对比
- [`../README.md`](../README.md) — 项目简介与快速开始
