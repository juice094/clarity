//! SQLite storage backend with FTS5
use crate::backends::StorageBackend;
use crate::bm25::IncrementalBm25Index;
use crate::store::DecayConfig;
use crate::types::{Fact, MemoryError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, instrument};

/// Cached incremental BM25 index paired with fact ID mappings.
#[derive(Debug, Clone)]
struct Bm25Cache {
    index: IncrementalBm25Index,
    /// Maps doc_idx -> fact_id
    fact_ids: Vec<i64>,
    /// Maps fact_id -> doc_idx (live docs only)
    id_to_idx: HashMap<i64, usize>,
}

impl Bm25Cache {
    fn new() -> Self {
        Self {
            index: IncrementalBm25Index::new(),
            fact_ids: Vec::new(),
            id_to_idx: HashMap::new(),
        }
    }

    fn add_fact(&mut self, id: i64, text: &str) {
        let idx = self.index.add_document(text);
        if idx < self.fact_ids.len() {
            self.fact_ids[idx] = id;
        } else {
            self.fact_ids.push(id);
        }
        self.id_to_idx.insert(id, idx);
    }

    fn remove_fact(&mut self, id: i64) {
        if let Some(idx) = self.id_to_idx.remove(&id) {
            self.index.remove_document(idx);
        }
    }

    fn score(&self, query: &str, id: i64) -> Option<f32> {
        let idx = self.id_to_idx.get(&id)?;
        Some(self.index.score(query, *idx))
    }
}

#[derive(Debug, Clone)]
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
    bm25_cache: Arc<RwLock<Option<Bm25Cache>>>,
}

impl SqliteStore {
    #[instrument(skip(db_path))]
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let db_path_str = db_path.to_string_lossy().to_string();
        let conn = tokio::task::spawn_blocking(move || {
            Connection::open(&db_path).map_err(MemoryError::Database)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))??;

        info!("Initializing SqliteStore at {}", db_path_str);
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            bm25_cache: Arc::new(RwLock::new(None)),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(MemoryError::Database)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            bm25_cache: Arc::new(RwLock::new(None)),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn enable_wal_mode(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("PRAGMA journal_mode=WAL", [])?;
        conn.execute("PRAGMA synchronous=NORMAL", [])?;
        debug!("WAL mode enabled");
        Ok(())
    }

    pub async fn save_session_note(
        &self,
        session_id: &str,
        section: &str,
        content: &str,
    ) -> Result<()> {
        let session_id = session_id.to_string();
        let section = section.to_string();
        let content = content.to_string();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO session_notes (session_id, section, content) VALUES (?1, ?2, ?3)",
                params![session_id, section, content],
            )?;
            Ok::<_, MemoryError>(())
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))??;
        Ok(())
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS facts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                fact TEXT NOT NULL,
                tags TEXT NOT NULL DEFAULT '[]',
                time TEXT,
                session_id TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
                fact, content='facts', content_rowid='id'
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS session_notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                section TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS facts_fts_insert AFTER INSERT ON facts BEGIN
                INSERT INTO facts_fts(rowid, fact) VALUES (new.id, new.fact);
            END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS facts_fts_delete AFTER DELETE ON facts BEGIN
                INSERT INTO facts_fts(facts_fts, rowid, fact) VALUES ('delete', old.id, old.fact);
            END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS facts_fts_update AFTER UPDATE ON facts BEGIN
                INSERT INTO facts_fts(facts_fts, rowid, fact) VALUES ('delete', old.id, old.fact);
                INSERT INTO facts_fts(rowid, fact) VALUES (new.id, new.fact);
            END",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_facts_session ON facts(session_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_facts_created ON facts(created_at)",
            [],
        )?;

        debug!("Schema initialization complete");
        Ok(())
    }

    fn row_to_fact(row: &rusqlite::Row) -> rusqlite::Result<Fact> {
        let tags_json: String = row.get(2)?;
        let tags: Vec<String> = serde_json::from_str(&tags_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok(Fact {
            id: row.get(0)?,
            fact: row.get(1)?,
            tags,
            time: row.get(3)?,
            session_id: row.get(4)?,
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?
                .with_timezone(&Utc),
        })
    }
}

#[async_trait]
impl StorageBackend for SqliteStore {
    async fn save_fact(
        &self,
        fact: &str,
        tags: &[String],
        time: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<i64> {
        let tags_json = serde_json::to_string(tags)?;
        let now = Utc::now().to_rfc3339();
        let fact = fact.to_string();
        let time = time.map(|s| s.to_string());
        let session_id = session_id.map(|s| s.to_string());
        let conn = self.conn.clone();

        let fact_for_db = fact.clone();
        let id = tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO facts (fact, tags, time, session_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![fact_for_db, tags_json, time, session_id, now],
            )?;
            Ok::<_, MemoryError>(conn.last_insert_rowid())
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))??;

        {
            let mut cache = self.bm25_cache.write();
            if let Some(ref mut c) = *cache {
                c.add_fact(id, &fact);
            }
        }

