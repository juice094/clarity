# Clarity vs Kimi CLI 快速参考

> 最后更新：2026-05-03

---

## 📊 功能对比速查表

| 功能 | Clarity | Kimi CLI | 优先级 | 状态 |
|------|---------|----------|--------|------|
| **Agent 核心** | ✅ ReAct | ✅ Soul | - | 生产就绪 |
| **子代理 Runner** | ✅ 已实现 | ✅ | P0 ✅ | **已完成** |
| **后台任务** | ✅ BackgroundTaskManager | ✅ | P0 ✅ | **已完成** |
| **持久化记忆** | ✅ SQLite + FTS5 | ✅ | P0 ✅ | **已完成** |
| **MCP 支持** | ✅ stdio/HTTP + auto-loading | ✅ | P1 ✅ | SSE stub |
| **并行子代理** | ✅ SubagentBatch + ParallelRunner | ✅ | P1 ✅ | **已完成** |
| **E2E 测试** | ✅ 256+ tests passing | ✅ | P1 ✅ | **已完成** |
| **Web UI** | ✅ Gateway 内嵌 chat.html | ✅ | P2 ✅ | **已完成** |
| **VS Code 扩展** | ❌ | ✅ | P3 | 暂缓 |

---

## 📈 实现进度

```
子代理系统     ████████████████████ 100% ✅ 已完成
后台任务       ████████████████████ 100% ✅ 已完成
持久化记忆     ████████████████████ 100% ✅ 已完成
MCP 支持       █████████████████░░░ 85%  ✅ stdio/HTTP 可用，SSE stub
E2E 测试       ████████████████████ 100% ✅ 已完成
Web UI         ████████████████████ 100% ✅ 已完成
```

---

## 🔗 关键文档索引

| 文档 | 说明 | 路径 |
|------|------|------|
| 详细对比分析 | 完整的功能对比和借鉴建议 | `KIMI_CLI_COMPARISON.md` |
| MCP 集成指南 | MCP 设计、配置与接入 | `mcp_integration_guide.md` |
| 通道架构 | Gateway 多渠道架构 | `channel_architecture.md` |
| 项目总体报告 | 当前状态、roadmap、已知问题 | `../PROJECT_REPORT.md` |
| Agent 指引 | 开发约定、最近改动、已知问题 | `../AGENTS.md` |

---

## 🛡️ 安全速查

- MCP stdio 命令已启用默认白名单校验。
- 如需放宽限制，设置环境变量：`CLARITY_MCP_ALLOWLIST="/usr/bin/npx,/opt/bin"`
