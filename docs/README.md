# Clarity 文档中心

> 项目文档索引和导航 | 当前版本：**v0.1.1** | 最后更新：2026-04-23

---

## 📋 新用户必读

| 文档 | 说明 | 阅读时间 |
|------|------|----------|
| [`../README.md`](../README.md) | 项目简介和快速开始 | 5 分钟 |
| [`../CHANGELOG.md`](../CHANGELOG.md) | 版本变更日志 | 3 分钟 |
| [`PROJECT_STATUS.md`](./PROJECT_STATUS.md) | 项目现状完整报告 | 10 分钟 |

---

## 🔍 技术文档

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`mcp_integration_guide.md`](./mcp_integration_guide.md) | MCP 协议集成指南（stdio/HTTP/SSE 三协议） | 开发者 |
| [`channel_architecture.md`](./channel_architecture.md) | 三层运行时架构（claw/window/cli） | 开发者、架构师 |
| [`llm_provider_refactor.md`](./llm_provider_refactor.md) | LLM Provider 重构说明 | 开发者 |
| [`tools_roadmap.md`](./tools_roadmap.md) | 工具系统路线图 | 开发者 |
| [`skill-mcp-protocol-relationship.md`](./skill-mcp-protocol-relationship.md) | Skill 与 MCP 协议关系 | 开发者 |
| [`tool-capability-layers.md`](./tool-capability-layers.md) | 工具能力分层设计 | 开发者 |

---

## 🛡️ 安全与风险

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`risk-assessment.md`](./risk-assessment.md) | 技术风险评估与缓解 | 架构师、安全负责人 |
| [`competitive-analysis.md`](./competitive-analysis.md) | 竞品分析（third_party 扫描） | 决策者、PM |

---

## 🗺️ 规划与路线

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`THIRD_PARTY_INTEGRATION_ROADMAP.md`](./THIRD_PARTY_INTEGRATION_ROADMAP.md) | 第三方集成路线图（devbase、syncthing） | 决策者、架构师 |
| [`SUBAGENT_PARALLEL_ANALYSIS.md`](./SUBAGENT_PARALLEL_ANALYSIS.md) | 子代理并行执行分析 | 开发者 |
| [`PARALLEL_IMPLEMENTATION_PLAN.md`](./PARALLEL_IMPLEMENTATION_PLAN.md) | 并行实现计划 | 开发者 |

---

## 📊 项目状态速览（v0.1.1 — 2026-04-23）

```
当前阶段: v0.1.1 安全加固 + 基础设施补全
代码状态: ✅ 可编译，382+ 测试通过，0 个警告
代码规模: ~122 个 Rust 源文件，6 个 crates
测试基线: cargo test --workspace --lib = 382 passed, 0 failed, 2 ignored
CI 状态:  check + test + clippy + fmt + audit + coverage (6 jobs × 3 OS)
安全状态: 2 个目录遍历漏洞已修复（S1/S2），cargo audit 已集成

核心能力:
  ✅ Plan Mode — 结构化计划 + 批量执行
  ✅ 并行子代理 — run_parallel() + Gateway API + TUI /parallel
  ✅ 后台任务 — BackgroundTaskManager + claw 托盘通知
  ✅ MCP 生态 — stdio / HTTP / SSE 三协议完整实现
  ✅ Memory — SQLite + BM25 混合搜索
  ✅ Skills — Markdown+YAML 技能系统
  ✅ 三层运行时 — claw（托盘）/ window（Web IDE）/ cli（TUI）
  ✅ Gateway — REST API + WebSocket + Session Store
```

---

## 🌐 生态定位

Clarity 是 **"personal AI standard runtime"** — 个人 AI 的标准运行时。

与相关项目的区分：

- **`devbase`**（开发者知识库管理器）：位于**抽象层**。成熟期后，Clarity 将通过 MCP 接口调用 devbase 获取环境上下文。
- **`syncthing-rust-rearch`**（P2P 同步引擎）：位于**实体层**。负责跨设备文件同步。
- **OpenClaw**（Node.js 个人 AI 助手）：应用层竞品。Clarity 在开发者场景中具备性能优势（Rust），但 Channels（消息平台）和 Voice 是长期差距。

---

## 🔗 快速链接

### 代码
- 核心 Agent: `crates/clarity-core/src/agent/`
- 工具注册表: `crates/clarity-core/src/registry.rs`
- MCP Client: `crates/clarity-core/src/mcp/`
- 记忆系统: `crates/clarity-memory/src/`
- Gateway: `crates/clarity-gateway/src/`

### 运行
```powershell
# TUI
cargo run -p clarity-tui

# Gateway
cargo run -p clarity-gateway

# 系统托盘（claw）
cargo run -p clarity-claw

# 测试
cargo test --workspace --lib
```

---

## 📝 文档更新记录

| 日期 | 更新内容 |
|------|----------|
| 2026-04-23 | v0.1.1：归档过时文档，重写索引，新增 PROJECT_STATUS.md |
| 2026-04-09 | 更新文档索引，新增第三方集成路线图 |
| 2026-04-04 | 根据实际代码状态更新 README 和 PROJECT_REPORT |
| 2026-04-03 | 新增实机验证报告、测试计划、路线分析 |
| 2026-04-03 | 归档旧版夸大文档 |
| 2026-04-02 | 初始文档创建 |

---

## 📦 归档文档

历史文档已移至 [`archive/`](./archive/)，保留作为历史参考：

| 文件 | 原作用 | 归档原因 |
|------|--------|----------|
| `EXECUTIVE_SUMMARY_2026-04-04.md` | 决策者摘要 | 数据严重过时（180+ tests → 382） |
| `ROADMAP_ANALYSIS_2026-04-03.md` | 路线分析 | 技术债务已全部解决 |
| `TEST_PLAN_2026-04-03.md` | 实测计划 | 状态从"待执行"变为"已完成" |
| `DEVELOPMENT_TRACKS_2026-04-03.md` | 开发轨道 | 已合并至 CHANGELOG |
| `PHASE_REPORT_2026-04-09.md` | 阶段报告 | 测试数据过期 |
| `REALITY_CHECK_AND_ROADMAP_2026-04-15.md` | 可靠性分析 | 测试数据过期 |
| `AI_HANDOFF.md` | AI 交接文档 | 完成度数字与代码不符 |
| `ARCHITECTURE.md` | 架构设计 | 含未实现组件 |
| `DEV_LOG.md` | 开发日志 | 代码量夸大 |
| `HUMAN_GUIDE.md` | 用户手册 | 基于过时假设 |
| `SESSION_SUMMARY.md` | 会话总结 | 描述不准确 |
| `PROJECT_REPORT_20260403.md` | 旧报告 | 未实机验证 |

---

**有疑问？** 请查阅 [`PROJECT_STATUS.md`](./PROJECT_STATUS.md) 获取最新现状，或查看 [`../CHANGELOG.md`](../CHANGELOG.md) 获取版本详情。
