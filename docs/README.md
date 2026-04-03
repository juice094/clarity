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

## 📊 项目状态速览（2026-04-04 更新）

```
当前阶段: Phase 2 完成 → Phase 3 实测
代码状态: ✅ 可编译，180+ 测试通过，3 个警告（未使用函数）
代码规模: ~650 KB，68 个 Rust 源文件
待测功能: TUI 真实 LLM, Gateway E2E, MCP 联调, 记忆闭环
关键缺失: PersistentMemoryStore 真实实现
推荐路线: 保守路线（先实测验证，再渐进扩展）
已完成: 子代理 Runner ✅
```

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
| 2026-04-04 | 根据实际代码状态更新 README 和 PROJECT_REPORT |
| 2026-04-03 | 新增实机验证报告、测试计划、路线分析 |
| 2026-04-03 | 归档旧版夸大文档 |
| 2026-04-02 | 初始文档创建 |

---

**有疑问？** 请查阅 [`EXECUTIVE_SUMMARY.md`](./EXECUTIVE_SUMMARY.md) 获取高层概述，或查看 [`PROJECT_REPORT.md`](../PROJECT_REPORT.md) 获取技术详情。
