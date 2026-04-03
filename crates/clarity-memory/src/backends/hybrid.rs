//! Hybrid storage - Hot cache + Cold storage
use crate::backends::file::FileStore;
use crate::backends::StorageBackend;
use crate::types::{Fact, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone)]
struct CachedFact {
    fact: Fact,
    last_access: i64,
}

#[derive(Debug)]
pub struct HybridStore {
    hot_cache: Arc<DashMap<i64, CachedFact>>,
    cold_storage: FileStore,
    cache_size: usize,
    access_counter: AtomicI64,
}

impl HybridStore {
    pub async fn new(cache_size: usize, cold_dir: impl AsRef<std::path::Path>, _sync_interval_secs: u64) -> Result<Self> {
        let cold_storage = FileStore::new(cold_dir).await?;
        Ok(Self {
            hot_cache: Arc::new(DashMap::with_capacity(cache_size)),
            cold_storage,
            cache_size,
            access_counter: AtomicI64::new(0),
        })
    }

    fn next_access(&self) -> i64 {
        self.access_counter.fetch_add(1, Ordering::SeqCst)
    }

    fn promote_to_cache(&self, fact: Fact) {
        if self.hot_cache.len() >= self.cache_size {
            self.evict_lru();
        }
        self.hot_cache.insert(fact.id, CachedFact {
            last_access: self.next_access(),
            fact,
        });
    }

    fn evict_lru(&self) {
        let mut lru_id: Option<(i64, i64)> = None;
        for entry in self.hot_cache.iter() {
            let access = entry.value().last_access;
            if lru_id.map(|(_, la)| access < la).unwrap_or(true) {
                lru_id = Some((*entry.key(), access));
            }
        }
        if let Some((id, _)) = lru_id {
            self.hot_cache.remove(&id);
        }
    }

    fn touch_cache(&self, id: i64) {
        if let Some(mut entry) = self.hot_cache.get_mut(&id) {
            entry.last_access = self.next_access();
        }
    }

    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            cache_size: self.hot_cache.len(),
            max_cache_size: self.cache_size,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub cache_size: usize,
    pub max_cache_size: usize,
}

#[async_trait]
impl StorageBackend for HybridStore {
    async fn save_fact(&self, fact: &str, tags: &[String], time: Option<&str>, session_id: Option<&str>) -> Result<i64> {
        let id = self.cold_storage.save_fact(fact, tags, time, session_id).await?;
        let fact_obj = Fact {
            id,
            fact: fact.to_string(),
            tags: tags.to_vec(),
            time: time.map(|s| s.to_string()),
            session_id: session_id.map(|s| s.to_string()),
            created_at: Utc::now(),
        };
        self.promote_to_cache(fact_obj);
        info!("Saved fact with id={} to hybrid store", id);
        Ok(id)
    }

    async fn get_fact(&self, id: i64) -> Result<Option<Fact>> {
        if let Some(entry) = self.hot_cache.get(&id) {
            self.touch_cache(id);
            return Ok(Some(entry.fact.clone()));
        }
        if let Some(fact) = self.cold_storage.get_fact(id).await? {
            self.promote_to_cache(fact.clone());
            return Ok(Some(fact));
        }
        Ok(None)
    }

    async fn delete_fact(&self, id: i64) -> Result<bool> {
        self.hot_cache.remove(&id);
        self.cold_storage.delete_fact(id).await
    }

    async fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<Fact>> {
        self.cold_storage.search_by_tags(tags, limit).await
    }

    async fn search_fulltext(&self, query: &str, limit: usize) -> Result<Vec<Fact>> {
        self.cold_storage.search_fulltext(query, limit).await
    }

    async fn get_facts_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<Fact>> {
        self.cold_storage.get_facts_by_session(session_id, limit).await
    }

    async fn get_facts_since(&self, since: DateTime<Utc>) -> Result<Vec<Fact>> {
        self.cold_storage.get_facts_since(since).await
    }

    async fn get_recent_facts(&self, limit: usize) -> Result<Vec<Fact>> {
        self.cold_storage.get_recent_facts(limit).await
    }

    async fn count_facts(&self) -> Result<i64> {
        self.cold_storage.count_facts().await
    }

    async fn clear_all(&self) -> Result<usize> {
        self.hot_cache.clear();
        self.cold_storage.clear_all().await
    }
}
