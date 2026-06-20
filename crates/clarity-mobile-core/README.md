# clarity-mobile-core

Mobile FFI core for Project Clarity (Android / iOS).

## 职责

- **跨平台 FFI 绑定** — 基于 `uniffi` 生成 Kotlin / Swift 绑定，供移动端调用
- **移动运行时封装** — 在手机上复用 `clarity-core` 的 Agent 循环、记忆与工具能力
- **轻量依赖** — 关闭 `local-llm` 默认 feature，避免 Candle / gemm-f16 在移动 ABI 上的 fullfp16 依赖
- **UDL 接口定义** — `clarity_mobile_core.udl` 声明移动端可见的 API 表面

## 构建

> 移动构建需要目标平台工具链（Android NDK / Xcode），桌面端仅做编译检查。

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
