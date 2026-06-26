---
type: template
id: test-case-template
title: Test Case Template
description: Standard template for OKF-stored Clarity test cases.
tags: [template, test-case]
timestamp: 2026-06-26T12:00:00Z
---

# Test Case Template

Use this template when adding a new test case to the OKF bundle.

```yaml
---
type: test-case
id: TC-XXX-NNN
title: Concise description of what is being tested
description: One-sentence summary of the test objective.
component: clarity-xxx
priority: high | medium | low
status: planned | implemented | skipped | obsolete
tags: [test, clarity-xxx, ...]
related_concepts: [concept-id-1, concept-id-2]
timestamp: 2026-06-26T12:00:00Z
---

# TC-XXX-NNN: Title

## Background

Why this test exists and which feature/behavior it covers.

## Preconditions

- List conditions that must be true before executing the test.
- Example: `models.toml` exists, env var `OPENAI_API_KEY` is set.

## Test Data

- Inputs, files, environment variables, or registry entries used.

## Steps

1. Step one.
2. Step two.
3. Step three.

## Expected Results

- Describe the expected output, state change, or behavior.

## Actual Results

- Leave blank for planned cases; fill in after execution.

## Notes

- Edge cases, known limitations, or follow-up questions.
```
