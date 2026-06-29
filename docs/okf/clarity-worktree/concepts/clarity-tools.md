---
id: clarity-tools
name: clarity-tools
type: tools
layer: infrastructure
depends_on: ["clarity-contract", "clarity-memory"]
consumed_by: ["clarity-core"]
---

# clarity-tools

Built-in tool library.

## Responsibilities

- File tools
- Shell/PowerShell tools
- Web search/fetch
- Devkit tools
- Task/team tools

## Notes

Split out from clarity-core to keep core smaller.
