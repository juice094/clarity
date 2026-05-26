---
title: 2026-05-11 协议层收敛与工程纪律建立 — 回顾
category: Note
date: 2026-05-11
tags: [note]
---

# 2026-05-11 协议层收敛与工程纪律建立 — 回顾

> Type: Sprint retrospective + methodology sediment
> Trigger: 用户报告 "egui 前端与协议层设计出现逻辑问题"
> Outcome: 10 commits，ADR-006 Phase A/B/C + S4-α 完成
> Full vault report: `C:\Users\22414\reports\clarity-protocol-convergence-2026-05-11.md`

---

## 1. 一句话总结

> **诊断**: 不是 egui 问题，是协议层 3 套并存的"幻象建筑"。
> **决策**: 单源化（ADR-006）+ 七条改动原则（CODE-CHANGE-PRINCIPLES）+ widget POC（S4-α）。
> **执行**: 10 个原子提交，删除 790 行死代码，0 测试失败，clippy 干净。

## 2. 三个核心方法论可复用经验

### 2.1 协议层债务诊断模式（Phantom Producer/Consumer Audit）

当一个项目有多代际协议并存时，按以下序列诊断：

```
1. grep 所有协议类型（enum / struct / trait）
2. 对每个类型，分别找：
   - producer (who sends this type?)
   - consumer (who reads this type?)
3. 三种结果：
   a. 都存在 → 健康路径
   b. 只有 producer → 幻象 producer（白发送，receiver 不存在）
   c. 只有 consumer → 幻象 consumer（白监听，sender 不存在）
4. b 和 c 必须删除或显式文档化（标注何时会有对方）
```

**应用本案**: Gen-2 (EventBus) 是情况 b，Gen-3 (view 通道) 是情况 c — 都判删。

### 2.2 反模式机械翻译可行性

UI 反模式（immediate-mode GUI 中的 retained-mode 风格）**可以被机械重写**为 idiomatic 模式，前提：
- 反模式必须可被**封装在自定 widget 内**（边界化，per EGUI_LAYOUT.md RULE 4）
- widget 必须有**单元测试锁定行为不变性**（per CODE-CHANGE-PRINCIPLES P4）
- 重构应**保持视觉一致**或显式说明视觉变化

**应用本案**: provider_tab 的 `painter.rect_filled × 2` 被替换为 `Frame::fill + Frame::stroke`。视觉略变（四边描边 vs 仅左侧 bar），但 idiomatic 度大幅提升。

### 2.3 多 Phase 重构纪律（P5 编译可分）

跨多 commit 的重构必须满足：
- 每个 commit **独立通过** `cargo build` + `clippy -D warnings` + tests
- Phase 间依赖通过 **grep 验证**而非靠假设（"Phase B 删 X 之前 grep 确认无外部 caller"）
- 允许 deprecation warnings 暂存在过渡期，但必须被 `#[allow(deprecated)]` **显式豁免**（不能让 clippy 红）

**应用本案**: ADR-006 Phase A 加 deprecation 标记 → Phase B 删 producer → Phase C.1 删 Gen-2 类型 → Phase C.2 删 view 通道。**每步独立 bisectable**。

## 3. 三个意外修正（执行中发现）

### 3.1 sidebar_card 已部分抽取

最初计划"抽取 sidebar_card"，发现 widget 已在 Sprint 41 抽取。**POC 目标转向 provider_row**。

**教训**: 重构前应当先重新审计，不能依赖之前的诊断报告。

### 3.2 ViewCommand 真有消费者

ADR-006 初版本声称 ViewCommand 零消费者。**事实校正**：clarity-tui::protocol_renderer + clarity-gateway::ws.rs 真实使用。

**教训**: ADR 应当在执行中**显式标注校正**，保留思维轨迹。修改后的 Phase D 从"移入 egui::view"改为"抽出 clarity-frontend-ir crate"。

### 3.3 egui_kittest 决议推迟

最初计划引入 egui_kittest 做 snapshot test。验证后：
- 需要 6+ 个传递依赖（dify / image / accesskit / kittest / etc）
- Windows CI 图形堆栈可能有问题
- `egui::Context::run` 已足够轻量验证 H2

**教训**: YAGNI。测试基础设施可以从最简单的 Context::run 开始。

## 4. 五个未解之谜（留给后续 session）

1. **`update()` 热路径仍有 I/O** — `check_mcp_config_reload` 中的 `std::fs::metadata` 违反自定 §1.1 铁律，待 S5 处理。
2. **panel-level allocate+Sense::click 仍有 4 处** — interface_tab.rs / sidebar.rs / 待 S4-β/γ 处理。
3. **chat/avatar.rs:8-10 painter UI text** — 头像绘制是合法装饰还是违规？需评估。
4. **ADR-007 Turn ID 是否真需要立项？** — 当前没有 active bug 报告事件穿插，是否优先级足够？
5. **`SettingsStore::settings_vm` 何时激活？** — S3 单源化核心问题，未启动。

