//! Hybrid storage - Hot cache + Cold storage
use crate::backends::file::FileStore;
use crate::backends::StorageBackend;
use crate::store::DecayConfig;
use crate::types::{Fact, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone)]
struct CachedFact {
    fact: Fact,
    last_access: i64,
}

pub struct HybridStore {
    hot_cache: Arc<DashMap<i64, CachedFact>>,
    cold_storage: FileStore,
    cache_size: usize,
    access_counter: AtomicI64,
    sync_handle: Option<tokio::task::JoinHandle<()>>,
    shutdown: Arc<AtomicI64>,
}

impl std::fmt::Debug for HybridStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HybridStore")
            .field("hot_cache", &self.hot_cache)
            .field("cold_storage", &self.cold_storage)
            .field("cache_size", &self.cache_size)
            .field("access_counter", &self.access_counter)
            .field("sync_handle", &self.sync_handle.is_some())
            .finish()
    }
}

impl HybridStore {
    pub async fn new(
        cache_size: usize,
        cold_dir: impl AsRef<std::path::Path>,
        sync_interval_secs: u64,
    ) -> Result<Self> {
        let cold_storage = FileStore::new(cold_dir).await?;
        let hot_cache = Arc::new(DashMap::<i64, CachedFact>::with_capacity(cache_size));
        let shutdown = Arc::new(AtomicI64::new(0));

        let sync_handle = if sync_interval_secs > 0 {
            let cache = hot_cache.clone();
            let cold = cold_storage.clone();
            let shutdown = shutdown.clone();
            Some(tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(sync_interval_secs));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    if shutdown.load(Ordering::Relaxed) != 0 {
                        break;
                    }
                    for entry in cache.iter() {
                        let cf = entry.value();
                        let _ = cold
                            .save_fact(
                                &cf.fact.fact,
                                &cf.fact.tags,
                                cf.fact.time.as_deref(),
                                cf.fact.session_id.as_deref(),
                            )
                            .await;
                    }
                }
            }))
        } else {
            None
        };

        Ok(Self {
            hot_cache,
            cold_storage,
            cache_size,
            access_counter: AtomicI64::new(0),
            sync_handle,
            shutdown,
        })
    }

    fn next_access(&self) -> i64 {
        self.access_counter.fetch_add(1, Ordering::SeqCst)
    }

    fn promote_to_cache(&self, fact: Fact) {
        if self.hot_cache.len() >= self.cache_size {
            self.evict_lru();
        }
        self.hot_cache.insert(
            fact.id,
            CachedFact {
                last_access: self.next_access(),
                fact,
            },
        );
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

    fn cached_facts<F>(&self, predicate: F) -> Vec<Fact>
    where
        F: Fn(&Fact) -> bool,
    {
        self.hot_cache
            .iter()
            .map(|e| e.value().fact.clone())
            .filter(predicate)
            .collect()
    }

    fn merge_facts(&self, cached: Vec<Fact>, cold: Vec<Fact>) -> Vec<Fact> {
        let mut seen = HashSet::new();
        let mut merged = Vec::with_capacity(cached.len() + cold.len());
        // Prefer cached facts (they may be more recent)
        for fact in cached {
            if seen.insert(fact.id) {
                merged.push(fact);
            }
        }
        for fact in cold {
            if seen.insert(fact.id) {
                merged.push(fact);
            }
        }
        merged
    }

    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            cache_size: self.hot_cache.len(),
            max_cache_size: self.cache_size,
        }
    }
}

impl Drop for HybridStore {
    fn drop(&mut self) {
        self.shutdown.store(1, Ordering::Relaxed);
        if let Some(handle) = self.sync_handle.take() {
            handle.abort();
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
    async fn save_fact(
        &self,
        fact: &str,
        tags: &[String],
        time: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<i64> {
        let id = self
            .cold_storage
            .save_fact(fact, tags, time, session_id)
            .await?;
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
            let fact = entry.fact.clone();
            drop(entry);
            self.touch_cache(id);
            return Ok(Some(fact));
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
        let cached = if tags.is_empty() {
            Vec::new()
        } else {
            self.cached_facts(|f| tags.iter().all(|tag| f.tags.contains(tag)))
        };
        let cold = self.cold_storage.search_by_tags(tags, limit).await?;
        let mut merged = self.merge_facts(cached, cold);
        merged.truncate(limit);
        Ok(merged)
    }

    async fn search_fulltext(
        &self,
        query: &str,
        limit: usize,
        decay: &DecayConfig,
    ) -> Result<Vec<Fact>> {
        let query_lower = query.to_lowercase();
        let cached = self.cached_facts(|f| f.fact.to_lowercase().contains(&query_lower));
        let cold = self
            .cold_storage
            .search_fulltext(query, limit * 5, decay)
            .await?;
        let merged = self.merge_facts(cached, cold);

        let now = Utc::now();
        let lambda = std::f64::consts::LN_2 / decay.half_life_days;
        let mut scored: Vec<(Fact, f64)> = merged
            .into_iter()
            .map(|fact| {
                let weight = if decay.enabled {
                    let age_days = (now - fact.created_at).num_days() as f64;
                    (-lambda * age_days).exp()
                } else {
                    1.0
                };
                (fact, weight)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let facts: Vec<Fact> = scored.into_iter().map(|(f, _)| f).take(limit).collect();
        Ok(facts)
    }

    async fn get_facts_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<Fact>> {
        let cached = self.cached_facts(|f| f.session_id.as_deref() == Some(session_id));
        let cold = self
            .cold_storage
            .get_facts_by_session(session_id, limit)
            .await?;
        let mut merged = self.merge_facts(cached, cold);
        merged.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        merged.truncate(limit);
        Ok(merged)
    }

    async fn get_facts_since(&self, since: DateTime<Utc>) -> Result<Vec<Fact>> {
        let cached = self.cached_facts(|f| f.created_at > since);
        let cold = self.cold_storage.get_facts_since(since).await?;
        let mut merged = self.merge_facts(cached, cold);
        merged.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(merged)
    }

    async fn get_recent_facts(&self, limit: usize) -> Result<Vec<Fact>> {
        let cached = self.cached_facts(|_| true);
        let cold = self.cold_storage.get_recent_facts(limit).await?;
        let mut merged = self.merge_facts(cached, cold);
        merged.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        merged.truncate(limit);
        Ok(merged)
    }

    async fn count_facts(&self) -> Result<i64> {
        self.cold_storage.count_facts().await
    }

    async fn clear_all(&self) -> Result<usize> {
        self.hot_cache.clear();
        self.cold_storage.clear_all().await
    }
}

#[cfg(test)]
mod simple_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_hybrid_store_backend() {
        let temp_dir = TempDir::new().unwrap();
        let store = HybridStore::new(100, temp_dir.path(), 0).await.unwrap();

        let id = store
            .save_fact("Hybrid store test", &["test".to_string()], None, None)
            .await
            .unwrap();

        let fact = store
            .get_fact(id)
            .await
            .unwrap()
            .expect("Fact should exist");
        assert_eq!(fact.fact, "Hybrid store test");

        let stats = store.cache_stats();
        assert_eq!(stats.cache_size, 1);
    }
}
