# Kimi Desktop v3 Conversation Style

Extracted from `ConversationView` CSS/JS (v3.0.15, 2026-06-06).

## Toggle

Set `ui_store.kimi_conversation_style = true` to switch from legacy bubble rendering to Kimi-style layout.

## Components

| Component | Kimi CSS Source | Status |
|-----------|----------------|--------|
| `message_bubble` | `.msg`, `.msg-user`, `.msg-assistant`, `.bubble` | Integrated |
| `thinking_block` | `.think-block` + shimmer + max-height:340px | Available |
| `tool_group` | `.tool-group` + dashed timeline | Available |
| `subagent_group` | `.subagent-group` + ordinal/status | Available |
| `approval_dock` | `.approval-prompt` (radius:20 + shadow) | Available |
| `knowledge_panel` | `.knowledge-panel` (386px right) | Available |

## Shared Primitives

All components reuse: `CardStyle`, `collapsible`, `streaming_cursor`, `step_list`.

## Next Steps

1. **Settings toggle**: Surface `kimi_conversation_style` in Settings UI
2. **ApprovalDock integration**: Wire into `panels::approval` rendering
3. **KnowledgePanel integration**: As `SidePanel::Right` variant
4. **Visual polish**: Run app, compare with Kimi screenshots, fine-tune colors/spacing
5. **Virtual list optimization**: Current `message_bubble` renders all messages; integrate with existing virtual scroll
