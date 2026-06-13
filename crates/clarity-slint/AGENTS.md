# Agent 指引 — clarity-slint

## 状态

**实验栈（Experimental）**。验证 Slint 作为 `clarity-egui` 替代桌面前端的可行性。

## 构建

```bash
# 不参与默认 workspace 构建
cargo build -p clarity-slint
```

## 测试

```bash
# 不参与默认测试；需要 Slint 工具链支持
cargo test -p clarity-slint
```

## 关键文件

- `src/main.rs` — Slint 二进制入口
- `ui/app.slint` — Slint UI 定义
- `build.rs` — Slint 构建脚本

## 阶段 0 目标

1. `cargo slint-run` 成功弹出窗口。
2. 输入文字点击发送，800ms 后 UI 更新为处理结果。
3. 处理期间 UI 不冻结，按钮禁用。
4. `cargo slint-clippy` 零警告。

## 架构约束

- 禁止依赖 `egui` / `eframe` / `epaint`
- 仅通过 `clarity-contract` / `clarity-wire` 与后端交互
- 默认 CI 使用 `--exclude clarity-slint`，不得改回默认构建

## 红线

- 不允许引入对 `clarity-core` 的直接依赖
- 不允许让 slint 成为默认前端，除非 Phase 0 目标全部达成并经过 ADR 评审
