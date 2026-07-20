# Clarity Android 通过 OpenClaw 连接格雷 Gateway 实验记录

> 生成时间：2026-07-07
> 测试设备：Pixel 7 API 36 模拟器（Android 16）
> 远端 Gateway：Gray-Cloud OpenClaw Gateway `ws://100.69.11.71:18789`
> 本地转发：`127.0.0.1:18790 → 100.69.11.71:18789`
> Token：`13ef094cc169523a711d4e508362bcc5192b7310`

## 1. 实验目标

验证 Clarity Android 端能否使用 OpenClaw 协议连接远端格雷 Gateway，完成标准 device-pairing 流程，获得 `operator.write` 等完整权限，最终替代 Kimi 客户端使用 Claw 功能。

## 2. 环境准备

1. **TCP 转发**：由于模拟器无法直接访问格雷内网 `100.69.x.x`，在 Windows 宿主机上启动 TCP 转发代理监听 `0.0.0.0:18790` 并转发到 `100.69.11.71:18789`。
2. **模拟器地址**：Android 模拟器通过 `10.0.2.2:18790` 访问宿主机转发端口。
3. **APK**：使用 `mobile/android` 工程构建 debug APK。

## 3. 已落地改动

### 3.1 Rust 侧

- `crates/clarity-contract/src/transport.rs`：给 `ClawTransport` trait 增加默认返回 Unsupported 的 `request_pairing` 方法。
- `crates/clarity-claw/src/client.rs`：
  - 修复 OpenClaw `res` 帧解析：优先读取 `result`/`error`，兼容格雷 Gateway 实际使用的 `payload` 包装格式。
  - 增加 `device.pair.request` 发送与错误翻译的 `tracing::info!` 日志。
- `crates/clarity-claw/src/transports/openclaw.rs` / `manager.rs`：暴露并代理 `request_pairing`。
- `crates/clarity-claw/src/device.rs`：增加路径可控的 `DeviceIdentity::load_or_generate_at` / `write_to_path` 以及 `load_paired_token_at` / `save_paired_token_at`。
- `crates/clarity-mobile-core/src/lib.rs`：
  - 实现 device identity 与 paired token 在移动 data dir 的持久化。
  - `build_openclaw_transport` 自动加载已保存的 device token，用 `TokenWithDevice` 重连。
  - 新增 `MobileRuntime::request_pairing()`，发送 `device.pair.request` 并轮询 `DevicePaired` 事件。
  - 修复 Android tracing subscriber 初始化，使 Rust 日志正确输出到 logcat（tag `ClarityRust`）。

### 3.2 UDL

- `crates/clarity-mobile-core/src/clarity_mobile_core.udl` 暴露 `request_pairing()` 和 `UiEvent.DevicePaired`。

### 3.3 Android Kotlin 侧

- `ChatScreen.kt`：在 Claw 模式下增加 **Pair Device** 按钮与 pairing 状态显示。
- `ChatViewModel.kt`：增加 `requestPairing()` 与状态字段。
- `EventHandler.kt`：处理 `DevicePaired` 事件。

## 4. 测试结果

| 步骤 | 状态 | 说明 |
|------|------|------|
| WebSocket 握手 | ✅ | 模拟器 → 宿主机转发 → 格雷 Gateway，101 Switching Protocols |
| `connect.challenge` | ✅ | 服务端返回 nonce |
| `connect` 认证 | ✅ | 返回 `hello-ok`，连接建立 |
| UI 显示 | ✅ | 应用显示 **"Connected to Gateway"** |
| 点击 **Pair Device** | ✅ | 按钮触发 `requestPairing()`，状态变为 **"Pairing..."** |
| 发送 `device.pair.request` | ✅ | Android 端正确发送 Ed25519 device id / public key |
| `device.pair.request` 响应 | ❌ | 服务端返回 `INVALID_REQUEST: missing scope: operator.admin` |
| 收到 assistant 回复 | ❌ | 当前 Token 无 `operator.write`，聊天消息仍被拒绝 |

