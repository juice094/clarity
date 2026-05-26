---
title: 并行会话协调记录 — 2026-05-11
category: Note
date: 2026-05-11
tags: [note]
---

# 并行会话协调记录 — 2026-05-11

> Type: Multi-session coordination + handoff state
> Status: 本会话暂停前端代码改动，等待并行会话收敛
> Trigger: 用户告知"前端设计当前正在其他会话进行相关调整"

---

## 1. 协调约定

### 本会话（协议层 + 配置层）

**已完成**：
- ADR-006 协议层收敛 Phase A/B/C
- S4-α/β widget 抽取（egui，已在并行会话调整前完成）
- S3.1 settings 真相源审计
- S3.2 settings 提交点集中
- Hybrid UI 认知修正

**已暂停**（避免与并行会话冲突）：
- 任何 `crates/clarity-egui/src/` 下的代码改动
- S4-γ/δ widget 抽取（前端工作）
- S3.3-5（settings 单源化后续）虽然主要触配置层，但路径会触及 `clarity-egui::components::settings::provider_tab.rs` 的 Apply 按钮逻辑，须等并行会话收敛

### 并行会话（前端设计）

按用户告知，正在调整 `clarity-egui` 相关。**未告知具体范围**，但可能触及：
- panel 布局 / widget 重新设计
- settings UI 重构（可能与 S3.3 重叠）
- 视觉设计调整

---

## 2. Merge 风险面分析

### 高风险（双向会话都可能动）

| 文件 | 本会话已动 | 并行可能动 | 缓解 |
|------|----------|----------|------|
| `crates/clarity-egui/src/widgets/provider_row.rs` | ✅ 新建 (S4-α) | 不确定 | git rerere / 三方合并 |
| `crates/clarity-egui/src/widgets/theme_card.rs` | ✅ 新建 (S4-β) | 不确定 | 同上 |
| `crates/clarity-egui/src/components/settings/provider_tab.rs` | ✅ 改 (S4-α) | 高概率 | 等并行 commit 后再处理 |
| `crates/clarity-egui/src/components/settings/interface_tab.rs` | ✅ 改 (S4-β) | 高概率 | 等并行 commit 后再处理 |
| `crates/clarity-egui/src/app_logic.rs` | ✅ 改 (S3.2) | 高概率 | 等并行 commit 后再处理 |
| `crates/clarity-egui/src/onboarding.rs` | ✅ 改 (S3.2) | 中概率 | 等并行 commit 后再处理 |

### 低风险（本会话独占）

| 文件 / 区域 | 性质 |
|----------|------|
| `crates/clarity-wire/src/` | 协议层，已完成 ADR-006 Phase A-C |
| `crates/clarity-core/src/agent/` | 后端 |
| `crates/clarity-llm/src/runtime.rs` | 配置层（S3.3 目标，但本会话未触及） |
| `docs/` 全部 | 文档 |
| `AGENTS.md` / `PROJECT_STATUS.md` | 项目说明 |
| `.gitignore` / CI 配置 | 工程基础设施 |

---

## 3. 推荐协调机制

### 短期（并行收敛前）

1. **本会话**：只做文档 + 协议层 + 后端 + 工程基础设施工作
2. **并行会话**：自由调整 `crates/clarity-egui/src/{panels,components,widgets}/`
3. **冲突点（settings/api_logic/onboarding）**：本会话**不再触碰**，等并行收敛

### 中期（并行收敛后）

1. 拉取并行会话的 commits
2. 检查 S3.2 集中的 3 个 helper（`commit_settings` 等）是否仍存在
3. 如被并行会话改写：重新评估 S3 路线图
4. 如保留：继续 S3.3

### 长期

考虑建立 **session 锁机制** 或 **branch 协议**：
- 每个会话在独立 branch 工作
- 通过 PR / merge 同步
- 而非共享 `main` 直接 commit

但当前项目用 main-direct-commit 模式，所以**约定优先于机制**。

---

## 4. 本会话剩余可做的工作（不触前端代码）

按 ROI 排序：

### A. 文档 / 协议层（强推荐）

- **ADR-007 立项**：Turn ID 注入 WireMessage（独立轨道，与前端无冲突）
- **Phase D 设计 RFC**：抽 `clarity-frontend-ir` crate 的接口设计（不实施）
- **CI 强制 P6/P7 lint 设计**：grep-based check-layout-tokens.sh 脚本

### B. 后端验证 / 工程基础设施

- **`cargo audit` 全面扫描**：依赖安全
- **新增 integration test**：覆盖协议层端到端（不动前端）
- **`clarity-core` God Crate 分解 RFC**：长期债务（27k 行）

### C. 知识沉淀

- **vault 报告更新**：把今日新增的 S3.1/S3.2 + Hybrid UI 修正补充进
  `C:\Users\22414\reports\clarity-protocol-convergence-2026-05-11.md`
- **方法论提炼**：从 ADR-006 + S4-α/β 总结"幻象建筑诊断方法论"作为可复用 playbook

### D. 不建议（即使不触前端代码也存在风险）

- **激进重构 `clarity-core`**：可能与并行会话的预期产生不一致
- **修改 wire 协议** 引入新字段：可能与并行会话开发新功能冲突
- **新增 ADR**：除非是已规划项（如 ADR-007），新决议应当全员同步

---

## 5. 当本会话结束时

无论何时停止本会话：
- 工作树必须干净（已确认）
- 所有 commit 必须独立 atomic（已确认）
- 任何并行会话拉取 main 后能 fast-forward 或简单 merge

如本会话决定休眠，**最后一个 commit 应当是文档/状态登记**，让并行会话能从 `git log` 看到当前状态。

---

End of coordination note.
