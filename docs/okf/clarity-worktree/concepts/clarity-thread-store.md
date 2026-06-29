---
id: clarity-thread-store
name: clarity-thread-store
type: thread-store
layer: infrastructure
depends_on: ["clarity-contract", "clarity-rollout"]
consumed_by: ["clarity-core"]
---

# clarity-thread-store

Thread persistence abstraction.

## Responsibilities

- `ThreadStore` trait
- `LocalThreadStore`
- `LiveThread`
- Thread lifecycle persistence

## Notes

Depends on clarity-rollout for JSONL event logs.
