//! Hermes-memory backend adapter for `clarity-memory`.
//!
//! This module provides [`HermesMemoryAdapter`], a `StorageBackend` implementation
//! that delegates to `hermes-memory-store::SqliteStore`. It lets the existing
//! `clarity_memory::MemoryStore` concrete type use hermes-memory as its storage
//! engine without changing the public `MemoryStore` API.
//!
//! The adapter bridges two different data models:
//!
//! - Hermes uses ULID-identified [`MemoryEntry`] values with `target`,
//!   `content`, `provenance`, and `tags`.
//! - Clarity uses monotonically increasing `i64` IDs and [`Fact`] values with
//!   `fact`, `tags`, `time`, and `session_id`.
//!
//! To preserve Clarity's integer IDs, the adapter maintains a persistent
//! `clarity_id <-> ULID` mapping in a sidecar JSON file next to the hermes
//! database. Special `clarity:*` tags carry `session_id` and `time` fields that
//! do not exist in the hermes schema.

use crate::backends::StorageBackend;
use crate::store::{DecayConfig, compute_decay_weight};
use crate::types::{Fact, MemoryError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use hermes_memory_core::{
    InitContext, MemoryBackend, MemoryEntry, MemoryTarget, NullScanner, Provenance, RecallMode,
    RecallOptions, WriteOp,
};
use hermes_memory_store::SqliteStore;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, instrument};
use ulid::Ulid;

/// Persistent mapping between Clarity's `i64` fact IDs and Hermes ULIDs.
#[derive(Debug, Serialize, Deserialize)]
struct IdMap {
    /// Next Clarity ID to assign.
    next_id: i64,
    /// ULID string -> Clarity ID.
    #[serde(default)]
    ulid_to_clarity: HashMap<String, i64>,
    /// Clarity ID -> ULID string.
    #[serde(default)]
    clarity_to_ulid: HashMap<i64, String>,
}

impl Default for IdMap {
    fn default() -> Self {
        Self {
            next_id: 1,
            ulid_to_clarity: HashMap::new(),
            clarity_to_ulid: HashMap::new(),
        }
    }
}

impl IdMap {
    /// Load the map from disk, or return a fresh map if the file does not exist.
    fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path).map_err(MemoryError::Io)?;
            if content.trim().is_empty() {
                Ok(Self::default())
            } else {
                let mut map: IdMap =
                    serde_json::from_str(&content).map_err(MemoryError::Serialization)?;
                if map.next_id <= 0 {
                    map.next_id = 1;
                }
                Ok(map)
            }
        } else {
            Ok(Self::default())
        }
    }

    /// Persist the map to disk.
    fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self).map_err(MemoryError::Serialization)?;
        std::fs::write(path, content).map_err(MemoryError::Io)?;
        Ok(())
    }

    /// Register a new ULID and return the assigned Clarity ID.
    fn insert(&mut self, ulid: &Ulid) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        let ulid_str = ulid.to_string();
        self.ulid_to_clarity.insert(ulid_str.clone(), id);
        self.clarity_to_ulid.insert(id, ulid_str);
        id
    }

    /// Remove a Clarity ID from the map and return its ULID, if any.
    fn remove(&mut self, id: i64) -> Option<String> {
        self.clarity_to_ulid.remove(&id).inspect(|ulid| {
            self.ulid_to_clarity.remove(ulid);
        })
    }

    /// Look up the ULID for a Clarity ID.
    fn get_ulid(&self, id: i64) -> Option<&str> {
        self.clarity_to_ulid.get(&id).map(|s| s.as_str())
    }

    /// Look up the Clarity ID for a ULID.
    fn get_clarity_id(&self, ulid: &Ulid) -> Option<i64> {
        self.ulid_to_clarity.get(&ulid.to_string()).copied()
    }

    /// Clear all mappings and reset the next ID counter.
    fn clear(&mut self) {
        self.next_id = 1;
        self.ulid_to_clarity.clear();
        self.clarity_to_ulid.clear();
    }
}

