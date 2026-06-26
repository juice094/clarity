---
id: clarity-telemetry
name: clarity-telemetry
type: telemetry
layer: infrastructure
depends_on:
- clarity-contract
consumed_by:
- clarity-gateway
title: clarity-telemetry
description: 'Unified telemetry: WideEvent, metrics, traces, config audit.'
tags:
- clarity
- infrastructure
- telemetry
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-telemetry

Unified telemetry: WideEvent, metrics, traces, config audit.

## Responsibilities

- `WideEvent`
- SQLite/GreptimeDB sinks
- Tracing layer
- Config audit

## Notes

Currently used by clarity-gateway.
