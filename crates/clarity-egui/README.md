# clarity-egui

Lightweight desktop GUI for Clarity built on egui + glow. Ships as a single native binary with no webview, no JS bundler, and no Chromium overhead.

## Why use this instead of...

- **Tauri** — Tauri bundles a webview (WebKit2GTK / WebView2) and requires a frontend build step; clarity-egui is pure Rust with immediate-mode rendering and sub-50MB binary size.
- **Dioxus Desktop** — Dioxus uses a web renderer (Wry) under the hood; clarity-egui targets raw GPU via glow for lower latency and deterministic frame pacing.

## Test

```bash
cargo test -p clarity-egui --lib
```

## 边界与稳定性

- **Stability tier**: Experimental
  - Experimental: API may change before v0.4.0
- **MSRV**: 1.78.0
- **反向依赖禁止** (No reverse dependencies):
  - 可依赖 clarity-core + clarity-memory + clarity-wire
- **Library/binary classification**:
  - Binary: application entry point, not a library