/// A `StorageBackend` that uses a hermes-memory SQLite store underneath.
#[derive(Debug)]
pub struct HermesMemoryAdapter {
    /// The underlying hermes SQLite backend.
    backend: Arc<SqliteStore>,
    /// Path to the sidecar ID mapping file.
    map_path: PathBuf,
    /// ID mapping guarded by a mutex so adapter operations serialize.
    id_map: Arc<Mutex<IdMap>>,
    /// Time-decay configuration applied to search results.
    decay_config: DecayConfig,
}

impl HermesMemoryAdapter {
    /// Create or open a hermes-backed memory store at the given database path.
    ///
    /// This is an async wrapper around the synchronous hermes backend; all
    /// blocking work runs inside `tokio::task::spawn_blocking`.
    #[instrument(skip(db_path))]
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let map_path = Self::map_path_for(&db_path);

        let backend = tokio::task::spawn_blocking({
            let db_path = db_path.clone();
            move || -> Result<Arc<SqliteStore>> {
                let store = SqliteStore::new(&db_path, Box::new(NullScanner)).map_err(|e| {
                    MemoryError::Hermes(hermes_memory_core::MemoryError::msg(format!(
                        "failed to open store: {e}"
                    )))
                })?;
                store.initialize(InitContext::default()).map_err(|e| {
                    MemoryError::Hermes(hermes_memory_core::MemoryError::msg(format!(
                        "failed to initialize store: {e}"
                    )))
                })?;
                Ok(Arc::new(store))
            }
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))??;

        let id_map = tokio::task::spawn_blocking({
            let map_path = map_path.clone();
            move || IdMap::load(&map_path)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))??;

