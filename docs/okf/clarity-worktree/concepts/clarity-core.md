---
id: clarity-core
name: clarity-core
type: core
layer: kernel
depends_on:
- clarity-contract
- clarity-wire
- clarity-memory
- clarity-mcp
- clarity-llm
- clarity-tools
- clarity-channels
- clarity-secrets
- clarity-thread-store
consumed_by:
- clarity-gateway
- clarity-egui
- clarity-tui
- clarity-claw
- clarity-headless
- clarity-mobile-core
- clarity-subagents
- clarity-telemetry
- clarity-anthropic-proxy
title: clarity-core
description: 'Agent kernel: ReAct/Plan loop, Approval, Skill, MCP integration.'
tags:
- clarity
- core
- kernel
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-core

Agent kernel: ReAct/Plan loop, Approval, Skill, MCP integration.

## Responsibilities

- Agent loop (`Agent`, `AgentController`, `Op`)
- ReAct/Plan execution
- Streaming event dispatch
- Approval runtime (Interactive/Smart/Plan/Yolo)
- Skill loading/discovery
- MCP integration
- Background task management
- Thread/session lifecycle
- `ViewState` UI state machine

## Notes

Must have zero dependencies on frontend or network crates.
