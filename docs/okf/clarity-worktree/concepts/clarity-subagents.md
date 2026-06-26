---
id: clarity-subagents
name: clarity-subagents
type: subagents
layer: infrastructure
depends_on:
- clarity-core
consumed_by: []
title: clarity-subagents
description: Sub-agent executor and parallel scheduler.
tags:
- clarity
- infrastructure
- subagents
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-subagents

Sub-agent executor and parallel scheduler.

## Responsibilities

- `SubAgentManager`
- `AgentPool`
- Team coordination
- Parallel execution

## Notes

Consumes clarity-core; not a dependency of core.
