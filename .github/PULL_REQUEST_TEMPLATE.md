## Summary

<!-- 一句话说明本 PR 解决的问题。如果是重构，说明为什么现在做。 -->

## Type

- [ ] `feat` — 新功能
- [ ] `fix` — Bug 修复
- [ ] `refactor` — 重构（无行为变更）
- [ ] `docs` — 文档
- [ ] `test` — 测试
- [ ] `ci` — 工程基础设施
- [ ] `chore` — 杂项

## CODE-CHANGE-PRINCIPLES Checklist

> 详见 [`docs/CODE-CHANGE-PRINCIPLES.md`](../docs/CODE-CHANGE-PRINCIPLES.md)。
> **任一项未勾选必须在下方"豁免说明"区显式解释**。

- [ ] **P1 单向迁移** — 未引入双向桥接 / 临时适配层（旧→新只切一次）
- [ ] **P2 删除优先** — 净增加代码已伴随等量删除（或属新功能 / 测试，已在 Type 中标记）
- [ ] **P3 单源真相** — 未引入第二份配置 / 第二条通道 / 第二组常量 / 第二份协议
- [ ] **P4 测试先行** — 被改动模块有先行测试覆盖（重构前已有 baseline 测试）
- [ ] **P5 编译可分** — 每个 commit 独立 `cargo build` 通过（`git rebase -i` 检查每步可编译）
- [ ] **P6 Theme Token** — `crates/clarity-egui/src/{panels,components,widgets}/` 中无未豁免的 `> 8.0` 硬编码像素值
- [ ] **P7 协议层不前瞻** — 协议层（`clarity-wire` / `clarity-contract`）变更同 PR 含 producer + consumer + e2e 测试

### 豁免说明（如有未勾选项）

<!--
格式：
- **P_X**: <原因>，<弥补措施>
例：
- **P4**: 本 PR 为新增 widget 首次出现，无 baseline；同 PR 含 5 个单元测试作为新 baseline
-->

## Verification

- [ ] `cargo clippy --workspace --lib --bins --tests -- -D warnings` PASS
- [ ] `cargo test --workspace --lib --no-fail-fast` 全部通过
- [ ] `cargo test --workspace --bins --no-fail-fast` 全部通过
- [ ] 当前测试基线维持或上升（参见 `PROJECT_STATUS.md`）

## Hybrid UI 影响

- [ ] 本 PR 触及 `clarity-egui`（GUI 腿）
- [ ] 本 PR 触及 `clarity-tui`（TUI 腿）
- [ ] 本 PR 触及共享后端（core / wire / contract / memory / gateway / llm）
- [ ] 跨腿一致性已验证（任何只服务于单一前端的改动已在描述中说明）

## Related

- ADR: <link to `docs/adr/ADR-XXX.md` if applicable>
- Issue: #<num>
- Related PR: #<num>

## Visual Diff (UI changes only)

<!-- 如本 PR 触及 UI 渲染：附 before/after 截图或动图，标注像素级差异区域 -->

---

_注：本模板由 ADR-006 + CODE-CHANGE-PRINCIPLES（2026-05-11）建立。_
