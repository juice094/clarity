//! Compaction result cache — avoids redundant LLM compaction calls.
//!
//! When the same message prefix appears across multiple turns (common with
//! persistent system prompts + early conversation), the LLM-based compaction
//! would re-compute identical summaries. This cache stores compaction results
//! keyed by the hash of the input messages, following syncthing-rust's
//! `syncthing-db/src/block_cache/` write-through pattern.
//!
//! # Design
//!
//! - **Deterministic key**: hash of (role, content) for all messages
//! - **LRU eviction**: bounded capacity (default 16), least-recently-used first
//! - **System prompt aware**: `invalidate_all()` when system prompt changes
//! - **TTL**: entries expire after 1 hour to avoid stale results across long sessions

use clarity_contract::Message;
use std::collections::{VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

/// Maximum cached compaction results.
const DEFAULT_CAPACITY: usize = 16;

/// TTL for cached entries.
const DEFAULT_TTL: Duration = Duration::from_secs(3600);

/// A cached compaction result.
#[derive(Debug, Clone)]
struct CacheEntry {
    compacted: Vec<Message>,
    created_at: Instant,
}

/// Cache for LLM-based context compaction results.
#[derive(Debug, Clone)]
pub struct CompactionCache {
    inner: VecDeque<(u64, CacheEntry)>,
    capacity: usize,
    ttl: Duration,
}

impl CompactionCache {
    /// Create a new cache with default capacity (16) and TTL (1 hour).
    pub fn new() -> Self {
        Self {
            inner: VecDeque::with_capacity(DEFAULT_CAPACITY),
            capacity: DEFAULT_CAPACITY,
            ttl: DEFAULT_TTL,
        }
    }

    /// Hash a message sequence into a cache key.
    pub fn hash_messages(messages: &[Message]) -> u64 {
        let mut hasher = DefaultHasher::new();
        for msg in messages {
            // Hash role as discriminant.
            let role_discriminant = match msg.role {
                clarity_contract::MessageRole::System => 0u8,
                clarity_contract::MessageRole::User => 1u8,
                clarity_contract::MessageRole::Assistant => 2u8,
                clarity_contract::MessageRole::Tool => 3u8,
            };
            role_discriminant.hash(&mut hasher);
            msg.content.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Look up a cached compaction result.
    ///
    /// Returns `None` if the key is not present or the entry has expired.
    pub fn get(&mut self, key: u64) -> Option<Vec<Message>> {
        let now = Instant::now();
        // Find and remove expired entries during lookup.
        self.inner
            .retain(|(_, entry)| now.duration_since(entry.created_at) < self.ttl);

        if let Some(pos) = self.inner.iter().position(|(k, _)| *k == key) {
            // Move to back (MRU): remove at pos, clone the compacted result,
            // then push_back so the entry becomes the most-recently-used.
            if let Some(entry) = self.inner.remove(pos) {
                // Clone BEFORE push_back to avoid a second borrow on self.inner.
                let compacted = entry.1.compacted.clone();
                self.inner.push_back(entry);
                return Some(compacted);
            }
        }
        None
    }

    /// Store a compaction result.
    pub fn put(&mut self, key: u64, compacted: Vec<Message>) {
        // Evict expired entries.
        let now = Instant::now();
        self.inner
            .retain(|(_, entry)| now.duration_since(entry.created_at) < self.ttl);

        // Evict LRU if at capacity.
        while self.inner.len() >= self.capacity {
            self.inner.pop_front();
        }

        self.inner.push_back((
            key,
            CacheEntry {
                compacted,
                created_at: now,
            },
        ));
    }

    /// Invalidate all cached entries.
    ///
    /// Call this when the system prompt changes, as cached compactions
    /// computed with a different system prompt may produce incorrect results.
    pub fn invalidate_all(&mut self) {
        self.inner.clear();
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for CompactionCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_hit_and_miss() {
        let mut cache = CompactionCache::new();
        let msgs = vec![Message::system("sys"), Message::user("hello")];
        let key = CompactionCache::hash_messages(&msgs);

        assert!(cache.get(key).is_none());
        cache.put(key, vec![Message::assistant("summary")]);
        assert!(cache.get(key).is_some());
    }

    #[test]
    fn test_different_messages_different_key() {
        let key1 = CompactionCache::hash_messages(&[Message::system("sys-a")]);
        let key2 = CompactionCache::hash_messages(&[Message::system("sys-b")]);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_same_messages_same_key() {
        let msgs1 = vec![Message::system("sys"), Message::user("hi")];
        let msgs2 = vec![Message::system("sys"), Message::user("hi")];
        assert_eq!(
            CompactionCache::hash_messages(&msgs1),
            CompactionCache::hash_messages(&msgs2)
        );
    }

    #[test]
    fn test_invalidate_all() {
        let mut cache = CompactionCache::new();
        let key = CompactionCache::hash_messages(&[Message::user("test")]);
        cache.put(key, vec![Message::assistant("summary")]);
        assert!(!cache.is_empty());
        cache.invalidate_all();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = CompactionCache {
            inner: VecDeque::new(),
            capacity: 2,
            ttl: Duration::from_secs(3600),
        };
        cache.put(1, vec![Message::assistant("a")]);
        cache.put(2, vec![Message::assistant("b")]);
        cache.put(3, vec![Message::assistant("c")]);

        // key 1 should be evicted (LRU).
        assert!(cache.get(1).is_none());
        assert!(cache.get(2).is_some());
        assert!(cache.get(3).is_some());
    }
}
