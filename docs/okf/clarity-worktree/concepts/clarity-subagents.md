---
id: clarity-subagents
name: clarity-subagents
type: subagents
layer: infrastructure
depends_on: ["clarity-core"]
consumed_by: [""]
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
