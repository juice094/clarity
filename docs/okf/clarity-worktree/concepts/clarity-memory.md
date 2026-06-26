---
id: clarity-memory
name: clarity-memory
type: memory
layer: infrastructure
depends_on:
- clarity-contract
consumed_by:
- clarity-core
- clarity-gateway
- clarity-mobile-core
title: clarity-memory
description: 'Hybrid memory: SQLite + BM25 + vector search.'
tags:
- clarity
- infrastructure
- memory
timestamp: '2026-06-26T11:28:50Z'
---

# clarity-memory

Hybrid memory: SQLite + BM25 + vector search.

## Responsibilities

- BM25 keyword retrieval
- Vector/cosine similarity search
- Chunking
- Four-level compaction/archive
- Session persistence

## Notes

Features: `sqlite`, `embedding`.
