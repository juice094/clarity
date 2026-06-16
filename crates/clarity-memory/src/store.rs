//! Storage layer for facts
//!
//! This module provides the main `MemoryStore` type which is the primary
//! interface for storing and retrieving facts. It uses SQLite with FTS5
//! for full-text search by default.
//!
//! For alternative backends, see the `backends` module.

use crate::types::{Fact, Result};
use std::path::Path;
use std::sync::Arc;

// Re-export backends for convenience
#[cfg(feature = "hermes")]
pub use crate::backends::HermesMemoryAdapter;
#[cfg(feature = "sqlite")]
pub use crate::backends::SqliteStore;
pub use crate::backends::{BackendConfig, StorageBackend, StorageFactory};
pub use crate::backends::{FileStore, HybridStore};

/// Configuration for time-decay weighting of search results.
#[derive(Debug, Clone, Copy)]
pub struct DecayConfig {
    /// Half-life in days. Default: 180 (6 months).
    pub half_life_days: f64,
    /// Whether to apply time decay. Default: true.
    pub enabled: bool,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            half_life_days: 180.0,
            enabled: true,
        }
    }
}

/// Compute exponential decay weight for a fact based on its age.
pub fn compute_decay_weight(created_at: chrono::DateTime<chrono::Utc>, decay: &DecayConfig) -> f64 {
    if !decay.enabled {
        return 1.0;
    }
    let age_days = (chrono::Utc::now() - created_at).num_days() as f64;
    let lambda = std::f64::consts::LN_2 / decay.half_life_days;
    (-lambda * age_days).exp()
}

/// Fact store with pluggable storage backends.
///
/// By default this uses the SQLite backend when the `sqlite` feature is
/// enabled. Other backends (file, hybrid, hermes) can be selected through
/// [`BackendConfig`] / [`StorageFactory`] or dedicated constructors such as
/// [`Self::new_hermes`].
#[derive(Debug, Clone)]
pub struct MemoryStore {
    inner: Arc<dyn StorageBackend>,
    decay_config: DecayConfig,
}

