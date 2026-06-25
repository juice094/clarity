---
id: clarity-openclaw
name: clarity-openclaw
type: openclaw
layer: infrastructure
depends_on: ["clarity-contract"]
consumed_by: ["clarity-core", "clarity-egui"]
---

# clarity-openclaw

OpenClaw/KimiClaw Gateway WebSocket client and device identity.

## Responsibilities

- Device discovery
- Paired token management
- Gateway WebSocket dialect detection
- Protocol translation fallback

## Notes

Internal Clarity mesh uses Gateway WebSocket; OpenClaw JSON-RPC is external fallback.
