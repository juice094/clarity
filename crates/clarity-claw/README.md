# clarity-claw

Clarity 分布式节点（Claw）的 client-side 统一入口：同时提供 UI 无关的客户端库和系统托盘常驻二进制。

## 职责

- **库（lib）**
  - Gateway WebSocket 原生协议客户端
  - OpenClaw / KimiClaw JSON-RPC 兼容层
  - 设备发现、Ed25519 设备身份、配对 token 管理
  - 角色上下文同步（可选 `mesh` feature，基于 syncthing-rust）
- **二进制（bin）**
  - 系统托盘常驻节点
  - 设备注册、心跳、任务/线程轮询
  - OS 通知与快速输入

## 命名说明

"Claw" 名字来自早期对外部 ZeroClaw / OpenClaw / KimiClaw 的参照，
在 Clarity 内部已重新定义为**分布式协作节点**概念。

- `clarity-claw`：client-side（本 crate）
- `clarity-gateway`：server-side
- `clarity-contract::claw_context` / `clarity-contract::federation`：跨 crate 共享契约

## 使用

```rust
use clarity_claw::{DeviceIdentity, OpenClawClient};

let device = DeviceIdentity::load_or_generate().unwrap();
let client = OpenClawClient::connect_with_device(
    "ws://127.0.0.1:18679",
    device,
    &device_token,
);

client.send_message("agent:main:main", "hello");
```

## Features

| Feature | 说明 |
|---------|------|
| `tray` | 启用系统托盘二进制所需的 GUI 依赖（`tao`/`tray-icon`/`notify-rust` 等）。二进制目标 `clarity-claw` 需要此 feature。 |
| `mesh` | 启用基于 `syncthing-rust` 的角色上下文离线同步。 |

## 测试

```bash
# 库默认 feature
cargo test -p clarity-claw --lib

# 带 mesh
cargo test -p clarity-claw --lib --features mesh

# 托盘二进制
cargo test -p clarity-claw --bin clarity-claw --features tray
```

## 边界与稳定性

- **Stability tier**: Experimental
- **MSRV**: 1.85（跟随 workspace）
- **库层面不依赖 `clarity-core` / `clarity-wire`**：
  - `clarity-core` 与 `clarity-wire` 仅由系统托盘二进制通过 `tray` feature 引入。
  - 前端依赖 `clarity-claw` 时应使用 `default-features = false`，避免拖入托盘 GUI 依赖。
