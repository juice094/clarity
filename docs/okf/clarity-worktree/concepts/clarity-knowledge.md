---
id: clarity-knowledge
name: clarity-knowledge
type: knowledge
layer: infrastructure
depends_on: ["clarity-contract", "clarity-memory"]
consumed_by: ["clarity-core"]
---

# clarity-knowledge

Local knowledge indexing and AI-native interaction with activation dynamics.

## Responsibilities

- File-system scanning and incremental indexing
- Hybrid retrieval (BM25 + vector + graph)
- In-memory knowledge graph
- Dynamic knowledge field with spreading activation
- File-system change detection

## Notes

No dependency on Obsidian/Syncthing; works with plain Markdown and wikilinks.
