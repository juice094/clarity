---
id: clarity-mcp
name: clarity-mcp
type: mcp
layer: infrastructure
depends_on:
- clarity-contract
- clarity-wire
consumed_by:
- clarity-llm
- clarity-core
title: clarity-mcp
description: MCP client with stdio / SSE / HTTP / WebSocket transports.
tags:
- clarity
- infrastructure
- mcp
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-mcp

MCP client with stdio / SSE / HTTP / WebSocket transports.

## Responsibilities

- MCP server lifecycle
- Command validation / allowlist
- Transport abstraction

## Notes

Includes a local `clarity-dev` MCP server for build tasks.
