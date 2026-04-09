//! SQLite storage backend with FTS5
use crate::backends::StorageBackend;
use crate::types::{Fact, MemoryError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, instrument};

#[derive(Debug, Clone)]
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    #[instrument(skip(db_path))]
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let db_path_str = db_path.to_string_lossy().to_string();
        let conn = tokio::task::spawn_blocking(move || {
            Connection::open(&db_path).map_err(MemoryError::Database)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))??;

        info!("Initializing SqliteStore at {}", db_path_str);
        let store = Self { conn: Arc::new(Mutex::new(conn)) };
        store.init_schema()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(MemoryError::Database)?;
        let store = Self { conn: Arc::new(Mutex::new(conn)) };
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
            )", [],
        )?;

        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
                fact, content='facts', content_rowid='id'
            )", [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS facts_fts_insert AFTER INSERT ON facts BEGIN
                INSERT INTO facts_fts(rowid, fact) VALUES (new.id, new.fact);
            END", [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS facts_fts_delete AFTER DELETE ON facts BEGIN
                INSERT INTO facts_fts(facts_fts, rowid, fact) VALUES ('delete', old.id, old.fact);
            END", [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS facts_fts_update AFTER UPDATE ON facts BEGIN
                INSERT INTO facts_fts(facts_fts, rowid, fact) VALUES ('delete', old.id, old.fact);
                INSERT INTO facts_fts(rowid, fact) VALUES (new.id, new.fact);
            END", [],
        )?;

        conn.execute("CREATE INDEX IF NOT EXISTS idx_facts_session ON facts(session_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_facts_created ON facts(created_at)", [])?;

        debug!("Schema initialization complete");
        Ok(())
    }

    fn row_to_fact(row: &rusqlite::Row) -> rusqlite::Result<Fact> {
        let tags_json: String = row.get(2)?;
        let tags: Vec<String> = serde_json::from_str(&tags_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;
        Ok(Fact {
            id: row.get(0)?,
            fact: row.get(1)?,
            tags,
            time: row.get(3)?,
            session_id: row.get(4)?,
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?
                .with_timezone(&Utc),
        })
    }
}

#[async_trait]
impl StorageBackend for SqliteStore {
    async fn save_fact(&self, fact: &str, tags: &[String], time: Option<&str>, session_id: Option<&str>) -> Result<i64> {
        let tags_json = serde_json::to_string(tags)?;
        let now = Utc::now().to_rfc3339();
        let fact = fact.to_string();
        let time = time.map(|s| s.to_string());
        let session_id = session_id.map(|s| s.to_string());
        let conn = self.conn.clone();

        let id = tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO facts (fact, tags, time, session_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![fact, tags_json, time, session_id, now],
            )?;
            Ok::<_, MemoryError>(conn.last_insert_rowid())
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))??;

        info!("Saved fact with id={}, tags={:?}", id, tags);
        Ok(id)
    }

    async fn get_fact(&self, id: i64) -> Result<Option<Fact>> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let result = conn.query_row(
                "SELECT id, fact, tags, time, session_id, created_at FROM facts WHERE id = ?",
                [id], Self::row_to_fact,
            ).optional()?;
            Ok::<_, MemoryError>(result)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn delete_fact(&self, id: i64) -> Result<bool> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let rows = conn.execute("DELETE FROM facts WHERE id = ?", [id])?;
            Ok::<_, MemoryError>(rows > 0)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<Fact>> {
        if tags.is_empty() { return Ok(Vec::new()); }
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

    async fn search_fulltext(&self, query: &str, limit: usize) -> Result<Vec<Fact>> {
        let query = query.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT f.id, f.fact, f.tags, f.time, f.session_id, f.created_at 
                 FROM facts f JOIN facts_fts fts ON f.id = fts.rowid
                 WHERE facts_fts MATCH ? ORDER BY rank LIMIT ?"
            )?;
            let rows = stmt.query_map(params![query, limit as i64], Self::row_to_fact)?;

            let mut facts = Vec::new();
            for row in rows { facts.push(row?); }
            Ok::<_, MemoryError>(facts)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn get_facts_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<Fact>> {
        let session_id = session_id.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, fact, tags, time, session_id, created_at FROM facts 
                 WHERE session_id = ? ORDER BY created_at DESC LIMIT ?"
            )?;
            let rows = stmt.query_map(params![session_id, limit as i64], Self::row_to_fact)?;

            let mut facts = Vec::new();
            for row in rows { facts.push(row?); }
            Ok::<_, MemoryError>(facts)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn get_facts_since(&self, since: DateTime<Utc>) -> Result<Vec<Fact>> {
        let since_str = since.to_rfc3339();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, fact, tags, time, session_id, created_at FROM facts 
                 WHERE created_at > ? ORDER BY created_at DESC"
            )?;
            let rows = stmt.query_map([&since_str], Self::row_to_fact)?;

            let mut facts = Vec::new();
            for row in rows { facts.push(row?); }
            Ok::<_, MemoryError>(facts)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn get_recent_facts(&self, limit: usize) -> Result<Vec<Fact>> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, fact, tags, time, session_id, created_at FROM facts 
                 ORDER BY created_at DESC LIMIT ?"
            )?;
            let rows = stmt.query_map([limit as i64], Self::row_to_fact)?;

            let mut facts = Vec::new();
            for row in rows { facts.push(row?); }
            Ok::<_, MemoryError>(facts)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn count_facts(&self) -> Result<i64> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM facts", [], |row| row.get(0))?;
            Ok::<_, MemoryError>(count)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn search_similar(&self, query: &str, limit: usize) -> Result<Vec<(Fact, f32)>> {
        let query = query.to_string();
        let conn = self.conn.clone();

        let facts = tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, fact, tags, time, session_id, created_at FROM facts"
            )?;
            let rows = stmt.query_map([], Self::row_to_fact)?;
            let mut facts = Vec::new();
            for row in rows { facts.push(row?); }
            Ok::<_, MemoryError>(facts)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))??;

        if facts.is_empty() {
            return Ok(Vec::new());
        }

        let fact_tuples: Vec<(i64, String)> = facts.iter().map(|f| (f.id, f.fact.clone())).collect();
        let mut vector_store = crate::embedding::VectorStore::new();
        vector_store.index_facts(&fact_tuples);
        let results = vector_store.search(&query, limit);

        let fact_map: std::collections::HashMap<i64, Fact> = facts.into_iter().map(|f| (f.id, f)).collect();
        Ok(results.into_iter().filter_map(|(id, _, score)| {
            fact_map.get(&id).cloned().map(|f| (f, score))
        }).collect())
    }

    async fn clear_all(&self) -> Result<usize> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let rows = conn.execute("DELETE FROM facts", [])?;
            Ok::<_, MemoryError>(rows)
        }).await.map_err(|e| MemoryError::InvalidInput(e.to_string()))?
    }

    async fn bulk_save_facts(&self, facts: &[(&str, Vec<String>, Option<&str>, Option<&str>)]) -> Result<Vec<i64>> {
        let facts: Vec<_> = facts.iter().map(|(f, t, time, sid)| {
            (f.to_string(), t.clone(), time.map(|s| s.to_string()), sid.map(|s| s.to_string()))
        }).collect();
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
