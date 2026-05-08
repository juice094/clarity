# clarity-memory

OpenHanako-style memory system for Clarity - a multi-level memory compilation framework for AI agents.

## Overview

This crate implements a four-level memory system inspired by [OpenHanako](https://github.com/mmoono/openhanako):

1. **Today** - Recent conversation summaries (last 24 hours)
2. **Week** - Aggregated daily summaries (last 7 days)
3. **Long-term** - Compressed historical context
4. **Facts** - Structured fact database with FTS5 search

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Memory System                         │
├─────────────────────────────────────────────────────────────┤
│  Session Store (JSONL)    │    Fact Store (SQLite + FTS5)   │
│  ├── conversations/       │    ├── facts                    │
│  ├── summaries/           │    ├── facts_fts                │
│  └── metadata             │    └── triggers                 │
├─────────────────────────────────────────────────────────────┤
│                    Four-Level Compiler                       │
│  ┌─────────┐    ┌─────────┐    ┌──────────┐    ┌─────────┐ │
│  │  Today  │───→│  Week   │───→│ Long-term│───→│  Facts  │ │
│  └─────────┘    └─────────┘    └──────────┘    └─────────┘ │
├─────────────────────────────────────────────────────────────┤
│                    Turn-Based Ticker                         │
│              (Triggers every N turns)                        │
└─────────────────────────────────────────────────────────────┘
```

## Features

- **SQLite + FTS5**: Fast full-text search over facts with JSON tag storage
- **JSONL Sessions**: Efficient append-only conversation storage
- **Four-Level Compilation**: Automatic summarization at different time scales
- **Turn-Based Triggering**: Configurable compilation thresholds
- **Fact Extraction**: LLM-powered metadata extraction from conversations
- **Fingerprint-Based Deduplication**: Avoid redundant compilations

## Usage

### Basic Example

```rust
use clarity_memory::{
    MemoryStore, SessionStore, MemoryCompiler, MemoryTicker, 
    CompileConfig, LlmClient
};
use std::sync::Arc;

// Initialize stores
let memory_store = MemoryStore::new("memory.db".as_ref())?;
let session_store = SessionStore::new("sessions")?;

// Create compiler
let llm_client: Arc<dyn LlmClient> = Arc::new(MyLlmClient::new());
let config = CompileConfig::default();
let compiler = MemoryCompiler::new(
    memory_store, 
    session_store.clone(), 
    llm_client, 
    config
);

// Set up turn-based ticker
let mut ticker = MemoryTicker::new("output", Some(6));
ticker.set_compile_callback(move || {
    // This will be called every 6 turns
    Box::pin(async move {
        compiler.compile_all("output".as_ref()).await
    })
});

// On each message
session_store.append_message("session-1", "user", "Hello!")?;
if let Some(future) = ticker.notify_turn("session-1") {
    let results = future.await?;
    println!("Compiled: {:?}", results);
}
```

### Fact Extraction

```rust
use clarity_memory::{FactExtractor, LlmClient};

let extractor = FactExtractor::new(llm_client, "gpt-4");
let facts = extractor.extract_facts(
    "User mentioned they love Rust programming"
).await?;

for fact in facts {
    println!("Fact: {}", fact.fact);
    println!("Tags: {:?}", fact.tags);
}
```

### Memory Search

```rust
// Search by tags
let facts = memory_store.search_by_tags(
    &["preference".to_string(), "tech".to_string()], 
    10
)?;

// Full-text search
let facts = memory_store.search_fulltext("Rust programming", 10)?;
```

## Memory Output Format

The final `memory.md` file has four sections:

```markdown
# Memory

## 1. Key Facts
- User prefers Rust over Python
- User is building an AI agent

## 2. Today
Summary of recent conversations...

## 3. This Week
Aggregated weekly summary...

## 4. Long-term
Compressed historical context...
```

## Comparison with OpenHanako

| Feature | OpenHanako (JS) | clarity-memory (Rust) |
|---------|-----------------|----------------------|
| Storage | SQLite + FTS5 | SQLite + FTS5 |
| Sessions | JSONL | JSONL |
| Compilation | 4 levels | 4 levels (ported) |
| Trigger | Turn-based | Turn-based |
| Facts | LLM extraction | LLM extraction |
| Deduplication | Fingerprint | Fingerprint (SHA256) |

## Integration with clarity-core

See `examples/integration_core.rs` for a complete example of integrating with `clarity-core`:

```rust
// In your Agent struct
pub struct Agent {
    memory_store: MemoryStore,
    session_store: SessionStore,
    ticker: MemoryTicker,
}

impl Agent {
    pub async fn process_message(&mut self, content: &str) -> Result<()> {
        self.session_store.append_message("session", "user", content)?;
        
        // Trigger compilation if threshold reached
        if let Some(future) = self.ticker.notify_turn("session") {
            future.await?;
        }
        Ok(())
    }
}
```

## Testing

```bash
# Run all tests
cargo test -p clarity-memory

# Run examples
cargo run --example basic_usage
cargo run --example integration_core
```

## 边界与稳定性

- **Stability tier**: Stable
  - Stable: API unlikely to change in minor releases
- **MSRV**: 1.78.0
- **反向依赖禁止** (No reverse dependencies):
  - 不得依赖 clarity-core
- **Library/binary classification**:
  - Library: designed for `use` by other crates

## License

MIT
