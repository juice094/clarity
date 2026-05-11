# Clarity Roadmap

> **本文件是 pointer**。完整路线图维护在 [`docs/ROADMAP.md`](docs/ROADMAP.md)。
>
> 此根目录 pointer 用于 GitHub 仓库首页文件列表的可发现性。

---

## 当前阶段（2026-05-11）

- ✅ **协议层收敛完成**（ADR-006 Phase A/B/C）：从 3 套并存的 Wire/Event/View 协议收敛为 `WireMessage` 单源真相，删除 ~790 行死代码
- ✅ **七条改动原则强制生效**（`docs/CODE-CHANGE-PRINCIPLES.md`）：所有 PR 必须自查 P1~P7
- ✅ **Hybrid UI 架构明确**：clarity-egui (GUI) 与 clarity-tui (TUI) 同等一线，共享后端
- 🟡 **Settings 单源化**（Sprint S3 进行中）：S3.1 审计 + S3.2 集中提交点已完成；S3.3-5 待并行会话收敛后启动

## 短期里程碑

- **S3.3** RuntimeProviderConfig 派生（修复 profile 切换 LLM 不 reload bug）
- **Phase D** 抽出 `clarity-frontend-ir` crate（Hybrid UI 跨前端 IR 共享）
- **ADR-007** Turn ID 注入 WireMessage（事件穿插预防）

## 长期愿景

请参阅：
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — 完整路线图（阶段一→二→三）
- [`docs/long-term-roadmap-2026.md`](docs/long-term-roadmap-2026.md) — 2026 全年路线
- [`docs/architecture-positioning.md`](docs/architecture-positioning.md) — 项目在更大生态中的定位（与 Kimi CLI / ZeroClaw / OpenClaw / devbase 的关系）

## 当前最高优先级

> 详见 [`AGENTS.md` §Current Phase](AGENTS.md#current-phase) 及 [`PROJECT_STATUS.md`](PROJECT_STATUS.md)。

---

_此 pointer 自 2026-05-11 起维护。任何路线图变更应当先更新 `docs/ROADMAP.md`，必要时同步更新本 pointer 的"当前阶段"段落。_
