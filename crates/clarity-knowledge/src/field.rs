//! Dynamic knowledge field: a graph whose nodes carry activation energy.
//!
//! A [`KnowledgeField`] wraps a [`KnowledgeGraph`] and a [`HybridRetriever`].
//! Retrieval is no longer a static similarity search: anchor nodes are
//! activated by the query, energy spreads through the graph, and the most
//! activated regions surface as results.

use crate::WatcherEvent;
use crate::error::Result;
use crate::extract::{ExtractedDocument, MarkdownExtractor};
use crate::graph::{Importance, KnowledgeGraph, NodeId, NodeKind};
use crate::recall_store::{OutcomeSignal, RecallEvent, RecallStore};
use crate::retrieval::HybridRetriever;
use crate::search::SearchQuery;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Configuration for activation dynamics in a knowledge field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldConfig {
    /// Number of spreading-activation iterations performed after each query.
    pub spreading_iterations: usize,
    /// Half-life for session/message node activation.
    pub message_half_life: Duration,
    /// Half-life for file/tag node activation.
    pub file_half_life: Duration,
    /// Number of top nodes that participate in lateral inhibition.
    pub top_k_inhibition: usize,
    /// Strength of lateral inhibition.
    pub inhibition_beta: f32,
    /// Activation threshold below which a node may be marked dormant.
    pub dormant_threshold: f32,
    /// Minimum time below threshold before a node is marked dormant.
    pub dormant_min_age: Duration,
}

impl Default for FieldConfig {
    fn default() -> Self {
        Self {
            spreading_iterations: 3,
            message_half_life: Duration::from_secs(600),
            file_half_life: Duration::from_secs(86_400),
            top_k_inhibition: 7,
            inhibition_beta: 0.3,
            dormant_threshold: 0.01,
            dormant_min_age: Duration::from_secs(3_600),
        }
    }
}

/// A single result from the knowledge field.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldResult {
    /// Path to the file associated with this node, if it is a file node.
    pub path: PathBuf,
    /// Human-readable title.
    pub title: Option<String>,
    /// Snippet or summary.
    pub snippet: String,
    /// Activation level after spreading.
    pub activation: f32,
    /// Original hybrid retrieval score, when this node was a direct match.
    pub semantic_score: f64,
    /// Tags matched by the query.
    pub matched_tags: Vec<String>,
}

/// A dynamic knowledge field combining hybrid retrieval with graph activation.
#[derive(Debug)]
pub struct KnowledgeField {
    graph: parking_lot::RwLock<KnowledgeGraph>,
    retriever: parking_lot::RwLock<HybridRetriever>,
    config: FieldConfig,
    recall_store: Option<std::sync::Arc<parking_lot::Mutex<RecallStore>>>,
}

impl KnowledgeField {
    /// Create an empty knowledge field with the given configuration.
    pub fn new(config: FieldConfig) -> Self {
        Self {
            graph: parking_lot::RwLock::new(KnowledgeGraph::new()),
            retriever: parking_lot::RwLock::new(HybridRetriever::new()),
            config,
            recall_store: None,
        }
    }

    /// Attach a recall-effectiveness store to the field.
    ///
    /// The store persists recall events and outcome signals so that the field
    /// can learn which memories correlate with successful sessions.
    pub fn with_recall_store(mut self, store: RecallStore) -> Self {
        self.recall_store = Some(std::sync::Arc::new(parking_lot::Mutex::new(store)));
        self
    }

