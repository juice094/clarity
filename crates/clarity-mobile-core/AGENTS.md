# Agent 指引 — clarity-mobile-core

## 构建

```bash
cargo check -p clarity-mobile-core
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
