---
id: code-review
name: Code Review
version: "1.0.0"
description: Systematic code review with security, performance, and style checks
tools:
  - file_read
  - grep
  - glob
tags:
  - review
  - quality
  - security
---

## Review Checklist

### Security
- [ ] No hardcoded secrets or API keys
- [ ] Input validation on all public interfaces
- [ ] SQL injection prevention (parameterized queries)
- [ ] Unsafe code blocks justified and minimal

### Performance
- [ ] No unnecessary allocations in hot paths
- [ ] Async operations use `tokio::spawn` when appropriate
- [ ] Database queries have appropriate indexes
- [ ] Large files are streamed, not buffered entirely

### Style & Maintainability
- [ ] Follows project `rustfmt` configuration
- [ ] Error handling uses `thiserror` or `anyhow` consistently
- [ ] Public APIs have doc comments
- [ ] Complex logic has inline comments explaining "why"

### Testing
- [ ] New features have unit tests
- [ ] Edge cases are covered
- [ ] Integration tests verify end-to-end behavior

## Review Output Format

```markdown
## Code Review: <file or PR>

### Summary
<one-paragraph overview>

### Issues Found
| Severity | Line | Description |
|----------|------|-------------|
| <critical|warning|note> | <line> | <description> |

### Recommendations
1. <specific actionable suggestion>
2. ...

### Approval
- [ ] Approved with no changes
- [ ] Approved with minor suggestions
- [ ] Changes requested
```
