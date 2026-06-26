---
type: test-case
id: TC-CORE-002
title: Tool registry registers and invokes a tool
description: Verify that ToolRegistry accepts a tool registration, returns its schema, and invokes it correctly.
component: clarity-core
priority: high
status: implemented
tags: [test, clarity-core, tools, registry]
related_concepts: [clarity-core, clarity-tools, tool-registry]
timestamp: 2026-06-26T12:00:00Z
---

# TC-CORE-002: Tool registry registers and invokes a tool

## Background

`clarity-core::registry::ToolRegistry` is the central registry for tools
available to the Agent loop. It must support registration, lookup by name,
schema enumeration, and safe invocation.

## Preconditions

- A `ToolRegistry` instance is available.
- A simple tool implementation (e.g. a no-op or echo tool).

## Test Data

- Tool name: `test_echo`.
- Schema: one required string argument `message`.
- Invocation argument: `{ "message": "hello" }`.

## Steps

1. Register the tool with `register`.
2. Call `get("test_echo")` and verify it returns the tool.
3. Call `list()` and verify the tool appears.
4. Call `get_schemas()` and verify the schema is present.
5. Invoke the tool and verify the output.

## Expected Results

- Registration succeeds.
- Lookup returns the registered tool.
- List and schema enumeration include the tool.
- Invocation returns the expected output.

## Actual Results

- Covered by `clarity-core/src/registry/tests.rs`.

## Notes

- Unregister and duplicate-registration edge cases are tested separately.
