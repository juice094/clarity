//! Storage backends for clarity-memory
//!
//! Provides pluggable storage implementations:
//! - `FileStore`: JSON file storage with atomic writes
//! - `SqliteStore`: SQLite with FTS5 full-text search (requires `sqlite` feature)
//! - `HybridStore`: Hot memory cache + cold file storage
//! - `MemoryStore`: Pure in-memory storage for testing

use crate::store::DecayConfig;
use crate::types::{Fact, Result};
use async_trait::async_trait;

mod file;
#[cfg(feature = "hermes")]
mod hermes;
mod hybrid;
#[cfg(feature = "sqlite")]
mod sqlite;

pub use file::FileStore;
#[cfg(feature = "hermes")]
pub use hermes::HermesMemoryAdapter;
pub use hybrid::HybridStore;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteStore;

/// Core trait for all storage backends
#[async_trait]
pub trait StorageBackend: Send + Sync + std::fmt::Debug {
    /// Store a fact and return its ID
    async fn save_fact(
        &self,
        fact: &str,
        tags: &[String],
        time: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<i64>;

    /// Get a fact by ID
    async fn get_fact(&self, id: i64) -> Result<Option<Fact>>;

    /// Delete a fact by ID
    async fn delete_fact(&self, id: i64) -> Result<bool>;

    /// Search facts by tags (matches all specified tags)
    async fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<Fact>>;

    /// Full-text search
    async fn search_fulltext(
        &self,
        query: &str,
        limit: usize,
        decay: &DecayConfig,
    ) -> Result<Vec<Fact>>;

    /// Get facts by session ID
    async fn get_facts_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<Fact>>;

    /// Get facts created after a specific time
    async fn get_facts_since(&self, since: chrono::DateTime<chrono::Utc>) -> Result<Vec<Fact>>;

    /// Get recent facts
    async fn get_recent_facts(&self, limit: usize) -> Result<Vec<Fact>>;

    /// Get total count of facts
    async fn count_facts(&self) -> Result<i64>;

    /// Clear all facts
    async fn clear_all(&self) -> Result<usize>;

    /// Save a session note section (only supported by some backends).
    async fn save_session_note(
        &self,
        _session_id: &str,
        _section: &str,
        _content: &str,
    ) -> Result<()> {
        Ok(())
    }

    /// Get facts by IDs
    async fn get_facts_by_ids(&self, ids: &[i64]) -> Result<Vec<Fact>> {
        let mut facts = Vec::new();
        for id in ids {
            if let Some(fact) = self.get_fact(*id).await? {
                facts.push(fact);
            }
        }
        Ok(facts)
    }

    /// Search with semantic similarity (if supported)
    async fn search_similar(
        &self,
        query: &str,
        limit: usize,
        decay: &DecayConfig,
    ) -> Result<Vec<(Fact, f32)>> {
        let facts = self.search_fulltext(query, limit, decay).await?;
        Ok(facts.into_iter().map(|f| (f, 1.0)).collect())
    }

    /// Search with pure TF-IDF cosine semantic similarity (if supported).
    ///
    /// Backends that do not maintain a vector index may return an empty
    /// result; callers can fall back to [`Self::search_similar`].
    async fn search_semantic(
        &self,
        _query: &str,
        _limit: usize,
        _decay: &DecayConfig,
    ) -> Result<Vec<(Fact, f32)>> {
        Ok(Vec::new())
    }

    /// Bulk save facts for better performance
    async fn bulk_save_facts(
        &self,
        facts: &[(&str, Vec<String>, Option<&str>, Option<&str>)],
    ) -> Result<Vec<i64>> {
        let mut ids = Vec::new();
        for (fact, tags, time, session_id) in facts {
            let id = self.save_fact(fact, tags, *time, *session_id).await?;
            ids.push(id);
        }
        Ok(ids)
    }
}

/// Storage backend configuration
#[derive(Debug, Clone)]
pub enum BackendConfig {
    /// File-based storage configuration
    File {
        /// Directory where fact files are stored
        dir: std::path::PathBuf,
        /// Whether to compress fact files
        compress: bool,
    },
    /// SQLite storage configuration
    #[cfg(feature = "sqlite")]
    Sqlite {
        /// Path to the SQLite database file
        db_path: std::path::PathBuf,
        /// Whether to enable WAL mode
        wal_mode: bool,
    },
    /// Hybrid storage configuration
    Hybrid {
        /// Maximum number of facts kept in the hot cache
        cache_size: usize,
        /// Directory for cold file storage
        cold_dir: std::path::PathBuf,
        /// Interval in seconds between cache-to-disk syncs
        sync_interval_secs: u64,
    },
    /// Hermes-memory SQLite storage configuration
    #[cfg(feature = "hermes")]
    Hermes {
        /// Path to the hermes SQLite database file
        db_path: std::path::PathBuf,
    },
}

impl BackendConfig {
    /// Create default file config
    pub fn file_default() -> Self {
        BackendConfig::File {
            dir: std::path::PathBuf::from("./memory_data"),
            compress: false,
        }
    }

