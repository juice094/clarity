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

> 详见 [`docs/development/CODE-CHANGE-PRINCIPLES.md`](../docs/development/CODE-CHANGE-PRINCIPLES.md)。
> **任一项未勾选必须在下方"豁免说明"区显式解释**。

- [ ] **P1 单向迁移** — 未引入双向桥接 / 临时适配层（旧→新只切一次）
- [ ] **P2 删除优先** — 净增加代码已伴随等量删除（或属新功能 / 测试，已在 Type 中标记）
- [ ] **P3 单源真相** — 未引入第二份配置 / 第二条通道 / 第二组常量 / 第二份协议
- [ ] **P4 测试先行** — 被改动模块有先行测试覆盖（重构前已有 baseline 测试）
- [ ] **P5 编译可分** — 每个 commit 独立 `cargo build` 通过
- [ ] **P6 Theme Token** — `crates/clarity-egui/src/{panels,components,widgets}/` 中无未豁免的 `> 8.0` 硬编码像素值
- [ ] **P7 协议层不前瞻** — 协议层（`clarity-wire` / `clarity-contract`）变更同 PR 含 producer + consumer + e2e 测试

### 豁免说明

<!-- 格式：- **P_X**: <原因>，<弥补措施> -->

## Verification

- [ ] `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` PASS
- [ ] `cargo test --workspace --lib --exclude clarity-slint` 全部通过
- [ ] `cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2` 全部通过
- [ ] 当前测试基线维持或上升

## Related

- ADR: <!-- 链接到 docs/adr/ADR-XXX.md，如适用 -->
- Issue: #<!-- 编号 -->
