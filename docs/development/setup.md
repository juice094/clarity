---
title: 开发环境搭建与验证
category: Development
date: 2026-06-25
tags: [development, setup, build, test]
---

# 开发环境搭建与验证

> 本文档维护 Clarity 的构建、运行、测试与验证命令。快速上下文请查阅根目录 [`AGENTS.md`](../../AGENTS.md)。

---

## 前置要求

| 工具 | 版本 | 说明 |
|------|------|------|
| Rust | 1.85+（推荐 1.94+） | MSRV 1.85，CI 使用 stable |
| Git | any | |
| PowerShell | 5.1+ / 7.x | 用于 `scripts/verify.ps1` |

可选：
- Windows 10/11 + MSVC 14.40+（桌面 GUI 与 CUDA）
- NVIDIA CUDA 12.6（`local-llm-cuda` feature）

---

## 常用命令

```bash
cd C:\Users\22414\dev\clarity

# 格式检查
cargo fmt --all -- --check

# 编译（不含实验性 clarity-slint）
cargo check --workspace --lib --bins --exclude clarity-slint

# Clippy 零警告
cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings

# 单元测试
cargo test --workspace --lib --exclude clarity-slint

# 二进制测试
cargo test --workspace --bins --exclude clarity-slint -- --test-threads=2

# 文档测试
cargo test --workspace --doc --exclude clarity-slint -- --test-threads=2

# 集成测试
cargo test -p clarity-integration-tests --lib

# 安全审计
cargo audit --deny unsound --deny yanked
```

---

## 当前测试基线

| 测试类型 | 通过 | 失败 | 忽略 |
|----------|------|------|------|
| `cargo test --workspace --lib --exclude clarity-slint` | 1554 | 0 | 0 |
| `cargo test --workspace --bins --exclude clarity-slint` | 275 | 0 | 2 |
| `cargo test --workspace --doc --exclude clarity-slint` | 34 | 0 | 3 |
| `cargo test -p clarity-integration-tests --lib` | 26 | 0 | 0 |
| `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` | 0 warning | 0 | - |
| `cargo fmt --all -- --check` | pass | 0 | - |

> 提交前必须保证上述命令全部通过。`clarity-slint` 为实验栈，不参与默认 CI。

---

## 运行各入口

```bash
# 桌面 GUI（主前端栈）
cargo run -p clarity-egui

# 终端 TUI
cargo run -p clarity-tui

# Web IDE / Gateway
cargo run -p clarity-gateway

# 系统托盘
cargo run -p clarity-claw

# 无头 CLI
cargo run -p clarity-headless -- health
cargo run -p clarity-headless -- --prompt "Hello" --provider local --output json

# 实验性 Slint 前端
cargo run -p clarity-slint
```

---

## Feature 与构建变体

| Feature | 作用 | 使用场景 |
|---------|------|----------|
| `local-llm` | 启用 Candle GGUF 本地推理 | 默认开启 |
| `local-llm-cuda` | 本地推理 CUDA 加速 | Windows + NVIDIA CUDA |
| `mcp` | 启用 MCP 集成 | `clarity-core` 默认 |
| `session-migration` | Session V1→V2 迁移工具 | `clarity-core` 可选 |
| `line-mode` | egui 行级渲染管线 | `clarity-egui` 可选 |
| `slack` / `discord` / `telegram` / `webhook` | Gateway 通道 feature | 默认仅 `webhook` |
| `telemetry-api` | Gateway 遥测 REST API | `clarity-gateway` 可选 |

CUDA 构建示例（Windows）：

```powershell
$env:NVCC_CCBIN="C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.40.33807\bin\Hostx64\x64"
cargo check -p clarity-llm --features local-llm-cuda
cargo run -p clarity-egui --features cuda
```

---

## 一键验收

```powershell
.\scripts\verify.ps1 --all -Strict
```

该脚本逐 crate 检查 README、AGENTS、编译、测试、Clippy、格式化。

---

## 测试纪律

- 新增 Rust 模块必须含 `#[cfg(test)]` 单元测试。
- Bug fix 必须配回归测试（先红后绿）。
- egui 面板/组件变更需补充手动 QA 或视觉回归检查。
- 性能改动需补充 benchmark 或延迟测量。

---

*最后更新：2026-06-25*