        info!("Saved fact with id={}, tags={:?}", id, tags);
        Ok(id)
    }

    async fn get_fact(&self, id: i64) -> Result<Option<Fact>> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let result = conn
                .query_row(
                    "SELECT id, fact, tags, time, session_id, created_at FROM facts WHERE id = ?",
                    [id],
                    Self::row_to_fact,
                )
                .optional()?;
            Ok::<_, MemoryError>(result)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn delete_fact(&self, id: i64) -> Result<bool> {
        let conn = self.conn.clone();
        let deleted = tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let rows = conn.execute("DELETE FROM facts WHERE id = ?", [id])?;
            Ok::<_, MemoryError>(rows > 0)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))??;

        if deleted {
            let mut cache = self.bm25_cache.write();
            if let Some(ref mut c) = *cache {
                c.remove_fact(id);
            }
        }

        Ok(deleted)
    }

    async fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<Fact>> {
        if tags.is_empty() {
            return Ok(Vec::new());
        }
        let tags = tags.to_vec();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT id, fact, tags, time, session_id, created_at FROM facts ORDER BY created_at DESC LIMIT ?")?;
            let rows = stmt.query_map([limit as i64], Self::row_to_fact)?;

            let mut facts = Vec::new();
            for row in rows {
                let fact = row?;
                if tags.iter().all(|tag| fact.tags.contains(tag)) {
                    facts.push(fact);
                    if facts.len() >= limit { break; }
                }
            }
            Ok::<_, MemoryError>(facts)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn search_fulltext(
        &self,
        query: &str,
        limit: usize,
        decay: &DecayConfig,
    ) -> Result<Vec<Fact>> {
        let query = query.to_string();
        let conn = self.conn.clone();
        let decay = *decay;

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT f.id, f.fact, f.tags, f.time, f.session_id, f.created_at, fts.rank
                 FROM facts f JOIN facts_fts fts ON f.id = fts.rowid
                 WHERE facts_fts MATCH ?
                 ORDER BY fts.rank",
            )?;
            let now = Utc::now();
            let lambda = std::f64::consts::LN_2 / decay.half_life_days;
            let mut scored: Vec<(Fact, f64)> = Vec::new();
            let rows = stmt.query_map([&query], |row| {
                let fact = Self::row_to_fact(row)?;
                let rank: f64 = row.get(6)?;
                Ok((fact, rank))
            })?;

            for row in rows {
                let (fact, rank) = row?;
                let weight = if decay.enabled {
                    let age_days = (now - fact.created_at).num_days() as f64;
                    (-lambda * age_days).exp()
                } else {
                    1.0
                };
                scored.push((fact, rank * weight));
            }
            scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let facts: Vec<Fact> = scored.into_iter().map(|(f, _)| f).take(limit).collect();
            Ok::<_, MemoryError>(facts)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn get_facts_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<Fact>> {
        let session_id = session_id.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, fact, tags, time, session_id, created_at FROM facts 
                 WHERE session_id = ? ORDER BY created_at DESC LIMIT ?",
            )?;
            let rows = stmt.query_map(params![session_id, limit as i64], Self::row_to_fact)?;

            let mut facts = Vec::new();
            for row in rows {
                facts.push(row?);
            }
            Ok::<_, MemoryError>(facts)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn get_facts_since(&self, since: DateTime<Utc>) -> Result<Vec<Fact>> {
        let since_str = since.to_rfc3339();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, fact, tags, time, session_id, created_at FROM facts 
                 WHERE created_at > ? ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([&since_str], Self::row_to_fact)?;

            let mut facts = Vec::new();
            for row in rows {
                facts.push(row?);
            }
            Ok::<_, MemoryError>(facts)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn get_recent_facts(&self, limit: usize) -> Result<Vec<Fact>> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, fact, tags, time, session_id, created_at FROM facts 
                 ORDER BY created_at DESC LIMIT ?",
            )?;
            let rows = stmt.query_map([limit as i64], Self::row_to_fact)?;

            let mut facts = Vec::new();
            for row in rows {
                facts.push(row?);
            }
            Ok::<_, MemoryError>(facts)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn count_facts(&self) -> Result<i64> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM facts", [], |row| row.get(0))?;
            Ok::<_, MemoryError>(count)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn search_similar(
        &self,
        query: &str,
        limit: usize,
        decay: &DecayConfig,
    ) -> Result<Vec<(Fact, f32)>> {
        let query = query.to_string();
        let decay = *decay;

        // Step 1: FTS5 recall — fetch candidate documents (5x limit for reranking pool)
        let candidates = self.search_fulltext(&query, limit * 5, &decay).await?;

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // Step 2: Build or reuse cached incremental BM25 index
        let cache_empty = {
            let cache = self.bm25_cache.read();
            cache.is_none()
        };

        if cache_empty {
            let conn = self.conn.clone();
            let all_facts: Vec<(i64, String)> = tokio::task::spawn_blocking(move || {
                let conn = conn.lock().unwrap();
                let mut stmt = conn.prepare("SELECT id, fact FROM facts")?;
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?;
                let mut facts = Vec::new();
                for row in rows {
                    facts.push(row?);
                }
                Ok::<_, MemoryError>(facts)
            })
            .await
            .map_err(|e| MemoryError::InvalidInput(e.to_string()))??;

            let mut cache_guard = self.bm25_cache.write();
            if cache_guard.is_none() {
                let mut cache = Bm25Cache::new();
                for (id, text) in all_facts {
                    cache.add_fact(id, &text);
                }
                *cache_guard = Some(cache);
            }
        }

        // Step 3: Rerank candidates using the cached index
        let cache = self.bm25_cache.read();
        let now = Utc::now();
        let lambda = std::f64::consts::LN_2 / decay.half_life_days;
        let mut scored: Vec<(Fact, f32)> = if let Some(ref c) = *cache {
            candidates
                .into_iter()
                .map(|fact| {
                    let bm25_score = c.score(&query, fact.id).unwrap_or(0.0);
                    let weight = if decay.enabled {
                        let age_days = (now - fact.created_at).num_days() as f64;
                        (-lambda * age_days).exp() as f32
                    } else {
                        1.0
                    };
                    (fact, bm25_score * weight)
                })
                .collect()
        } else {
            candidates.into_iter().map(|f| (f, 1.0)).collect()
        };
        drop(cache);

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);

        Ok(scored)
    }

    async fn clear_all(&self) -> Result<usize> {
        let conn = self.conn.clone();

        let rows = tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let rows = conn.execute("DELETE FROM facts", [])?;
            Ok::<_, MemoryError>(rows)
        })
        .await
        .map_err(|e| MemoryError::InvalidInput(e.to_string()))??;

        let mut cache = self.bm25_cache.write();
        *cache = None;

        Ok(rows)
    }

    async fn bulk_save_facts(
        &self,
        facts: &[(&str, Vec<String>, Option<&str>, Option<&str>)],
    ) -> Result<Vec<i64>> {
        let facts: Vec<_> = facts
            .iter()
            .map(|(f, t, time, sid)| {
                (
                    f.to_string(),
                    t.clone(),
                    time.map(|s| s.to_string()),
                    sid.map(|s| s.to_string()),
                )
            })
            .collect();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let mut conn = conn.lock().unwrap();
            let tx = conn.transaction()?;
            let mut ids = Vec::with_capacity(facts.len());
            let now = Utc::now().to_rfc3339();

            for (fact, tags, time, session_id) in facts {
                let tags_json = serde_json::to_string(&tags)?;
                tx.execute(
                    "INSERT INTO facts (fact, tags, time, session_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![fact, tags_json, time, session_id, now],
                )?;
                ids.push(tx.last_insert_rowid());
            }

            tx.commit()?;
            Ok::<_, MemoryError>(ids)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::DecayConfig;
    use chrono::Duration;

    fn insert_fact_with_time(store: &SqliteStore, text: &str, created_at: DateTime<Utc>) -> i64 {
        let conn = store.conn.lock().unwrap();
        let tags = serde_json::to_string(&Vec::<String>::new()).unwrap();
        conn.execute(
            "INSERT INTO facts (fact, tags, time, session_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![text, tags, None::<String>, None::<String>, created_at.to_rfc3339()],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[tokio::test]
    async fn test_decay_disabled() {
        let store = SqliteStore::new_in_memory().unwrap();
        let old_time = Utc::now() - Duration::days(365);
        let id = insert_fact_with_time(&store, "old decay fact", old_time);

        let decay = DecayConfig {
            enabled: false,
            ..Default::default()
        };
        let results = store.search_fulltext("decay", 10, &decay).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
    }

    #[tokio::test]
    async fn test_decay_6_months() {
        let store = SqliteStore::new_in_memory().unwrap();
        let old_time = Utc::now() - Duration::days(180);
        insert_fact_with_time(&store, "decay test fact", old_time);
        insert_fact_with_time(&store, "decay test fact", Utc::now());

        let decay = DecayConfig::default();
        let results = store.search_fulltext("decay", 10, &decay).await.unwrap();
        assert_eq!(results.len(), 2);
        // Recent fact should come first
        assert!(results[0].created_at > results[1].created_at);

        let similar = store.search_similar("decay", 10, &decay).await.unwrap();
        assert_eq!(similar.len(), 2);
        let recent_score = similar[0].1;
        let old_score = similar[1].1;
        let ratio = recent_score / old_score;
        assert!(
            ratio > 1.8 && ratio < 2.2,
            "expected ratio ~2.0, got {}",
            ratio
        );
    }

    #[tokio::test]
    async fn test_decay_recent() {
        let store = SqliteStore::new_in_memory().unwrap();
        let old_time = Utc::now() - Duration::days(365);
        insert_fact_with_time(&store, "decay test fact", old_time);
        let recent_id = insert_fact_with_time(&store, "decay test fact", Utc::now());

        let decay = DecayConfig::default();
        let results = store.search_fulltext("decay", 10, &decay).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, recent_id);
    }
}