## 5. 五个教训（给未来）

1. **不要建协议未有消费者就 commit**（P7 强制）— Gen-2/Gen-3 都是这个错误的受害者
2. **不要让 `#[allow(dead_code)]` 标记超过 1 个 sprint** — 幻象建筑起点
3. **ADR 执行中发现错误应当显式校正**，不要悄悄修主决议 — 思维轨迹比"完美 ADR"更重要
4. **重构的最小可验证单元应当 ≤ 3 工作日** — 否则风险/收益失衡
5. **测试基础设施可以从最简单的 Context::run 开始**，不必先引入完整框架 — YAGNI

## 6. 关键 commit 速查

```
40851b1e  docs:        S4-α 文档登记
69cf941f  refactor:    S4-α 抽取 provider_row widget
77c53234  docs:        ADR-006 Phase A/B/C 完成登记
2867c0a5  refactor:    Phase C.2 删除 Wire view 通道
1c15621a  refactor:    Phase C.1 删除 Gen-2 整文件
dd07b42f  refactor:    Phase B 删除 producer 路径
99f0f5a2  docs:        Sprint 43+ 登记
c8e71fdb  refactor:    Phase A 弃用标记
65c62456  docs:        七条原则 + ADR-006
febe6cc1  (baseline)
```

## 7. 新成员阅读路径

```
1. AGENTS.md §Current Phase                            # 当前活跃迭代
2. PROJECT_STATUS.md                                   # 测试基线 + 活跃 ADR
3. docs/CODE-CHANGE-PRINCIPLES.md                      # 七条原则（强制 PR 自查）
4. docs/adr/ADR-006-protocol-layer-convergence.md      # 协议收敛背景
5. crates/clarity-egui/EGUI_LAYOUT.md                  # egui 5 条铁律
6. crates/clarity-egui/docs/layout-audit-architecture-crisis.md  # 反模式审计
7. crates/clarity-egui/src/widgets/provider_row.rs     # widget 范本（含测试）
```

## 8. 阶段定位

| 阶段 | 状态 | 下一步触发条件 |
|------|------|--------------|
| ADR-006 Phase A/B/C | DONE | — |
| ADR-006 Phase D (frontend IR) | 阻塞 | 等 SettingsViewModel 激活（S3） |
| ADR-006 Phase E / ADR-007 (Turn ID) | 阻塞 | 等 active bug 或主动立项 |
| S4-α widget POC | DONE | — |
| S4-β interface_tab | 立即可启动 | — |
| S4-γ sidebar.rs:159 MCP | 等 S4-β | — |
| S4-δ chat/avatar | 等 S4-γ | — |
| S3 SettingsViewModel 激活 | 阻塞 | 等 S4 全量完成 |
| S5 update() 热路径瘦身 | 阻塞 | 等 S3 |
| S6 UI snapshot test 矩阵 | 阻塞 | 等所有 widget 抽完 |

总剩余路径约 16-18 工作日。

---

## 9. 会话中期补丁：Hybrid UI 认知修正（2026-05-11）

> 本回顾文档初版含有隐性偏见：把 clarity-tui 框架为"边缘消费者"或"维护模式"。
> 用户在 S3.2 commit 后指出 Clarity 实际是 **Hybrid UI（GUI × TUI 混血）** 架构，
> egui 与 tui **同等一线**，共享后端，前端多态。

### 影响修正

- 凡早期文档中"维护模式 clarity-tui" / "主力 GUI 栈 clarity-egui" / "secondary tui"
  均已修订（详见 `PROJECT_STATUS.md §Current Stack Positioning`）。
- ADR-006 §1.3 校正（"ViewCommand 真有 tui 消费者"）实际上**就是 Hybrid UI 架构的
  明证**——而非"幸运地避免了破坏 tui"。
- Phase D（抽出 `clarity-frontend-ir` crate 供 egui + tui 共享）的优先级因此
  **上升**——这不是可选优化，而是 Hybrid UI 的核心基础设施。

### 后续 session 注意事项

任何只服务于单一前端的协议层 / 数据结构都应当被审视：
- 是因为该功能本质上是 GUI/TUI 独有（合理）；
- 还是因为视野局限而错过了 Hybrid UI 复用机会（应当修正）。

S4-α/β 的 widget 抽取是 egui-only 工作，这本身是合理的（widget 是 GUI 独有概念），
但**等价的 tui 反模式扫描**也应作为后续工作的一部分（待立项）。

---

End of retrospective.
