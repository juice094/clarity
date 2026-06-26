---
id: clarity-rollout
name: clarity-rollout
type: rollout
layer: infrastructure
depends_on:
- clarity-contract
consumed_by:
- clarity-thread-store
title: clarity-rollout
description: JSONL rollout persistence for thread event logs.
tags:
- clarity
- infrastructure
- rollout
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-rollout

JSONL rollout persistence for thread event logs.

## Responsibilities

- `RolloutRecorder`
- `RolloutItem`
- Compaction/replacement history
- Event replay

## Notes

API design inspired by OpenAI Codex; original Clarity implementation.
