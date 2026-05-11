# Clarity Architecture — Pointer

> 顶层 `ARCHITECTURE.md` 不再独立维护内容，已拆分为两份**单一职责**的文档。
> 上次拆分：2026-05-11（按 §9 架构健康纪律）。

---

## 你要找的内容在哪里？

| 想了解 | 去看 | 说明 |
|--------|------|------|
| **代码级 crate 拓扑 / 模块边界 / 数据流** | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | 代码级精确架构参考，跟随源码演进 |
| **项目定位 / 与 Kimi CLI · ZeroClaw · OpenClaw · devbase 的关系 / Hard Veto** | [`docs/architecture-positioning.md`](docs/architecture-positioning.md) | 五层架构 + 项目角色矩阵 + 演进路线图（阶段维度） |
| **当前 Sprint 状态 / 测试基线 / 已知问题** | [`AGENTS.md`](AGENTS.md) §Current Phase | Agent 开发上下文 |
| **版本变更与迁移说明** | [`CHANGELOG.md`](CHANGELOG.md) | 用户视角的变更日志 |
| **历史 Sprint 摘要（17 及更早）** | [`docs/sprint-archive.md`](docs/sprint-archive.md) | 归档查阅 |
| **产品交付节奏与版本路线** | [`docs/ROADMAP.md`](docs/ROADMAP.md) | 阶段一→二→三 |

---

## 为什么拆？

**单源真相**：以前同一份 `ARCHITECTURE.md` 同时承担"代码级技术参考""项目层级定位""当前 Sprint 状态"三种职责，导致：

- 文档与代码不同步（README/AGENTS 测试数字滞后）
- 上下文压缩后 Agent 对外部项目关系产生混乱
- 单一文件难以维护

按 §9 架构健康纪律的"分发即耦合"原则，每份文档只承担一种职责。
