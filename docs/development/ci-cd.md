---
title: CI/CD 与发布
category: Development
date: 2026-06-13
tags: [ci, cd, release, github-actions]
---

# CI/CD 与发布

> 配置文件位于 `.github/workflows/`。本地验证命令见 [`setup.md`](./setup.md)。

---

## CI 流水线（`.github/workflows/ci.yml`）

| Job | 触发条件 | 平台 |
|-----|----------|------|
| `check` | push/PR to `main` | ubuntu / windows / macos |
| `test` | push/PR to `main` | ubuntu / windows / macos |
| `integration-test` | push/PR to `main` | ubuntu |
| `binary-test` | push/PR to `main` | ubuntu |
| `doc-test` | push/PR to `main` | ubuntu |
| `clippy` | push/PR to `main` | ubuntu / windows / macos |
| `fmt` | push/PR to `main` | ubuntu / windows / macos |
| `audit` | push/PR to `main` | ubuntu |
| `doc-guard` | push/PR to `main` | ubuntu |
| `coverage` | push/PR to `main` | ubuntu（`cargo-llvm-cov`） |

Ubuntu runner 需安装：`libglib2.0-dev pkg-config libgtk-3-dev libxdo-dev`。

---

## Release 流水线（`.github/workflows/release.yml`）

- 由 `v*` tag 触发。
- Windows：构建 `clarity-egui` release → 自签名 → `cargo-wix` 生成 `.msi`。
- Linux：构建 release 二进制。
- 通过 `softprops/action-gh-release@v2` 发布 artifact。

---

*最后更新：2026-06-13*
