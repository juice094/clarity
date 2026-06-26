---
type: test-case
id: TC-MEM-002
title: BM25 index ranks relevant documents first
description: Verify that the BM25 full-text index scores and ranks documents by term relevance.
component: clarity-memory
priority: high
status: implemented
tags: [test, clarity-memory, bm25, search]
related_concepts: [clarity-memory, bm25-index]
timestamp: 2026-06-26T12:00:00Z
---

# TC-MEM-002: BM25 index ranks relevant documents first

## Background

`clarity-memory::bm25` implements a BM25 full-text search index over memory
content. It must tokenize documents, build an index, and return ranked results
for a query.

## Preconditions

- A `Bm25Index` instance is available.

## Test Data

- Document 1: "Rust is a systems programming language with memory safety."
- Document 2: "Python is popular for data science and scripting."
- Document 3: "Memory safety in Rust comes from ownership and borrowing."
- Query: "Rust memory safety"

## Steps

1. Add the three documents to the index.
2. Search for the query.
3. Inspect the top-ranked document IDs.

## Expected Results

- Documents 1 and 3 appear before Document 2.
- Document 3 (explicit phrase match) scores at least as high as Document 1.
- A query with no matching terms returns an empty result.

## Actual Results

- Covered by `clarity-memory/src/bm25.rs` doctests and unit tests.

## Notes

- The index is rebuilt incrementally as documents are added.
