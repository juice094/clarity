---
type: test-case
id: TC-MEM-001
title: In-memory store adds and queries memories
description: Verify that the in-memory memory store accepts observations and returns relevant results on query.
component: clarity-memory
priority: high
status: implemented
tags: [test, clarity-memory, memory-store, crud]
related_concepts: [clarity-memory, memory-store]
timestamp: 2026-06-26T12:00:00Z
---

# TC-MEM-001: In-memory store adds and queries memories

## Background

`clarity-memory` provides both in-memory and persistent memory stores. The
in-memory variant is used in tests and short-lived contexts. It must support
adding observations and querying by relevance or recency.

## Preconditions

- An `InMemoryMemoryStore` instance is available.

## Test Data

- Observation 1: "User prefers Rust for systems programming."
- Observation 2: "User likes Python for scripting."
- Query: "What language does the user prefer for systems code?"

## Steps

1. Add both observations to the store.
2. Query the store with the query string.
3. Inspect returned memories.

## Expected Results

- Both observations are stored.
- The query returns Observation 1 with higher relevance than Observation 2.
- `clear()` removes all observations.

## Actual Results

- Covered by `clarity-memory/src/memory/store/tests.rs`.

## Notes

- Persistent store variants (SQLite, hermes) are covered by integration tests.
