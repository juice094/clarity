---
title: Clarity 文档中心
category: Index
date: 2026-06-13
tags: [index, moc]
---

# Clarity 文档中心 #moc

> 项目文档索引和导航 | 当前版本：**v0.3.0** | 最后更新：2026-06-13
> 本文档为 **Map of Content**，所有文档均带有 frontmatter 标签，支持 Obsidian Graph View 和标签筛选。

---

## 快速入口

| 按分类 | 标签 |
|--------|------|
| 架构文档 | `#architecture` |
| 决策记录 | `#adr` |
| 开发指南 | `#development` |
| 执行计划 | `#planning` |
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
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | 环境搭建、贡献流程、编码规范 | 5 分钟 |

---

## 开发指南

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`development/setup.md`](./development/setup.md) | 构建、运行、测试、验证命令 | 开发者 |
| [`development/provider-config.md`](./development/provider-config.md) | Provider/模型别名/`models.toml` 配置 | 开发者 |
| [`development/CODE-CHANGE-PRINCIPLES.md`](./development/CODE-CHANGE-PRINCIPLES.md) | 七条代码变更原则（P1~P7） | 开发者 |
| [`development/API_CONTRACT.md`](./development/API_CONTRACT.md) | Gateway HTTP/WebSocket API 契约 | 前后端开发者 |
| [`development/OPERATIONS.md`](./development/OPERATIONS.md) | 运维与部署注记 | 运维、维护者 |
| [`development/QUICK_REFERENCE.md`](./development/QUICK_REFERENCE.md) | 常用命令速查 | 开发者 |
| [`development/test_governance.md`](./development/test_governance.md) | 测试策略与纪律 | 开发者 |
| [`development/unwrap-debt-map.md`](./development/unwrap-debt-map.md) | unwrap 债务地图 | 维护者 |
| [`development/ci-cd.md`](./development/ci-cd.md) | CI/CD 与发布流水线 | 维护者 |
| [`development/AI_HANDOVER.md`](./development/AI_HANDOVER.md) | 跨 AI 会话交接规范 | Agent / AI 协作者 |

---

## 架构文档

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`ARCHITECTURE.md`](./ARCHITECTURE.md) | 代码级架构参考（crate 拓扑、模块边界、数据流） | 开发者、架构师 |
| [`architecture/tech-stack.md`](./architecture/tech-stack.md) | 技术栈与 crate 职责速查 | 开发者 |
| [`architecture/architecture-positioning.md`](./architecture/architecture-positioning.md) | **项目定位文档**（五层架构、竞品关系、Hard Veto） | 开发者、架构师 |
| [`architecture/channel_architecture.md`](./architecture/channel_architecture.md) | 三层运行时架构（claw/window/cli） | 开发者、架构师 |
| [`architecture/CORE_PURPOSE.md`](./architecture/CORE_PURPOSE.md) | 核心目的声明 | 开发者 |
| [`architecture/COUPLING_AUDIT.md`](./architecture/COUPLING_AUDIT.md) | 架构耦合审计 | 架构师 |
| [`architecture/MAP.md`](./architecture/MAP.md) | 能力地图 | 架构师 |
| [`architecture/pretext-ui-theory.md`](./architecture/pretext-ui-theory.md) | Pretext UI 设计理论 | 前端开发者 |
| [`architecture/renderline-pipeline.md`](./architecture/renderline-pipeline.md) | 行级渲染管线 | 前端开发者 |
| [`architecture/agent-state-machine.md`](./architecture/agent-state-machine.md) | Agent 状态机设计 | 开发者 |
| [`architecture/ai-protocol.md`](./architecture/ai-protocol.md) | AI 协议规范 | 开发者 |
| [`architecture/RISKS.md`](./architecture/RISKS.md) | 架构风险 | 架构师 |

---

## 规划与路线

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`planning/current-phase.md`](./planning/current-phase.md) | 当前阶段、Sprint 焦点、已知问题 | 全员 |
| [`planning/PROJECT_STATUS.md`](./planning/PROJECT_STATUS.md) | 项目现状报告（指标 + 功能清单 + 债务） | 全员 |
| [`planning/ROADMAP.md`](./planning/ROADMAP.md) | **统一路线图**（策略 + 详细功能规划，当前 v0.4.0） | 决策者、开发者 |
| [`planning/FUTURE_DIRECTION.md`](./planning/FUTURE_DIRECTION.md) | 长期技术路线图 Phase A→D（注意其时效注） | 开发者、架构师 |
| [`planning/CLARITY_CLAUDE_ALIGNMENT_ROADMAP.md`](./planning/CLARITY_CLAUDE_ALIGNMENT_ROADMAP.md) | Claude Code 能力对齐路线图（含进度快照） | 开发者 |
| [`planning/CLARITY_MODULE_RESEARCH.md`](./planning/CLARITY_MODULE_RESEARCH.md) | 模块解构与工程路线对照（2026-06 快照，注意时效注） | 开发者 |
| [`planning/claw-mesh-phase3-design.md`](./planning/claw-mesh-phase3-design.md) | Claw Mesh 分布式上下文同步设计（骨架已落地） | 架构师 |
| [`planning/optimization-plan-2026-07-06.md`](./planning/optimization-plan-2026-07-06.md) | 前端优化批次计划 | 开发者 |
| [`planning/architecture-audit-2026-07-06.md`](./planning/architecture-audit-2026-07-06.md) | 前端架构审计报告（5 项改造已落地） | 架构师 |
| [`planning/sprint-archive.md`](./planning/sprint-archive.md) | 历史 Sprint 摘要 | 维护者 |
| [`planning/methodology-shape-up.md`](./planning/methodology-shape-up.md) | 工程方法论速查 | 规划者 |
| [`planning/plans/`](./planning/plans/) | 概念参考文档 | 开发者 |
| [`planning/archive/`](./planning/archive/) | 已归档历史规划（BACKLOG、长期路线、已完成 sprint 计划等） | 维护者 |

