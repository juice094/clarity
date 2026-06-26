---
title: Clarity Architecture Health
category: Development
date: 2026-06-25
tags: [architecture, health, metrics, iteration]
---

# Clarity 架构健康迭代基础

> 本文档为**架构健康迭代**提供度量基准、检查清单与改进节奏。它不定义一次性目标，而是记录「当前状态 → 下一步改进」的循环。
>
> 自动采集脚本：[`scripts/arch_health.py`](../../scripts/arch_health.py)  
> 测试人员入口：[`docs/testing/TESTER_GUIDE.md`](../../docs/testing/TESTER_GUIDE.md)  
> 权威运行基线：[`AGENTS.md`](../../AGENTS.md) §6

---

## 1. 架构健康指标体系

### 1.1 规模指标

| 指标 | 当前基线 | 采集方式 |
|------|---------|---------|
| Workspace members | 24（23 crates + tests/integration） | `scripts/arch_health.py` |
| Crate directories | 23 | `scripts/arch_health.py` |
| Rust source files | 693 | `scripts/arch_health.py` |
| Non-blank Rust lines | 176,114 | `scripts/arch_health.py` |
| `unsafe` 代码 | 1 处（`clarity-memory` 白名单） | `AGENTS.md` §7.1 |

### 1.2 质量指标

| 指标 | 当前基线 | 采集方式 |
|------|---------|---------|
| lib tests | 1554 passed / 0 failed | `scripts/test_runner.py` |
| bin tests | 275 passed / 0 failed | `scripts/test_runner.py` |
| doc tests | 34 passed / 0 failed | `scripts/test_runner.py` |
| integration tests | 26 passed / 0 failed | `scripts/test_runner.py` |
| clippy warnings | 0 | `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` |
| fmt diff | 0 | `cargo fmt --all -- --check` |
| audit unsound/yanked | 0 | `cargo audit --deny unsound --deny yanked` |

### 1.3 耦合指标

| 指标 | 目标 | 当前状态 |
|------|------|---------|
| `clarity-contract` 内部依赖 | 0 | ✅ 0 |
| `clarity-core` 依赖前端/网络 crate | 0 | ✅ 0 |
| 前端 crate 互相 import | 0 | ✅ 0 |
| `clarity-slint` 依赖 `clarity-core` | 否 | ✅ 否 |

---

## 2. 健康检查节奏

### 每次 PR 前

```powershell
python scripts/doctor.py
python scripts/test_runner.py
cargo fmt --all -- --check
```

### 每次 Release 前

```powershell
python scripts/arch_health.py --json target/arch-health-<version>.json
python scripts/test_runner.py --json target/test-report-<version>.json
```

### 每季度架构回顾

1. 对比 `target/arch-health-*.json` 趋势。
2. 检查是否有 crate 内部依赖数异常增长。
3. 检查新增 crate 是否已同步到所有拓扑文档（`AGENTS.md` §13）。
4. 检查 `unwrap()`/`expect()` 债务是否增长。

---

## 3. 关键不变量（不可违反）

1. `clarity-core` 不依赖任何前端 crate 或网络 crate。
2. `clarity-contract` 不依赖任何内部 crate。
3. 前端 crate 之间不互相 import；跨前端状态/事件走 `clarity-wire`。
4. 禁止在异步上下文中执行阻塞 I/O；使用 `tokio::task::spawn_blocking`。

违反任何一条必须在合并前修复。

---

## 4. 改进方向（持续迭代）

| 方向 | 当前状态 | 下一步 |
|------|---------|--------|
| 测试覆盖 | 1889+ tests 全绿 | 引入 egui UI snapshot / kittest |
| 架构解耦 | 22 活跃 crate 分层清晰 | 继续拆分 `clarity-core` 中过大的子模块 |
| 文档同步 | OKF bundle + 拓扑文档已对齐 | 每次新增 crate 自动检查 5+ 文档 |
| 本地推理 | Candle GGUF 已落地 | 增加模型量化/加载性能基准 |
| 移动端 | FFI 核心已落地 | 推进 Android/iOS 完整 UI |

---

## 5. 历史基线

| 日期 | Rust files | Non-blank lines | lib tests | bin tests | doc tests | integration |
|------|-----------|-----------------|----------|----------|----------|-------------|
| 2026-06-25 | 693 | 176,114 | 1554 | 275 | 34 | 26 |

---

## 6. 相关脚本与文档

- [`scripts/arch_health.py`](../../scripts/arch_health.py) — 采集规模/耦合指标
- [`scripts/test_runner.py`](../../scripts/test_runner.py) — 统一测试矩阵与报告
- [`scripts/doctor.py`](../../scripts/doctor.py) — 环境健康检查
- [`docs/testing/TESTER_GUIDE.md`](../../docs/testing/TESTER_GUIDE.md) — QA 上手文档
- [`AGENTS.md`](../../AGENTS.md) — 开发运行上下文与基线
- [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) — 代码级精确架构
