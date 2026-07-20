//! Local knowledge indexing and AI-native interaction for Project Clarity.
//!
//! `clarity-knowledge` treats the local file system as a knowledge base that
//! Agents can query, navigate, and modify. It does not depend on Obsidian,
//! Syncthing, or any external service; it works with plain files and open
//! conventions such as Markdown, YAML frontmatter, and wikilinks.
//!
//! ## Layer overview
//!
//! - [`index`]: file-system scanning, incremental indexing, and search API.
//! - [`extract`]: parsing Markdown, wikilinks, tags, and frontmatter.
//! - [`graph`]: in-memory knowledge graph of files, headings, blocks, and tags.
//! - [`field`]: dynamic knowledge field with activation and spreading retrieval.
//! - [`search`]: query types and result types for hybrid retrieval.
//! - [`watcher`]: file-system change detection.
//! - [`error`]: shared error types.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod error;
pub mod export;
pub mod extract;
pub mod field;
pub mod graph;
pub mod index;
pub mod recall_store;
pub mod retrieval;
pub mod search;
pub mod vault_config;
pub mod watcher;

pub use error::{KnowledgeError, Result};
pub use export::ObsidianExporter;
pub use extract::{ExtractedDocument, MarkdownExtractor, WikiLink};
pub use field::{FieldConfig, FieldResult, KnowledgeField};
pub use graph::{EdgeKind, KnowledgeGraph, NodeId, NodeKind};
pub use index::{FileEvent, InMemoryIndex, KnowledgeIndex, SourceConfig};
pub use recall_store::{OutcomeSignal, OutcomeSignalType, RecallEvent, RecallStore};
pub use retrieval::{HybridRetriever, ParsedQuery};
pub use search::{SearchQuery, SearchResult};
pub use vault_config::{
    AppConfig, DailyNotesConfig, LinkResolver, NewFileLocation, TemplatesConfig, VaultConfig,
    ViewMode,
};
pub use watcher::{FileWatcher, NotifyWatcher, WatcherEvent};

/// Re-export of the current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
