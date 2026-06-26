---
id: clarity-gateway
name: clarity-gateway
type: gateway
layer: presentation
depends_on:
- clarity-core
- clarity-wire
- clarity-memory
- clarity-telemetry
consumed_by: []
title: clarity-gateway
description: Axum HTTP/WebSocket server and Web IDE.
tags:
- clarity
- gateway
- presentation
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-gateway

Axum HTTP/WebSocket server and Web IDE.

## Responsibilities

- Public API on :18790
- Admin + Web UI on :18800
- Session store
- SSE/WebSocket endpoints
- MCP server exposure

## Notes

Can be built as bin or lib.
