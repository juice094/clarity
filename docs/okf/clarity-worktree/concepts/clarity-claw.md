---
id: clarity-claw
name: clarity-claw
type: claw
layer: presentation
depends_on: ["clarity-core"]
consumed_by: [""]
---

# clarity-claw

System-tray background monitor.

## Responsibilities

- Tray icon
- OS notifications
- Gateway WebSocket client
- Task monitoring

## Notes

Only communicates through Gateway WebSocket; not an external OpenClaw adapter.
