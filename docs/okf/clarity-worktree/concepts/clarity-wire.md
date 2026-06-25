---
id: clarity-wire
name: clarity-wire
type: wire
layer: contract
depends_on: ["clarity-contract"]
consumed_by: ["clarity-core", "clarity-egui", "clarity-tui", "clarity-gateway", "clarity-claw", "clarity-headless", "clarity-mobile-core", "clarity-slint"]
---

# clarity-wire

UI ↔ Agent event bus using SPMC channels.

## Responsibilities

- `WireMessage` protocol
- `ViewCommand`
- `WireBroadcaster`

## Notes

Cross-frontend communication must go through this crate.