impl MemoryStore {
    /// Create a new MemoryStore at the given database path.
    ///
    /// Uses the SQLite backend when the `sqlite` feature is enabled, otherwise
    /// falls back to the file backend.
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        #[cfg(feature = "sqlite")]
        {
            let inner = Arc::new(SqliteStore::new(db_path).await?);
            Ok(Self {
                inner,
                decay_config: DecayConfig::default(),
            })
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let inner = Arc::new(FileStore::new(db_path).await?);
            Ok(Self {
                inner,
                decay_config: DecayConfig::default(),
            })
        }
    }

    /// Create an in-memory store for testing.
    ///
    /// # Panics
    /// Panics if called outside a Tokio runtime when the `sqlite` feature is
    /// disabled (because FileStore initialization is async).
    pub fn new_in_memory() -> Result<Self> {
        #[cfg(feature = "sqlite")]
        {
            let inner = Arc::new(SqliteStore::new_in_memory()?);
            Ok(Self {
                inner,
                decay_config: DecayConfig::default(),
            })
        }
        #[cfg(not(feature = "sqlite"))]
        {
            // FileStore::new is async.  For test convenience we block_on here,
            // but this MUST only be called from a Tokio runtime thread.
            let temp_dir =
                std::env::temp_dir().join(format!("clarity_memory_{}", std::process::id()));
            std::fs::create_dir_all(&temp_dir).map_err(crate::types::MemoryError::Io)?;

            let inner = Arc::new(tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(FileStore::new(&temp_dir))
            })?);
            Ok(Self {
                inner,
                decay_config: DecayConfig::default(),
            })
        }
    }

    /// Create a MemoryStore backed by hermes-memory.
    ///
    /// Available only when the `hermes` feature is enabled.
    #[cfg(feature = "hermes")]
    pub async fn new_hermes(db_path: impl AsRef<Path>) -> Result<Self> {
        let inner = Arc::new(HermesMemoryAdapter::new(db_path).await?);
        Ok(Self {
            inner,
            decay_config: DecayConfig::default(),
        })
    }

    /// Create a MemoryStore, choosing the backend from `CLARITY_MEMORY_BACKEND`.
    ///
    /// Falls back to the default SQLite backend when the variable is unset,
    /// unknown, or requests hermes without the `hermes` feature.
    pub async fn new_auto(db_path: impl AsRef<Path>) -> Result<Self> {
        match std::env::var("CLARITY_MEMORY_BACKEND").as_deref() {
            #[cfg(feature = "hermes")]
            Ok("hermes") => Self::new_hermes(db_path).await,
            Ok(value) if value.eq_ignore_ascii_case("hermes") => {
                tracing::warn!(
                    "CLARITY_MEMORY_BACKEND=hermes requested but the `hermes` feature is disabled; falling back to sqlite"
                );
                Self::new(db_path).await
            }
            _ => Self::new(db_path).await,
        }
    }

    /// Set a custom decay configuration.
    pub fn with_decay_config(mut self, config: DecayConfig) -> Self {
        self.decay_config = config;
        self
    }

    /// Save a fact to the store
    pub async fn save_fact(
        &self,
        fact: &str,
        tags: &[String],
        time: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<i64> {
        self.inner.save_fact(fact, tags, time, session_id).await
    }

    /// Search facts by tags
    pub async fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<Fact>> {
        self.inner.search_by_tags(tags, limit).await
    }

    /// Full-text search using FTS5
    pub async fn search_fulltext(&self, query: &str, limit: usize) -> Result<Vec<Fact>> {
        self.inner
            .search_fulltext(query, limit, &self.decay_config)
            .await
    }

    /// Hybrid search: FTS5 recall + BM25 reranking
    ///
    /// Uses FTS5 for fast candidate retrieval, then reranks using BM25 scoring
    /// for better relevance on short-text documents (facts).
    pub async fn search_hybrid(&self, query: &str, limit: usize) -> Result<Vec<(Fact, f32)>> {
        self.inner
            .search_similar(query, limit, &self.decay_config)
            .await
    }

    /// Semantic search: TF-IDF cosine similarity over all facts.
    ///
    /// Unlike [`Self::search_hybrid`], this performs a dense-style semantic
    /// ranking without requiring the query terms to match the FTS5 index. It
    /// is most useful for paraphrase-style recall. Backends that do not
    /// implement semantic search return an empty list, in which case callers
    /// should fall back to [`Self::search_hybrid`].
    pub async fn search_semantic(&self, query: &str, limit: usize) -> Result<Vec<(Fact, f32)>> {
        self.inner
            .search_semantic(query, limit, &self.decay_config)
            .await
    }

    /// Get facts by session ID
    pub async fn get_facts_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<Fact>> {
        self.inner.get_facts_by_session(session_id, limit).await
    }

    /// Get facts created after a specific time
    pub async fn get_facts_since(&self, since: chrono::DateTime<chrono::Utc>) -> Result<Vec<Fact>> {
        self.inner.get_facts_since(since).await
    }

    /// Delete a fact by ID
    pub async fn delete_fact(&self, id: i64) -> Result<bool> {
        self.inner.delete_fact(id).await
    }

    /// Get the most recent facts
    pub async fn get_recent_facts(&self, limit: usize) -> Result<Vec<Fact>> {
        self.inner.get_recent_facts(limit).await
    }

    /// Clear all facts from the store
    pub async fn clear_all(&self) -> Result<usize> {
        self.inner.clear_all().await
    }

    /// Get total count of facts
    pub async fn count_facts(&self) -> Result<i64> {
        self.inner.count_facts().await
    }

    /// Get a fact by ID
    pub async fn get_fact(&self, id: i64) -> Result<Option<Fact>> {
        self.inner.get_fact(id).await
    }

    /// Bulk save facts for better performance
    pub async fn bulk_save_facts(&self, facts: &[FactTuple<'_>]) -> Result<Vec<i64>> {
        self.inner.bulk_save_facts(facts).await
    }

    /// Save a session note section to the store.
    pub async fn save_session_note(
        &self,
        session_id: &str,
        section: &str,
        content: &str,
    ) -> Result<()> {
        self.inner
            .save_session_note(session_id, section, content)
            .await
    }
}

/// Fact tuple for bulk operations: (content, tags, source, scope)
pub type FactTuple<'a> = (&'a str, Vec<String>, Option<&'a str>, Option<&'a str>);

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_store() -> (TempDir, MemoryStore) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = MemoryStore::new(&db_path).await.unwrap();
        (temp_dir, store)
    }

    #[tokio::test]
    async fn test_save_and_retrieve_fact() {
        let (_temp, store) = create_test_store().await;

        let id = store
            .save_fact(
                "User likes Rust programming",
                &["preference".to_string(), "tech".to_string()],
                Some("2024-01-15"),
                Some("session-1"),
            )
            .await
            .unwrap();

        let fact = store
            .get_fact(id)
            .await
            .unwrap()
            .expect("Fact should exist");
        assert_eq!(fact.fact, "User likes Rust programming");
        assert_eq!(fact.tags, vec!["preference", "tech"]);
        assert_eq!(fact.time, Some("2024-01-15".to_string()));
        assert_eq!(fact.session_id, Some("session-1".to_string()));
    }

    #[tokio::test]
    async fn test_search_by_tags() {
        let (_temp, store) = create_test_store().await;

        store
            .save_fact(
                "User likes Rust",
                &["preference".to_string(), "tech".to_string()],
                None,
                None,
            )
            .await
            .unwrap();
        store
            .save_fact(
                "User likes Python",
                &["preference".to_string(), "tech".to_string()],
                None,
                None,
            )
            .await
            .unwrap();
        store
            .save_fact("Meeting at 3pm", &["schedule".to_string()], None, None)
            .await
            .unwrap();

        let results = store
            .search_by_tags(&["preference".to_string()], 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        let results = store
            .search_by_tags(&["preference".to_string(), "tech".to_string()], 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        let results = store
            .search_by_tags(&["schedule".to_string()], 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fact, "Meeting at 3pm");
    }

    #[tokio::test]
    async fn test_fulltext_search() {
        let (_temp, store) = create_test_store().await;

        store
            .save_fact(
                "Rust is a systems programming language",
                &["tech".to_string()],
                None,
                None,
            )
            .await
            .unwrap();
        store
            .save_fact(
                "Python is great for data science",
                &["tech".to_string(), "data".to_string()],
                None,
                None,
            )
            .await
            .unwrap();

        let results = store.search_fulltext("Rust", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].fact.contains("Rust"));

        let results = store
            .search_fulltext("programming language", 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_semantic_search() {
        let (_temp, store) = create_test_store().await;

        store
            .save_fact(
                "User likes Rust programming language",
                &["preference".to_string(), "tech".to_string()],
                None,
                None,
            )
            .await
            .unwrap();
        store
            .save_fact(
                "User enjoys Python programming",
                &["preference".to_string(), "tech".to_string()],
                None,
                None,
            )
            .await
            .unwrap();
        store
            .save_fact("User has a dog named Max", &["pet".to_string()], None, None)
            .await
            .unwrap();

        let results = store.search_semantic("programming", 2).await.unwrap();
        assert_eq!(results.len(), 2);
        let ids: Vec<i64> = results.iter().map(|(fact, _)| fact.id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));

        // Query with a term shared by both programming facts.
        let results = store.search_semantic("Python", 1).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].0.fact.contains("Python"));
    }

    #[tokio::test]
    async fn test_delete_fact() {
        let (_temp, store) = create_test_store().await;

        let id = store.save_fact("Test fact", &[], None, None).await.unwrap();
        assert!(store.get_fact(id).await.unwrap().is_some());

        assert!(store.delete_fact(id).await.unwrap());
        assert!(store.get_fact(id).await.unwrap().is_none());

        assert!(!store.delete_fact(999).await.unwrap());
    }

    #[tokio::test]
    async fn test_count_facts() {
        let (_temp, store) = create_test_store().await;

        assert_eq!(store.count_facts().await.unwrap(), 0);

        store.save_fact("Fact 1", &[], None, None).await.unwrap();
        assert_eq!(store.count_facts().await.unwrap(), 1);

        store.save_fact("Fact 2", &[], None, None).await.unwrap();
        assert_eq!(store.count_facts().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_bulk_save() {
        let (_temp, store) = create_test_store().await;

        let facts = vec![
            ("Fact 1", vec!["a".to_string()], None, None),
            ("Fact 2", vec!["b".to_string()], None, None),
            ("Fact 3", vec!["c".to_string()], None, None),
        ];

        let ids = store.bulk_save_facts(&facts).await.unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(store.count_facts().await.unwrap(), 3);
    }
}