    /// Create default SQLite config
    #[cfg(feature = "sqlite")]
    pub fn sqlite_default() -> Self {
        BackendConfig::Sqlite {
            db_path: std::path::PathBuf::from("./memory.db"),
            wal_mode: true,
        }
    }

    /// Create default hybrid config
    pub fn hybrid_default() -> Self {
        BackendConfig::Hybrid {
            cache_size: 1000,
            cold_dir: std::path::PathBuf::from("./memory_cold"),
            sync_interval_secs: 60,
        }
    }
}

/// Factory for creating storage backends
pub struct StorageFactory;

impl StorageFactory {
    /// Create a storage backend from config
    pub async fn create(config: BackendConfig) -> Result<Box<dyn StorageBackend>> {
        match config {
            BackendConfig::File { dir, .. } => {
                let store = FileStore::new(dir).await?;
                Ok(Box::new(store))
            }
            #[cfg(feature = "sqlite")]
            BackendConfig::Sqlite { db_path, wal_mode } => {
                let store = SqliteStore::new(&db_path).await?;
                if wal_mode {
                    store.enable_wal_mode()?;
                }
                Ok(Box::new(store))
            }
            BackendConfig::Hybrid {
                cache_size,
                cold_dir,
                sync_interval_secs,
            } => {
                let store = HybridStore::new(cache_size, cold_dir, sync_interval_secs).await?;
                Ok(Box::new(store))
            }
            #[cfg(feature = "hermes")]
            BackendConfig::Hermes { db_path } => {
                let adapter = HermesMemoryAdapter::new(db_path).await?;
                Ok(Box::new(adapter))
            }
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;

    /// Test helper to verify backend behavior
    pub async fn test_backend_basic<B: StorageBackend>(backend: &B) -> Result<()> {
        let id = backend
            .save_fact(
                "Test fact",
                &["tag1".to_string(), "tag2".to_string()],
                None,
                Some("session-1"),
            )
            .await?;
        assert!(id > 0);

        let fact = backend.get_fact(id).await?.expect("Fact should exist");
        assert_eq!(fact.fact, "Test fact");
        assert_eq!(fact.tags, vec!["tag1", "tag2"]);
        assert_eq!(fact.session_id, Some("session-1".to_string()));

        assert_eq!(backend.count_facts().await?, 1);

        let results = backend.search_by_tags(&["tag1".to_string()], 10).await?;
        assert_eq!(results.len(), 1);

        let results = backend
            .search_by_tags(&["tag1".to_string(), "tag2".to_string()], 10)
            .await?;
        assert_eq!(results.len(), 1);

        assert!(backend.delete_fact(id).await?);
        assert!(!backend.delete_fact(id).await?);

        Ok(())
    }

    /// Test helper for search operations
    pub async fn test_backend_search<B: StorageBackend>(backend: &B) -> Result<()> {
        backend
            .save_fact(
                "Rust programming language",
                &["tech".to_string(), "rust".to_string()],
                None,
                None,
            )
            .await?;
        backend
            .save_fact(
                "Python for data science",
                &["tech".to_string(), "python".to_string()],
                None,
                None,
            )
            .await?;
        backend
            .save_fact(
                "Machine learning basics",
                &["ai".to_string(), "ml".to_string()],
                None,
                None,
            )
            .await?;

        let results = backend
            .search_fulltext("Rust", 10, &DecayConfig::default())
            .await?;
        assert_eq!(results.len(), 1);

        let results = backend.search_by_tags(&["tech".to_string()], 10).await?;
        assert_eq!(results.len(), 2);

        Ok(())
    }
}
