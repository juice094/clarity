# Clarity Project Status

> **本文件是 pointer**。完整项目状态报告维护在 [`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md)。
>
> 此根目录 pointer 用于 GitHub 仓库首页的可发现性。

---

## 当前状态（2026-05-15）

- **版本**: v0.3.2 → v0.3.4-rc
- **测试基线**: 927 passed, 0 failed, 7 ignored
- **Clippy**: 0 warnings (`-D warnings`)
- **CI**: 7-job 全绿（check / test / clippy / fmt / audit / doc-guard / release）
- **活跃 Sprint**: S3.3（Settings 单源化）/ Sprint 37（prompt_cache_key 策略层）

## 关键交付

- ✅ 协议层收敛完成（ADR-006 Phase A/B/C）：删除 ~790 行死代码
- ✅ Settings 审计 S3.1 + 集中提交点 S3.2 完成
- ✅ 隐私整改：真实人名替换为 `"Kin"`，`docs/PRIVACY_REVIEW.md` 建立
- ✅ egui 设计系统硬化：CJK 字体 297KB、Phosphor 图标、错误气泡增强

## 技术债务

- 🔴 `clarity-egui` 纯逻辑测试 32+，UI 渲染测试仍为缺口
- 🟡 预留模块（autodream / daemon / personality / server）全模块 blanket allow
- 🟡 根目录 `archive/files.txt` 3.9MB 历史快照

---

*详见 [`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md) 完整报告（含前后端 Parity 矩阵、竞品对比、技术债务明细）。*