    /// Index or re-index a document.
    ///
    /// The document is added to the hybrid retriever and its metadata is
    /// reflected in the knowledge graph (file node, tags, outgoing links).
    pub fn index_document(&self, doc: ExtractedDocument) -> Result<()> {
        let mut graph = self.graph.write();
        let path = doc.path.clone();
        let file_id = NodeId::new(path.to_string_lossy());
        graph.upsert_node(
            file_id.clone(),
            doc.title.clone().unwrap_or_default(),
            NodeKind::File,
        );
        graph.set_importance(&file_id, Importance::High);

        for tag in &doc.tags {
            let tag_id = NodeId::new(format!("tag:{}", tag));
            graph.upsert_node(tag_id.clone(), tag.clone(), NodeKind::Tag);
            graph.set_importance(&tag_id, Importance::Low);
            graph.add_edge(
                file_id.clone(),
                tag_id,
                crate::graph::EdgeKind::TaggedWith,
                doc.title.clone().unwrap_or_default(),
                tag.clone(),
            );
        }

        for link in &doc.links {
            let target_label = link.alias.clone().unwrap_or_else(|| link.target.clone());
            let target_id = NodeId::new(link.target.clone());
            graph.upsert_node(target_id.clone(), target_label.clone(), NodeKind::File);
            graph.add_edge(
                file_id.clone(),
                target_id,
                crate::graph::EdgeKind::LinksTo,
                doc.title.clone().unwrap_or_default(),
                target_label,
            );
        }

        self.retriever.write().add_document(doc)?;
        Ok(())
    }

    /// Remove a document from the field.
    pub fn remove_document(&self, path: &Path) -> Result<()> {
        self.retriever.write().remove_document(path)?;
        self.graph
            .write()
            .remove_node(&NodeId::new(path.to_string_lossy()))?;
        Ok(())
    }

