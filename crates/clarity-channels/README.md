# clarity-channels

External communication channels for Clarity — a pluggable, platform-agnostic messaging abstraction.

## 职责

- **Channel trait** — async abstraction for sending/receiving messages across platforms
- **WeChat iLink** — migrated ZeroClaw-compatible implementation with QR login, AES encryption, and HMAC auth
- **Platform stubs** — Discord / Slack / Telegram webhook/bot adapters (to be hardened)
- **Retry / backoff** — shared retry policy for HTTP-based channels

## 关键类型

- `Channel` — async trait implemented by every messaging backend
- `ChannelMessage` / `Attachment` / `AttachmentKind` — normalized message model
- `ChannelError` — unified error enum for all platforms
- `zeroclaw::WeChatChannel` — WeChat iLink channel implementation
- `retry::RetryPolicy` — exponential backoff helper

## 测试

```bash
cargo test -p clarity-channels --lib
```

## 边界与稳定性

- **Stability tier**: Experimental
  - API may change while platform adapters are finalized
- **MSRV**: 1.85.0
- **反向依赖禁止** (No reverse dependencies):
  - 不得依赖任何 frontend crate（egui/tui/gateway/slint）
- **Library/binary classification**:
  - Library: designed for `use` by other crates

## Retry policy

Use `clarity_channels::retry::RetryPolicy` for outbound HTTP calls:

```rust
use clarity_channels::retry::RetryPolicy;
use std::time::Duration;

let policy = RetryPolicy::new()
    .with_max_attempts(5)
    .with_base_delay(Duration::from_millis(250));
```
