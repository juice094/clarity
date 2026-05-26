---
title: 风险点与待优化项登记簿
category: Architecture
date: 2026-05-16
tags: [architecture]
---

# 风险点与待优化项登记簿

> 性质：人工维护清单。非自动生成，需 PM 定期审计更新。
> 更新频率：建议每周日审查一次。

---

## 一、已确认风险（有证据）

| ID | 风险描述 | 证据 | 严重程度 | 缓解状态 |
|----|---------|------|---------|---------|
| R-01 | **状态混淆**：AI 将规划意图陈述为完成事实 | SESSION_SUMMARY.md 审计记录 | 🔴 高 | 未缓解 |
| R-02 | **精确性幻觉**：AI 混合虚构数字与真实锚点 | 577 tests 声称 vs README 524 记录 | 🔴 高 | 已缓解（SNAPSHOT.md 硬事实化） |
| R-03 | **前端契约绕过**：AI 可能直接调用后端内部 API | `AppState` 同时持有 `InMemoryApprovalRuntime` 和 `ModeAwareApprovalRuntime` 的历史 bug | 🟡 中 | 部分缓解（已修复生产 bug） |
| R-04 | **设计漂移**：前端需求腐蚀后端核心抽象 | `ensure_llm` God Function 历史债务 | 🟡 中 | 已缓解（三层解耦 Sprint 13） |
| R-05 | **能力孤岛闲置**：11 个后端能力未激活 | map-islands.md 索引 | 🟡 中 | 未缓解 |
| R-06 | **测试基线文档不同步**：README/ARCHITECTURE.md 数字滞后 | README 524 vs 实际 577 | 🟢 低 | 已缓解（SNAPSHOT.md 动态记录） |
| R-07 | **跨项目认知断层**：clarity 与 devbase 接口未定义 | 双方通过 MCP 松耦合，无显式契约版本 | 🟡 中 | 未缓解 |

---

## 二、待优化项（无紧急性，但影响长期健康）

| ID | 优化项 | 当前状态 | 目标状态 | 阻塞条件 |
|----|--------|---------|---------|---------|
| O-01 | 架构地图自动重新扫描验证 | 人工生成 SNAPSHOT.md | 脚本自动生成 + diff 告警 | 无阻塞，可立即实施 |
| O-02 | 前端契约边界静态检查 | 人工审查 | CI 扫描 egui 源码禁止直接引用 `approval::InMemoryApprovalRuntime` 等内部模块 | 需 CI 资源 |
| O-03 | 能力孤岛激活优先级队列 | 无明确优先级 | PM 定义 P0/P1/P2 激活顺序 | 需 PM 决策 |
| O-04 | egui UI 渲染测试基线 | 零渲染测试 | headless 存在性验证或 `build_*_commands()` 纯函数测试 | Sprint 10 D4 已规划 |
| O-05 | 多窗口进程模型设计 | 单窗口单进程 | 独立窗口/独立进程可行性评估 | 需架构决策 |
| O-06 | 主题系统与人格表达绑定 | 单一主题 | 不同 Agent 角色/窗口可加载不同主题 | 需设计决策 |

---

## 三、前端倒逼后端的边界条件（本次会话新增）

| 可倒逼 | 不可倒逼 |
|--------|---------|
| 能力孤岛激活（暴露已有后端能力） | 核心 trait 重构（ApprovalRuntime、LlmProvider） |
| WireMessage 协议扩展 | 后端性能优化（无基准数据） |
| UI 交互模式发现（异步接口设计） | 绕过 Wire 协议的直接调用 |

**强制执行规则**：每次前端会话生成代码后，PM 审查 `crates/clarity-egui/src` 是否出现 `clarity-core` 内部模块（`subagents::`, `background::`, `approval::InMemoryApprovalRuntime`）的直接引用。

---

*本文件由 PM 人工维护。AI 会话只读，不得修改。*
