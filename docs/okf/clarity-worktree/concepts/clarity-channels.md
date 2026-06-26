---
id: clarity-channels
name: clarity-channels
type: channels
layer: infrastructure
depends_on:
- clarity-contract
consumed_by:
- clarity-core
title: clarity-channels
description: External communication channel abstraction.
tags:
- channels
- clarity
- infrastructure
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-channels

External communication channel abstraction.

## Responsibilities

- WeChat iLink (`chkit`) implementation
- Webhook adapter (enabled by default)
- Discord/Slack/Telegram stubs (disabled pending rustls-webpki fix)

## Notes

Not a full multi-channel bot matrix.
