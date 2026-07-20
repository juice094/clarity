//! File-system indexing and search for knowledge sources.

use crate::error::{KnowledgeError, Result};
use crate::extract::{ExtractedDocument, MarkdownExtractor, WikiLink};
use crate::graph::{EdgeKind, KnowledgeGraph, NodeId, NodeKind};
use crate::retrieval::HybridRetriever;
use crate::search::{SearchQuery, SearchResult};
use crate::vault_config::{AppConfig, LinkResolver};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Configuration for a knowledge source directory.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceConfig {
    /// Root directory of the source.
    pub root: PathBuf,
    /// Glob patterns to include (empty means include all).
    pub include_patterns: Vec<String>,
    /// Glob patterns to exclude.
    pub exclude_patterns: Vec<String>,
}

impl SourceConfig {
    /// Create a source config for a single directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
        }
    }

    /// Add an include glob pattern.
    pub fn include(mut self, pattern: impl Into<String>) -> Self {
        self.include_patterns.push(pattern.into());
        self
    }

    /// Add an exclude glob pattern.
    pub fn exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }
}

/// A high-level event describing a file change in a knowledge source.
#[derive(Debug, Clone, PartialEq)]
pub enum FileEvent {
    /// A file was discovered or updated.
    Indexed(PathBuf),
    /// A file was removed from the index.
    Removed(PathBuf),
}

/// Trait for knowledge indexes.
#[async_trait::async_trait]
pub trait KnowledgeIndex: Send + Sync {
    /// Register a source directory to be indexed.
    async fn add_source(&self, config: SourceConfig) -> Result<()>;

    /// Index or re-index a single file.
    async fn index_file(&self, path: &Path) -> Result<()>;

    /// Remove a file from the index.
    async fn remove_file(&self, path: &Path) -> Result<()>;

    /// Search the index.
    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>>;

    /// Return the current knowledge graph.
    fn graph(&self) -> Result<KnowledgeGraph>;
}

/// Simple in-memory knowledge index used for early validation.
///
/// This implementation stores extracted documents and a graph in memory. It is
/// suitable for tests and small vaults; production use should layer on SQLite
/// persistence.
///
/// Create with [`InMemoryIndex::new`].
pub struct InMemoryIndex {
    sources: parking_lot::RwLock<Vec<SourceConfig>>,
    documents: parking_lot::RwLock<HashMap<PathBuf, ExtractedDocument>>,
    graph: parking_lot::RwLock<KnowledgeGraph>,
    retriever: parking_lot::RwLock<HybridRetriever>,
    extractor: MarkdownExtractor,
}

impl InMemoryIndex {
    /// Create a new empty in-memory index.
    ///
    /// # Errors
    ///
    /// Returns an error if the internal Markdown extractor fails to compile its
    /// regexes. This should not happen in practice.
    pub fn new() -> Result<Self> {
        Ok(Self {
            sources: parking_lot::RwLock::new(Vec::new()),
            documents: parking_lot::RwLock::new(HashMap::new()),
            graph: parking_lot::RwLock::new(KnowledgeGraph::new()),
            retriever: parking_lot::RwLock::new(HybridRetriever::new()),
            extractor: MarkdownExtractor::new()?,
        })
    }

    /// Scan all registered sources and index every matching file.
    pub fn scan_all(&self) -> Result<Vec<FileEvent>> {
        let sources = self.sources.read().clone();
        let mut events = Vec::new();

        for source in sources {
            for entry in walkdir::WalkDir::new(&source.root)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                if !is_included(path, &source) {
                    continue;
                }

                self.index_file_sync(path)?;
                events.push(FileEvent::Indexed(path.to_path_buf()));
            }
        }

