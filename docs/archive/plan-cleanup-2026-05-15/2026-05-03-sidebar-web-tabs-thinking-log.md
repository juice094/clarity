# Plan: Sidebar Web Tabs + Thinking Log Migration

## Context

Target: Move "web browsing" and "AI thinking/tool-call trace" out of the Chat scroll area into the left Sidebar.
- **Web tabs**: URL list in Sidebar → click → async fetch → preview in Chat area (reuse existing file-preview glass card).
- **Thinking log**: Tool-call trace currently rendered below message list → migrate to a collapsible panel in Sidebar.

## Design Decisions

### 1. PreviewItem Enum (replaces `preview_file: Option<(String, String)>`)

Current `UiStore.preview_file` is a tuple `(name, content)`. We need to distinguish file preview vs web-page preview for icon and title rendering.

```rust
pub enum PreviewItem {
    File { name: String, content: String },
    WebPage { title: String, url: String, content: String },
}
```

Migration path:
- `UiStore.preview_file` → `UiStore.preview_item: Option<PreviewItem>`
- All read/write sites updated (`app_logic.rs`, `panels/task.rs`, `panels/chat/mod.rs`)

### 2. WebTab Model

```rust
pub struct WebTab {
    pub title: String,
    pub url: String,
}
```

Stored in `UiStore.web_tabs: Vec<WebTab>`, persisted to `gui-settings.json`.

### 3. Sidebar Layout (top to bottom)

```
[×] collapse button
── Category Nav ──
[Emotion] [Knowledge] [Engineering]

┌─ 🌐 Web Tabs ▼ ───────────────┐  ← default expanded
│ PyTorch Docs            [×]   │
│ arXiv 论文              [×]   │
│ + Add link                    │
└───────────────────────────────┘

┌─ 🔧 Tools & Tasks ▶ ──────────┐  ← existing, keep default collapsed
│ ...                           │
└───────────────────────────────┘

┌─ 💭 Thinking Log ▼ ───────────┐  ← default collapsed, auto-expand when is_loading
│ 🔍 web_search("ReadFrog")    │
│ 📄 file_read("Cargo.toml")   │
│ ✅ Think("分析需求...")       │
└───────────────────────────────┘

[🛠 Skills] [中/EN] [Token]
```

### 4. Async Web Fetch Flow

```
User clicks URL in Sidebar
    → app.runtime.spawn(async fetch)
        → reqwest::get(url) → html_to_text()
        → ui_tx.send(UiEvent::WebPageFetched { title, url, content })
    → process_events() handles UiEvent
        → ui_store.preview_item = Some(PreviewItem::WebPage { ... })
    → next frame: Chat area renders glass card preview
```

HTML-to-text logic is copied from `clarity-core/src/tools/web.rs::WebFetchTool::html_to_text` (regex-based, no external deps beyond `regex` and `html_escape`).

### 5. Thinking Log Data Source

Current source: `chat_store.tool_calls: Vec<ToolCallInfo>` (global, cleared on each `send()`).
- No backend change needed for MVP.
- Rendering: iterate `tool_calls`, show icon + name + arguments (truncated) + status dot.
- Future: extend `ToolCallInfo` with `thinking_text` field when `ThinkTool` output is wired to frontend.

## File Change List