**关键 Wire 日志片段**（来自 Android logcat）：

```text
ClarityRust: OpenClaw sending device.pair.request request={"id":"1","method":"device.pair.request","params":{"clientId":"clarity-mobile",...,"scopes":["operator.admin","operator.read","operator.write","operator.approvals","operator.pairing","operator.talk.secrets"],"type":"req"}}
ClarityRust: OpenClaw res parsed id=1 method=Some("device.pair.request") ok=false raw={"type":"res","id":"1","ok":false,"error":{"code":"INVALID_REQUEST","message":"missing scope: operator.admin"}}
ClarityEvent: handleEvent Error(code=transport_error, message=OpenClaw device.pair.request failed: missing scope: operator.admin)
```

## 5. 关键发现

1. **协议解析已修正**：原先 `clarity-claw` 仅识别 `payload` 字段，格雷 Gateway 对成功响应使用 `payload`、对错误响应使用 `error`。当前实现优先读取 `result`/`error` 并回退到 `payload`。
2. **device-pairing 链路已打通**：Android 端可生成/持久化 Ed25519 设备身份、发送 `device.pair.request`、正确显示错误/等待状态。
3. **当前阻塞点是 Gateway Token 权限**：`device.pair.request` 要求调用方连接具备 `operator.admin` scope。当前 Token `13ef094cc169523a711d4e508362bcc5192b7310` 在 Gateway 侧未被授予该 scope，因此请求在到达 pending 列表前即被拒绝。
4. **日志可观测性已修复**：Android logcat 现在可正确捕获 `ClarityRust` tag 的 DEBUG/INFO 日志，便于后续调试。

## 6. 已知问题与后续工作

| 问题 | 影响 | 后续工作 |
|------|------|----------|
| Gateway Token 缺少 `operator.admin` | 无法发起 `device.pair.request` | 向格雷申请带 `operator.admin` 的 token；或让格雷管理员手动将本设备加入已配对列表 |
| Gateway Token 缺少 `operator.write` | 无法发送 `chat.send` | device-pairing 成功后，使用 device token 重连可获得完整权限 |
| 本地 TCP 转发依赖宿主机 | 真机无法使用 `10.0.2.2` | 真机直接填格雷公网 IP + 端口；生产环境需要公网 Gateway 或 VPN |
| session key 硬编码为 `agent:main:main` | 不支持多 session / 子代理 | 增加 UI 选择或从服务端 `sessions.list` 动态获取 |

## 7. 复现步骤

1. 启动 TCP 转发（宿主机监听 `0.0.0.0:18790` 转发到 `100.69.11.71:18789`）。
2. 安装 APK：
   ```bash
   cd mobile/android
   bash rust/build-android.sh
   ./gradlew.bat assembleDebug
   adb install -r app/build/outputs/apk/debug/app-debug.apk
   ```
3. 打开应用，ProviderSetup 中默认已填 `openclaw://10.0.2.2:18790/ws`。
4. 在 Token 字段填入格雷提供的 token。
5. 点击 **Connect via Claw**，状态变为 **Connected to Gateway**。
6. 在聊天界面点击 **Pair Device**。
7. 观察 pairing 状态与错误提示。

## 8. 结论

Clarity Android 端已完成标准 device-pairing 流程的实现，能够正确发送配对请求并解析 Gateway 响应。当前阻塞点是格雷 Gateway 给定的 Token 未被授予 `operator.admin` scope，导致 `device.pair.request` 被拒绝、设备无法进入 pending 配对列表。下一步需要格雷侧提供一个具备 `operator.admin` 的 token，或由格雷管理员直接在 Gateway 上将 Android 设备标记为已配对；配对成功后，Android 端会自动保存 device token 并用 `TokenWithDevice` 模式重连，从而获得 `operator.write` 等完整权限。
