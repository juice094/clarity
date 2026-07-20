//! Content-addressed tool result cache.
//!
//! Caches tool execution results keyed by (tool_name, arguments_hash, working_dir_hash)
//! to eliminate redundant tool calls within a single agent turn. Pattern follows
//! syncthing-rust's `syncthing-db/src/block_cache/` write-through LRU design.
//!
//! # Design
//!
//! - **Content-addressed**: cache key is a hash of the tool name + canonical JSON args + cwd
//! - **TTL-based invalidation**: each entry has a configurable TTL (default 30s)
//! - **Volatile tool exclusion**: tools like `shell`, `web_fetch`, `ask_user` are never cached
//! - **LRU eviction**: bounded capacity (default 128 entries), least-recently-used evicted first
//!
//! # Safety
//!
//! The cache is per-turn — cleared at the start of each agent turn so stale
//! results from a previous turn never leak into the next. Within a turn, the
//! TTL ensures that cached results age out even for long-running agent loops.

use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

// ============================================================================
// ToolCacheConfig
// ============================================================================

/// Configuration for the tool result cache.
#[derive(Debug, Clone)]
pub struct ToolCacheConfig {
    /// Maximum number of cached entries (default 128).
    pub capacity: usize,
    /// Default TTL for cached results in seconds (default 30).
    pub default_ttl_secs: u64,
    /// Tool names that should NEVER be cached (e.g., "shell", "web_fetch").
    pub volatile_tools: HashSet<String>,
}

impl Default for ToolCacheConfig {
    fn default() -> Self {
        let mut volatile = HashSet::new();
        volatile.insert("shell".to_string());
        volatile.insert("bash".to_string());
        volatile.insert("powershell".to_string());
        volatile.insert("web_fetch".to_string());
        volatile.insert("ask_user".to_string());
        volatile.insert("task".to_string());
        volatile.insert("cron_create".to_string());
        Self {
            capacity: 128,
            default_ttl_secs: 30,
            volatile_tools: volatile,
        }
    }
}

// ============================================================================
// ToolCacheKey
// ============================================================================

/// Compound key for tool result caching.
///
/// Two tool calls produce the same key only if they share:
/// - The same tool name
/// - The same canonical JSON arguments
/// - The same working directory (shell results depend on cwd)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolCacheKey {
    tool_name: String,
    arguments_hash: u64,
    working_dir_hash: u64,
}

// ============================================================================
// ToolCacheEntry
// ============================================================================

/// A cached tool result with expiration metadata.
#[derive(Debug, Clone)]
struct ToolCacheEntry {
    /// The cached tool result.
    result: Value,
    /// When this entry was created.
    cached_at: Instant,
    /// Time-to-live for this entry.
    ttl: Duration,
}

impl ToolCacheEntry {
    fn is_expired(&self, now: Instant) -> bool {
        now.duration_since(self.cached_at) >= self.ttl
    }
}

// ============================================================================
// ToolResultCache
// ============================================================================

/// Content-addressed LRU cache for tool execution results.
///
/// # Example
///
/// ```rust,no_run
/// use clarity_core::agent::tool_cache::{ToolResultCache, ToolCacheConfig};
///
/// let mut cache = ToolResultCache::new(ToolCacheConfig::default());
///
/// // First call — cache miss, execute the tool.
/// let key = cache.make_key("read", &serde_json::json!({"path": "Cargo.toml"}), "/app");
/// assert!(cache.get(&key).is_none());
///
/// // ... execute tool, get result ...
///
/// // Store result.
/// cache.put(key, serde_json::json!("[package]\nname = \"clarity\""));
///
/// // Second call with same args — cache hit.
/// let key2 = cache.make_key("read", &serde_json::json!({"path": "Cargo.toml"}), "/app");
/// assert!(cache.get(&key2).is_some());
/// ```
#[derive(Debug, Clone)]
pub struct ToolResultCache {
    inner: HashMap<ToolCacheKey, ToolCacheEntry>,
    /// LRU order: front = least recently used, back = most recently used.
    lru_order: VecDeque<ToolCacheKey>,
    config: ToolCacheConfig,
}

impl ToolResultCache {
    /// Create a new cache with the given configuration.
    pub fn new(config: ToolCacheConfig) -> Self {
        Self {
            inner: HashMap::with_capacity(config.capacity),
            lru_order: VecDeque::with_capacity(config.capacity),
            config,
        }
    }

    /// Check whether a tool should be cached.
    ///
    /// Returns `false` for volatile tools (shell, web_fetch, etc.) and
    /// `true` for deterministic/idempotent tools (read, glob, grep, etc.).
    pub fn is_cacheable(&self, tool_name: &str) -> bool {
        !self.config.volatile_tools.contains(tool_name)
    }

    /// Build a cache key from tool name, arguments, and working directory.
    pub fn make_key(&self, tool_name: &str, arguments: &Value, working_dir: &str) -> ToolCacheKey {
        let arguments_hash = stable_hash(&canonical_json(arguments));
        let working_dir_hash = stable_hash(working_dir);
        ToolCacheKey {
            tool_name: tool_name.to_string(),
            arguments_hash,
            working_dir_hash,
        }
    }

    /// Look up a cached result.
    ///
    /// Returns `None` if the key is not present or the entry has expired.
    /// On cache hit, the entry is promoted to the MRU position.
    pub fn get(&mut self, key: &ToolCacheKey) -> Option<Value> {
        let entry = self.inner.get(key)?;
        let now = Instant::now();
        if entry.is_expired(now) {
            self.inner.remove(key);
            self.lru_order.retain(|k| k != key);
            return None;
        }
        // Promote to MRU.
        self.lru_order.retain(|k| k != key);
        self.lru_order.push_back(key.clone());
        Some(entry.result.clone())
    }