| # | File | Change Type | Description |
|---|------|-------------|-------------|
| 1 | `crates/clarity-egui/src/ui/types.rs` | Modify | Add `PreviewItem` enum, `WebTab` struct |
| 2 | `crates/clarity-egui/src/stores/mod.rs` | Modify | Replace `preview_file` with `preview_item`; add `web_tabs`, `web_tabs_expanded`, `thinking_log_expanded` |
| 3 | `crates/clarity-egui/src/app_logic.rs` | Modify | Update `preview_file` init; add `web_tabs` init |
| 4 | `crates/clarity-egui/src/panels/task.rs` | Modify | Update `preview_file` write → `preview_item` |
| 5 | `crates/clarity-egui/src/panels/chat/mod.rs` | Modify | Match `PreviewItem` for rendering; different icon/title |
| 6 | `crates/clarity-egui/src/panels/chat/message_list.rs` | Modify | Remove `tool_call_bubble` rendering |
| 7 | `crates/clarity-egui/src/ui/render.rs` | Modify | Remove or deprecate `tool_call_bubble` |
| 8 | `crates/clarity-egui/src/panels/sidebar.rs` | Modify | Insert Web Tabs panel + Thinking Log panel |
| 9 | `crates/clarity-egui/src/components/web_tabs.rs` | **New** | Web tab list UI: scrollable list, delete button, "+ Add" button |
| 10 | `crates/clarity-egui/src/components/thinking_log.rs` | **New** | Thinking log UI: tool call list with status icons |
| 11 | `crates/clarity-egui/src/services/web_fetch.rs` | **New** | Async `fetch_web_page(url) -> Result<String, String>` + html_to_text |
| 12 | `crates/clarity-egui/src/ui/types.rs` | Modify | Add `UiEvent::WebPageFetched` variant |
| 13 | `crates/clarity-egui/src/handlers/mod.rs` | Modify | Route `WebPageFetched` event |
| 14 | `crates/clarity-egui/src/handlers/chat.rs` | Modify | Add `on_web_page_fetched` handler |
| 15 | `crates/clarity-egui/src/settings.rs` | Modify | Serialize/deserialize `web_tabs` in `gui-settings.json` |

## Implementation Order

**Phase A — Data Model (files 1–3)**
- Add types, update stores, update init logic
- Compile check

**Phase B — Chat Preview Migration (files 4–7)**
- Replace `preview_file` with `preview_item` everywhere
- Remove `tool_call_bubble` from message_list
- Compile check

**Phase C — Sidebar Panels (files 8–10)**
- Add `web_tabs.rs` component
- Add `thinking_log.rs` component
- Wire into `sidebar.rs`
- Compile check

**Phase D — Async Fetch (files 11–14)**
- Implement `services/web_fetch.rs`
- Add `UiEvent::WebPageFetched`
- Wire click handler → spawn fetch → event → preview_item
- Compile check

**Phase E — Persistence (file 15)**
- Save/load `web_tabs` from `gui-settings.json`
- Full test suite

## Risk & Mitigation

| Risk | Mitigation |
|------|-----------|
| Borrow checker issues in `sidebar.rs` with multiple `app` field accesses | Extract `&theme` and `&mut ui_store` references before closures; clone URL strings before async spawn |
| `html_to_text` regex compilation per fetch (slow) | Use `lazy_static` or `once_cell` for compiled regexes in `web_fetch.rs` |
| Web fetch blocks UI if not truly async | Must use `app.runtime.spawn`, never block `update()` thread |
| `gui-settings.json` backward compat | Use `#[serde(default)]` on new `web_tabs` field; old files load as empty vec |
| Sidebar vertical overflow (too many panels) | All panels collapsible; ScrollArea already present; set max heights |

## Testing Strategy

1. `cargo check -p clarity-egui` after each phase
2. `cargo test --workspace --lib` at end
3. Manual verification:
   - Add a web tab (e.g., `https://doc.rust-lang.org/book/`)
   - Click → observe glass card preview in Chat area
   - Close preview → verify chat scrolls clean
   - Send a message that triggers tool calls → observe Thinking Log auto-populate
   - Restart app → verify web tabs persist

## Subagent Decomposition

Recommended delegation:
- **Coder A**: Phase A + Phase B (data model + Chat preview migration) — 6 files, mechanical changes
- **Coder B**: Phase C + Phase D (Sidebar panels + async fetch) — new components + wiring
- **Coder A (resume)**: Phase E (persistence) after Coder B completes

Both subagents need the plan file as context.
