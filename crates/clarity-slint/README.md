# clarity-slint

Clarity 的 Slint 前端验证 crate。当前处于**阶段 0**：验证 tokio 异步运行时与 Slint UI 事件循环的桥接可行性。

## 环境要求

- Rust 1.85+（与 workspace MSRV 一致）
- `slint-viewer` 1.16.1：用于独立预览 `.slint` 文件

```bash
cargo install slint-viewer --version 1.16.1 --locked
```

## 开发命令

```bash
# 快速检查
cargo slint-check

# 运行最小桥接验证
cargo slint-run

# Clippy 严格检查
cargo slint-clippy

# 独立预览 .slint 文件（无需编译 Rust）
slint-viewer crates/clarity-slint/ui/app.slint --auto-reload
```

## 阶段 0 目标

1. `cargo slint-run` 成功弹出窗口。
2. 输入文字点击发送，800ms 后 UI 更新为处理结果。
3. 处理期间 UI 不冻结，按钮禁用。
4. `cargo slint-clippy` 零警告。

## 架构约束

- 禁止依赖 `egui` / `eframe` / `epaint`。
- 仅通过 `clarity-contract` / `clarity-wire` 与后端交互。
