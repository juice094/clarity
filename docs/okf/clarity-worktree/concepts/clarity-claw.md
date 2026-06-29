---
id: clarity-claw
name: clarity-claw
type: claw
layer: presentation
depends_on: ["clarity-contract"]
consumed_by: ["clarity-egui"]
---

# clarity-claw

Unified client-side Claw node: UI-agnostic library + system-tray binary.

## Responsibilities

- Gateway WebSocket client
- OpenClaw/KimiClaw JSON-RPC compatibility layer
- Device discovery / identity / pairing
- Role-context sync
- Tray icon and OS notifications
- Task monitoring

## Notes

Merged from former clarity-openclaw; internal Clarity mesh uses Gateway WebSocket, OpenClaw JSON-RPC is external fallback.
