---
type: test-case
id: TC-CORE-001
title: Approval request can be created and resolved
description: Verify that the approval runtime accepts a request, surfaces it to consumers, and records the resolved outcome.
component: clarity-core
priority: high
status: implemented
tags: [test, clarity-core, approval, runtime]
related_concepts: [clarity-core, approval-flow]
timestamp: 2026-06-26T12:00:00Z
---

# TC-CORE-001: Approval request can be created and resolved

## Background

`clarity-core::approval` implements a four-mode approval system. The runtime
must support creating a pending request, listing pending requests, and resolving
a request via approve/reject/cancel without race conditions.

## Preconditions

- An `ApprovalRuntime` instance is available.
- A `ToolCallRequest` or equivalent operation that requires approval.

## Test Data

- Tool name: `shell`.
- Arguments: `{ "command": "echo hello" }`.
- Approval mode: `interactive`.

## Steps

1. Create an approval request with `create_request`.
2. Verify `list_pending()` returns the request.
3. Call `approve(request_id)`.
4. Verify `list_pending()` no longer returns the request.
5. Verify the resolved record is stored.

## Expected Results

- `create_request` returns a stable `request_id`.
- `list_pending()` contains the request before resolution.
- `approve` succeeds and the request disappears from pending.
- The resolved record records the outcome and timestamp.

## Actual Results

- Covered by `clarity-core/src/approval/tests.rs`.

## Notes

- Smart-mode batch grants and yolo-mode auto-approval are separate scenarios.
