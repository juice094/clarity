# clarity-mobile-core

Mobile FFI core for Project Clarity (Android / iOS).

## 职责

- **跨平台 FFI 绑定** — 基于 `uniffi` 生成 Kotlin / Swift 绑定，供移动端调用
- **移动运行时封装** — 在手机上复用 `clarity-core` 的 Agent 循环、记忆与工具能力
- **轻量依赖** — 关闭 `local-llm` 默认 feature，避免 Candle / gemm-f16 在移动 ABI 上的 fullfp16 依赖
- **UDL 接口定义** — `clarity_mobile_core.udl` 声明移动端可见的 API 表面

## 当前能力

- 本地 Agent 模式：直接调用 `clarity-llm` provider（OpenAI / Kimi / DeepSeek / Anthropic / DeepSeek 设备登录）
- Thread 生命周期：`create_thread` / `list_threads` / `switch_thread`
- 事件流：`poll_event` 返回 `UiEvent`（TurnBegin / ContentPart / ToolCall / ToolResult / TurnEnd / Usage / Error 等）
- Claw Gateway 远程模式：通过 `clarity-claw::TransportManager` 连接桌面/网关，作为 thin client 使用
- 审批桥接：`MobileApprovalRuntime` 把 `clarity-core` 的审批请求转发为移动端弹窗

## 已知限制

- Android UI 目前为 demo 级别，精致度尚未追平 DeepSeek 等商业客户端。
- `clarity-wire` 事件转发当前使用**原始通道**（`wire.ui_side(false)`），以确保所有事件及时投递；合并通道在移动端的 flush 行为待进一步调优。
- iOS 绑定尚未接入。

## 构建

> 移动构建需要目标平台工具链（Android NDK / Xcode），桌面端仅做编译检查。

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
