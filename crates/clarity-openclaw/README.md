# clarity-openclaw

OpenClaw / KimiClaw Gateway client for Project Clarity.

Provides a reusable, UI-agnostic library for:

- Connecting to an OpenClaw Gateway over WebSocket.
- Device-paired authentication (Ed25519 challenge signing).
- Discovering local and remote OpenClaw devices.
- Sending chat messages and receiving streaming agent replies.

This crate intentionally has no dependency on any frontend or GUI crate, so it
can be consumed by `clarity-egui`, `clarity-tui`, `clarity-gateway`, and
`clarity-claw` alike.

## Usage

```rust
use clarity_openclaw::{DeviceIdentity, OpenClawClient};

let device = DeviceIdentity::load_or_generate().unwrap();
let client = OpenClawClient::connect_with_device(
    "ws://127.0.0.1:18679",
    device,
    &device_token,
);

client.send_message("agent:main:main", "hello");
```

See the `clarity-egui` examples (`openclaw_pair`, `openclaw_device_check`) for
end-to-end pairing and chat flows.

## License

AGPL-3.0-or-later, consistent with the rest of Project Clarity.
