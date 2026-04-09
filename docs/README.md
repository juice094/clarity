# Clarity 文档中心

> 项目文档索引和导航

---

## 📋 新用户必读

| 文档 | 说明 | 阅读时间 |
|------|------|----------|
| [`../README.md`](../README.md) | 项目简介和快速开始 | 5 分钟 |
| [`EXECUTIVE_SUMMARY.md`](./EXECUTIVE_SUMMARY.md) | 决策者摘要 | 3 分钟 |

---

## 🔍 技术文档

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`../PROJECT_REPORT.md`](../PROJECT_REPORT.md) | 完整技术验证报告（实机测试版） | 开发者、技术负责人 |
| [`mcp_integration_guide.md`](./mcp_integration_guide.md) | MCP 协议集成指南 | 开发者 |
| [`llm_provider_refactor.md`](./llm_provider_refactor.md) | LLM Provider 重构说明 | 开发者 |
| [`tools_roadmap.md`](./tools_roadmap.md) | 工具系统路线图 | 开发者 |

---

## 🧪 测试与验证

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`TEST_PLAN.md`](./TEST_PLAN.md) | 详细实测计划（10 项测试） | QA、开发者 |
| [`test_governance.md`](./test_governance.md) | 测试治理规范 | 团队 |

---

## 🗺️ 规划与路线

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`ROADMAP_ANALYSIS.md`](./ROADMAP_ANALYSIS.md) | 推进路线分析和选项对比 | 决策者、PM |
| [`THIRD_PARTY_INTEGRATION_ROADMAP.md`](./THIRD_PARTY_INTEGRATION_ROADMAP.md) | 第三方集成路线图（MCP、devbase、syncthing） | 决策者、PM、架构师 |
| [`governance_report_20260403.md`](./governance_report_20260403.md) | 项目治理报告 | 团队 |

---

## 📦 归档文档（历史参考）

旧版文档已移至 [`../archive/`](../archive/)，保留作为历史参考：

| 文件 | 原作用 | 归档原因 |
|------|--------|----------|
| `AI_HANDOFF.md` | AI 交接文档 | 完成度数字与代码不符 |
| `ARCHITECTURE.md` | 架构设计 | 含未实现组件 |
| `DEV_LOG.md` | 开发日志 | 代码量夸大 |
| `HUMAN_GUIDE.md` | 用户手册 | 基于过时假设 |
| `SESSION_SUMMARY.md` | 会话总结 | 描述不准确 |
| `PROJECT_REPORT_20260403.md` | 旧报告 | 未实机验证 |

---

## 📊 项目状态速览（2026-04-09 更新）

```
当前阶段: Phase 3 实测 → Phase 4 集成扩展
代码状态: ✅ 可编译，~380+ 测试通过，3 个警告（未使用变量/可变修饰符）
代码规模: ~750 KB，91 个 Rust 源文件
待测功能: TUI 真实 LLM, Gateway 渠道（Discord/Telegram）, MCP 真实 server 联调
关键缺失: BackgroundTaskManager Gateway/TUI 集成，MCP `mcp.json` 配置支持
推荐路线: 保守路线（先实测验证，再渐进扩展）
已完成: 子代理 Runner ✅, PersistentMemoryStore 真实实现 ✅, Gateway WebSocket ✅
```

---

## 🌐 生态定位（2026-04-05）

Clarity 是用户本地 Agent 执行栈的**应用层**，专注于 LLM reasoning、工具调用（MCP）和动作执行。

与两个相关项目的区分与关系：

- **`devbase`**（开发者知识库管理器）：位于**抽象层**，负责把用户桌面上的 Git 仓库、编译器版本、环境健康状态结构化为可查询的知识库。成熟期后，Clarity 将通过 MCP 接口调用 `devbase` 获取环境上下文。
- **`syncthing-rust-rearch`**（P2P 同步引擎）：位于**实体层**，负责跨设备的块级文件同步。成熟期后，Clarity 可能通过配置接口告知 syncthing 哪些目录需要被同步。

> **当前阶段声明**：三者独立开发，融合尚未成熟。Clarity 先完善自身的 Agent 执行框架，待 `devbase` 和 `syncthing-rust-rearch` 的接口稳定后再进行协议级对接。

---

## 🔗 快速链接

### 代码
- 核心 Agent: `crates/clarity-core/src/agent.rs`
- 工具注册表: `crates/clarity-core/src/registry.rs`
- MCP Client: `crates/clarity-core/src/mcp.rs`
- 记忆系统: `crates/clarity-memory/src/`

### 运行
```powershell
# TUI
cargo run -p clarity-tui

# Gateway
cargo run -p clarity-gateway

# 测试
cargo test --workspace
```

---

## 📝 文档更新记录

| 日期 | 更新内容 |
|------|----------|
| 2026-04-09 | 更新文档索引，新增第三方集成路线图，更新项目状态速览 |
| 2026-04-04 | 根据实际代码状态更新 README 和 PROJECT_REPORT |
| 2026-04-03 | 新增实机验证报告、测试计划、路线分析 |
| 2026-04-03 | 归档旧版夸大文档 |
| 2026-04-02 | 初始文档创建 |

---

**有疑问？** 请查阅 [`EXECUTIVE_SUMMARY.md`](./EXECUTIVE_SUMMARY.md) 获取高层概述，或查看 [`PROJECT_REPORT.md`](../PROJECT_REPORT.md) 获取技术详情。
