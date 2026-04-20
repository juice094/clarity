---
id: bug-investigate
name: Bug Investigation
version: "1.0.0"
description: Structured root-cause analysis for bugs and incidents
tools:
  - file_read
  - grep
  - bash
  - glob
tags:
  - debug
  - incident
  - support
---

## Investigation Framework

Follow the 5 Whys method to reach root cause.

### Phase 1: Reproduction
1. Read the bug report or error message carefully
2. Identify the minimal steps to reproduce
3. Run the failing command or test locally
4. Capture full error output and stack traces

### Phase 2: Context Gathering
1. Check recent commits that touched relevant files (`git log --oneline -- <path>`)
2. Review related configuration files
3. Check environment differences (dev vs prod)
4. Look for related issues or logs

### Phase 3: Hypothesis & Validation
1. Formulate 2-3 possible causes
2. For each hypothesis, identify a quick validation test
3. Run validations and record results
4. Eliminate hypotheses that don't match evidence

### Phase 4: Fix & Verify
1. Implement the minimal fix for the root cause
2. Run the reproduction steps to confirm fix
3. Run full test suite to prevent regressions
4. Document the root cause for future reference

## Output Format

```markdown
## Bug Investigation: <title>

### Symptoms
<what the user observed>

### Root Cause
<the underlying issue identified via 5 Whys>

### Evidence
- <fact 1>
- <fact 2>

### Fix
<description of the fix>

### Prevention
<how to prevent this class of bug in the future>
```