    /// Store a tool result in the cache.
    ///
    /// If the cache is at capacity, the least recently used entry is evicted.
    /// Expired entries are cleaned up before insertion.
    pub fn put(&mut self, key: ToolCacheKey, result: Value) {
        // Evict expired entries first.
        let now = Instant::now();
        self.inner.retain(|_, entry| !entry.is_expired(now));
        self.lru_order.retain(|k| self.inner.contains_key(k));

        // If at capacity, evict LRU.
        while self.inner.len() >= self.config.capacity {
            if let Some(lru_key) = self.lru_order.pop_front() {
                self.inner.remove(&lru_key);
            } else {
                break;
            }
        }

        self.inner.insert(
            key.clone(),
            ToolCacheEntry {
                result,
                cached_at: now,
                ttl: Duration::from_secs(self.config.default_ttl_secs),
            },
        );
        self.lru_order.push_back(key);
    }

    /// Clear all cached entries.
    pub fn clear(&mut self) {
        self.inner.clear();
        self.lru_order.clear();
    }

    /// Number of cached entries (including expired ones pending eviction).
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Canonicalize JSON arguments to ensure consistent hashing.
///
/// Two JSON values that represent the same logical arguments should produce
/// the same canonical form even if key order differs.
fn canonical_json(value: &Value) -> String {
    // serde_json serializes objects with sorted keys by default, so we
    // just serialize and use that as the canonical form.
    serde_json::to_string(value).unwrap_or_else(|_| format!("{:?}", value))
}

/// Compute a stable hash of a string slice.
fn stable_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cache() -> ToolResultCache {
        ToolResultCache::new(ToolCacheConfig::default())
    }

    #[test]
    fn test_cache_hit_and_miss() {
        let mut cache = test_cache();
        let args = serde_json::json!({"path": "Cargo.toml"});
        let key = cache.make_key("read", &args, "/app");

        assert!(cache.get(&key).is_none(), "empty cache should miss");

        cache.put(key.clone(), serde_json::json!("result"));
        assert!(cache.get(&key).is_some(), "cache should hit after put");
    }

    #[test]
    fn test_same_args_same_key() {
        let cache = test_cache();
        let args1 = serde_json::json!({"path": "Cargo.toml"});
        let args2 = serde_json::json!({"path": "Cargo.toml"});
        let key1 = cache.make_key("read", &args1, "/app");
        let key2 = cache.make_key("read", &args2, "/app");
        assert_eq!(key1, key2, "same logical args should produce same key");
    }

    #[test]
    fn test_different_args_different_key() {
        let cache = test_cache();
        let args1 = serde_json::json!({"path": "Cargo.toml"});
        let args2 = serde_json::json!({"path": "README.md"});
        let key1 = cache.make_key("read", &args1, "/app");
        let key2 = cache.make_key("read", &args2, "/app");
        assert_ne!(key1, key2, "different args should produce different keys");
    }

    #[test]
    fn test_different_cwd_different_key() {
        let cache = test_cache();
        let args = serde_json::json!({"path": "Cargo.toml"});
        let key1 = cache.make_key("read", &args, "/app");
        let key2 = cache.make_key("read", &args, "/other");
        assert_ne!(key1, key2, "different cwd should produce different keys");
    }

    #[test]
    fn test_volatile_tools_not_cacheable() {
        let cache = test_cache();
        assert!(!cache.is_cacheable("shell"));
        assert!(!cache.is_cacheable("bash"));
        assert!(!cache.is_cacheable("web_fetch"));
        assert!(cache.is_cacheable("read"));
        assert!(cache.is_cacheable("glob"));
        assert!(cache.is_cacheable("grep"));
    }

    #[test]
    fn test_lru_eviction() {
        let config = ToolCacheConfig {
            capacity: 2,
            ..ToolCacheConfig::default()
        };
        let mut cache = ToolResultCache::new(config);

        let key_a = cache.make_key("read", &serde_json::json!({"path": "a"}), "/app");
        let key_b = cache.make_key("read", &serde_json::json!({"path": "b"}), "/app");
        let key_c = cache.make_key("read", &serde_json::json!({"path": "c"}), "/app");

        cache.put(key_a.clone(), serde_json::json!("a"));
        cache.put(key_b.clone(), serde_json::json!("b"));
        // Access key_a to make it MRU.
        let _ = cache.get(&key_a);

        // Insert key_c — should evict key_b (LRU).
        cache.put(key_c.clone(), serde_json::json!("c"));

        assert!(cache.get(&key_a).is_some(), "key_a (MRU) should survive");
        assert!(cache.get(&key_b).is_none(), "key_b (LRU) should be evicted");
        assert!(cache.get(&key_c).is_some(), "key_c should be present");
    }

    #[test]
    fn test_clear_empties_cache() {
        let mut cache = test_cache();
        let key = cache.make_key("read", &serde_json::json!({"path": "x"}), "/app");
        cache.put(key, serde_json::json!("x"));
        assert!(!cache.is_empty());
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_key_hashing_deterministic() {
        let cache = test_cache();
        let args = serde_json::json!({"path": "lib.rs", "limit": 100});
        // Keys are equal only if all three components match.
        let k1 = cache.make_key("read", &args, "/app");
        let k2 = cache.make_key("read", &args, "/app");
        assert_eq!(k1, k2);
    }
}
