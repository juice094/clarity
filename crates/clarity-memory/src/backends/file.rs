//! File-based storage backend with atomic writes
use crate::backends::StorageBackend;
use crate::bm25::IncrementalBm25Index;
use crate::store::{DecayConfig, compute_decay_weight};
use crate::types::{Fact, MemoryError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

/// File-based storage backend with atomic writes.
#[derive(Debug, Clone)]
pub struct FileStore {
    dir: PathBuf,
    index: Arc<DashMap<i64, FactFile>>,
    next_id: Arc<AtomicI64>,
    meta_path: PathBuf,
    tags_index: Arc<DashMap<String, Vec<i64>>>,
    /// In-memory BM25 index used to rank facts in `search_similar`.
    /// ponytail: rebuilt lazily on first search if missing; replace with
    /// persisted index if facts exceed a few thousand.
    bm25_index: Arc<Mutex<IncrementalBm25Index>>,
    /// Maps fact id to BM25 doc index.
    bm25_id_map: Arc<DashMap<i64, usize>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FactFile {
    id: i64,
    fact: String,
    tags: Vec<String>,
    time: Option<String>,
    session_id: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoreMetadata {
    next_id: i64,
    version: u32,
}

impl FileStore {
    /// Create a new file-backed store in the given directory.
    pub async fn new(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let meta_path = dir.join("_meta.json");

        if !dir.exists() {
            fs::create_dir_all(&dir).await.map_err(MemoryError::Io)?;
            info!("Created file storage directory at {:?}", dir);
        }

        let store = Self {
            dir,
            index: Arc::new(DashMap::new()),
            next_id: Arc::new(AtomicI64::new(1)),
            meta_path,
            tags_index: Arc::new(DashMap::new()),
            bm25_index: Arc::new(Mutex::new(IncrementalBm25Index::new())),
            bm25_id_map: Arc::new(DashMap::new()),
        };

        store.load_index().await?;
        Ok(store)
    }

    fn fact_path(&self, id: i64) -> PathBuf {
        let subdir = (id % 100).to_string();
        self.dir.join(subdir).join(format!("{}.json", id))
    }

    async fn load_index(&self) -> Result<()> {
        if self.meta_path.exists() {
            let content = fs::read_to_string(&self.meta_path)
                .await
                .map_err(MemoryError::Io)?;
            if let Ok(meta) = serde_json::from_str::<StoreMetadata>(&content) {
                self.next_id.store(meta.next_id, Ordering::SeqCst);
            }
        }

        let mut entries = fs::read_dir(&self.dir).await.map_err(MemoryError::Io)?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();

            if path == self.meta_path {
                continue;
            }

            if path.is_dir() {
                if let Ok(mut sub_entries) = fs::read_dir(&path).await {
                    while let Ok(Some(sub_entry)) = sub_entries.next_entry().await {
                        let sub_path = sub_entry.path();
                        if sub_path.extension().map(|e| e == "json").unwrap_or(false) {
                            let _ = self.load_fact_file(&sub_path).await;
                        }
                    }
                }
            } else if path.extension().map(|e| e == "json").unwrap_or(false) {
                let _ = self.load_fact_file(&path).await;
            }
        }

        debug!("Loaded {} facts into index", self.index.len());
        Ok(())
    }

    async fn load_fact_file(&self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path).await.map_err(MemoryError::Io)?;
        let fact_file: FactFile = serde_json::from_str(&content)?;

        let current_next = self.next_id.load(Ordering::SeqCst);
        if fact_file.id >= current_next {
            self.next_id.store(fact_file.id + 1, Ordering::SeqCst);
        }

        self.index.insert(fact_file.id, fact_file.clone());

        {
            let mut bm25 = self.bm25_index.lock();
            let doc_idx = bm25.add_document(&fact_file.fact);
            self.bm25_id_map.insert(fact_file.id, doc_idx);
        }

        for tag in &fact_file.tags {
            self.tags_index
                .entry(tag.clone())
                .or_default()
                .push(fact_file.id);
        }

        Ok(())
    }

    async fn save_metadata(&self) -> Result<()> {
        let meta = StoreMetadata {
            next_id: self.next_id.load(Ordering::SeqCst),
            version: 1,
        };
        let content = serde_json::to_string_pretty(&meta)?;

        let temp_path = self.meta_path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path)
            .await
            .map_err(MemoryError::Io)?;
        file.write_all(content.as_bytes())
            .await
            .map_err(MemoryError::Io)?;
        file.flush().await.map_err(MemoryError::Io)?;
        drop(file);

        fs::rename(&temp_path, &self.meta_path)
            .await
            .map_err(MemoryError::Io)?;

        Ok(())
    }

    async fn write_fact_file(&self, fact: &FactFile) -> Result<()> {
        let path = self.fact_path(fact.id);

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await.map_err(MemoryError::Io)?;
            }
        }

        let content = serde_json::to_string_pretty(fact)?;

        let temp_path = path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path)
            .await
            .map_err(MemoryError::Io)?;
        file.write_all(content.as_bytes())
            .await
            .map_err(MemoryError::Io)?;
        file.flush().await.map_err(MemoryError::Io)?;
        drop(file);

        fs::rename(&temp_path, &path)
            .await
            .map_err(MemoryError::Io)?;

        Ok(())
    }

    async fn delete_fact_file(&self, id: i64) -> Result<()> {
        let path = self.fact_path(id);
        if path.exists() {
            fs::remove_file(&path).await.map_err(MemoryError::Io)?;
        }
        Ok(())
    }

    fn add_to_tags_index(&self, id: i64, tags: &[String]) {
        for tag in tags {
            self.tags_index.entry(tag.clone()).or_default().push(id);
        }
    }

    fn remove_from_tags_index(&self, id: i64, tags: &[String]) {
        for tag in tags {
            if let Some(mut entry) = self.tags_index.get_mut(tag) {
                entry.retain(|&x| x != id);
            }
        }
    }
}

