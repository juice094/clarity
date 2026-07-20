# Agent 指引 — clarity-mobile-core

## 构建

```bash
cargo check -p clarity-mobile-core
```

Android 完整构建：

```bash
bash mobile/android/rust/build-android.sh
cd mobile/android && ./gradlew assembleDebug
```

## 测试

```bash
cargo test -p clarity-mobile-core --lib
```

## 关键文件

- `src/lib.rs` — FFI 入口与 mobile runtime 初始化
- `clarity_mobile_core.udl` — UniFFI 接口定义
- `build.rs` — UniFFI scaffolding 生成
- `uniffi-bindgen.rs` — `uniffi-bindgen` CLI 入口

## 变更注意事项

- 保持 `clarity-core` 使用 `default-features = false`，避免移动端引入 Candle / gemm-f16。
- UDL 变更后需要重新运行 `uniffi-bindgen` 生成绑定，并同步移动端 SDK。
- `clarity-wire` 事件转发当前使用**原始通道**（`wire.ui_side(false)`），如需改为合并通道，必须在真机/模拟器上验证事件不会丢失。
- `send_message` 已对 `agent.run()` 增加 90 秒超时，失败/超时会通过 `UiEvent::Error` 回传 UI；调整超时请同步更新文档与测试。