        Ok(events)
    }

    /// Update all wikilinks after a file has been renamed.
    ///
    /// Callers must already have moved the file on disk from `from` to `to`.
    /// The method scans every indexed document, replaces links that pointed to
    /// `from` with a link to `to`, writes the updated files back, and re-indexes
    /// them. It returns the list of files whose contents were modified.
    ///
    /// # Errors
    ///
    /// Returns an error if a link cannot be resolved, if a file cannot be read
    /// or written, or if `to` is outside the vault root when the original link
    /// used a relative path.
    pub fn rename_file(&self, vault_root: &Path, from: &Path, to: &Path) -> Result<Vec<PathBuf>> {
        let mut resolver = LinkResolver::new(AppConfig::default());
        let paths: Vec<PathBuf> = self
            .documents
            .read()
            .keys()
            .cloned()
            .chain(std::iter::once(to.to_path_buf()))
            .collect();
        resolver.index_paths(vault_root, paths);

        let docs: Vec<ExtractedDocument> = self.documents.read().values().cloned().collect();
        let mut updated = Vec::new();

        for doc in docs {
            let mut content = doc.content.clone();
            let mut changed = false;

            for link in &doc.links {
                if link.target.is_empty() {
                    continue;
                }
                let resolved = resolver.resolve_wikilink(link, &doc.path)?;
                if resolved == from {
                    let new_target = Self::compute_new_target(link, vault_root, from, to)?;
                    let new_raw = Self::build_new_raw(link, &new_target);
                    content = content.replace(&link.raw, &new_raw);
                    changed = true;
                }
            }

            if changed {
                std::fs::write(&doc.path, &content)?;
                self.index_file_sync(&doc.path)?;
                updated.push(doc.path.clone());
            }
        }

        self.graph
            .write()
            .remove_node(&NodeId::new(from.to_string_lossy()))?;

        if to.exists() {
            self.index_file_sync(to)?;
        }

        Ok(updated)
    }

    fn compute_new_target(
        link: &WikiLink,
        vault_root: &Path,
        from: &Path,
        to: &Path,
    ) -> Result<String> {
        if link.target.contains('/') {
            let rel = to.strip_prefix(vault_root).map_err(|_| {
                KnowledgeError::Io(std::io::Error::other(
                    "renamed file is outside the vault root",
                ))
            })?;
            Ok(rel.with_extension("").to_string_lossy().replace('\\', "/"))
        } else if from.file_stem() != to.file_stem() {
            Ok(to
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default())
        } else {
            Ok(link.target.clone())
        }
    }

    fn build_new_raw(link: &WikiLink, new_target: &str) -> String {
        let mut raw = String::new();
        if link.is_embed {
            raw.push('!');
        }
        raw.push_str("[[");
        raw.push_str(new_target);
        if let Some(heading) = &link.heading {
            raw.push('#');
            raw.push_str(heading);
        }
        if let Some(block_id) = &link.block_id {
            raw.push('^');
            raw.push_str(block_id);
        }
        if let Some(alias) = &link.alias {
            raw.push('|');
            raw.push_str(alias);
        }
        raw.push_str("]]");
        raw
    }

    fn index_file_sync(&self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path)?;
        let doc = self.extractor.extract(path, &content)?;

        let mut graph = self.graph.write();
        let file_id = NodeId::new(path.to_string_lossy());
        graph.upsert_node(
            file_id.clone(),
            doc.title.clone().unwrap_or_default(),
            NodeKind::File,
        );

        for tag in &doc.tags {
            let tag_id = NodeId::new(format!("tag:{}", tag));
            graph.upsert_node(tag_id.clone(), tag.clone(), NodeKind::Tag);
            graph.add_edge(
                file_id.clone(),
                tag_id,
                EdgeKind::TaggedWith,
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
                EdgeKind::LinksTo,
                doc.title.clone().unwrap_or_default(),
                target_label,
            );
        }

        self.documents
            .write()
            .insert(path.to_path_buf(), doc.clone());
        self.retriever.write().add_document(doc)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl KnowledgeIndex for InMemoryIndex {
    async fn add_source(&self, config: SourceConfig) -> Result<()> {
        self.sources.write().push(config);
        Ok(())
    }

    async fn index_file(&self, path: &Path) -> Result<()> {
        self.index_file_sync(path)
    }

    async fn remove_file(&self, path: &Path) -> Result<()> {
        self.documents.write().remove(path);
        self.retriever.write().remove_document(path)?;
        Ok(())
    }

    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        let graph = self.graph.read().clone();
        self.retriever.write().search(&query, &graph)
    }

    fn graph(&self) -> Result<KnowledgeGraph> {
        Ok(self.graph.read().clone())
    }
}

fn is_included(path: &Path, source: &SourceConfig) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    if ext != "md" {
        return false;
    }

    if !source.include_patterns.is_empty() {
        let s = path.to_string_lossy();
        if !source.include_patterns.iter().any(|p| s.contains(p)) {
            return false;
        }
    }

    let s = path.to_string_lossy();
    if source.exclude_patterns.iter().any(|p| s.contains(p)) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn scan_and_search() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = std::fs::File::create(dir.path().join("rust.md")).unwrap();
        writeln!(f, "# Rust\nRust is a systems language. [[memory]]").unwrap();

        let mut f2 = std::fs::File::create(dir.path().join("python.md")).unwrap();
        writeln!(f2, "# Python\nPython is dynamic.").unwrap();

        let index = InMemoryIndex::new().unwrap();
        index
            .add_source(SourceConfig::new(dir.path()))
            .await
            .unwrap();
        index.scan_all().unwrap();

        let results = index.search(SearchQuery::new("rust")).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title.as_deref(), Some("Rust"));

        let graph = index.graph().unwrap();
        assert_eq!(graph.node_count(), 3); // rust.md, python.md, memory
        assert_eq!(graph.edge_count(), 1);
    }

    #[tokio::test]
    async fn rename_file_updates_short_wikilinks() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let a = vault.join("a.md");
        let b = vault.join("b.md");
        let x = vault.join("x.md");

        std::fs::write(&a, "# A\n").unwrap();
        std::fs::write(&b, "See [[a]].").unwrap();

        let index = InMemoryIndex::new().unwrap();
        index.add_source(SourceConfig::new(vault)).await.unwrap();
        index.scan_all().unwrap();

        std::fs::rename(&a, &x).unwrap();
        let updated = index.rename_file(vault, &a, &x).unwrap();

        assert_eq!(updated, vec![b.clone()]);
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "See [[x]].");

        let graph = index.graph().unwrap();
        assert!(graph.backlinks(&NodeId::new(x.to_string_lossy())).is_some());
        assert!(graph.backlinks(&NodeId::new(a.to_string_lossy())).is_none());
    }

    #[tokio::test]
    async fn rename_file_updates_relative_wikilinks() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let notes = vault.join("notes");
        let other = vault.join("other");
        std::fs::create_dir(&notes).unwrap();
        std::fs::create_dir(&other).unwrap();

        let a = notes.join("a.md");
        let b = vault.join("b.md");
        let x = other.join("x.md");

        std::fs::write(&a, "# A\n").unwrap();
        std::fs::write(&b, "See [[notes/a|A note]] and ![[notes/a#heading]].").unwrap();

        let index = InMemoryIndex::new().unwrap();
        index.add_source(SourceConfig::new(vault)).await.unwrap();
        index.scan_all().unwrap();

        std::fs::rename(&a, &x).unwrap();
        let updated = index.rename_file(vault, &a, &x).unwrap();

        assert_eq!(updated, vec![b.clone()]);
        assert_eq!(
            std::fs::read_to_string(&b).unwrap(),
            "See [[other/x|A note]] and ![[other/x#heading]]."
        );
    }
}