#[async_trait]
impl StorageBackend for FileStore {
    async fn save_fact(
        &self,
        fact: &str,
        tags: &[String],
        time: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<i64> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let now = Utc::now().to_rfc3339();

        let fact_file = FactFile {
            id,
            fact: fact.to_string(),
            tags: tags.to_vec(),
            time: time.map(|s| s.to_string()),
            session_id: session_id.map(|s| s.to_string()),
            created_at: now,
        };

        self.write_fact_file(&fact_file).await?;
        self.index.insert(id, fact_file.clone());
        self.add_to_tags_index(id, tags);

        {
            let mut bm25 = self.bm25_index.lock();
            let doc_idx = bm25.add_document(&fact_file.fact);
            self.bm25_id_map.insert(id, doc_idx);
        }

        self.save_metadata().await?;

        info!("Saved fact with id={}", id);
        Ok(id)
    }

    async fn get_fact(&self, id: i64) -> Result<Option<Fact>> {
        if let Some(entry) = self.index.get(&id) {
            let ff = entry.value();
            Ok(Some(Fact {
                id: ff.id,
                fact: ff.fact.clone(),
                tags: ff.tags.clone(),
                time: ff.time.clone(),
                session_id: ff.session_id.clone(),
                created_at: DateTime::parse_from_rfc3339(&ff.created_at)
                    .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
                    .with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }

    async fn delete_fact(&self, id: i64) -> Result<bool> {
        if let Some((_, fact_file)) = self.index.remove(&id) {
            self.remove_from_tags_index(id, &fact_file.tags);
            self.delete_fact_file(id).await?;

            if let Some((_, doc_idx)) = self.bm25_id_map.remove(&id) {
                let mut bm25 = self.bm25_index.lock();
                bm25.remove_document(doc_idx);
            }

            self.save_metadata().await?;

            info!("Deleted fact with id={}", id);
            Ok(true)
        } else {
            warn!("Attempted to delete non-existent fact with id={}", id);
            Ok(false)
        }
    }

    async fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<Fact>> {
        if tags.is_empty() {
            return Ok(Vec::new());
        }

        let mut candidate_ids: Option<Vec<i64>> = None;

        for tag in tags {
            if let Some(entry) = self.tags_index.get(tag) {
                let ids = entry.value().clone();
                match candidate_ids {
                    None => candidate_ids = Some(ids),
                    Some(ref mut current) => {
                        current.retain(|id| ids.contains(id));
                    }
                }
            } else {
                return Ok(Vec::new());
            }
        }

        let ids = candidate_ids.unwrap_or_default();
        let mut facts = Vec::new();

        for id in ids.into_iter().take(limit) {
            if let Some(fact) = self.get_fact(id).await? {
                facts.push(fact);
            }
        }

        debug!("Found {} facts matching tags {:?}", facts.len(), tags);
        Ok(facts)
    }

    async fn search_fulltext(
        &self,
        query: &str,
        limit: usize,
        decay: &DecayConfig,
    ) -> Result<Vec<Fact>> {
        let query_lower = query.to_lowercase();
        let mut scored = Vec::new();

        for entry in self.index.iter() {
            let ff = entry.value();
            if ff.fact.to_lowercase().contains(&query_lower) {
                if let Some(fact) = self.get_fact(ff.id).await? {
                    let weight = compute_decay_weight(fact.created_at, decay);
                    scored.push((fact, weight));
                }
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let facts: Vec<Fact> = scored.into_iter().map(|(f, _)| f).take(limit).collect();

        debug!("Found {} facts matching query '{}'", facts.len(), query);
        Ok(facts)
    }

    async fn search_similar(
        &self,
        query: &str,
        limit: usize,
        decay: &DecayConfig,
    ) -> Result<Vec<(Fact, f32)>> {
        let query = query.to_string();
        let scored_idx: Vec<(i64, f32)> = {
            let bm25 = self.bm25_index.lock();
            self.bm25_id_map
                .iter()
                .filter_map(|entry| {
                    let id = *entry.key();
                    let doc_idx = *entry.value();
                    let score = bm25.score(&query, doc_idx);
                    if score > 0.0 { Some((id, score)) } else { None }
                })
                .collect()
        };

        let mut scored: Vec<(Fact, f32)> = Vec::with_capacity(scored_idx.len());
        for (id, score) in scored_idx {
            if let Some(fact) = self.get_fact(id).await? {
                let weight = compute_decay_weight(fact.created_at, decay) as f32;
                scored.push((fact, score * weight));
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored)
    }

    async fn get_facts_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<Fact>> {
        let mut facts: Vec<Fact> = Vec::new();

        for entry in self.index.iter() {
            let ff = entry.value();
            if ff.session_id.as_deref() == Some(session_id) {
                if let Some(fact) = self.get_fact(ff.id).await? {
                    facts.push(fact);
                    if facts.len() >= limit {
                        break;
                    }
                }
            }
        }

        facts.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(facts)
    }

    async fn get_facts_since(&self, since: DateTime<Utc>) -> Result<Vec<Fact>> {
        let mut facts = Vec::new();

        for entry in self.index.iter() {
            let ff = entry.value();
            let created_at = DateTime::parse_from_rfc3339(&ff.created_at)
                .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
                .with_timezone(&Utc);

            if created_at > since {
                if let Some(fact) = self.get_fact(ff.id).await? {
                    facts.push(fact);
                }
            }
        }

        facts.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(facts)
    }

    async fn get_recent_facts(&self, limit: usize) -> Result<Vec<Fact>> {
        let mut facts: Vec<Fact> = Vec::new();

        for entry in self.index.iter() {
            if let Some(fact) = self.get_fact(entry.value().id).await? {
                facts.push(fact);
            }
        }

        facts.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        facts.truncate(limit);
        Ok(facts)
    }

    async fn count_facts(&self) -> Result<i64> {
        Ok(self.index.len() as i64)
    }

    async fn clear_all(&self) -> Result<usize> {
        let count = self.index.len();
        self.index.clear();
        self.tags_index.clear();
        self.bm25_id_map.clear();
        *self.bm25_index.lock() = IncrementalBm25Index::new();

        let mut entries = fs::read_dir(&self.dir).await.map_err(MemoryError::Io)?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path).await;
            } else if path != self.meta_path {
                let _ = fs::remove_file(&path).await;
            }
        }

        self.next_id.store(1, Ordering::SeqCst);
        self.save_metadata().await?;

        info!("Cleared all {} facts", count);
        Ok(count)
    }

    async fn bulk_save_facts(
        &self,
        facts: &[(&str, Vec<String>, Option<&str>, Option<&str>)],
    ) -> Result<Vec<i64>> {
        let mut ids = Vec::with_capacity(facts.len());

        for (fact, tags, time, session_id) in facts {
            let id = self.save_fact(fact, tags, *time, *session_id).await?;
            ids.push(id);
        }

        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_store() -> (TempDir, FileStore) {
        let temp_dir = TempDir::new().unwrap();
        let store = FileStore::new(temp_dir.path()).await.unwrap();
        (temp_dir, store)
    }

    #[tokio::test]
    async fn test_save_and_retrieve() {
        let (_temp, store) = create_test_store().await;
        let id = store
            .save_fact("Test fact", &["tag1".to_string()], None, Some("session-1"))
            .await
            .unwrap();
        let fact = store
            .get_fact(id)
            .await
            .unwrap()
            .expect("Fact should exist");
        assert_eq!(fact.fact, "Test fact");
        assert_eq!(fact.tags, vec!["tag1"]);
    }
}