        info!("HermesMemoryAdapter initialized at {:?}", map_path);
        Ok(Self {
            backend,
            map_path,
            id_map: Arc::new(Mutex::new(id_map)),
            decay_config: DecayConfig::default(),
        })
    }

    /// Create an in-memory hermes-backed store for testing.
    pub async fn new_in_memory() -> Result<Self> {
        let temp_dir =
            std::env::temp_dir().join(format!("clarity_hermes_memory_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).map_err(MemoryError::Io)?;
        Self::new(temp_dir.join("memory.db")).await
    }

    /// Set a custom decay configuration.
    pub fn with_decay_config(mut self, config: DecayConfig) -> Self {
        self.decay_config = config;
        self
    }

    /// Return the sidecar mapping file path for a given database path.
    fn map_path_for(db_path: &Path) -> PathBuf {
        PathBuf::from(format!("{}.clarity-hermes-map.json", db_path.display()))
    }

    /// Build the write tags for a new fact, encoding Clarity-specific fields as
    /// reserved `clarity:*` tags.
    fn build_write_tags(
        tags: &[String],
        time: Option<&str>,
        session_id: Option<&str>,
    ) -> Vec<String> {
        let mut result = tags.to_vec();
        if let Some(t) = time {
            result.push(format!("clarity:time:{}", t));
        }
        if let Some(s) = session_id {
            result.push(format!("clarity:session:{}", s));
        }
        result
    }

    /// Parse the reserved `clarity:*` tags out of a hermes entry, returning the
    /// user tags, optional time, and optional session ID.
    fn parse_entry_tags(entry: &MemoryEntry) -> (Vec<String>, Option<String>, Option<String>) {
        let mut tags = Vec::new();
        let mut time = None;
        let mut session_id = None;
        for tag in &entry.tags {
            if let Some(value) = tag.strip_prefix("clarity:time:") {
                time = Some(value.to_string());
            } else if let Some(value) = tag.strip_prefix("clarity:session:") {
                session_id = Some(value.to_string());
            } else {
                tags.push(tag.clone());
            }
        }
        (tags, time, session_id)
    }

    /// Convert a hermes entry into a Clarity fact using the given Clarity ID.
    fn entry_to_fact(entry: &MemoryEntry, clarity_id: i64) -> Fact {
        let (tags, time, session_id) = Self::parse_entry_tags(entry);
        Fact {
            id: clarity_id,
            fact: entry.content.clone(),
            tags,
            time,
            session_id,
            created_at: entry.created_at,
        }
    }

    /// Recall options for a given search mode and limit.
    fn recall_opts(limit: usize, mode: RecallMode) -> RecallOptions {
        RecallOptions {
            limit,
            mode,
            target_filter: Some(MemoryTarget::Memory),
            ..RecallOptions::default()
        }
    }
}

#[async_trait]
impl StorageBackend for HermesMemoryAdapter {
    #[instrument(skip(self))]
    async fn save_fact(
        &self,
        fact: &str,
        tags: &[String],
        time: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<i64> {
        let fact = fact.to_string();
        let tags = Self::build_write_tags(tags, time, session_id);
        let backend = Arc::clone(&self.backend);
        let map_path = self.map_path.clone();
        let id_map = Arc::clone(&self.id_map);

        tokio::task::spawn_blocking(move || -> Result<i64> {
            let mut map = id_map.lock();
            backend
                .write(WriteOp::Add {
                    target: MemoryTarget::Memory,
                    content: fact,
                    provenance: Some(Provenance::Agent),
                    tags,
                })
                .map_err(MemoryError::Hermes)?;

            let entries = backend
                .list_entries(Some(MemoryTarget::Memory), 1)
                .map_err(MemoryError::Hermes)?;
            let ulid = match entries.first().map(|e| e.id) {
                Some(id) => id,
                None => {
                    return Err(MemoryError::Storage(
                        "hermes write did not create an entry".to_string(),
                    ));
                }
            };

            let clarity_id = map.insert(&ulid);
            map.save(&map_path)?;
            debug!("Saved fact clarity_id={} ulid={}", clarity_id, ulid);
            Ok(clarity_id)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    #[instrument(skip(self))]
    async fn get_fact(&self, id: i64) -> Result<Option<Fact>> {
        let backend = Arc::clone(&self.backend);
        let id_map = Arc::clone(&self.id_map);

        tokio::task::spawn_blocking(move || -> Result<Option<Fact>> {
            let map = id_map.lock();
            let ulid = match map.get_ulid(id) {
                Some(u) => u.parse::<Ulid>().map_err(|e| {
                    MemoryError::Storage(format!("invalid mapped ulid for id {}: {}", id, e))
                })?,
                None => return Ok(None),
            };

            match backend
                .get_entry_by_id(&ulid)
                .map_err(MemoryError::Hermes)?
            {
                Some(entry) => Ok(Some(Self::entry_to_fact(&entry, id))),
                None => Ok(None),
            }
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    #[instrument(skip(self))]
    async fn delete_fact(&self, id: i64) -> Result<bool> {
        let backend = Arc::clone(&self.backend);
        let map_path = self.map_path.clone();
        let id_map = Arc::clone(&self.id_map);

        tokio::task::spawn_blocking(move || -> Result<bool> {
            let mut map = id_map.lock();
            let ulid = match map.get_ulid(id) {
                Some(u) => u.parse::<Ulid>().map_err(|e| {
                    MemoryError::Storage(format!("invalid mapped ulid for id {}: {}", id, e))
                })?,
                None => return Ok(false),
            };

            let entry = match backend
                .get_entry_by_id(&ulid)
                .map_err(MemoryError::Hermes)?
            {
                Some(e) => e,
                None => return Ok(false),
            };

            backend
                .write(WriteOp::Remove {
                    target: MemoryTarget::Memory,
                    old_text: entry.content,
                })
                .map_err(MemoryError::Hermes)?;

            map.remove(id);
            map.save(&map_path)?;
            debug!("Deleted fact clarity_id={}", id);
            Ok(true)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    #[instrument(skip(self))]
    async fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<Fact>> {
        if tags.is_empty() {
            return Ok(Vec::new());
        }
        let backend = Arc::clone(&self.backend);
        let id_map = Arc::clone(&self.id_map);
        let tags = tags.to_vec();

        tokio::task::spawn_blocking(move || -> Result<Vec<Fact>> {
            let map = id_map.lock();
            let count = backend
                .entry_count_by_target(MemoryTarget::Memory)
                .map_err(MemoryError::Hermes)?;
            let entries = backend
                .list_entries(Some(MemoryTarget::Memory), count)
                .map_err(MemoryError::Hermes)?;

            let mut facts = Vec::new();
            for entry in entries {
                if tags.iter().all(|t| entry.tags.contains(t)) {
                    if let Some(id) = map.get_clarity_id(&entry.id) {
                        facts.push(Self::entry_to_fact(&entry, id));
                        if facts.len() >= limit {
                            break;
                        }
                    }
                }
            }
            Ok(facts)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    #[instrument(skip(self))]
    async fn search_fulltext(
        &self,
        query: &str,
        limit: usize,
        _decay: &DecayConfig,
    ) -> Result<Vec<Fact>> {
        let query = query.to_string();
        let backend = Arc::clone(&self.backend);
        let id_map = Arc::clone(&self.id_map);
        let decay = self.decay_config;

        tokio::task::spawn_blocking(move || -> Result<Vec<Fact>> {
            let map = id_map.lock();
            let result = backend
                .recall(&query, Self::recall_opts(limit, RecallMode::Keyword))
                .map_err(MemoryError::Hermes)?;

            let mut scored: Vec<(Fact, f32)> = result
                .entries
                .iter()
                .filter_map(|e| {
                    let clarity_id = map.get_clarity_id(&e.entry.id)?;
                    let fact = Self::entry_to_fact(&e.entry, clarity_id);
                    let weight = compute_decay_weight(fact.created_at, &decay) as f32;
                    Some((fact, e.score * weight))
                })
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            Ok(scored.into_iter().map(|(f, _)| f).take(limit).collect())
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    #[instrument(skip(self))]
    async fn get_facts_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<Fact>> {
        let session_id = session_id.to_string();
        let backend = Arc::clone(&self.backend);
        let id_map = Arc::clone(&self.id_map);

        tokio::task::spawn_blocking(move || -> Result<Vec<Fact>> {
            let map = id_map.lock();
            let count = backend
                .entry_count_by_target(MemoryTarget::Memory)
                .map_err(MemoryError::Hermes)?;
            let entries = backend
                .list_entries(Some(MemoryTarget::Memory), count)
                .map_err(MemoryError::Hermes)?;

            let needle = format!("clarity:session:{}", session_id);
            let mut facts = Vec::new();
            for entry in entries {
                if entry.tags.contains(&needle) {
                    if let Some(id) = map.get_clarity_id(&entry.id) {
                        facts.push(Self::entry_to_fact(&entry, id));
                        if facts.len() >= limit {
                            break;
                        }
                    }
                }
            }
            Ok(facts)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    #[instrument(skip(self))]
    async fn get_facts_since(&self, since: DateTime<Utc>) -> Result<Vec<Fact>> {
        let backend = Arc::clone(&self.backend);
        let id_map = Arc::clone(&self.id_map);

        tokio::task::spawn_blocking(move || -> Result<Vec<Fact>> {
            let map = id_map.lock();
            let count = backend
                .entry_count_by_target(MemoryTarget::Memory)
                .map_err(MemoryError::Hermes)?;
            let entries = backend
                .list_entries(Some(MemoryTarget::Memory), count)
                .map_err(MemoryError::Hermes)?;

            let mut facts: Vec<Fact> = entries
                .into_iter()
                .filter(|e| e.created_at > since)
                .filter_map(|e| {
                    let id = map.get_clarity_id(&e.id)?;
                    Some(Self::entry_to_fact(&e, id))
                })
                .collect();
            facts.sort_by_key(|b| std::cmp::Reverse(b.created_at));
            Ok(facts)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    #[instrument(skip(self))]
    async fn get_recent_facts(&self, limit: usize) -> Result<Vec<Fact>> {
        let backend = Arc::clone(&self.backend);
        let id_map = Arc::clone(&self.id_map);

        tokio::task::spawn_blocking(move || -> Result<Vec<Fact>> {
            let map = id_map.lock();
            let entries = backend
                .list_entries(Some(MemoryTarget::Memory), limit)
                .map_err(MemoryError::Hermes)?;
            Ok(entries
                .into_iter()
                .filter_map(|e| {
                    let id = map.get_clarity_id(&e.id)?;
                    Some(Self::entry_to_fact(&e, id))
                })
                .collect())
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    #[instrument(skip(self))]
    async fn count_facts(&self) -> Result<i64> {
        let backend = Arc::clone(&self.backend);
        tokio::task::spawn_blocking(move || -> Result<i64> {
            let count = backend
                .entry_count_by_target(MemoryTarget::Memory)
                .map_err(MemoryError::Hermes)?;
            Ok(count as i64)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    #[instrument(skip(self))]
    async fn clear_all(&self) -> Result<usize> {
        let backend = Arc::clone(&self.backend);
        let map_path = self.map_path.clone();
        let id_map = Arc::clone(&self.id_map);

        tokio::task::spawn_blocking(move || -> Result<usize> {
            let mut map = id_map.lock();
            let count = backend
                .entry_count_by_target(MemoryTarget::Memory)
                .map_err(MemoryError::Hermes)?;
            let entries = backend
                .list_entries(Some(MemoryTarget::Memory), count)
                .map_err(MemoryError::Hermes)?;

            for entry in entries {
                backend
                    .write(WriteOp::Remove {
                        target: MemoryTarget::Memory,
                        old_text: entry.content,
                    })
                    .map_err(MemoryError::Hermes)?;
            }

            map.clear();
            map.save(&map_path)?;
            info!("Cleared all hermes-backed facts");
            Ok(count)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    #[instrument(skip(self))]
    async fn search_similar(
        &self,
        query: &str,
        limit: usize,
        _decay: &DecayConfig,
    ) -> Result<Vec<(Fact, f32)>> {
        let query = query.to_string();
        let backend = Arc::clone(&self.backend);
        let id_map = Arc::clone(&self.id_map);
        let decay = self.decay_config;

        tokio::task::spawn_blocking(move || -> Result<Vec<(Fact, f32)>> {
            let map = id_map.lock();
            let result = backend
                .recall(&query, Self::recall_opts(limit, RecallMode::Hybrid))
                .map_err(MemoryError::Hermes)?;

            let mut scored: Vec<(Fact, f32)> = result
                .entries
                .iter()
                .filter_map(|e| {
                    let clarity_id = map.get_clarity_id(&e.entry.id)?;
                    let fact = Self::entry_to_fact(&e.entry, clarity_id);
                    let weight = compute_decay_weight(fact.created_at, &decay) as f32;
                    Some((fact, e.score * weight))
                })
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(limit);
            Ok(scored)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    #[instrument(skip(self))]
    async fn search_semantic(
        &self,
        query: &str,
        limit: usize,
        _decay: &DecayConfig,
    ) -> Result<Vec<(Fact, f32)>> {
        let query = query.to_string();
        let backend = Arc::clone(&self.backend);
        let id_map = Arc::clone(&self.id_map);
        let decay = self.decay_config;

        tokio::task::spawn_blocking(move || -> Result<Vec<(Fact, f32)>> {
            let map = id_map.lock();
            let result = backend
                .recall(&query, Self::recall_opts(limit, RecallMode::Vector))
                .map_err(MemoryError::Hermes)?;

            let mut scored: Vec<(Fact, f32)> = result
                .entries
                .iter()
                .filter_map(|e| {
                    let clarity_id = map.get_clarity_id(&e.entry.id)?;
                    let fact = Self::entry_to_fact(&e.entry, clarity_id);
                    let weight = compute_decay_weight(fact.created_at, &decay) as f32;
                    Some((fact, e.score * weight))
                })
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(limit);
            Ok(scored)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_adapter() -> (tempfile::TempDir, HermesMemoryAdapter) {
        let temp_dir = tempfile::tempdir().unwrap();
        let adapter = HermesMemoryAdapter::new(temp_dir.path().join("memory.db"))
            .await
            .unwrap();
        (temp_dir, adapter)
    }

    #[tokio::test]
    async fn test_save_and_get_fact() {
        let (_temp, adapter) = create_test_adapter().await;

        let id = adapter
            .save_fact(
                "User likes Rust",
                &["preference".to_string(), "tech".to_string()],
                Some("2024-01-15"),
                Some("session-1"),
            )
            .await
            .unwrap();
        assert!(id > 0);

        let fact = adapter
            .get_fact(id)
            .await
            .unwrap()
            .expect("fact should exist");
        assert_eq!(fact.fact, "User likes Rust");
        assert_eq!(fact.tags, vec!["preference", "tech"]);
        assert_eq!(fact.time, Some("2024-01-15".to_string()));
        assert_eq!(fact.session_id, Some("session-1".to_string()));
    }

    #[tokio::test]
    async fn test_search_fulltext() {
        let (_temp, adapter) = create_test_adapter().await;

        adapter
            .save_fact(
                "Rust is a systems language",
                &["tech".to_string()],
                None,
                None,
            )
            .await
            .unwrap();
        adapter
            .save_fact(
                "Python is great for data",
                &["tech".to_string()],
                None,
                None,
            )
            .await
            .unwrap();

        let results = adapter
            .search_fulltext("Rust", 10, &DecayConfig::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].fact.contains("Rust"));
    }

    #[tokio::test]
    async fn test_search_by_tags() {
        let (_temp, adapter) = create_test_adapter().await;

        adapter
            .save_fact("Fact A", &["a".to_string(), "b".to_string()], None, None)
            .await
            .unwrap();
        adapter
            .save_fact("Fact B", &["b".to_string()], None, None)
            .await
            .unwrap();

        let results = adapter
            .search_by_tags(&["a".to_string()], 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fact, "Fact A");
    }

    #[tokio::test]
    async fn test_delete_fact() {
        let (_temp, adapter) = create_test_adapter().await;

        let id = adapter
            .save_fact("Delete me", &["test".to_string()], None, None)
            .await
            .unwrap();
        assert!(adapter.delete_fact(id).await.unwrap());
        assert!(adapter.get_fact(id).await.unwrap().is_none());
        assert!(!adapter.delete_fact(999).await.unwrap());
    }

    #[tokio::test]
    async fn test_count_and_clear() {
        let (_temp, adapter) = create_test_adapter().await;

        assert_eq!(adapter.count_facts().await.unwrap(), 0);

        adapter
            .save_fact("Fact 1", &["x".to_string()], None, None)
            .await
            .unwrap();
        adapter
            .save_fact("Fact 2", &["x".to_string()], None, None)
            .await
            .unwrap();

        assert_eq!(adapter.count_facts().await.unwrap(), 2);

        let cleared = adapter.clear_all().await.unwrap();
        assert_eq!(cleared, 2);
        assert_eq!(adapter.count_facts().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_persists_across_reopen() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("memory.db");

        let adapter = HermesMemoryAdapter::new(&db_path).await.unwrap();
        let id = adapter
            .save_fact("Persisted fact", &["test".to_string()], None, None)
            .await
            .unwrap();
        drop(adapter);

        let adapter = HermesMemoryAdapter::new(&db_path).await.unwrap();
        let fact = adapter
            .get_fact(id)
            .await
            .unwrap()
            .expect("fact should persist");
        assert_eq!(fact.fact, "Persisted fact");
    }
}
