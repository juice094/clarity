---
id: clarity-gateway
name: clarity-gateway
type: gateway
layer: presentation
depends_on: ["clarity-core", "clarity-wire", "clarity-memory", "clarity-telemetry"]
consumed_by: [""]
---

# clarity-gateway

Axum HTTP/WebSocket server and Web IDE.

## Responsibilities

- Public API on :18790
- Admin + Web UI on :18800
- Session store
- SSE/WebSocket endpoints
- MCP server exposure
- Optional Anthropic Messages API (`/v1/messages`) via `anthropic-api` feature

## Notes

Can be built as bin or lib.
