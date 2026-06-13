# clarity-egui

Lightweight desktop GUI for Clarity built on egui + glow. Ships as a single native binary with no webview, no JS bundler, and no Chromium overhead.

## Why use this instead of...

- **Tauri** — Tauri bundles a webview (WebKit2GTK / WebView2) and requires a frontend build step; clarity-egui is pure Rust with immediate-mode rendering and sub-50MB binary size.
- **Dioxus Desktop** — Dioxus uses a web renderer (Wry) under the hood; clarity-egui targets raw GPU via glow for lower latency and deterministic frame pacing.

## Test

```bash
cargo test -p clarity-egui --bin clarity-egui
```

## 边界与稳定性

- **Stability tier**: Beta / Primary desktop frontend
  - egui 是当前主力桌面栈；Tauri 已归档，`clarity-slint` 为实验栈
  - Sprint S5 正在进行 Pretext 单页 / 三栏布局迁移，迁移期间保持 `cargo clippy` 零警告
- **MSRV**: 1.78.0
- **反向依赖禁止** (No reverse dependencies):
  - 可依赖 clarity-core + clarity-memory + clarity-wire
- **Library/binary classification**:
  - Binary: application entry point, not a library
