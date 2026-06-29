---
id: clarity-rollout
name: clarity-rollout
type: rollout
layer: infrastructure
depends_on: ["clarity-contract"]
consumed_by: ["clarity-thread-store"]
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