    /// Index all `.md` files under a directory into the field.
    ///
    /// This is the entry point for connecting an external note vault (e.g.
    /// Obsidian) to the dynamic knowledge field used by sessions. Each file is
    /// parsed for wikilinks, tags, and frontmatter.
    ///
    /// Returns the number of files indexed. Failures for individual files are
    /// logged and skipped.
    pub fn index_directory(&self, root: &Path) -> Result<usize> {
        let extractor = MarkdownExtractor::new()?;
        let mut indexed = 0;

        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("Failed to read vault file {:?}: {}", path, e);
                    continue;
                }
            };

            match extractor.extract(path, &content) {
                Ok(doc) => {
                    if let Err(e) = self.index_document(doc) {
                        tracing::warn!("Failed to index vault file {:?}: {}", path, e);
                    } else {
                        indexed += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to extract vault file {:?}: {}", path, e);
                }
            }
        }

        tracing::info!("Indexed {} markdown files from vault {:?}", indexed, root);
        Ok(indexed)
    }

    /// Apply a single file-system watcher event to the field.
    ///
    /// This is the incremental counterpart to [`Self::index_directory`]. It
    /// creates, updates, removes, or renames the affected document without
    /// re-scanning the entire vault.
    pub fn apply_watcher_event(&self, event: &WatcherEvent) -> Result<()> {
        self.apply_watcher_events(std::slice::from_ref(event))
    }

    /// Apply a batch of watcher events efficiently.
    ///
    /// Events are deduplicated by path so that a flurry of `Create`/`Modify`
    /// notifications for the same file only triggers indexing once. Renames are
    /// split into a removal at the old path and an index at the new path.
    pub fn apply_watcher_events(&self, events: &[WatcherEvent]) -> Result<()> {
        use std::collections::HashMap;

        // Collapse multiple events for the same path into the latest effective
        // operation. We keep Removed as terminal and Created/Modified as index.
        let mut ops: HashMap<PathBuf, WatcherEvent> = HashMap::new();
        for event in events {
            match event {
                WatcherEvent::Created(path) | WatcherEvent::Modified(path) => {
                    ops.insert(path.clone(), WatcherEvent::Created(path.clone()));
                }
                WatcherEvent::Removed(path) => {
                    ops.insert(path.clone(), WatcherEvent::Removed(path.clone()));
                }
                WatcherEvent::Renamed { from, to } => {
                    // A rename removes the old identity and creates the new one.
                    ops.insert(from.clone(), WatcherEvent::Removed(from.clone()));
                    ops.insert(to.clone(), WatcherEvent::Created(to.clone()));
                }
            }
        }

        // Apply removals first so that a rapid remove + recreate does not leave
        // stale index entries behind.
        for event in ops.values() {
            if let WatcherEvent::Removed(path) = event {
                self.remove_document(path)?;
            }
        }

        for event in ops.values() {
            if let WatcherEvent::Created(path) = event {
                self.index_file(path)?;
            }
        }

        Ok(())
    }

    /// Index a single markdown file into the field.
    fn index_file(&self, path: &Path) -> Result<()> {
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            return Ok(());
        }
        let extractor = MarkdownExtractor::new()?;
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to read vault file {:?}: {}", path, e);
                return Ok(());
            }
        };
        let doc = extractor.extract(path, &content)?;
        self.index_document(doc)?;
        Ok(())
    }

    /// Search the knowledge field.
    ///
    /// Direct matches from the hybrid retriever are injected with activation
    /// energy, then spreading activation is run for the configured number of
    /// iterations. Results are ranked by final activation.
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<FieldResult>> {
        let direct = {
            let graph = self.graph.read();
            let mut retriever = self.retriever.write();
            retriever.search(query, &graph)?
        };

        if direct.is_empty() {
            return Ok(Vec::new());
        }

        {
            let mut graph = self.graph.write();
            graph.reset_activation();
            let now = Instant::now();

            for result in &direct {
                let node_id = NodeId::new(result.path.to_string_lossy());
                let energy = 0.3 + (result.score as f32).clamp(0.0, 1.0) * 0.7;
                graph.inject_activation(&node_id, energy, now);
            }

            graph.spreading_activation(self.config.spreading_iterations);
            graph.lateral_inhibition(self.config.top_k_inhibition, self.config.inhibition_beta);
        }

        let retriever = self.retriever.read();

        // Direct hits are guaranteed in the result set and ranked by retriever
        // score so the most query-relevant files appear first.
        let mut direct_results: Vec<FieldResult> = direct
            .iter()
            .map(|r| {
                let (title, snippet) = retriever
                    .get_document(&r.path)
                    .map(|doc| {
                        (
                            doc.title.clone(),
                            make_snippet(&doc.content, &query.text.to_lowercase()),
                        )
                    })
                    .unwrap_or_else(|| (None, String::new()));
                FieldResult {
                    path: r.path.clone(),
                    title,
                    snippet,
                    activation: 0.0,
                    semantic_score: r.score,
                    matched_tags: r.matched_tags.clone(),
                }
            })
            .collect();

        {
            let graph = self.graph.read();
            for result in &mut direct_results {
                let node_id = NodeId::new(result.path.to_string_lossy());
                if let Some(node) = graph.nodes().find(|n| n.id == node_id) {
                    result.activation = node.effective_activation();
                }
            }
        }

        direct_results.sort_by(|a, b| {
            b.semantic_score
                .partial_cmp(&a.semantic_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Log the recall event for feedback-loop analysis.
        if let Some(store) = &self.recall_store {
            let memory_ids: Vec<String> = direct_results
                .iter()
                .map(|r| r.path.to_string_lossy().to_string())
                .collect();
            let event = RecallEvent {
                id: None,
                session_id: query.session_id.clone(),
                query: query.text.clone(),
                memory_ids,
                project: None,
            };
            if let Err(e) = store.lock().log_recall_event(event) {
                tracing::warn!("Failed to log recall event: {}", e);
            }
        }

        // Neighbor results (activated by graph spreading) fill any remaining
        // slots after direct hits.
        let direct_paths: std::collections::HashSet<_> =
            direct.iter().map(|r| r.path.clone()).collect();
        let mut neighbor_results = Vec::new();
        for (path, activation) in self.top_activated(query.limit.max(1)) {
            if direct_paths.contains(&path) {
                continue;
            }
            let (title, snippet) = retriever
                .get_document(&path)
                .map(|doc| {
                    (
                        doc.title.clone(),
                        make_snippet(&doc.content, &query.text.to_lowercase()),
                    )
                })
                .unwrap_or_else(|| (None, String::new()));
            neighbor_results.push(FieldResult {
                path,
                title,
                snippet,
                activation,
                semantic_score: 0.0,
                matched_tags: Vec::new(),
            });
        }

        neighbor_results.sort_by(|a, b| {
            b.activation
                .partial_cmp(&a.activation)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut results = direct_results;
        let remaining = query.limit.saturating_sub(results.len());
        results.extend(neighbor_results.into_iter().take(remaining));
        results.truncate(query.limit);
        Ok(results)
    }

    /// Inject activation energy into a node.
    pub fn inject_activation(&self, id: &NodeId, energy: f32) {
        self.graph
            .write()
            .inject_activation(id, energy, Instant::now());
    }

    /// Decay all activations based on the current time and mark stale low-energy
    /// nodes as dormant.
    pub fn decay(&self, now: Instant) {
        let mut graph = self.graph.write();
        graph.decay_activation(now, self.config.file_half_life);
        graph.mark_dormant(
            now,
            self.config.dormant_threshold,
            self.config.dormant_min_age,
        );
    }

    /// Return the `k` most activated file nodes.
    pub fn top_activated(&self, k: usize) -> Vec<(PathBuf, f32)> {
        // Filter to file nodes before taking the top k. The underlying graph
        // contains tag and link-target nodes; asking for only `k` total nodes
        // and then filtering can drop every file node when tags dominate
        // activation.
        self.graph
            .read()
            .top_activated(usize::MAX)
            .into_iter()
            .filter(|n| n.kind == NodeKind::File)
            .take(k)
            .map(|n| (PathBuf::from(&n.id.0), n.effective_activation()))
            .collect()
    }

    /// Return a clone of the underlying graph.
    pub fn graph(&self) -> KnowledgeGraph {
        self.graph.read().clone()
    }

    /// Record an outcome signal for the recall-effectiveness feedback loop.
    ///
    /// Requires that a recall store has been attached via
    /// [`Self::with_recall_store`]. Signals are correlated with recall events
    /// that share the same `session_id`.
    pub fn record_outcome_signal(&self, signal: OutcomeSignal) {
        let Some(store) = &self.recall_store else {
            return;
        };
        if let Err(e) = store.lock().record_outcome_signal(signal) {
            tracing::warn!("Failed to record outcome signal: {}", e);
        }
    }

    /// Apply recall-effectiveness feedback to the knowledge graph.
    ///
    /// Computes per-memory effectiveness over the given time window and bumps
    /// or lowers node importance accordingly. Positive scores raise importance;
    /// negative scores lower it.
    pub fn apply_recall_feedback(&self, window: Duration) {
        let Some(store) = &self.recall_store else {
            return;
        };
        let boosts = match store.lock().memory_boosts(window) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Failed to compute memory boosts: {}", e);
                return;
            }
        };

        let mut graph = self.graph.write();
        for (memory_id, score) in boosts {
            let node_id = NodeId::new(memory_id);
            let Some(idx) = graph.node_index(&node_id) else {
                continue;
            };
            let current = graph.node_importance(idx);
            let next = if score > 0.5 {
                match current {
                    Importance::Ephemeral => Importance::Low,
                    Importance::Low => Importance::Medium,
                    Importance::Medium => Importance::High,
                    Importance::High | Importance::Critical => Importance::Critical,
                }
            } else if score < -0.5 {
                match current {
                    Importance::Critical => Importance::High,
                    Importance::High => Importance::Medium,
                    Importance::Medium => Importance::Low,
                    Importance::Low | Importance::Ephemeral => Importance::Ephemeral,
                }
            } else {
                current
            };
            graph.set_importance(&node_id, next);
        }
    }

    /// Return a clone of the extracted document for a path, if indexed.
    pub fn get_document(&self, path: &Path) -> Option<ExtractedDocument> {
        self.retriever.read().get_document(path).cloned()
    }
}

fn make_snippet(content: &str, query: &str) -> String {
    let lower = content.to_lowercase();
    if let Some(byte_pos) = lower.find(query) {
        // Work in character indices so slicing is always valid for multi-byte
        // characters (e.g. CJK).
        let match_start_char = lower[..byte_pos].chars().count();
        let match_end_char = match_start_char + query.chars().count();
        let start = match_start_char.saturating_sub(80);
        let end = (match_end_char + 120).min(content.chars().count());
        content.chars().skip(start).take(end - start).collect()
    } else {
        content.chars().take(200).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{ExtractedDocument, MarkdownExtractor};
    use std::io::Write;

    fn doc(path: &str, title: &str, content: &str, tags: &[&str]) -> ExtractedDocument {
        ExtractedDocument {
            path: PathBuf::from(path),
            title: Some(title.to_string()),
            content: content.to_string(),
            frontmatter: serde_json::Value::Null,
            links: Vec::new(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            headings: Vec::new(),
        }
    }

    #[test]
    fn search_activates_linked_documents() {
        let field = KnowledgeField::new(FieldConfig::default());
        field
            .index_document(doc("rust.md", "Rust", "Rust is fast.", &["lang"]))
            .unwrap();
        field
            .index_document(doc(
                "async.md",
                "Async",
                "Async Rust uses futures. [[rust]]",
                &["lang"],
            ))
            .unwrap();

        let query = SearchQuery::new("rust").with_limit(5);
        let results = field.search(&query).unwrap();

        assert!(!results.is_empty());
        let rust = results.iter().find(|r| r.path == *"rust.md");
        let async_doc = results.iter().find(|r| r.path == *"async.md");
        assert!(rust.is_some());
        assert!(
            async_doc.is_some_and(|r| r.activation > 0.0),
            "linked document should be activated by spreading"
        );
    }

    #[test]
    fn top_activated_filters_to_files() {
        let field = KnowledgeField::new(FieldConfig::default());
        field.index_document(doc("a.md", "A", "A", &[])).unwrap();
        field.index_document(doc("b.md", "B", "B", &[])).unwrap();

        field.inject_activation(&NodeId::new("b.md"), 1.0);
        let top = field.top_activated(1);

        assert_eq!(top.len(), 1);
        assert_eq!(top[0].0, PathBuf::from("b.md"));
    }

    #[test]
    fn search_returns_only_file_nodes() {
        let field = KnowledgeField::new(FieldConfig::default());
        field
            .index_document(doc(
                "activation.md",
                "Activation",
                "Spreading activation moves energy through the graph.",
                &["knowledge"],
            ))
            .unwrap();

        let query = SearchQuery::new("spreading energy").with_limit(10);
        let results = field.search(&query).unwrap();

        for result in &results {
            assert!(
                !result.path.to_string_lossy().starts_with("tag:"),
                "search should not return tag nodes, got {:?}",
                result.path
            );
        }
        let activation = results.iter().find(|r| r.path == *"activation.md");
        assert!(activation.is_some(), "activation.md should be in results");
    }

    #[test]
    fn index_document_extracts_links() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.md");
        let mut f = std::fs::File::create(&a).unwrap();
        writeln!(f, "# A\nSee [[b]].").unwrap();

        let extractor = MarkdownExtractor::new().unwrap();
        let content = std::fs::read_to_string(&a).unwrap();
        let doc = extractor.extract(&a, &content).unwrap();

        let field = KnowledgeField::new(FieldConfig::default());
        field.index_document(doc).unwrap();

        let graph = field.graph();
        assert_eq!(graph.node_count(), 2); // a.md + b
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn search_finds_chinese_title() {
        let field = KnowledgeField::new(FieldConfig::default());
        field
            .index_document(doc("chinese.md", "中文笔记", "这是一份内容。", &[]))
            .unwrap();

        let results = field
            .search(&SearchQuery::new("中文笔记").with_limit(5))
            .unwrap();
        assert!(
            results.iter().any(|r| r.path == *"chinese.md"),
            "Chinese query should match Chinese title"
        );
    }

    #[test]
    fn search_finds_chinese_content() {
        let field = KnowledgeField::new(FieldConfig::default());
        field
            .index_document(doc("chinese.md", "Note", "这是一份中文笔记内容。", &[]))
            .unwrap();

        let results = field
            .search(&SearchQuery::new("中文笔记").with_limit(5))
            .unwrap();
        assert!(
            results.iter().any(|r| r.path == *"chinese.md"),
            "Chinese query should match Chinese content body"
        );
    }

    #[test]
    fn index_directory_loads_markdown_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("rust.md"), "# Rust\nRust is fast. #rust").unwrap();
        std::fs::write(
            dir.path().join("async.md"),
            "# Async\nAsync Rust uses futures. [[rust]]",
        )
        .unwrap();
        std::fs::write(dir.path().join("notes.txt"), "not markdown").unwrap();

        let field = KnowledgeField::new(FieldConfig::default());
        let indexed = field.index_directory(dir.path()).unwrap();

        assert_eq!(indexed, 2, "only .md files should be indexed");

        let results = field
            .search(&SearchQuery::new("futures").with_limit(5))
            .unwrap();
        assert!(!results.is_empty());
        assert!(
            results
                .iter()
                .any(|r| r.path == dir.path().join("async.md"))
        );

        // rust.md should be activated by spreading from async.md via [[rust]] link.
        let activated = field.top_activated(5);
        assert!(
            activated
                .iter()
                .any(|(p, _)| p == &dir.path().join("rust.md"))
        );
    }

    #[test]
    fn apply_watcher_events_batches_and_dedupes() {
        let dir = tempfile::tempdir().unwrap();
        let field = KnowledgeField::new(FieldConfig::default());

        let a = dir.path().join("a.md");
        let b = dir.path().join("b.md");
        std::fs::write(&a, "# A\nFirst.").unwrap();
        std::fs::write(&b, "# B\nFirst.").unwrap();

        field
            .apply_watcher_events(&[
                WatcherEvent::Created(a.clone()),
                WatcherEvent::Modified(a.clone()),
                WatcherEvent::Created(b.clone()),
            ])
            .unwrap();

        // Two creates + one spurious modify for the same file should result in
        // exactly two indexed documents.
        assert_eq!(field.graph().node_count(), 2);
        let results = field
            .search(&SearchQuery::new("First").with_limit(5))
            .unwrap();
        assert_eq!(results.len(), 2);

        // Remove + recreate in one batch should still leave the file indexed.
        std::fs::write(&a, "# A\nSecond.").unwrap();
        field
            .apply_watcher_events(&[
                WatcherEvent::Removed(a.clone()),
                WatcherEvent::Created(a.clone()),
            ])
            .unwrap();
        let results = field
            .search(&SearchQuery::new("Second").with_limit(5))
            .unwrap();
        assert!(results.iter().any(|r| r.path == a));
    }

    #[test]
    fn apply_watcher_event_increments_index() {
        let dir = tempfile::tempdir().unwrap();
        let field = KnowledgeField::new(FieldConfig::default());

        let created = dir.path().join("created.md");
        std::fs::write(&created, "# Created\nNew content.").unwrap();
        field
            .apply_watcher_event(&WatcherEvent::Created(created.clone()))
            .unwrap();

        let results = field
            .search(&SearchQuery::new("New content").with_limit(5))
            .unwrap();
        assert!(results.iter().any(|r| r.path == created));

        std::fs::write(&created, "# Created\nUpdated content.").unwrap();
        field
            .apply_watcher_event(&WatcherEvent::Modified(created.clone()))
            .unwrap();

        let results = field
            .search(&SearchQuery::new("Updated content").with_limit(5))
            .unwrap();
        assert!(results.iter().any(|r| r.path == created));

        field
            .apply_watcher_event(&WatcherEvent::Removed(created.clone()))
            .unwrap();
        let results = field
            .search(&SearchQuery::new("Updated content").with_limit(5))
            .unwrap();
        assert!(!results.iter().any(|r| r.path == created));
    }

    #[test]
    fn recall_feedback_adjusts_importance() {
        use crate::recall_store::{OutcomeSignal, OutcomeSignalType, RecallStore};

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("recall.db");
        let field = KnowledgeField::new(FieldConfig::default())
            .with_recall_store(RecallStore::open(&db_path).unwrap());

        let a = dir.path().join("a.md");
        std::fs::write(&a, "# A\nRust is fast.").unwrap();
        field
            .apply_watcher_event(&WatcherEvent::Created(a.clone()))
            .unwrap();

        let query = SearchQuery::new("rust").with_session_id("s1").with_limit(5);
        let _results = field.search(&query).unwrap();

        // A strong positive signal should bump the file node to Critical.
        field.record_outcome_signal(OutcomeSignal {
            session_id: Some("s1".to_string()),
            signal_type: OutcomeSignalType::ExplicitBoost,
            value: 6,
            source: None,
            project: None,
        });
        field.apply_recall_feedback(Duration::from_secs(60));

        let graph = field.graph();
        let node = graph
            .nodes()
            .find(|n| n.id.0 == a.to_string_lossy())
            .unwrap();
        assert_eq!(node.importance, Importance::Critical);
    }
}