---

## 安全与审计

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`security/operations.md`](./security/operations.md) | 安全与运维细则 | 开发者、运维 |
| [`security/THREAT_MODEL.md`](./security/THREAT_MODEL.md) | STRIDE 威胁模型 | 安全负责人 |
| [`security/risk-assessment.md`](./security/risk-assessment.md) | 技术风险评估 | 架构师 |
| [`security/PRIVACY_REVIEW.md`](./security/PRIVACY_REVIEW.md) | 隐私整改记录 | 维护者 |
| [`audits/`](./audits/) | 各类审计报告 | 维护者 |

---

## GUI 与 UI 设计

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`ui_design_theory.md`](./ui-design/ui_design_theory.md) | UI 设计理论 | 前端开发者 |
| [`ui-audit-rebuttal-20260509.md`](./ui-design/ui-audit-rebuttal-20260509.md) | UI 审计与修复 | 前端开发者 |
| [`ui-design/plan-panel-design-draft.md`](./ui-design/plan-panel-design-draft.md) | Plan 面板设计稿 | 前端开发者 |
| [`ui-design/snapshot-panel-design-draft.md`](./ui-design/snapshot-panel-design-draft.md) | Snapshot 面板设计稿 | 前端开发者 |
| [`research/UI_COMPARISON.md`](./research/UI_COMPARISON.md) | UI 竞品对比 | 前端开发者 |
| [`research/EGUI_AESTHETICS_INFO.md`](./research/EGUI_AESTHETICS_INFO.md) | egui 美学参考 | 前端开发者 |

---

## 研究、比较与参考

| 文档 | 说明 | 目标读者 |
|------|------|----------|
| [`comparisons/OPENCLAW_GAP_ANALYSIS.md`](./comparisons/OPENCLAW_GAP_ANALYSIS.md) | OpenClaw 差距分析 | 决策者 |
| [`comparisons/competitive-analysis.md`](./comparisons/competitive-analysis.md) | 竞品分析 | 决策者 |
| [`comparisons/KIMI_CLI_COMPARISON.md`](./comparisons/KIMI_CLI_COMPARISON.md) | Kimi CLI 对比 | 决策者 |
| [`research/`](./research/) | 研究参考 | 开发者 |
| [`references/`](./references/) | 深度参考 | 开发者 |
| [`adr/`](./adr/) | 架构决策记录（ADR-001 ~ ADR-018） | 开发者、架构师 |
| [`notes/`](./notes/) | 会议纪要、审计、临时笔记 | 维护者 |

---

## 文档维护契约

<!-- DOC-CONTRACT: 本文档只维护索引和链接。不维护功能详情、架构细节或历史变更——这些参见对应的目标文档。 -->

| 信息类型 | 权威来源 |
|----------|----------|
| 代码级架构 | [`ARCHITECTURE.md`](./ARCHITECTURE.md) |
| 技术栈 | [`architecture/tech-stack.md`](./architecture/tech-stack.md) |
| 项目定位与 Hard Veto | [`architecture/architecture-positioning.md`](./architecture/architecture-positioning.md) |
| 版本变更历史 | [`../CHANGELOG.md`](../CHANGELOG.md) |
| 当前阶段 | [`planning/current-phase.md`](./planning/current-phase.md) |
| 未来规划 | [`planning/ROADMAP.md`](./planning/ROADMAP.md) |
| 开发环境 | [`../AGENTS.md`](../AGENTS.md) + [`development/setup.md`](./development/setup.md) |
| 测试数字 | [`../README.md`](../README.md) + [`development/setup.md`](./development/setup.md) |

---

**有疑问？** 请查阅 [`../AGENTS.md`](../AGENTS.md) 获取最新开发指南，或查看 [`../CHANGELOG.md`](../CHANGELOG.md) 获取版本详情。
