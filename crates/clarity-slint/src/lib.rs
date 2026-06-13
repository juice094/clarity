#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)
)]
//! `clarity-slint` — Slint 前端验证 crate。
//!
//! 本 crate 是 `clarity-egui` 的潜在替代方案，当前处于阶段 1：
//! 验证桌面三栏布局、数据绑定与回调机制。
//!
//! 架构约束：
//! - 禁止依赖 `egui` / `eframe` / `epaint` 及其衍生 crate。
//! - 仅通过 `clarity-contract` 与 `clarity-wire` 与后端交互，
//!   不直接耦合 `clarity-core` 的实现细节。

#![warn(missing_docs)]

pub mod app_state;
pub mod bridge;

/// Slint UI 模块（由 build.rs 在编译时从 .slint 文件生成）。
#[allow(missing_docs)]
pub mod ui {
    slint::include_modules!();
}
