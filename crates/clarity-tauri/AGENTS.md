# Agent 指引 — clarity-tauri

## 状态

**已归档（Archived）**。本 crate 不再参与默认 workspace 构建，仅作为历史参考保留。

## 构建

```bash
# 仅当显式需要时构建
cargo build -p clarity-tauri
```

## 测试

```bash
# 不参与默认测试；需要 Tauri 工具链支持
cargo test -p clarity-tauri
```

## 关键文件

- `src/main.rs` — Tauri 二进制入口
- `tauri.conf.json` — Tauri 配置
- `frontend/` — Web 前端源码
- `icons/` — 图标资源

## 约定

- 不得修改本 crate 的公共 API；所有新功能应加入 `clarity-egui` 或 `clarity-slint`
- 如果必须修复关键 bug，需要同步更新本 AGENTS.md 中的状态说明

## 红线

- 不允许让 `clarity-tauri` 重新成为默认 workspace 成员，除非产品决策变更并记录 ADR
