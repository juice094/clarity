---
title: Privacy Review Guideline — Clarity Engineering
category: Document
date: 2026-05-16
tags: [document, ui]
---

# Privacy Review Guideline — Clarity Engineering

> Version: 2026-05-15  
> Trigger: Real-name persona identifier discovered in code and active docs.  
> Status: Resolved (commit TBD). Ongoing prevention.

---

## 1. 问题摘要

2026-05-15 EOD，识别到旧文档（`FUTURE_DIRECTION.md`, `archive/2026-04-26-...`）使用了一个**真实人名**作为默认 persona 标识符。该名字被本会话在未质疑来源的情况下沿用，写入了：

- 6 个 Rust 源文件（`endpoint.rs`, `app_logic.rs`, `settings.rs`, `stores/mod.rs`, `persona_switcher.rs`, `widgets/mod.rs`）
- 7 个活跃文档（`CHANGELOG.md`, `handoff.md`, `EOD report`, `ADR-015.md`, `ARCHITECTURE.md`, `BACKLOG.md`, `AGENTS.md` 中无匹配）
- 已推送至 GitHub `origin/main`

**整改结果**：已全部替换为 `"Kin"`（家人）。

---

## 2. 历史文档清单（保留原始文本，新代码不得引用）

以下文件中的旧命名保留为历史记录，**禁止在新代码或活跃文档中复制/引用**：

| 文件 | 说明 |
|---|---|
| `docs/planning/FUTURE_DIRECTION.md` L166-205 | Identity 枚举旧定义 |
| `docs/archive/2026-04-26-cluster-as-single-node.md` C6 | Gray Anchor Hard-binding 旧规划 |

---

## 3. 工程规范（预防再犯）

### 3.1 硬编码命名审查清单

任何涉及以下类别的字符串常量，**必须在实现前通过显式确认**：

- [ ] 真实人名（first name, last name, nickname）
- [ ] 地理位置（城市、街道、地标）
- [ ] 公司/产品名（第三方商标）
- [ ] 联系方式（email, phone, handle）

**确认方式**：在 PR / commit message / 会话记录中写一句：
> "命名来源：虚构 / 已获授权 / 公开领域"

### 3.2 旧文档 ≠ 执行规范

- `docs/archive/**`：已归档，仅作考古参考，**不可直接执行**
- `docs/planning/FUTURE_DIRECTION.md`：远景设计，执行前必须重新确认每个命名和每个接口
- `docs/planning/BACKLOG.md`：任务清单，不等于技术规范
- **只有 `docs/adr/ADR-*.md` + 代码本身构成当前规范**

### 3.3 自动化检测（建议）

在 CI 中加入预提交钩子或脚本：

```bash
#!/bin/bash
# scripts/privacy-scan.sh
# 扫描常见人名模式（简化示例，可扩展）
grep -riE "\bgray\b" crates/ docs/ reports/ \
  --include="*.rs" --include="*.md" \
  | grep -v "from_gray\|gray pixels\|gray scale" \
  && echo "PRIVACY_VIOLATION" && exit 1
exit 0
```

### 3.4 责任归属

| 阶段 | 责任人 | 动作 |
|---|---|---|
| 设计 | 主 Agent / 架构作者 | 命名时执行 §3.1 清单 |
| 实现 | 编码 Agent | 引用旧文档前，重新确认命名合规 |
| 审查 | 主 Agent | Commit 前运行 §3.3 扫描 |
| 发布 | 主 Agent | 更新本文件，登记新发现的敏感命名 |

---

## 4. 整改 Commit 索引

| Commit | 动作 |
|---|---|
| `7f78de59` | 首次引入（从旧文档沿用） |
| `f1ada913` | EOD 文档（未识别问题） |
| **TBD** | 全面替换为 `"Kin"` + 本规范新建 |

---

*本文件随每次隐私审查事件更新。最近一次更新：2026-05-15。*
