# tool_parser CLAUDE.md

## What it does

Parses tool calls from LLM text output into structured `Vec<ToolCall>`. Handles 4 formats
provider-agnostically: JSON, XML, MiniMax, Perl. Used by the agent loop to extract
tool invocations from non-native-tool-calling providers (prompt-guided tool use).

## Regex Architecture

All patterns are `LazyLock<Regex>` at module level — compiled once, reused. Never create
regexes inside parse functions. Patterns use `(?s)` (dot-matches-newline) for multi-line
tool bodies.

| Regex | Matches | Example |
|-------|---------|---------|
| `RE_TOOL` | `<tool name="..." .../>` or `<tool>...</tool>` | Self-closing + container |
| `RE_ARG` | `<arg key="...">...</arg>` or `<parameter name="...">...</parameter>` | Argument extraction |
| `RE_ATTR` | `key="value"` pairs | Attribute parsing for self-closing tags |
| `RE_INVOKE` | `<invoke name="...">...</invoke>` | Anthropic legacy format |
| `RE_PARAM` | `<parameter name="...">...</parameter>` | Inside `<invoke>` |
| `RE_GENERIC_ARG` | `<command>Get-ChildItem</command>` | Generic child tags as args |
| `RE_MINIMAX` | ` ```name\n{...}\n``` ` | MiniMax code-block format |
| — | `$name->({...})` | Perl-style (brace-counter, no regex) |

## Format Detection

`detect_tool_format()` scans content for distinctive markers. Order matters —
XML checked first because `<tool` is unambiguous. JSON checked last because
`"name"` can appear in non-tool text.

```
<tool → Xml | ``` + { → Minimax | $ + ->( → Perl | "name"+"arguments" → Json
```

## XML Parser: Three Sub-Patterns

The XML parser (`parse_xml_tool_calls`) is the most complex — it handles
three argument-passing conventions from different model families:

1. **Self-closing attributes**: `<tool name="sh" command="ls" timeout="30"/>`
   → `RE_ATTR` extracts `<tool>` tag attributes (skip `name` itself)

2. **Container with arg tags**: `<tool name="sh"><arg key="command">ls</arg></tool>`
   → `RE_ARG` extracts `<arg key="...">value</arg>` pairs from inner text

3. **Generic child tags (fallback)**: `<tool name="powershell"><command>ls</command></tool>`
   → `RE_GENERIC_ARG` extracts `<key>value</key>` when args are empty
   → Skips if open/close tag names mismatch, or match `arg`/`parameter` regex keywords

## Key Edge Cases

- **Self-closing vs container**: Same `RE_TOOL` regex matches both; inner capture is empty
  string for self-closing → args come from attributes instead
- **Invoke dedup**: After capturing `<tool>` patterns, check if `<invoke>` matches the
  same `name` and skip duplicate — prevents double-counting
- **Numeric attrs**: `serde_json::from_str::<Value>("30")` → `Value::Number(30)`, not
  string — intentional, models pass numeric params as numbers
- **Empty args fallback**: If `RE_ARG` finds nothing AND `RE_GENERIC_ARG` finds nothing,
  args is `{}` — valid, some models call parameterless tools
- **MiniMax non-JSON**: Content that doesn't start with `{` or `[` is wrapped in
  `{"content": "..."}` — preserves non-JSON arguments

## Adding a New Format

1. Add variant to `ToolFormat` enum
2. Add `parse_*` function with `Vec<ToolCall>` return
3. Add match arm in `parse_tool_calls()` dispatch
4. Add detection heuristic in `detect_tool_format()`
5. Add test for parse + test for detection

## Test Convention

Each format gets at least one happy-path test. XML gets extra tests for
self-closing attrs, generic child tags, and `<invoke>` format. Test data
is inline strings — no external fixtures.

```bash
cargo test -p clarity-core tool_parser
```
