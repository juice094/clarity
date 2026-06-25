---
id: clarity-channels
name: clarity-channels
type: channels
layer: infrastructure
depends_on: ["clarity-contract"]
consumed_by: ["clarity-core"]
---

# clarity-channels

External communication channel abstraction.

## Responsibilities

- WeChat iLink (`chkit`) implementation
- Webhook adapter (enabled by default)
- Discord/Slack/Telegram stubs (disabled pending rustls-webpki fix)

## Notes

Not a full multi-channel bot matrix.
