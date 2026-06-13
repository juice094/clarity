---
title: 当前阶段与已知问题
category: Planning
date: 2026-06-13
tags: [planning, status, current-phase]
---

# 当前阶段与已知问题

> 详细项目状态、测试基线、功能清单与技术债务见 [`PROJECT_STATUS.md`](./PROJECT_STATUS.md)。历史 Sprint 摘要见 [`sprint-archive.md`](./sprint-archive.md)。

---

## 当前阶段

- **版本**：v0.3.0（`Cargo.toml`），v0.3.4-rc（开发中）
- **默认分支**：`main`
- **Rust 版本**：2024 edition · MSRV 1.85
- **许可证**：AGPL-3.0-or-later

### 近期交付焦点

- **Gray Migration 通道层**：ZeroClaw WeChat iLink 已移植至 `clarity-channels::zeroclaw`；Gateway 已切至 DeepSeek `deepseek-v4-pro`。
- **Provider/Secret 体系**：Stage A/B/C 已完成，支持 `enc2:` 加密 key、`models.toml` per-alias 配置、`ReliableProvider` 链式 failover、`runtime_router` 与 OAuth device flow。
- **egui 模块整理**：ViewState 单源化、panels/ 目录重组、widget 提取、design system 落地、layout shell 接入点。
- **S6 Pretext 三栏布局 Phase A**：新增 `LeftRailSection` / `RightRailSection` 与 `ViewState` rail 字段；移除 `UiStore.sidebar_collapsed`；`clarity-egui` 形成左 icon rail + 中主舞台 + 右工具 rail 的单页面外壳。

---

## 当前测试基线

| 测试类型 | 通过 | 失败 | 忽略 |
|----------|------|------|------|
| `cargo test --workspace --lib --exclude clarity-slint` | 1093 | 0 | 8 |
| `cargo test --workspace --bins --exclude clarity-slint` | 139 | 0 | 0 |
| `cargo test --workspace --doc --exclude clarity-slint` | 34 | 0 | 3 |
| `cargo test -p clarity-integration-tests --lib` | 16 | 0 | 0 |
| `cargo clippy --workspace --lib --bins --tests --exclude clarity-slint -- -D warnings` | 0 warning | 0 | - |
| `cargo fmt --all -- --check` | pass | 0 | - |

---

## 已知问题与注意事项

| 问题 | 状态 | 说明 |
|------|------|------|
| Discord/Telegram 默认禁用 | 🔒 等待上游 | `rustls-webpki` CVEs via `serenity` |
| Gateway HTTP Chat Completions 默认无状态 | 📝 设计如此 | WebSocket 支持完整 session；HTTP 可传 `session_id` |
| `clarity-egui` 迁移期 dead code | 🟡 临时宽限 | 迁移期 `main.rs` 顶部保留 `#![allow(dead_code)]`，完成后移除 |
| Tokenizer 离线依赖 | ✅ 已缓解 | 自动检测同目录 `tokenizer.json`；损坏文件 (<1KB) 报错 |
| 文件 sniff 误报 | ✅ 已修复 | 已知文本扩展名 bypass magic sniff |
| 跨目录文件读取 | ✅ 已修复 | `resolve_path()` 允许绝对路径直接通过 |
| Windows bash 工具注册 | ✅ 已修复 | Windows 仅注册 PowerShellTool |

---

*最后更新：2026-06-13*
