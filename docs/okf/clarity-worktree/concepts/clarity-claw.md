---
id: clarity-claw
name: clarity-claw
type: claw
layer: presentation
depends_on:
- clarity-core
consumed_by: []
title: clarity-claw
description: System-tray background monitor.
tags:
- clarity
- claw
- presentation
timestamp: '2026-06-26T11:28:50Z'
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
